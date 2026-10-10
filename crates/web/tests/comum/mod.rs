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
    let resposta = roteador(estado)
        .oneshot(pedido)
        .await
        .expect("o roteador responde");
    let (partes, corpo) = resposta.into_parts();
    let bytes = corpo.collect().await.expect("corpo lido").to_bytes();
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
