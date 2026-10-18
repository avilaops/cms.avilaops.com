use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cms_dados::{
    RUNTIME_READ_SCOPE, RuntimeAuthorization, create_runtime_code, exchange_runtime_code,
    register_runtime_client, revoke_runtime_session, rotate_runtime_tokens, runtime_client,
    runtime_session_by_access_token,
};
use cms_dominio::Conta;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

const REDIRECT_URI: &str = "com.avilaops.runtime:/oauth/callback";
const VERIFIER: &str = "runtime-verifier-with-more-than-forty-three-characters-123";

async fn create_app(pool: &PgPool, slug: &str, bundle_id: &str) -> Uuid {
    sqlx::query_scalar(
        r#"
        insert into app (slug, nome, bundle_id, criado_por)
        values ($1, 'Runtime test', $2, 'owner-sub')
        returning id
        "#,
    )
    .bind(slug)
    .bind(bundle_id)
    .fetch_one(pool)
    .await
    .expect("app should be created")
}

fn account() -> Conta {
    Conta {
        sub: "member-sub".into(),
        email: "member@avilaops.com".into(),
        nome: "Runtime Member".into(),
        equipe: false,
    }
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

async fn authorize(pool: &PgPool, app_id: Uuid) -> (Uuid, String) {
    let client_id =
        register_runtime_client(pool, app_id, "Runtime iOS", &[REDIRECT_URI.to_string()])
            .await
            .expect("runtime client should be registered");
    let scopes = vec![RUNTIME_READ_SCOPE.to_string()];
    let code = create_runtime_code(
        pool,
        &RuntimeAuthorization {
            client_id,
            app_id,
            account: &account(),
            scopes: &scopes,
            redirect_uri: REDIRECT_URI,
            challenge: &challenge(VERIFIER),
        },
    )
    .await
    .expect("authorization code should be created");
    (client_id, code)
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn runtime_client_is_bound_to_one_app_and_redirect_allowlist(pool: PgPool) {
    let app_id = create_app(&pool, "runtime-client", "com.avilaops.runtime").await;
    let other_app_id = create_app(&pool, "other-runtime-client", "com.avilaops.other").await;
    let client_id =
        register_runtime_client(&pool, app_id, "Runtime iOS", &[REDIRECT_URI.to_string()])
            .await
            .expect("runtime client should be registered");

    let client = runtime_client(&pool, client_id)
        .await
        .expect("client lookup should succeed")
        .expect("client should exist");
    assert_eq!(client.app_id, app_id);
    assert_ne!(client.app_id, other_app_id);
    assert_eq!(client.redirect_uris, vec![REDIRECT_URI]);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn runtime_client_rejects_redirects_outside_bundle_and_cardinality(pool: PgPool) {
    let app_id = create_app(&pool, "runtime-redirects", "com.avilaops.runtime").await;
    assert!(
        register_runtime_client(
            &pool,
            app_id,
            "Runtime iOS",
            &["com.attacker.runtime:/oauth/callback".to_string()],
        )
        .await
        .is_err()
    );
    let too_many = (0..6)
        .map(|index| format!("https://runtime{index}.example.test/callback"))
        .collect::<Vec<_>>();
    assert!(
        register_runtime_client(&pool, app_id, "Runtime iOS", &too_many)
            .await
            .is_err()
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn code_exchange_uses_pkce_once_and_only_persists_token_hashes(pool: PgPool) {
    let app_id = create_app(&pool, "runtime-exchange", "com.avilaops.runtime").await;
    let (client_id, code) = authorize(&pool, app_id).await;

    let tokens = exchange_runtime_code(&pool, &code, client_id, REDIRECT_URI, VERIFIER)
        .await
        .expect("valid PKCE exchange should succeed");
    let session = runtime_session_by_access_token(&pool, &tokens.access)
        .await
        .expect("session lookup should succeed")
        .expect("access token should resolve");
    assert_eq!(session.client_id, client_id);
    assert_eq!(session.app_id, app_id);
    assert_eq!(session.account, account());
    assert!(session.has_scope(RUNTIME_READ_SCOPE));

    let stored: (String, String) =
        sqlx::query_as("select access_hash, refresh_hash from runtime_oauth_session where id = $1")
            .bind(session.id)
            .fetch_one(&pool)
            .await
            .expect("stored token hashes should exist");
    assert_ne!(stored.0, tokens.access);
    assert_ne!(stored.1, tokens.refresh);
    assert!(!stored.0.contains(&tokens.access));
    assert!(!stored.1.contains(&tokens.refresh));

    assert!(
        exchange_runtime_code(&pool, &code, client_id, REDIRECT_URI, VERIFIER)
            .await
            .is_err(),
        "authorization code must be single-use"
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn invalid_pkce_consumes_code_without_issuing_tokens(pool: PgPool) {
    let app_id = create_app(&pool, "runtime-invalid-pkce", "com.avilaops.runtime").await;
    let (client_id, code) = authorize(&pool, app_id).await;

    assert!(
        exchange_runtime_code(&pool, &code, client_id, REDIRECT_URI, "wrong-verifier")
            .await
            .is_err()
    );
    assert!(
        exchange_runtime_code(&pool, &code, client_id, REDIRECT_URI, VERIFIER)
            .await
            .is_err(),
        "a failed verifier attempt must still consume the code"
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn refresh_rotation_invalidates_old_credentials_and_revoke_closes_session(pool: PgPool) {
    let app_id = create_app(&pool, "runtime-rotation", "com.avilaops.runtime").await;
    let (client_id, code) = authorize(&pool, app_id).await;
    let first = exchange_runtime_code(&pool, &code, client_id, REDIRECT_URI, VERIFIER)
        .await
        .expect("code exchange should succeed");
    let rotated = rotate_runtime_tokens(&pool, &first.refresh, client_id)
        .await
        .expect("refresh rotation should succeed");

    assert!(
        runtime_session_by_access_token(&pool, &first.access)
            .await
            .expect("old access lookup should succeed")
            .is_none()
    );
    assert!(
        rotate_runtime_tokens(&pool, &first.refresh, client_id)
            .await
            .is_err()
    );

    let session = runtime_session_by_access_token(&pool, &rotated.access)
        .await
        .expect("new access lookup should succeed")
        .expect("rotated access should resolve");
    revoke_runtime_session(&pool, session.id)
        .await
        .expect("session revocation should succeed");
    assert!(
        runtime_session_by_access_token(&pool, &rotated.access)
            .await
            .expect("revoked access lookup should succeed")
            .is_none()
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn expired_access_and_refresh_tokens_fail_closed(pool: PgPool) {
    let app_id = create_app(&pool, "runtime-expiration", "com.avilaops.runtime").await;
    let (client_id, code) = authorize(&pool, app_id).await;
    let tokens = exchange_runtime_code(&pool, &code, client_id, REDIRECT_URI, VERIFIER)
        .await
        .expect("code exchange should succeed");

    sqlx::query(
        "update runtime_oauth_session set access_expires_at = now() - interval '1 second', refresh_expires_at = now() - interval '1 second'",
    )
    .execute(&pool)
    .await
    .expect("test should expire session");

    assert!(
        runtime_session_by_access_token(&pool, &tokens.access)
            .await
            .expect("expired access lookup should succeed")
            .is_none()
    );
    assert!(
        rotate_runtime_tokens(&pool, &tokens.refresh, client_id)
            .await
            .is_err()
    );
}
