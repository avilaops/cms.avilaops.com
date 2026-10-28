//! Metadata-only Langfuse OTLP tracing for MCP operations.
use std::{collections::HashMap, future::Future, sync::OnceLock, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use opentelemetry::{
    Context, FutureExt, KeyValue,
    trace::{Status, TraceContextExt, Tracer, TracerProvider},
};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource,
    trace::{Sampler, SdkTracerProvider},
};

static PROVIDER: OnceLock<Option<SdkTracerProvider>> = OnceLock::new();
const SERVICE: &str = "cms";

struct Configuration {
    endpoint: String,
    authorization: String,
    environment: String,
    sample_rate: f64,
}

fn configuration(
    get: impl Fn(&str) -> Option<String>,
) -> Result<Option<Configuration>, &'static str> {
    if get("LANGFUSE_ENABLED").as_deref() != Some("true") {
        return Ok(None);
    }
    let required = |name| {
        get(name)
            .filter(|value| !value.trim().is_empty())
            .ok_or("missing Langfuse configuration")
    };
    let base = required("LANGFUSE_BASE_URL")?;
    let url = reqwest::Url::parse(&base).map_err(|_| "invalid Langfuse URL")?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "::1"));
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Langfuse requires HTTPS or literal loopback without URL credentials");
    }
    let public = required("LANGFUSE_PUBLIC_KEY")?;
    let secret = required("LANGFUSE_SECRET_KEY")?;
    let environment = get("LANGFUSE_TRACING_ENVIRONMENT").unwrap_or_else(|| "development".into());
    if !matches!(
        environment.as_str(),
        "development" | "test" | "staging" | "production"
    ) {
        return Err("invalid Langfuse environment");
    }
    let sample_rate = get("LANGFUSE_SAMPLE_RATE")
        .unwrap_or_else(|| "1".into())
        .parse::<f64>()
        .map_err(|_| "invalid Langfuse sample rate")?;
    if !sample_rate.is_finite() || !(0.0..=1.0).contains(&sample_rate) {
        return Err("invalid Langfuse sample rate");
    }
    Ok(Some(Configuration {
        endpoint: format!("{}/api/public/otel/v1/traces", base.trim_end_matches('/')),
        authorization: format!("Basic {}", STANDARD.encode(format!("{public}:{secret}"))),
        environment,
        sample_rate,
    }))
}

/// Call once at application startup. Configuration/export failures never block the service.
pub fn init() {
    PROVIDER.get_or_init(|| {
        let config = match configuration(|name| std::env::var(name).ok()) {
            Ok(Some(config)) => config,
            Ok(None) => return None,
            Err(_) => {
                tracing::warn!("Langfuse tracing disabled: invalid or missing configuration");
                return None;
            }
        };
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(config.endpoint)
            .with_timeout(Duration::from_secs(3))
            .with_headers(HashMap::from([
                ("Authorization".into(), config.authorization),
                ("x-langfuse-ingestion-version".into(), "4".into()),
            ]))
            .build();
        match exporter {
            Ok(exporter) => Some(
                SdkTracerProvider::builder()
                    .with_batch_exporter(exporter)
                    .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                        config.sample_rate,
                    ))))
                    .with_resource(
                        Resource::builder()
                            .with_service_name(SERVICE)
                            .with_attributes([
                                KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                                KeyValue::new("deployment.environment.name", config.environment),
                            ])
                            .build(),
                    )
                    .build(),
            ),
            Err(_) => {
                tracing::warn!("Langfuse tracing disabled: exporter unavailable");
                None
            }
        }
    });
}

/// Flush the bounded batch queue off the async runtime before process exit.
pub async fn shutdown() {
    if let Some(Some(provider)) = PROVIDER.get() {
        let provider = provider.clone();
        let _ = tokio::task::spawn_blocking(move || provider.shutdown()).await;
    }
}

/// Names and kinds must be fixed application constants, never user arguments.
pub async fn observe<T, E, F>(
    name: &'static str,
    kind: &'static str,
    future: F,
    rejected: impl FnOnce(&T) -> bool,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    let Some(Some(provider)) = PROVIDER.get() else {
        return future.await;
    };
    observe_with_provider(provider, name, kind, future, rejected).await
}

async fn observe_with_provider<T, E, F>(
    provider: &SdkTracerProvider,
    name: &'static str,
    kind: &'static str,
    future: F,
    rejected: impl FnOnce(&T) -> bool,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    let tracer = provider.tracer(SERVICE);
    let span = tracer.start_with_context(name, &Context::current());
    let context = Context::current().with_span(span);
    context.span().set_attributes([
        KeyValue::new("langfuse.observation.type", kind),
        KeyValue::new("langfuse.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("langfuse.environment", tracing_environment()),
        KeyValue::new("langfuse.observation.metadata.service", SERVICE),
        KeyValue::new("langfuse.observation.input", r#"{"content":"omitted"}"#),
    ]);
    // If this future is dropped by a timeout or cancellation, retain a safe error.
    context
        .span()
        .set_status(Status::error("operation interrupted"));
    let result = future.with_context(context.clone()).await;
    let failed = match &result {
        Ok(value) => rejected(value),
        Err(_) => true,
    };
    context.span().set_attribute(KeyValue::new(
        "langfuse.observation.output",
        if failed {
            r#"{"status":"error"}"#
        } else {
            r#"{"status":"success"}"#
        },
    ));
    context.span().set_status(if failed {
        Status::error("operation failed")
    } else {
        Status::Ok
    });
    context.span().end();
    result
}

fn tracing_environment() -> String {
    match std::env::var("LANGFUSE_TRACING_ENVIRONMENT").as_deref() {
        Ok("production") => "production",
        Ok("staging") => "staging",
        Ok("test") => "test",
        _ => "development",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_sdk::trace::InMemorySpanExporter;

    fn configured(name: &str) -> Option<String> {
        match name {
            "LANGFUSE_ENABLED" => Some("true".into()),
            "LANGFUSE_BASE_URL" => Some("https://example.invalid".into()),
            "LANGFUSE_PUBLIC_KEY" => Some("public-fixture".into()),
            "LANGFUSE_SECRET_KEY" => Some("secret-fixture".into()),
            _ => None,
        }
    }

    #[test]
    fn requires_opt_in_complete_configuration_and_safe_destination() {
        assert!(configuration(|_| None).unwrap().is_none());
        assert!(configuration(|key| (key == "LANGFUSE_ENABLED").then(|| "true".into())).is_err());
        for base in [
            "http://example.invalid",
            "https://user:password@example.invalid",
            "https://example.invalid?token=private",
            "https://example.invalid#private",
        ] {
            assert!(
                configuration(|key| if key == "LANGFUSE_BASE_URL" {
                    Some(base.into())
                } else {
                    configured(key)
                })
                .is_err()
            );
        }
        let config = configuration(configured).unwrap().unwrap();
        assert_eq!(
            config.endpoint,
            "https://example.invalid/api/public/otel/v1/traces"
        );
        assert_eq!(config.environment, "development");
        for value in ["NaN", "-0.1", "1.1", "bad"] {
            assert!(
                configuration(|key| if key == "LANGFUSE_SAMPLE_RATE" {
                    Some(value.into())
                } else {
                    configured(key)
                })
                .is_err()
            );
        }
    }

    #[tokio::test]
    async fn preserves_results_errors_and_parenting_without_exporting_payloads() {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let result = observe_with_provider(
            &provider,
            "render-document",
            "span",
            async {
                observe_with_provider(
                    &provider,
                    "validate-document",
                    "tool",
                    async { Ok::<_, &'static str>("private-xml-and-document") },
                    |_| false,
                )
                .await
            },
            |_| false,
        )
        .await;
        assert_eq!(result, Ok("private-xml-and-document"));
        let error = observe_with_provider(
            &provider,
            "render-document",
            "tool",
            async { Err::<(), _>("private-upstream-error") },
            |_| false,
        )
        .await;
        assert_eq!(error, Err("private-upstream-error"));
        let rejected = observe_with_provider(
            &provider,
            "render-document",
            "tool",
            async { Ok::<_, ()>(true) },
            |value| *value,
        )
        .await;
        assert_eq!(rejected, Ok(true));
        provider.force_flush().unwrap();
        let spans = exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 4);
        assert_eq!(
            spans[0].span_context.trace_id(),
            spans[1].span_context.trace_id()
        );
        assert_eq!(spans[0].parent_span_id, spans[1].span_context.span_id());
        assert!(matches!(spans[0].status, Status::Ok));
        assert!(matches!(spans[2].status, Status::Error { .. }));
        assert!(matches!(spans[3].status, Status::Error { .. }));
        let serialized = format!("{spans:?}");
        assert!(!serialized.contains("private-"));
        assert!(!serialized.contains("secret-fixture"));
        provider.shutdown().unwrap();
    }

    #[tokio::test]
    async fn dropped_observation_is_an_interrupted_error() {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let timed = tokio::time::timeout(
            Duration::from_millis(1),
            observe_with_provider(
                &provider,
                "cancelled-tool",
                "tool",
                std::future::pending::<Result<(), ()>>(),
                |_| false,
            ),
        )
        .await;
        assert!(timed.is_err());
        provider.force_flush().unwrap();
        let spans = exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].status, Status::Error { .. }));
        provider.shutdown().unwrap();
    }
}
