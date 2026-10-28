//! Read-only delivery of published AAD definitions to native runtimes.

use axum::http::header::{
    AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH, WWW_AUTHENTICATE,
};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use cms_dados::{PublishedRuntimeDefinition, RUNTIME_READ_SCOPE};
use uuid::Uuid;

use crate::Estado;
use crate::app_contract_types::{DetalheErro, ErroJson, RuntimeJson, valor};

fn add_common_headers(response: &mut Response, correlation_id: Uuid) {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(
        "x-correlation-id",
        HeaderValue::from_str(&correlation_id.to_string()).expect("UUID is a valid header value"),
    );
}

fn json_response(status: StatusCode, code: &str, message: &str) -> Response {
    let mut response = (
        status,
        valor(ErroJson {
            error: DetalheErro {
                code: code.to_string(),
                message: message.to_string(),
            },
        })
        .to_string(),
    )
        .into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn unauthorized(origin: &str) -> Response {
    let mut response = json_response(
        StatusCode::UNAUTHORIZED,
        "unauthenticated",
        "Autenticação runtime necessária.",
    );
    let challenge = format!(
        "Bearer resource_metadata=\"{origin}/.well-known/oauth-protected-resource/api/runtime\""
    );
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response.headers_mut().insert(WWW_AUTHENTICATE, value);
    }
    response
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    (!token.is_empty() && !token.contains(char::is_whitespace)).then_some(token)
}

fn successful(definition: PublishedRuntimeDefinition, etag: &str) -> Response {
    let mut response = (
        StatusCode::OK,
        valor(RuntimeJson {
            app_id: definition.slug,
            environment: definition.environment,
            version: definition.version,
            definition: definition.definition,
        })
        .to_string(),
    )
        .into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response.headers_mut().insert(
        ETAG,
        HeaderValue::from_str(etag).expect("generated ETag is a valid header value"),
    );
    response
}

pub async fn serve(
    state: &Estado,
    origin: &str,
    method: &Method,
    headers: &HeaderMap,
    path: &str,
) -> Response {
    let correlation_id = Uuid::new_v4();
    let mut response = if method != Method::GET {
        json_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "Use GET.",
        )
    } else if let Some(route) = crate::app_routes::localizar(method, path)
        .filter(|route| route.endpoint.operation == crate::app_routes::Operacao::GetRuntime)
    {
        tracing::debug!(operation = route.endpoint.id, "operação do runtime");
        let slug = route.parametro("slug").expect("slug da rota runtime");
        let environment = route
            .parametro("environment")
            .expect("ambiente da rota runtime");
        let Some(access_token) = bearer(headers) else {
            let mut response = unauthorized(origin);
            add_common_headers(&mut response, correlation_id);
            return response;
        };
        let session =
            match cms_dados::runtime_session_by_access_token(&state.pool, access_token).await {
                Ok(Some(session)) => session,
                Ok(None) => {
                    let mut response = unauthorized(origin);
                    add_common_headers(&mut response, correlation_id);
                    return response;
                }
                Err(error) => {
                    tracing::error!(%error, "falha ao autenticar runtime");
                    let mut response = json_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal_error",
                        "Erro interno.",
                    );
                    add_common_headers(&mut response, correlation_id);
                    return response;
                }
            };
        if !session.has_scope(RUNTIME_READ_SCOPE) {
            json_response(
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "O token não possui runtime:read.",
            )
        } else {
            match cms_dados::published_runtime_definition(
                &state.pool,
                &session.account,
                session.app_id,
                slug,
                environment,
            )
            .await
            {
                Ok(Some(definition)) => {
                    let etag = format!(
                        "\"{}:{}:{}\"",
                        definition.app_id, definition.environment, definition.version_id
                    );
                    if headers
                        .get(IF_NONE_MATCH)
                        .and_then(|value| value.to_str().ok())
                        .is_some_and(|value| {
                            value.split(',').any(|candidate| candidate.trim() == etag)
                        })
                    {
                        let mut response = StatusCode::NOT_MODIFIED.into_response();
                        response.headers_mut().insert(
                            ETAG,
                            HeaderValue::from_str(&etag)
                                .expect("generated ETag is a valid header value"),
                        );
                        response
                    } else {
                        successful(definition, &etag)
                    }
                }
                Ok(None) => json_response(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "Aplicativo ou ambiente não encontrado.",
                ),
                Err(error) => {
                    tracing::error!(%error, "falha ao entregar definição runtime");
                    json_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal_error",
                        "Erro interno.",
                    )
                }
            }
        }
    } else {
        json_response(StatusCode::NOT_FOUND, "not_found", "Não encontrado.")
    };
    add_common_headers(&mut response, correlation_id);
    response
}
