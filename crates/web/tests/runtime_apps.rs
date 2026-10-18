mod comum;

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cms_dados::{RUNTIME_READ_SCOPE, RuntimeAuthorization};
use cms_dominio::Conta;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use comum::{HOST_DO_PAINEL, Resposta, estado, responder};

const REDIRECT_URI: &str = "com.avilaops.runtime:/oauth/callback";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

fn account() -> Conta {
    Conta {
        sub: "sub-ana".into(),
        email: "ana@example.test".into(),
        nome: "Ana".into(),
        equipe: false,
    }
}

async fn published_app(pool: &PgPool, slug: &str) -> Uuid {
    let app = cms_dados::criar_app(
        pool,
        &account(),
        slug,
        "Published Runtime",
        &format!("com.avilaops.{slug}"),
        Uuid::new_v4(),
    )
    .await
    .expect("app should be created");
    let version_id: Uuid = sqlx::query_scalar(
        r#"
        insert into app_versao (
            app_id, numero, schema_version, minimum_runtime_version,
            definicao, criado_por
        )
        values ($1, 7, '1.0.0', '1.0.0', $2, 'sub-ana')
        returning id
        "#,
    )
    .bind(app.id)
    .bind(json!({
        "schemaVersion": "1.0.0",
        "appId": slug,
        "minimumRuntimeVersion": "1.0.0",
        "navigation": { "initialRoute": "home" },
        "screens": [{ "id": "home", "route": "home", "title": "Home", "components": [] }]
    }))
    .fetch_one(pool)
    .await
    .expect("version should be created");
    sqlx::query(
        "insert into app_ambiente (app_id, nome, versao_id, atualizado_por) values ($1, 'production', $2, 'sub-ana')",
    )
    .bind(app.id)
    .bind(version_id)
    .execute(pool)
    .await
    .expect("version should be published");
    app.id
}

async fn runtime_token(pool: &PgPool, app_id: Uuid) -> String {
    let client_id = cms_dados::register_runtime_client(
        pool,
        app_id,
        "Runtime iOS",
        &[REDIRECT_URI.to_string()],
    )
    .await
    .expect("runtime client should be registered");
    let scopes = vec![RUNTIME_READ_SCOPE.to_string()];
    let code = cms_dados::create_runtime_code(
        pool,
        &RuntimeAuthorization {
            client_id,
            app_id,
            account: &account(),
            scopes: &scopes,
            redirect_uri: REDIRECT_URI,
            challenge: CHALLENGE,
        },
    )
    .await
    .expect("code should be created");
    cms_dados::exchange_runtime_code(pool, &code, client_id, REDIRECT_URI, VERIFIER)
        .await
        .expect("code should be exchanged")
        .access
}

async fn get(pool: &PgPool, path: &str, token: Option<&str>, etag: Option<&str>) -> Resposta {
    let mut request = Request::builder().uri(path).header("host", HOST_DO_PAINEL);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if let Some(etag) = etag {
        request = request.header("if-none-match", etag);
    }
    responder(
        estado(pool, PathBuf::from("missing-media")),
        request.body(Body::empty()).expect("valid request"),
    )
    .await
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn runtime_serves_only_published_definition_and_honors_etag_after_auth(pool: PgPool) {
    let app_id = published_app(&pool, "runtime-delivery").await;
    let token = runtime_token(&pool, app_id).await;
    let path = "/api/runtime/v1/apps/runtime-delivery/environments/production";

    let response = get(&pool, path, Some(&token), None).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.corpo);
    assert_eq!(response.cabecalho("cache-control"), "private, no-store");
    assert!(!response.cabecalho("x-correlation-id").is_empty());
    let etag = response.cabecalho("etag").to_string();
    assert!(!etag.is_empty());
    let body: Value = serde_json::from_str(&response.corpo).expect("runtime JSON");
    assert_eq!(body["appId"], "runtime-delivery");
    assert_eq!(body["environment"], "production");
    assert_eq!(body["version"], 7);
    assert_eq!(body["definition"]["appId"], "runtime-delivery");

    let not_modified = get(&pool, path, Some(&token), Some(&etag)).await;
    assert_eq!(not_modified.status, StatusCode::NOT_MODIFIED);
    assert!(not_modified.corpo.is_empty());
    assert_eq!(not_modified.cabecalho("etag"), etag);

    let unauthenticated = get(&pool, path, None, Some(&etag)).await;
    assert_eq!(unauthenticated.status, StatusCode::UNAUTHORIZED);
    assert!(
        unauthenticated
            .cabecalho("www-authenticate")
            .contains("/.well-known/oauth-protected-resource/api/runtime")
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn runtime_hides_cross_app_missing_environment_and_revoked_membership(pool: PgPool) {
    let app_id = published_app(&pool, "runtime-authorized").await;
    published_app(&pool, "runtime-other").await;
    let token = runtime_token(&pool, app_id).await;

    for path in [
        "/api/runtime/v1/apps/runtime-other/environments/production",
        "/api/runtime/v1/apps/runtime-authorized/environments/staging",
        "/api/runtime/v1/apps/runtime-authorized/environments/unknown",
    ] {
        assert_eq!(
            get(&pool, path, Some(&token), None).await.status,
            StatusCode::NOT_FOUND
        );
    }

    sqlx::query("delete from app_participacao where app_id = $1 and conta = 'sub-ana'")
        .bind(app_id)
        .execute(&pool)
        .await
        .expect("membership should be revoked");
    assert_eq!(
        get(
            &pool,
            "/api/runtime/v1/apps/runtime-authorized/environments/production",
            Some(&token),
            None,
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn mcp_and_runtime_tokens_cannot_cross_resource_boundaries(pool: PgPool) {
    let app_id = published_app(&pool, "runtime-token-separation").await;
    let runtime = runtime_token(&pool, app_id).await;
    let mcp = cms_dados::criar_conexao_de_servico(
        &pool,
        "runtime-separation-test",
        &["sites:ler".to_string()],
    )
    .await
    .expect("MCP token should be created");

    let runtime_path = "/api/runtime/v1/apps/runtime-token-separation/environments/production";
    assert_eq!(
        get(&pool, runtime_path, Some(&mcp), None).await.status,
        StatusCode::UNAUTHORIZED
    );

    let request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", HOST_DO_PAINEL)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {runtime}"))
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))
        .expect("valid MCP request");
    let response = responder(estado(&pool, PathBuf::from("missing-media")), request).await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
}
