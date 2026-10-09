//! O servidor HTTP: resolve o site pelo host e serve o que está publicado.

mod admin;
mod midia;
mod publico;
mod resposta;
mod visao;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct Configuracao {
    /// Todo site nasce em `<slug>.<dominio_base>`.
    pub dominio_base: String,
    /// `https` em produção.
    pub esquema: String,
    /// Uma pasta por site, com as variantes de imagem.
    pub diretorio_de_midia: PathBuf,
    /// O token com que o n8n encerra eventos. Sem ele, a rota de volta
    /// recusa todo pedido.
    pub token_do_n8n: Option<Segredo>,
    /// A chave do IndexNow. Cada site a serve em `/<chave>.txt`, que é como o
    /// buscador confere que o aviso veio de quem responde pelo endereço.
    pub chave_do_indexnow: Option<String>,
}

/// Um segredo de configuração. Não aparece em registro nem em `Debug`.
#[derive(Clone)]
pub struct Segredo(String);

impl Segredo {
    pub fn novo(valor: impl Into<String>) -> Self {
        Self(valor.into())
    }

    pub fn expor(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segredo(***)")
    }
}

#[derive(Debug, Clone)]
pub struct Estado {
    pub pool: PgPool,
    pub configuracao: Arc<Configuracao>,
}

pub fn roteador(estado: Estado) -> Router {
    Router::new()
        .route("/api/saude", get(saude))
        .route(
            "/api/admin/eventos/{id}/encerrar",
            post(admin::encerrar_evento),
        )
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
