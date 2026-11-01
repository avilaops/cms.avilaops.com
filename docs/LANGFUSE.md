# Langfuse tracing

Tracing is optional and disabled by default. This service emits metadata-only
MCP tool observations through the official Rust OpenTelemetry 0.33.1 SDK and
OTLP/HTTP protobuf. The destination is Langfuse v4
(`/api/public/otel/v1/traces`, ingestion-version header 4).

## Runtime configuration

Configure these variables in the service's secret manager or authorized runtime
environment, never in committed files or browser bundles:

- `LANGFUSE_ENABLED=true`
- `LANGFUSE_BASE_URL`: explicit Langfuse project region or self-hosted base URL.
  HTTPS is required outside literal loopback; URL credentials, queries and
  fragments are rejected.
- `LANGFUSE_PUBLIC_KEY` and `LANGFUSE_SECRET_KEY`: project API key pair.
- `LANGFUSE_TRACING_ENVIRONMENT`: `development` (default), `test`, `staging`
  or `production`.
- `LANGFUSE_SAMPLE_RATE`: finite fraction from 0 to 1 (default 1); parent sampling
  decisions are respected.

Missing/invalid configuration leaves tracing off and does not block application
work. Exporter setup failures emit a fixed diagnostic without printing keys or
remote error details. Export uses a bounded SDK batch queue and a three-second
HTTP timeout; graceful process exit drains the provider off the async runtime.
No global OpenTelemetry provider is replaced and ordinary HTTP/SQL logs are not
exported to Langfuse.

## Captured data

Only fixed operation names, observation types, service/version/environment,
timing, parent-child relationships and success/error classifications are exported.
Observation input explicitly says content is omitted; output contains status only.
No XML, PDF, document/access key, connection identifier, arguments, result body,
token, header, raw error message or personal data is captured. Tool calls are
`tool` observations, never synthetic LLM generations. Model/token/cost fields
are intentionally absent because this service does not invoke an LLM.

## Validation and rollout

All builds and application tests run exclusively in the existing GitHub CI.
Tests cover unsafe/missing configuration, sampling bounds, parent-child spans,
content exclusion, unchanged results/errors, rejected results and cancellation.
Instrumentation does not constitute a deployment.

After an authorized deployment with project credentials, run one legitimate
operation, fetch its observations using the Langfuse CLI, and audit against
[the current tracing best practices](https://langfuse.com/docs/observability/best-practices).
Confirm nested observations, status classification, absence of private content,
and successful delivery on shutdown. No live Langfuse trace has been verified
until this step is completed.

Reference: [Langfuse OpenTelemetry integration](https://langfuse.com/integrations/native/opentelemetry).
