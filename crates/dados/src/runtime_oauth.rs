//! Persistência OAuth isolada para clientes nativos do runtime AAD.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cms_dominio::Conta;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{ErroDeDados, hash_de_conteudo};

pub const RUNTIME_READ_SCOPE: &str = "runtime:read";
const ACCESS_LIFETIME_SECONDS: i64 = 3600;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeOAuthError {
    #[error(transparent)]
    Data(#[from] ErroDeDados),
    #[error("invalid grant")]
    InvalidGrant,
}

impl From<sqlx::Error> for RuntimeOAuthError {
    fn from(error: sqlx::Error) -> Self {
        Self::Data(ErroDeDados::Banco(error))
    }
}

fn secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn valid_scopes(scopes: &[String]) -> bool {
    !scopes.is_empty() && scopes.iter().all(|scope| scope == RUNTIME_READ_SCOPE)
}

fn valid_redirect_uri(uri: &str, bundle_id: &str) -> bool {
    if uri.is_empty()
        || uri.len() > 512
        || uri.contains('#')
        || uri
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return false;
    }
    if let Some(remainder) = uri.strip_prefix("https://") {
        let authority = remainder.split(['/', '?']).next().unwrap_or_default();
        return !authority.is_empty() && !authority.contains('@');
    }
    uri.strip_prefix(bundle_id)
        .is_some_and(|remainder| remainder.starts_with(":/") && !remainder.starts_with("://"))
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct RuntimeClient {
    pub id: Uuid,
    pub app_id: Uuid,
    pub name: String,
    pub redirect_uris: Vec<String>,
}

pub async fn register_runtime_client(
    pool: &PgPool,
    app_id: Uuid,
    name: &str,
    redirect_uris: &[String],
) -> Result<Uuid, RuntimeOAuthError> {
    let bundle_id = sqlx::query_scalar::<_, String>("select bundle_id from app where id = $1")
        .bind(app_id)
        .fetch_optional(pool)
        .await?
        .ok_or(RuntimeOAuthError::InvalidGrant)?;
    if name.trim().is_empty()
        || !(1..=5).contains(&redirect_uris.len())
        || redirect_uris
            .iter()
            .any(|uri| !valid_redirect_uri(uri, &bundle_id))
    {
        return Err(RuntimeOAuthError::InvalidGrant);
    }
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into runtime_oauth_client (app_id, name, redirect_uris)
        values ($1, $2, $3)
        returning id
        "#,
    )
    .bind(app_id)
    .bind(name)
    .bind(redirect_uris)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn runtime_client(pool: &PgPool, id: Uuid) -> Result<Option<RuntimeClient>, ErroDeDados> {
    Ok(sqlx::query_as::<_, RuntimeClient>(
        "select id, app_id, name, redirect_uris from runtime_oauth_client where id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAuthorization<'a> {
    pub client_id: Uuid,
    pub app_id: Uuid,
    pub account: &'a Conta,
    pub scopes: &'a [String],
    pub redirect_uri: &'a str,
    pub challenge: &'a str,
}

pub async fn create_runtime_code(
    pool: &PgPool,
    authorization: &RuntimeAuthorization<'_>,
) -> Result<String, RuntimeOAuthError> {
    if !valid_scopes(authorization.scopes) || authorization.challenge.is_empty() {
        return Err(RuntimeOAuthError::InvalidGrant);
    }
    let code = secret();
    let result = sqlx::query(
        r#"
        insert into runtime_oauth_code (
            hash, client_id, app_id, account_sub, account_email, account_name,
            account_team, scopes, redirect_uri, challenge, expires_at
        )
        select $1, id, app_id, $4, $5, $6, $7, $8, $9, $10,
               now() + interval '5 minutes'
        from runtime_oauth_client
        where id = $2 and app_id = $3 and $9 = any(redirect_uris)
        "#,
    )
    .bind(hash_de_conteudo(code.as_bytes()))
    .bind(authorization.client_id)
    .bind(authorization.app_id)
    .bind(&authorization.account.sub)
    .bind(&authorization.account.email)
    .bind(&authorization.account.nome)
    .bind(authorization.account.equipe)
    .bind(authorization.scopes)
    .bind(authorization.redirect_uri)
    .bind(authorization.challenge)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(RuntimeOAuthError::InvalidGrant);
    }
    Ok(code)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTokens {
    pub access: String,
    pub refresh: String,
    pub expires_in_seconds: i64,
    pub scopes: Vec<String>,
}

#[derive(FromRow)]
struct ExchangedCode {
    client_id: Uuid,
    app_id: Uuid,
    account_sub: String,
    account_email: String,
    account_name: String,
    account_team: bool,
    scopes: Vec<String>,
    redirect_uri: String,
    challenge: String,
}

pub async fn exchange_runtime_code(
    pool: &PgPool,
    code: &str,
    client_id: Uuid,
    redirect_uri: &str,
    verifier: &str,
) -> Result<RuntimeTokens, RuntimeOAuthError> {
    let mut transaction = pool.begin().await?;
    let row = sqlx::query_as::<_, ExchangedCode>(
        r#"
        update runtime_oauth_code set used_at = now()
        where hash = $1 and used_at is null and expires_at > now()
        returning client_id, app_id, account_sub, account_email, account_name,
                  account_team, scopes, redirect_uri, challenge
        "#,
    )
    .bind(hash_de_conteudo(code.as_bytes()))
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(RuntimeOAuthError::InvalidGrant)?;
    transaction.commit().await?;

    if row.client_id != client_id
        || row.redirect_uri != redirect_uri
        || row.challenge != challenge_for(verifier)
    {
        return Err(RuntimeOAuthError::InvalidGrant);
    }

    let (access, refresh) = (secret(), secret());
    sqlx::query(
        r#"
        insert into runtime_oauth_session (
            client_id, app_id, account_sub, account_email, account_name,
            account_team, scopes, access_hash, access_expires_at,
            refresh_hash, refresh_expires_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8,
                now() + make_interval(secs => $9), $10, now() + interval '30 days')
        "#,
    )
    .bind(row.client_id)
    .bind(row.app_id)
    .bind(row.account_sub)
    .bind(row.account_email)
    .bind(row.account_name)
    .bind(row.account_team)
    .bind(&row.scopes)
    .bind(hash_de_conteudo(access.as_bytes()))
    .bind(ACCESS_LIFETIME_SECONDS as f64)
    .bind(hash_de_conteudo(refresh.as_bytes()))
    .execute(pool)
    .await?;

    Ok(RuntimeTokens {
        access,
        refresh,
        expires_in_seconds: ACCESS_LIFETIME_SECONDS,
        scopes: row.scopes,
    })
}

pub async fn rotate_runtime_tokens(
    pool: &PgPool,
    refresh: &str,
    client_id: Uuid,
) -> Result<RuntimeTokens, RuntimeOAuthError> {
    let (access, next_refresh) = (secret(), secret());
    let scopes = sqlx::query_scalar::<_, Vec<String>>(
        r#"
        update runtime_oauth_session
        set access_hash = $3,
            access_expires_at = now() + make_interval(secs => $4),
            refresh_hash = $5,
            refresh_expires_at = now() + interval '30 days'
        where refresh_hash = $1 and client_id = $2 and revoked_at is null
          and refresh_expires_at > now()
        returning scopes
        "#,
    )
    .bind(hash_de_conteudo(refresh.as_bytes()))
    .bind(client_id)
    .bind(hash_de_conteudo(access.as_bytes()))
    .bind(ACCESS_LIFETIME_SECONDS as f64)
    .bind(hash_de_conteudo(next_refresh.as_bytes()))
    .fetch_optional(pool)
    .await?
    .ok_or(RuntimeOAuthError::InvalidGrant)?;

    Ok(RuntimeTokens {
        access,
        refresh: next_refresh,
        expires_in_seconds: ACCESS_LIFETIME_SECONDS,
        scopes,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSession {
    pub id: Uuid,
    pub client_id: Uuid,
    pub app_id: Uuid,
    pub account: Conta,
    pub scopes: Vec<String>,
}

impl RuntimeSession {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|candidate| candidate == scope)
    }
}

#[derive(FromRow)]
struct RuntimeSessionRow {
    id: Uuid,
    client_id: Uuid,
    app_id: Uuid,
    account_sub: String,
    account_email: String,
    account_name: String,
    account_team: bool,
    scopes: Vec<String>,
}

pub async fn runtime_session_by_access_token(
    pool: &PgPool,
    access: &str,
) -> Result<Option<RuntimeSession>, ErroDeDados> {
    let row = sqlx::query_as::<_, RuntimeSessionRow>(
        r#"
        select id, client_id, app_id, account_sub, account_email, account_name,
               account_team, scopes
        from runtime_oauth_session
        where access_hash = $1 and revoked_at is null and access_expires_at > now()
        "#,
    )
    .bind(hash_de_conteudo(access.as_bytes()))
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| RuntimeSession {
        id: row.id,
        client_id: row.client_id,
        app_id: row.app_id,
        account: Conta {
            sub: row.account_sub,
            email: row.account_email,
            nome: row.account_name,
            equipe: row.account_team,
        },
        scopes: row.scopes,
    }))
}

pub async fn revoke_runtime_session(pool: &PgPool, id: Uuid) -> Result<(), ErroDeDados> {
    sqlx::query(
        "update runtime_oauth_session set revoked_at = now() where id = $1 and revoked_at is null",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
