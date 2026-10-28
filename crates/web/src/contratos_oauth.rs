use serde_json::{Value, json};

use crate::contratos::esquema;
use crate::oauth_routes::{OAUTH_ENDPOINTS, OAuthOperation};
use crate::oauth_types::*;

/// Suprime valores confidenciais preservando seus tipos, para não esconder
/// uma quebra de contrato (por exemplo, token nulo ou objeto em vez de string).
pub fn redact_body(mut body: Value) -> Value {
    fn mask(value: &mut Value) {
        match value {
            Value::String(text) => *text = "[REDACTED]".to_string(),
            Value::Array(items) => items.iter_mut().for_each(mask),
            Value::Object(fields) => fields.values_mut().for_each(mask),
            Value::Number(_) => *value = json!(0),
            Value::Bool(_) => *value = json!(false),
            _ => {}
        }
    }
    fn visit(value: &mut Value) {
        match value {
            Value::Array(items) => items.iter_mut().for_each(visit),
            Value::Object(fields) => {
                for (key, value) in fields {
                    if [
                        "access_token",
                        "refresh_token",
                        "client_secret",
                        "password",
                        "code",
                        "code_verifier",
                        "code_challenge",
                        "state",
                        "redirect_uris",
                    ]
                    .contains(&key.as_str())
                    {
                        mask(value);
                    } else {
                        visit(value);
                    }
                }
            }
            _ => {}
        }
    }
    visit(&mut body);
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacao_nao_exporta_credenciais_e_preserva_erros_de_tipo() {
        let body = json!({ "access_token": "sentinela-privada", "refresh_token": null,
            "client_secret": { "unexpected": "sentinela-privada" },
            "redirect_uris": ["https://example.test/?secret=sentinela-privada"],
            "scope": "runtime:read", "expires_in": 3600 });
        let redacted = redact_body(body);
        assert!(!redacted.to_string().contains("sentinela-privada"));
        assert_eq!(redacted["refresh_token"], Value::Null);
        assert!(redacted["client_secret"].is_object());
        assert_eq!(redacted["scope"], "runtime:read");
        assert_eq!(redacted["expires_in"], 3600);
    }
}

pub fn exportar() -> Value {
    let operations: Vec<_> = OAUTH_ENDPOINTS.iter().map(|endpoint| {
        use OAuthOperation::*;
        let browser = matches!(endpoint.operation, McpAuthorize | McpConsent | RuntimeAuthorize | RuntimeConsent);
        let request = match endpoint.operation {
            Register => esquema::<crate::conector::PedidoDeRegistro>(false),
            McpToken => esquema::<crate::conector::PedidoDeToken>(false),
            RuntimeToken => esquema::<crate::runtime_oauth::TokenRequest>(false),
            McpAuthorize | McpConsent => esquema::<crate::conector::PedidoDeAutorizacao>(false),
            RuntimeAuthorize | RuntimeConsent => esquema::<crate::runtime_oauth::AuthorizationRequest>(false),
            _ => Value::Null,
        };
        let response = match endpoint.operation {
            McpMetadata | RuntimeMetadata => esquema::<AuthorizationMetadata>(true),
            McpResource | McpResourceAlias | RuntimeResource => esquema::<ResourceMetadata>(true),
            Register => esquema::<RegistrationResponse>(true),
            McpToken | RuntimeToken => esquema::<TokenResponse>(true),
            _ => esquema::<String>(true),
        };
        let mut responses = serde_json::Map::new();
        for status in [400, 401, 403, 405, 500, 503] {
            responses.insert(status.to_string(), json!({ "anyOf": [esquema::<OAuthError>(true), esquema::<String>(true)] }));
        }
        let status = match endpoint.operation { Register => "201", McpConsent | RuntimeConsent => "303", _ => "200" };
        responses.insert(status.into(), if status == "303" { Value::Null } else { response });
        if browser { responses.insert("302".into(), Value::Null); }
        let mut response_headers = serde_json::Map::new();
        for code in responses.keys() {
            let mut headers = Vec::new();
            if matches!(code.as_str(), "302" | "303") { headers.push("location"); }
            else if code == "200" || !browser && code == "201" { headers.extend(["content-type", "cache-control"]); }
            response_headers.insert(code.clone(), json!(headers));
        }
        json!({
            "id": endpoint.id, "method": endpoint.method, "path": endpoint.path,
            "authentication": if browser { "avila_sso" } else { "public_oauth_client" },
            "request": request,
            "requestEncoding": if endpoint.method == "GET" { "query" } else if endpoint.operation == Register { "json" } else { "form" },
            "requiredHeaders": if matches!(endpoint.operation, McpConsent | RuntimeConsent) { vec!["origin"] } else { vec![] },
            "responses": responses, "responseHeaders": response_headers,
            "successStatuses": [status], "redacted": browser || matches!(endpoint.operation, McpToken | RuntimeToken),
        })
    }).collect();
    json!({ "schemaVersion": 1, "module": "cms-oauth-v1", "operations": operations,
        "policies": ["CMS-ACCESS-004", "CMS-ACCESS-005"],
        "tests": ["crates/web/tests/painel.rs", "crates/web/tests/runtime_oauth.rs", "crates/web/tests/oauth_contracts.rs"],
    })
}
