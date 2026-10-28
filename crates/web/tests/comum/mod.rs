//! O que as suítes de integração têm em comum: falar com o roteador como um
//! navegador falaria, contra o banco que o teste recebeu.

// Cada suíte usa uma parte destes auxiliares.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cms_web::{Configuracao, Estado, Segredo, roteador};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

pub const DOMINIO_BASE: &str = "sites.teste";
pub const CHAVE_DO_INDEXNOW: &str = "0123456789abcdef0123456789abcdef";
pub const HOST_DO_PAINEL: &str = "cms.teste";
pub const TOKEN_DO_N8N: &str = "token-de-volta-do-teste";

/// Apenas a suíte isolada usa este coletor. Não recebe cabeçalhos, cookies ou
/// tokens; argumentos e resultados são fixtures sintéticas do teste.
#[cfg(feature = "contracts")]
pub fn capturar_mcp(method: &str, params: &serde_json::Value, response: &serde_json::Value) {
    if let Ok(directory) = std::env::var("CMS_MCP_RESPONSES") {
        let record = serde_json::json!({
            "executionRevision": std::env::var("CMS_CONTRACT_REVISION").ok(),
            "runId": std::env::var("CMS_CONTRACT_RUN").ok(),
            "method": method, "params": params, "requestId": 1, "response": response,
        });
        std::fs::create_dir_all(&directory).expect("diretório MCP");
        let file = std::path::Path::new(&directory).join(format!("{}.json", uuid::Uuid::new_v4()));
        std::fs::write(file, record.to_string()).expect("captura MCP sintética");
    }
}

pub struct Resposta {
    pub status: StatusCode,
    pub cabecalhos: axum::http::HeaderMap,
    pub corpo: String,
}

impl Resposta {
    pub fn cabecalho(&self, nome: &str) -> &str {
        self.cabecalhos
            .get(nome)
            .and_then(|valor| valor.to_str().ok())
            .unwrap_or("")
    }
}

pub fn estado(pool: &PgPool, diretorio_de_midia: PathBuf) -> Estado {
    Estado {
        pool: pool.clone(),
        configuracao: Arc::new(Configuracao {
            dominio_base: DOMINIO_BASE.into(),
            esquema: "https".into(),
            diretorio_de_midia,
            token_do_n8n: Some(Segredo::novo(TOKEN_DO_N8N)),
            chave_do_indexnow: Some(CHAVE_DO_INDEXNOW.into()),
            host_do_painel: Some(HOST_DO_PAINEL.into()),
            limites_de_criacao: Default::default(),
            limite_de_midia_por_site: 50 * 1024 * 1024,
            ips_do_servidor: vec!["203.0.113.7".parse().expect("ip")],
        }),
        auth: None,
        cache: cms_web::Cache::novo(),
    }
}

pub async fn pedir_em(estado: Estado, host: &str, caminho: &str) -> Resposta {
    let pedido = Request::builder()
        .uri(caminho)
        .header("host", host)
        .body(Body::empty())
        .expect("pedido válido");
    responder(estado, pedido).await
}

/// Um `POST` com corpo JSON e, se houver, o cabeçalho `authorization`.
pub async fn postar(
    pool: &PgPool,
    caminho: &str,
    autorizacao: Option<&str>,
    corpo: &str,
) -> Resposta {
    let mut pedido = Request::builder()
        .method("POST")
        .uri(caminho)
        .header("host", HOST_DO_PAINEL)
        .header("content-type", "application/json");
    if let Some(autorizacao) = autorizacao {
        pedido = pedido.header("authorization", autorizacao);
    }
    let pedido = pedido
        .body(Body::from(corpo.to_string()))
        .expect("pedido válido");
    responder(estado(pool, PathBuf::from("midia-que-nao-existe")), pedido).await
}

pub async fn responder(estado: Estado, pedido: Request<Body>) -> Resposta {
    #[cfg(feature = "contracts")]
    let contrato_pedido = (pedido.method().to_string(), pedido.uri().path().to_string());
    #[cfg(feature = "contracts")]
    let (pedido, grant_type) = if contrato_pedido.0 == "POST"
        && matches!(
            contrato_pedido.1.as_str(),
            "/oauth/token" | "/oauth/runtime/token"
        ) {
        let (parts, body) = pedido.into_parts();
        let bytes = body
            .collect()
            .await
            .expect("formulário OAuth lido")
            .to_bytes();
        let fields =
            serde_urlencoded::from_bytes::<Vec<(String, String)>>(&bytes).unwrap_or_default();
        let grants: Vec<_> = fields
            .iter()
            .filter(|(name, _)| name == "grant_type")
            .collect();
        let grant = if grants.len() == 1
            && matches!(grants[0].1.as_str(), "authorization_code" | "refresh_token")
        {
            Some(grants[0].1.clone())
        } else {
            None
        };
        (Request::from_parts(parts, Body::from(bytes)), grant)
    } else {
        (pedido, None)
    };
    let resposta = roteador(estado)
        .oneshot(pedido)
        .await
        .expect("o roteador responde");
    let (partes, corpo) = resposta.into_parts();
    let bytes = corpo.collect().await.expect("corpo lido").to_bytes();
    #[cfg(feature = "contracts")]
    if let Ok(directory) = std::env::var("CMS_OAUTH_RESPONSES")
        && (contrato_pedido.1.starts_with("/oauth/")
            || contrato_pedido.1.starts_with("/.well-known/oauth-"))
    {
        let mut headers: serde_json::Map<String, serde_json::Value> =
            ["content-type", "cache-control"]
                .into_iter()
                .filter_map(|name| {
                    partes
                        .headers
                        .get(name)
                        .and_then(|value| value.to_str().ok())
                        .map(|value| (name.to_string(), serde_json::json!(value)))
                })
                .collect();
        if partes.headers.contains_key("location") {
            headers.insert("location".to_string(), serde_json::json!("[REDACTED]"));
        }
        let json = partes
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|content_type| content_type.contains("application/json"));
        let body = if bytes.is_empty() {
            serde_json::Value::Null
        } else if json {
            cms_web::contratos_oauth::redact_body(
                serde_json::from_slice(&bytes).expect("OAuth JSON"),
            )
        } else {
            serde_json::json!("[CONTENT REDACTED]")
        };
        let record = serde_json::json!({
            "executionRevision": std::env::var("CMS_CONTRACT_REVISION").ok(),
            "runId": std::env::var("CMS_CONTRACT_RUN").ok(),
            "method": contrato_pedido.0, "path": contrato_pedido.1,
            "status": partes.status.as_u16(), "headers": headers, "body": body,
            "contentRedacted": true,
            "grantType": grant_type,
        });
        std::fs::create_dir_all(&directory).expect("diretório OAuth");
        let file = std::path::Path::new(&directory).join(format!("{}.json", uuid::Uuid::new_v4()));
        std::fs::write(file, record.to_string()).expect("captura OAuth sanitizada");
    }
    #[cfg(feature = "contracts")]
    if let Ok(directory) = std::env::var("CMS_CONTRACT_RESPONSES")
        && (contrato_pedido.1.starts_with("/api/v1/apps")
            || contrato_pedido.1.starts_with("/api/runtime/v1/apps/"))
    {
        // Somente respostas sintéticas de testes. Nunca registra cookie,
        // autorização, códigos OAuth, parâmetros ou corpos de pedidos.
        let headers: serde_json::Map<String, serde_json::Value> =
            ["cache-control", "x-correlation-id", "etag"]
                .into_iter()
                .filter_map(|name| {
                    partes
                        .headers
                        .get(name)
                        .and_then(|value| value.to_str().ok())
                        .map(|value| (name.to_string(), serde_json::json!(value)))
                })
                .collect();
        let body = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("resposta de API em JSON")
        };
        let record = serde_json::json!({
            "executionRevision": std::env::var("CMS_CONTRACT_REVISION").ok(),
            "runId": std::env::var("CMS_CONTRACT_RUN").ok(),
            "method": contrato_pedido.0, "path": contrato_pedido.1,
            "status": partes.status.as_u16(), "headers": headers, "body": body,
        });
        std::fs::create_dir_all(&directory).expect("diretório de contratos");
        let file = std::path::Path::new(&directory).join(format!("{}.json", uuid::Uuid::new_v4()));
        std::fs::write(file, record.to_string()).expect("resposta de contrato registrada");
    }
    Resposta {
        status: partes.status,
        cabecalhos: partes.headers,
        corpo: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

pub async fn pedir(pool: &PgPool, host: &str, caminho: &str) -> Resposta {
    pedir_em(
        estado(pool, PathBuf::from("midia-que-nao-existe")),
        host,
        caminho,
    )
    .await
}

pub fn host(slug: &str) -> String {
    format!("{slug}.{DOMINIO_BASE}")
}
