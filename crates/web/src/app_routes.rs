//! Catálogo único de operações da API de apps e do runtime.

use axum::http::Method;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operacao {
    ListApps, CreateApp, GetApp, GetDraft, SaveDraft, ValidateDraft,
    ListVersions, CreateVersion, GetVersion, ListEnvironments, PublishVersion,
    ListMembers, PutMember, DeleteMember, ListAudit, GetRuntime,
}

pub(crate) struct Endpoint {
    pub id: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub operation: Operacao,
}

macro_rules! endpoints {
    ($(($id:literal, $method:literal, $path:literal, $operation:ident)),+ $(,)?) => {
        pub(crate) const ENDPOINTS: &[Endpoint] = &[$(Endpoint {
            id: $id, method: $method, path: $path, operation: Operacao::$operation,
        }),+];
    };
}

endpoints![
    ("listApps", "GET", "/api/v1/apps", ListApps),
    ("createApp", "POST", "/api/v1/apps", CreateApp),
    ("getApp", "GET", "/api/v1/apps/{appId}", GetApp),
    ("getDraft", "GET", "/api/v1/apps/{appId}/draft", GetDraft),
    ("saveDraft", "PUT", "/api/v1/apps/{appId}/draft", SaveDraft),
    ("validateDraft", "POST", "/api/v1/apps/{appId}/validate", ValidateDraft),
    ("listVersions", "GET", "/api/v1/apps/{appId}/versions", ListVersions),
    ("createVersion", "POST", "/api/v1/apps/{appId}/versions", CreateVersion),
    ("getVersion", "GET", "/api/v1/apps/{appId}/versions/{number}", GetVersion),
    ("listEnvironments", "GET", "/api/v1/apps/{appId}/environments", ListEnvironments),
    ("publishVersion", "PUT", "/api/v1/apps/{appId}/environments/{environment}", PublishVersion),
    ("listMembers", "GET", "/api/v1/apps/{appId}/members", ListMembers),
    ("putMember", "PUT", "/api/v1/apps/{appId}/members", PutMember),
    ("deleteMember", "DELETE", "/api/v1/apps/{appId}/members", DeleteMember),
    ("listAudit", "GET", "/api/v1/apps/{appId}/audit", ListAudit),
    ("getRuntime", "GET", "/api/runtime/v1/apps/{slug}/environments/{environment}", GetRuntime),
];

pub(crate) struct Rota<'a> {
    pub endpoint: &'static Endpoint,
    path: &'a str,
}

impl Rota<'_> {
    pub fn parametro(&self, name: &str) -> Option<&str> {
        self.endpoint.path.split('/').zip(self.path.split('/')).find_map(|(pattern, value)| {
            (pattern.strip_prefix('{').and_then(|s| s.strip_suffix('}')) == Some(name)).then_some(value)
        })
    }
}

pub(crate) fn localizar<'a>(method: &Method, path: &'a str) -> Option<Rota<'a>> {
    ENDPOINTS.iter().find(|endpoint| {
        if endpoint.method != method.as_str() { return false; }
        let pattern: Vec<_> = endpoint.path.split('/').collect();
        let parts: Vec<_> = path.split('/').collect();
        pattern.len() == parts.len() && pattern.iter().zip(parts).all(|(expected, actual)| {
            (expected.starts_with('{') && expected.ends_with('}')) || *expected == actual
        })
    }).map(|endpoint| Rota { endpoint, path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todas_as_operacoes_do_catalogo_sao_alcancaveis_sem_ambiguidades() {
        let mut ids = std::collections::HashSet::new();
        for endpoint in ENDPOINTS {
            assert!(ids.insert(endpoint.id));
            let path = endpoint.path.replace("{appId}", "00000000-0000-0000-0000-000000000001")
                .replace("{number}", "1").replace("{environment}", "production").replace("{slug}", "app");
            let method = Method::from_bytes(endpoint.method.as_bytes()).expect("método");
            assert_eq!(localizar(&method, &path).expect("rota").endpoint.id, endpoint.id);
        }
        assert!(localizar(&Method::GET, "/api/v1/apps/id/draft/extra").is_none());
        assert!(localizar(&Method::POST, "/api/runtime/v1/apps/app/environments/production").is_none());
    }
}
