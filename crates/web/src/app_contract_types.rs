//! DTOs de respostas usados pelos handlers e pela exportação de contratos.

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct DetalheErro {
    pub code: String,
    pub message: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct ErroJson {
    pub error: DetalheErro,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct RascunhoJson {
    pub draft_revision: i64,
    pub definition: Value,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct RevisaoJson {
    pub draft_revision: i64,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct DefinicaoVersionadaJson {
    pub number: i32,
    #[cfg_attr(feature = "contracts", schemars(with = "cms_dominio::app_builder::AppDefinition"))]
    pub definition: Value,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct AtualizadoJson {
    pub updated: bool,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidacaoValidaJson {
    pub valid: bool,
    pub draft_revision: i64,
    pub errors: Vec<DetalheErro>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct ValidacaoInvalidaJson {
    pub valid: bool,
    pub errors: Vec<DetalheErro>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct VersaoCriadaJson {
    pub id: Uuid,
    pub number: i32,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicadoJson {
    pub environment: String,
    pub version: i32,
    pub version_id: Uuid,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeJson {
    pub app_id: String,
    pub environment: String,
    pub version: i32,
    #[cfg_attr(feature = "contracts", schemars(with = "cms_dominio::app_builder::AppDefinition"))]
    pub definition: Value,
}

pub(crate) fn valor<T: Serialize>(body: T) -> Value {
    serde_json::to_value(body).expect("DTO de resposta sempre serializa")
}

pub(crate) fn validacao_invalida(code: &str, message: impl ToString) -> Value {
    valor(ValidacaoInvalidaJson {
        valid: false,
        errors: vec![DetalheErro { code: code.to_string(), message: message.to_string() }],
    })
}
