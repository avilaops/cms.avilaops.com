//! Exportação de contratos dos tipos e do catálogo usados pelo servidor.
//! Disponível apenas na feature de ferramentas de CI `contracts`.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::{Value, json};

use crate::app_contract_types::*;
use crate::app_routes::{ENDPOINTS, Operacao};
use crate::apps::*;

fn esquema<T: JsonSchema>(serialize: bool) -> Value {
    let mut settings = SchemaSettings::draft2020_12();
    settings.inline_subschemas = true;
    if serialize {
        settings = settings.for_serialize();
    }
    serde_json::to_value(settings.into_generator().into_root_schema_for::<T>())
        .expect("schema sempre serializa")
}

/// Contrato de transporte. Validação contextual continua nos handlers e domínio.
pub fn exportar() -> Value {
    let operations: Vec<_> = ENDPOINTS.iter().map(|endpoint| {
        use Operacao::*;
        let request = match endpoint.operation {
            CreateApp => esquema::<NovoApp>(false),
            PutMember | DeleteMember => esquema::<AlteracaoDeParticipante>(false),
            PublishVersion => esquema::<Publicacao>(false),
            SaveDraft | ValidateDraft => json!(true),
            _ => Value::Null,
        };
        let response = match endpoint.operation {
            ListApps => esquema::<Vec<AppJson>>(true),
            CreateApp | GetApp => esquema::<AppJson>(true),
            GetDraft => esquema::<RascunhoJson>(true),
            SaveDraft => esquema::<RevisaoJson>(true),
            ValidateDraft => esquema::<ValidacaoValidaJson>(true),
            ListVersions => esquema::<Vec<VersaoJson>>(true),
            CreateVersion => esquema::<VersaoCriadaJson>(true),
            GetVersion => esquema::<DefinicaoVersionadaJson>(true),
            ListEnvironments => esquema::<Vec<AmbienteJson>>(true),
            PublishVersion => esquema::<PublicadoJson>(true),
            ListMembers => esquema::<Vec<ParticipanteJson>>(true),
            PutMember | DeleteMember => esquema::<AtualizadoJson>(true),
            ListAudit => esquema::<Vec<AuditoriaJson>>(true),
            GetRuntime => esquema::<RuntimeJson>(true),
        };
        let mut responses = serde_json::Map::new();
        let mut response_headers = serde_json::Map::new();
        for status in [400, 401, 403, 404, 409, 413, 422, 428, 500] {
            responses.insert(status.to_string(), esquema::<ErroJson>(true));
        }
        responses.insert(if endpoint.operation == CreateApp { "201" } else { "200" }.into(), response.clone());
        if endpoint.operation == CreateVersion {
            responses.insert("201".into(), response);
        }
        if endpoint.operation == ValidateDraft {
            responses.insert("422".into(), json!({ "anyOf": [
                esquema::<ErroJson>(true), esquema::<ValidacaoInvalidaJson>(true)
            ] }));
        }
        if endpoint.operation == GetRuntime {
            responses.insert("304".into(), Value::Null);
        }
        for status in responses.keys() {
            let mut headers = vec!["cache-control", "x-correlation-id"];
            if status == "200" && matches!(endpoint.operation, GetDraft | SaveDraft | GetRuntime)
                || status == "304"
            {
                headers.push("etag");
            }
            response_headers.insert(status.clone(), json!(headers));
        }
        let mut required_headers = Vec::new();
        if matches!(endpoint.method, "POST" | "PUT" | "DELETE") {
            required_headers.push("origin");
        }
        if endpoint.operation == SaveDraft { required_headers.push("if-match"); }
        if endpoint.operation == CreateVersion { required_headers.push("idempotency-key"); }
        json!({
            "id": endpoint.id, "method": endpoint.method, "path": endpoint.path,
            "authentication": if endpoint.operation == GetRuntime { "runtime:read" } else { "avila_sso" },
            "request": request, "requiredHeaders": required_headers,
            "responses": responses, "responseHeaders": response_headers,
        })
    }).collect();
    json!({
        "schemaVersion": 1, "module": "cms-apps-v1", "operations": operations,
        "aadValidation": serde_json::from_str::<Value>(cms_dominio::app_builder::JSON_SCHEMA)
            .expect("schema AAD versionado"),
        "limits": { "definitionBytes": cms_dominio::app_builder::MAX_DEFINITION_BYTES },
        "policies": ["CMS-ACCESS-005"],
        "tests": ["crates/web/tests/apps_api.rs", "crates/web/tests/runtime_apps.rs", "crates/web/tests/contratos.rs"],
    })
}
