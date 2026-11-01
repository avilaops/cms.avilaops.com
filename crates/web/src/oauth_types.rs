//! Respostas OAuth compartilhadas pelos dois recursos, com contratos próprios.

use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct OAuthError {
    pub error: String,
    pub error_description: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct AuthorizationMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_endpoint: Option<String>,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct ResourceMetadata {
    pub resource: String,
    pub authorization_servers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes_supported: Option<Vec<String>>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct RegistrationResponse {
    pub client_id: Uuid,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub token_endpoint_auth_method: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: String,
}
