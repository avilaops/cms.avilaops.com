//! O servidor HTTP: resolve o site pelo host e serve o que está publicado.

mod midia;
mod publico;
mod resposta;
mod visao;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct Configuracao {
    /// Todo site nasce em `<slug>.<dominio_base>`.
    pub dominio_base: String,
    /// `https` em produção.
    pub esquema: String,
    /// Uma pasta por site, com as variantes de imagem.
    pub diretorio_de_midia: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Estado {
    pub pool: PgPool,
    pub configuracao: Arc<Configuracao>,
}

pub fn roteador(estado: Estado) -> Router {
    Router::new()
        .route("/api/saude", get(saude))
        .fallback(publico::atender)
        .with_state(estado)
}

/// 200 quando o banco responde, 503 quando não. É o que o despachante de
/// deploy consulta antes de dar a troca de versão por boa.
async fn saude(State(estado): State<Estado>) -> (StatusCode, &'static str) {
    if cms_dados::banco_responde(&estado.pool).await {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "banco indisponível")
    }
}
