mod comum;

use std::collections::HashMap;
use std::path::PathBuf;

use axum::body::Body;
use axum::extract::Query;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use cms_dominio::Conta;
use cms_integracoes::auth::ClienteAuth;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use comum::{HOST_DO_PAINEL, Resposta, estado, responder};

const REDIRECT_URI: &str = "com.avilaops.runtime:/oauth/callback";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

async fn session(
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let signed_in = headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("avila_sso=a.b.ana"));
    if signed_in && query.get("app").is_some_and(|app| app == "cms") {
        (
            StatusCode::OK,
            Json(json!({
                "autenticado": true,
                "permitido": true,
                "sessao": {
                    "sub": "sub-ana",
                    "email": "ana@example.test",
                    "nome": "Ana",
                    "papel": "CLIENTE"
                }
            })),
        )
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "autenticado": false })),
        )
    }
}

async fn test_auth() -> ClienteAuth {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("free port");
    let address = listener.local_addr().expect("auth address");
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/api/session", get(session)))
            .await
            .expect("test auth");
    });
    ClienteAuth::novo(&format!("http://{address}"), "cms").expect("auth client")
}

async fn request(pool: &PgPool, method: &str, path: &str, cookie: bool, body: &str) -> Resposta {
    let mut state = estado(pool, PathBuf::from("missing-media"));
    state.auth = Some(test_auth().await);
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", HOST_DO_PAINEL)
        .header("content-type", "application/x-www-form-urlencoded");
    if cookie {
        builder = builder
            .header("cookie", "avila_sso=a.b.ana")
            .header("origin", "https://cms.teste");
    }
    responder(
        state,
        builder
            .body(Body::from(body.to_string()))
            .expect("valid request"),
    )
    .await
}

async fn app_and_client(pool: &PgPool) -> (Uuid, Uuid) {
    let account = Conta {
        sub: "sub-ana".into(),
        email: "ana@example.test".into(),
        nome: "Ana".into(),
        equipe: false,
    };
    let app = cms_dados::criar_app(
        pool,
        &account,
        "runtime-oauth-http",
        "Runtime OAuth HTTP",
        "com.avilaops.runtime",
        Uuid::new_v4(),
    )
    .await
    .expect("app should be created");
    let client = cms_dados::register_runtime_client(
        pool,
        app.id,
        "Runtime iOS",
        &[REDIRECT_URI.to_string()],
    )
    .await
    .expect("client should be registered");
    (app.id, client)
}

fn authorize_path(client: Uuid) -> String {
    format!(
        "/oauth/runtime/authorize?response_type=code&client_id={client}&redirect_uri={}&code_challenge={CHALLENGE}&code_challenge_method=S256&scope=runtime%3Aread&state=state-123",
        "com.avilaops.runtime%3A%2Foauth%2Fcallback"
    )
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn native_authorization_requires_login_registered_redirect_and_membership(pool: PgPool) {
    let (_, client) = app_and_client(&pool).await;
    let path = authorize_path(client);

    let signed_out = request(&pool, "GET", &path, false, "").await;
    assert_eq!(signed_out.status, StatusCode::FOUND);

    let page = request(&pool, "GET", &path, true, "").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.corpo);
    assert!(page.corpo.contains("Runtime OAuth HTTP"));
    assert!(page.corpo.contains("runtime:read"));

    let wrong_redirect = path.replace(
        "com.avilaops.runtime%3A%2Foauth%2Fcallback",
        "com.attacker.app%3A%2Foauth%2Fcallback",
    );
    assert_eq!(
        request(&pool, "GET", &wrong_redirect, true, "")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    let missing_state = path.replace("&state=state-123", "");
    assert_eq!(
        request(&pool, "GET", &missing_state, true, "").await.status,
        StatusCode::BAD_REQUEST
    );
    let long_challenge = path.replace(CHALLENGE, &format!("{CHALLENGE}A"));
    assert_eq!(
        request(&pool, "GET", &long_challenge, true, "")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn native_code_exchange_and_refresh_preserve_state_and_oauth_errors(pool: PgPool) {
    let (_, client) = app_and_client(&pool).await;
    let body = format!(
        "response_type=code&client_id={client}&redirect_uri=com.avilaops.runtime%3A%2Foauth%2Fcallback&code_challenge={CHALLENGE}&code_challenge_method=S256&scope=runtime%3Aread&state=state-123&decision=authorize"
    );
    let authorized = request(&pool, "POST", "/oauth/runtime/authorize", true, &body).await;
    assert_eq!(
        authorized.status,
        StatusCode::SEE_OTHER,
        "{}",
        authorized.corpo
    );
    let location = authorized.cabecalho("location");
    assert!(location.contains("state=state-123"), "{location}");
    let code = location
        .split("code=")
        .nth(1)
        .and_then(|value| value.split('&').next())
        .expect("authorization code");

    let token_body = format!(
        "grant_type=authorization_code&code={code}&code_verifier={VERIFIER}&client_id={client}&redirect_uri=com.avilaops.runtime%3A%2Foauth%2Fcallback"
    );
    let exchanged = request(&pool, "POST", "/oauth/runtime/token", false, &token_body).await;
    assert_eq!(exchanged.status, StatusCode::OK, "{}", exchanged.corpo);
    let tokens: Value = serde_json::from_str(&exchanged.corpo).expect("token JSON");
    assert_eq!(tokens["scope"], "runtime:read");
    assert_eq!(tokens["token_type"], "Bearer");

    let refresh = tokens["refresh_token"].as_str().expect("refresh token");
    let refreshed = request(
        &pool,
        "POST",
        "/oauth/runtime/token",
        false,
        &format!("grant_type=refresh_token&refresh_token={refresh}&client_id={client}"),
    )
    .await;
    assert_eq!(refreshed.status, StatusCode::OK, "{}", refreshed.corpo);
    let refresh_replay = request(
        &pool, "POST", "/oauth/runtime/token", false,
        &format!("grant_type=refresh_token&refresh_token={refresh}&client_id={client}"),
    ).await;
    assert_eq!(refresh_replay.status, StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_str(&refresh_replay.corpo).expect("OAuth error");
    assert_eq!(error["error"], "invalid_grant");

    let reused = request(&pool, "POST", "/oauth/runtime/token", false, &token_body).await;
    assert_eq!(reused.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        serde_json::from_str::<Value>(&reused.corpo).expect("error JSON")["error"],
        "invalid_grant"
    );
}
