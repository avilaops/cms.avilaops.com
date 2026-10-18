//! OAuth para runtimes nativos. O recurso, os clientes e os tokens são
//! deliberadamente separados do conector MCP.

use askama::Template;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use cms_dados::{RUNTIME_READ_SCOPE, RuntimeAuthorization, RuntimeOAuthError, RuntimeTokens};
use cms_dominio::Conta;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::Estado;
use crate::resposta::{ErroWeb, html_privado_com_retorno, redirecionar, simples};

fn json_response(status: StatusCode, body: Value) -> Response {
    let mut response = (status, body.to_string()).into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn oauth_error(status: StatusCode, code: &str, description: &str) -> Response {
    json_response(
        status,
        json!({ "error": code, "error_description": description }),
    )
}

pub fn authorization_metadata(origin: &str) -> Response {
    json_response(
        StatusCode::OK,
        json!({
            "issuer": origin,
            "authorization_endpoint": format!("{origin}/oauth/runtime/authorize"),
            "token_endpoint": format!("{origin}/oauth/runtime/token"),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "scopes_supported": [RUNTIME_READ_SCOPE],
        }),
    )
}

pub fn protected_resource_metadata(origin: &str) -> Response {
    json_response(
        StatusCode::OK,
        json!({
            "resource": format!("{origin}/api/runtime"),
            "authorization_servers": [origin],
            "scopes_supported": [RUNTIME_READ_SCOPE],
        }),
    )
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct AuthorizationRequest {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    code_challenge_method: String,
    scope: String,
    state: String,
}

#[derive(Template)]
#[template(path = "autorizar_runtime.html")]
struct AuthorizationPage {
    app_name: String,
    account: String,
    request: AuthorizationRequest,
}

async fn validate_request(
    state: &Estado,
    account: &Conta,
    request: &AuthorizationRequest,
) -> Result<Result<(Uuid, Uuid, String), Response>, ErroWeb> {
    let reject = |message: &'static str| Ok(Err(simples(StatusCode::BAD_REQUEST, message)));
    let Ok(client_id) = Uuid::parse_str(&request.client_id) else {
        return reject("Pedido de autorização inválido: cliente desconhecido.");
    };
    let Some(client) = cms_dados::runtime_client(&state.pool, client_id).await? else {
        return reject("Pedido de autorização inválido: cliente desconhecido.");
    };
    if !client.redirect_uris.contains(&request.redirect_uri) {
        return reject("Pedido de autorização inválido: retorno não registrado.");
    }
    let valid_challenge = request.code_challenge.len() == 43
        && request.code_challenge.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        });
    if request.response_type != "code"
        || request.code_challenge_method != "S256"
        || !valid_challenge
        || request.scope != RUNTIME_READ_SCOPE
        || request.state.is_empty()
        || request.state.len() > 256
        || request.state.chars().any(char::is_control)
    {
        return reject("Pedido de autorização inválido: use código, PKCE S256 e runtime:read.");
    }
    let app_name =
        match cms_dados::runtime_app_name_for_member(&state.pool, &account.sub, client.app_id)
            .await?
        {
            Some(name) => name,
            None => return reject("Pedido de autorização inválido: aplicativo indisponível."),
        };
    Ok(Ok((client_id, client.app_id, app_name)))
}

pub async fn authorization_page(
    state: &Estado,
    account: &Conta,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let request = serde_urlencoded::from_str::<AuthorizationRequest>(uri.query().unwrap_or(""))
        .unwrap_or_default();
    let (_, _, app_name) = match validate_request(state, account, &request).await? {
        Ok(validated) => validated,
        Err(response) => return Ok(response),
    };
    let redirect_uri = request.redirect_uri.clone();
    let body = AuthorizationPage {
        app_name,
        account: account.email.clone(),
        request,
    }
    .render()?;
    Ok(html_privado_com_retorno(
        StatusCode::OK,
        body,
        &redirect_uri,
    ))
}

fn with_parameters(address: &str, parameters: &[(&str, &str)]) -> String {
    let query = serde_urlencoded::to_string(parameters).unwrap_or_default();
    let separator = if address.contains('?') { '&' } else { '?' };
    format!("{address}{separator}{query}")
}

pub async fn authorize(state: &Estado, account: &Conta, body: &[u8]) -> Result<Response, ErroWeb> {
    let fields = serde_urlencoded::from_bytes::<Vec<(String, String)>>(body).unwrap_or_default();
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let request = AuthorizationRequest {
        response_type: field("response_type"),
        client_id: field("client_id"),
        redirect_uri: field("redirect_uri"),
        code_challenge: field("code_challenge"),
        code_challenge_method: field("code_challenge_method"),
        scope: field("scope"),
        state: field("state"),
    };
    let (client_id, app_id, _) = match validate_request(state, account, &request).await? {
        Ok(validated) => validated,
        Err(response) => return Ok(response),
    };
    if field("decision") != "authorize" {
        return Ok(redirecionar(
            StatusCode::SEE_OTHER,
            &with_parameters(
                &request.redirect_uri,
                &[("error", "access_denied"), ("state", &request.state)],
            ),
        ));
    }
    let scopes = vec![RUNTIME_READ_SCOPE.to_string()];
    let code = cms_dados::create_runtime_code(
        &state.pool,
        &RuntimeAuthorization {
            client_id,
            app_id,
            account,
            scopes: &scopes,
            redirect_uri: &request.redirect_uri,
            challenge: &request.code_challenge,
        },
    )
    .await;
    let code = match code {
        Ok(code) => code,
        Err(RuntimeOAuthError::Data(error)) => return Err(error.into()),
        Err(RuntimeOAuthError::InvalidGrant) => {
            return Ok(oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "O pedido de autorização não confere.",
            ));
        }
    };
    Ok(redirecionar(
        StatusCode::SEE_OTHER,
        &with_parameters(
            &request.redirect_uri,
            &[("code", &code), ("state", &request.state)],
        ),
    ))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TokenRequest {
    grant_type: String,
    code: String,
    code_verifier: String,
    client_id: String,
    redirect_uri: String,
    refresh_token: String,
}

fn tokens_json(tokens: RuntimeTokens) -> Response {
    json_response(
        StatusCode::OK,
        json!({
            "access_token": tokens.access,
            "token_type": "Bearer",
            "expires_in": tokens.expires_in_seconds,
            "refresh_token": tokens.refresh,
            "scope": tokens.scopes.join(" "),
        }),
    )
}

pub async fn token(state: &Estado, body: &[u8]) -> Result<Response, ErroWeb> {
    let request = serde_urlencoded::from_bytes::<TokenRequest>(body).unwrap_or_default();
    let invalid_grant = || {
        oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "Código, verificador ou token que não confere.",
        )
    };
    let Ok(client_id) = Uuid::parse_str(&request.client_id) else {
        return Ok(invalid_grant());
    };
    let result = match request.grant_type.as_str() {
        "authorization_code" => {
            cms_dados::exchange_runtime_code(
                &state.pool,
                &request.code,
                client_id,
                &request.redirect_uri,
                &request.code_verifier,
            )
            .await
        }
        "refresh_token" => {
            cms_dados::rotate_runtime_tokens(&state.pool, &request.refresh_token, client_id).await
        }
        _ => {
            return Ok(oauth_error(
                StatusCode::BAD_REQUEST,
                "unsupported_grant_type",
                "Use authorization_code ou refresh_token.",
            ));
        }
    };
    match result {
        Ok(tokens) => Ok(tokens_json(tokens)),
        Err(RuntimeOAuthError::InvalidGrant) => Ok(invalid_grant()),
        Err(RuntimeOAuthError::Data(error)) => Err(error.into()),
    }
}
