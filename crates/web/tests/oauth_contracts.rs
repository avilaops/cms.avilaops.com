mod comum;

use std::path::PathBuf;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use sqlx::PgPool;
use comum::{HOST_DO_PAINEL, estado, responder};

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn descoberta_oauth_distingue_recursos_e_preserva_alias_mcp(pool: PgPool) {
    for path in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/oauth-authorization-server/runtime",
        "/.well-known/oauth-protected-resource",
        "/.well-known/oauth-protected-resource/mcp",
        "/.well-known/oauth-protected-resource/api/runtime",
    ] {
        let request = Request::builder().uri(path).header("host", HOST_DO_PAINEL)
            .body(Body::empty()).expect("pedido");
        let response = responder(estado(&pool, PathBuf::from("missing-media")), request).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.cabecalho("cache-control"), "no-store");
        let body: Value = serde_json::from_str(&response.corpo).expect("metadados JSON");
        if path.contains("authorization-server") {
            assert_eq!(body["code_challenge_methods_supported"], serde_json::json!(["S256"]));
            let native = path.ends_with("/runtime");
            assert_eq!(body["token_endpoint"], if native { "https://cms.teste/oauth/runtime/token" } else { "https://cms.teste/oauth/token" });
            assert_eq!(body.get("registration_endpoint").is_none(), native);
        } else {
            assert_eq!(body["resource"], if path.ends_with("/api/runtime") { "https://cms.teste/api/runtime" } else { "https://cms.teste/mcp" });
        }
    }
}

#[cfg(feature = "contracts")]
#[test]
fn contrato_exporta_todas_as_operacoes_oauth_sem_confundir_credenciais() {
    let contract = cms_web::contratos_oauth::exportar();
    let operations = contract["operations"].as_array().expect("operações");
    assert_eq!(operations.len(), 12);
    let token = operations.iter().find(|op| op["id"] == "runtimeToken").expect("token nativo");
    assert_eq!(token["requestEncoding"], "form");
    assert_eq!(token["redacted"], true);
    assert_eq!(token["responses"]["200"]["properties"]["expires_in"]["type"], "integer");
    let consent = operations.iter().find(|op| op["id"] == "mcpConsent").expect("consentimento");
    assert_eq!(consent["authentication"], "avila_sso");
    assert_eq!(consent["responses"]["303"], Value::Null);
}
