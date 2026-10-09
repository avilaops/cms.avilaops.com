//! A fila de eventos: gravar na transação do fato, reivindicar para entrega e
//! encerrar com o resultado que o n8n devolve.

use chrono::{DateTime, Utc};
use cms_dominio::eventos::Evento;
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::ErroDeDados;

/// Grava o evento. Roda na conexão da transação do fato: ou os dois ficam, ou
/// nenhum.
pub(crate) async fn emitir(
    conexao: &mut PgConnection,
    site_id: Uuid,
    chave: &str,
    evento: &Evento,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "insert into evento (site_id, tipo, chave, dados) values ($1, $2, $3, $4)",
        site_id,
        evento.tipo(),
        chave,
        Json(evento) as _
    )
    .execute(&mut *conexao)
    .await?;
    Ok(())
}

/// Um evento separado para entrega, com o que é preciso saber do site.
#[derive(Debug, Clone, PartialEq)]
pub struct EventoAEntregar {
    pub id: Uuid,
    pub tipo: String,
    pub chave: String,
    pub dados: Value,
    pub ocorrido_em: DateTime<Utc>,
    pub site_id: Uuid,
    pub site_slug: String,
    pub site_nome: String,
    pub site_situacao: String,
    pub site_provisorio_definitivo: bool,
    pub dominio_ativo: Option<String>,
}

/// Separa até `limite` eventos pendentes para esta instância entregar.
///
/// A reivindicação já conta a tentativa e empurra a próxima: 1 minuto, 2, 4,
/// até o teto de uma hora. Se a entrega falhar, ou o processo cair no meio,
/// o evento volta sozinho. Duas instâncias não pegam o mesmo evento.
pub async fn reivindicar_eventos(
    pool: &PgPool,
    limite: i64,
) -> Result<Vec<EventoAEntregar>, ErroDeDados> {
    let eventos = sqlx::query_as!(
        EventoAEntregar,
        r#"
        with reivindicado as (
            update evento
            set tentativas = tentativas + 1,
                proxima_tentativa_em =
                    now() + make_interval(mins => least(power(2, least(tentativas, 6))::int, 60))
            where id in (
                select id from evento
                where situacao = 'pendente' and proxima_tentativa_em <= now()
                order by ocorrido_em
                limit $1
                for update skip locked
            )
            returning id, site_id, tipo, chave, dados, ocorrido_em
        )
        select r.id as "id!", r.tipo as "tipo!", r.chave as "chave!", r.dados as "dados!",
               r.ocorrido_em as "ocorrido_em!", r.site_id as "site_id!",
               s.slug as "site_slug!", s.situacao as "site_situacao!",
               s.provisorio_definitivo as "site_provisorio_definitivo!",
               coalesce(s.perfil ->> 'nome', '') as "site_nome!",
               (select d.host from dominio d
                where d.site_id = s.id and d.situacao = 'ativo') as dominio_ativo
        from reivindicado r
        join site s on s.id = r.site_id
        order by r.ocorrido_em
        "#,
        limite
    )
    .fetch_all(pool)
    .await?;
    Ok(eventos)
}

/// O n8n recebeu. Se ele já encerrou o evento antes de esta marca chegar, a
/// marca não desfaz o encerramento.
pub async fn marcar_evento_entregue(pool: &PgPool, id: Uuid) -> Result<(), ErroDeDados> {
    sqlx::query!(
        r#"
        update evento set situacao = 'entregue', entregue_em = now()
        where id = $1 and situacao = 'pendente'
        "#,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encerramento {
    Encerrado,
    /// Já tinha sido encerrado: o resultado gravado não muda.
    JaEncerrado,
    Inexistente,
}

/// Fecha o evento com o resultado da automação.
pub async fn encerrar_evento(
    pool: &PgPool,
    id: Uuid,
    resultado: &Value,
) -> Result<Encerramento, ErroDeDados> {
    let encerrados = sqlx::query!(
        r#"
        update evento
        set situacao = 'encerrado', encerrado_em = now(), resultado = $2,
            entregue_em = coalesce(entregue_em, now())
        where id = $1 and situacao <> 'encerrado'
        "#,
        id,
        resultado
    )
    .execute(pool)
    .await?
    .rows_affected();
    if encerrados > 0 {
        return Ok(Encerramento::Encerrado);
    }
    let existe = sqlx::query_scalar!(
        r#"select exists(select 1 from evento where id = $1) as "existe!""#,
        id
    )
    .fetch_one(pool)
    .await?;
    Ok(if existe {
        Encerramento::JaEncerrado
    } else {
        Encerramento::Inexistente
    })
}
