//! Acesso ao banco. SQLx é o dono do esquema: as migrações moram aqui, e as
//! consultas são conferidas na compilação.

mod documentos;
mod eventos;
pub mod fluxo;
mod sites;

use sqlx::PgPool;
use sqlx::migrate::Migrator;

pub use documentos::{
    Ausencia, ItemDeNavegacao, ausencia, documento_publicado, documentos_publicados, navegacao,
    publicar,
};
pub use eventos::{
    Encerramento, EventoAEntregar, encerrar_evento, marcar_evento_entregue, reivindicar_eventos,
};
pub use sites::{
    SiteGravado, apagar_site, ativar_dominio, criar_site, mudar_situacao, site_por_dominio,
    site_por_slug,
};

pub static MIGRADOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum ErroDeDados {
    #[error("falha no banco: {0}")]
    Banco(#[from] sqlx::Error),
    #[error("dado gravado não pôde ser lido: {0}")]
    DadoInvalido(String),
}

/// Usado pela rota de saúde: o serviço só está apto se o banco responde.
pub async fn banco_responde(pool: &PgPool) -> bool {
    sqlx::query_scalar!("select 1 as \"um!\"")
        .fetch_one(pool)
        .await
        .is_ok()
}
