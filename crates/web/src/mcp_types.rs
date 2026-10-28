//! Tipos do transporte MCP usados na resposta real e na exportação.

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) enum RpcVersion {
    #[serde(rename = "2.0")]
    V2,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct RpcSuccess<T> {
    pub jsonrpc: RpcVersion,
    pub id: Value,
    pub result: T,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct RpcErrorDetail {
    pub code: i32,
    pub message: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct RpcError {
    pub jsonrpc: RpcVersion,
    pub id: Value,
    pub error: RpcErrorDetail,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) enum TextKind {
    #[serde(rename = "text")]
    Text,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct TextContent {
    pub r#type: TextKind,
    pub text: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct ToolResult {
    pub content: Vec<TextContent>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolAnnotations {
    pub read_only_hint: bool,
    pub destructive_hint: bool,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub annotations: ToolAnnotations,
    pub input_schema: Value,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
pub(crate) struct ToolsList {
    pub tools: Vec<ToolDescriptor>,
}

pub(crate) fn success<T: Serialize>(id: Value, result: T) -> Value {
    crate::app_contract_types::valor(RpcSuccess { jsonrpc: RpcVersion::V2, id, result })
}

pub(crate) fn error(id: Value, code: i32, message: &str) -> Value {
    crate::app_contract_types::valor(RpcError {
        jsonrpc: RpcVersion::V2,
        id,
        error: RpcErrorDetail { code, message: message.to_string() },
    })
}
