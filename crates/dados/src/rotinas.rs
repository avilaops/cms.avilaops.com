//! O catálogo de rotinas e o que elas leem e limpam no banco.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::ErroDeDados;

/// Tenta pegar a vez de rodar uma rotina. Só devolve `true` para uma
/// instância por intervalo: a trava é a linha da rotina.
pub async fn reivindicar_rotina(
    pool: &PgPool,
    nome: &str,
    intervalo_em_segundos: i64,
) -> Result<bool, ErroDeDados> {
    let pegas = sqlx::query!(
        r#"
        update rotina set ultima_execucao = now()
        where nome = $1 and ultima_execucao <= now() - make_interval(secs => $2)
        "#,
        nome,
        intervalo_em_segundos as f64
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(pegas > 0)
}

/// Um documento cuja publicação agendada já venceu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agendado {
    pub documento_id: Uuid,
    pub site_id: Uuid,
    pub agendado_para: DateTime<Utc>,
    pub agendado_por: String,
}

pub async fn agendados_vencidos(pool: &PgPool) -> Result<Vec<Agendado>, ErroDeDados> {
    let agendados = sqlx::query_as!(
        Agendado,
        r#"
        select id as documento_id, site_id, agendado_para as "agendado_para!",
               coalesce(agendado_por, '') as "agendado_por!"
        from documento
        where agendado_para is not null and agendado_para <= now()
        order by agendado_para
        limit 50
        "#
    )
    .fetch_all(pool)
    .await?;
    Ok(agendados)
}

/// Apaga o histórico e o registro de chamadas com mais de um ano. Devolve
/// quantas linhas saíram.
pub async fn limpar_historico(pool: &PgPool) -> Result<u64, ErroDeDados> {
    let historico =
        sqlx::query!("delete from historico where criado_em < now() - interval '1 year'")
            .execute(pool)
            .await?
            .rows_affected();
    let chamadas =
        sqlx::query!("delete from chamada_mcp where criado_em < now() - interval '1 year'")
            .execute(pool)
            .await?
            .rows_affected();
    let eventos = sqlx::query!(
        "delete from evento where situacao = 'encerrado' and encerrado_em < now() - interval '1 year'"
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(historico + chamadas + eventos)
}
