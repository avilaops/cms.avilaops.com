//! O que as suítes de integração têm em comum: falar com o roteador como um
//! navegador falaria, contra o banco que o teste recebeu.

// Cada suíte usa uma parte destes auxiliares.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cms_web::{Configuracao, Estado, roteador};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

pub const DOMINIO_BASE: &str = "sites.teste";

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
        }),
    }
}

pub async fn pedir_em(estado: Estado, host: &str, caminho: &str) -> Resposta {
    let pedido = Request::builder()
        .uri(caminho)
        .header("host", host)
        .body(Body::empty())
        .expect("pedido válido");
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
