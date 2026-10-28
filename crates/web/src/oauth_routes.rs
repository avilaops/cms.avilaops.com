use axum::http::Method;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OAuthOperation {
    McpMetadata,
    McpResource,
    McpResourceAlias,
    RuntimeMetadata,
    RuntimeResource,
    Register,
    McpToken,
    RuntimeToken,
    McpAuthorize,
    McpConsent,
    RuntimeAuthorize,
    RuntimeConsent,
}

pub(crate) struct OAuthEndpoint {
    pub id: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub operation: OAuthOperation,
}

macro_rules! endpoints {
    ($(($id:literal, $method:literal, $path:literal, $operation:ident)),+ $(,)?) => {
        pub(crate) const OAUTH_ENDPOINTS: &[OAuthEndpoint] = &[$(OAuthEndpoint {
            id: $id, method: $method, path: $path, operation: OAuthOperation::$operation,
        }),+];
    };
}

endpoints![
    (
        "mcpMetadata",
        "GET",
        "/.well-known/oauth-authorization-server",
        McpMetadata
    ),
    (
        "mcpResource",
        "GET",
        "/.well-known/oauth-protected-resource",
        McpResource
    ),
    (
        "mcpResourceAlias",
        "GET",
        "/.well-known/oauth-protected-resource/mcp",
        McpResourceAlias
    ),
    (
        "runtimeMetadata",
        "GET",
        "/.well-known/oauth-authorization-server/runtime",
        RuntimeMetadata
    ),
    (
        "runtimeResource",
        "GET",
        "/.well-known/oauth-protected-resource/api/runtime",
        RuntimeResource
    ),
    ("register", "POST", "/oauth/register", Register),
    ("mcpToken", "POST", "/oauth/token", McpToken),
    ("runtimeToken", "POST", "/oauth/runtime/token", RuntimeToken),
    ("mcpAuthorize", "GET", "/oauth/authorize", McpAuthorize),
    ("mcpConsent", "POST", "/oauth/authorize", McpConsent),
    (
        "runtimeAuthorize",
        "GET",
        "/oauth/runtime/authorize",
        RuntimeAuthorize
    ),
    (
        "runtimeConsent",
        "POST",
        "/oauth/runtime/authorize",
        RuntimeConsent
    ),
];

pub(crate) fn localizar(method: &Method, path: &str) -> Option<&'static OAuthEndpoint> {
    OAUTH_ENDPOINTS
        .iter()
        .find(|endpoint| endpoint.method == method.as_str() && endpoint.path == path)
}
