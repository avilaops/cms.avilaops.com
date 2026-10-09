//! Conferência de migrações, usada pelo despachante de deploy: havendo
//! pendência, ele faz o dump do banco antes de migrar.

use sqlx::PgPool;

use crate::Erro;

/// As versões que o binário conhece e o banco ainda não aplicou.
pub async fn pendentes(pool: &PgPool) -> Result<Vec<i64>, Erro> {
    // Em banco novo a tabela de controle ainda não existe: tudo está pendente.
    let tabela_existe: bool =
        sqlx::query_scalar("select to_regclass('_sqlx_migrations') is not null")
            .fetch_one(pool)
            .await?;
    let aplicadas: Vec<i64> = if tabela_existe {
        sqlx::query_scalar("select version from _sqlx_migrations where success")
            .fetch_all(pool)
            .await?
    } else {
        Vec::new()
    };
    Ok(cms_dados::MIGRADOR
        .iter()
        .map(|migracao| migracao.version)
        .filter(|versao| !aplicadas.contains(versao))
        .collect())
}
