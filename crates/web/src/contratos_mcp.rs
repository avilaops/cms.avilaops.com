//! Transporte e catálogo exportados do código executado. Payloads textuais
//! das ferramentas ainda exigem contratos e validação específicos.

use serde_json::{Value, json};
use crate::contratos::esquema;
use crate::mcp_types::{RpcError, RpcSuccess, ToolResult, ToolsList};

pub fn exportar() -> Value {
    json!({
        "schemaVersion": 1,
        "module": "cms-mcp-v1",
        "protocolVersion": crate::conector::VERSAO_DO_PROTOCOLO,
        "path": "/mcp",
        "method": "POST",
        "authentication": "oauth_mcp",
        "tools": crate::conector::catalogo_contratado(),
        "transport": {
            "toolsList": esquema::<RpcSuccess<ToolsList>>(true),
            "toolResult": esquema::<RpcSuccess<ToolResult>>(true),
            "error": esquema::<RpcError>(true),
        },
        "policies": ["CMS-ACCESS-002", "CMS-ACCESS-004", "CMS-PUBLICATION-002"],
        "payloadsVerified": false,
    })
}
