//! As mensagens que os visitantes mandam pelos formulários dos sites.

use chrono::{DateTime, Utc};
use cms_dominio::eventos::{CampoRecebido, Evento, MensagemRecebida};
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ErroDeDados;
use crate::eventos::emitir;

/// Quantas mensagens um site aceita a cada dez minutos. É o freio contra
/// enxurrada: passou disso, o formulário pede para tentar mais tarde.
const MAXIMO_EM_DEZ_MINUTOS: i64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recebimento {
    Guardada(Uuid),
    /// O site recebeu mensagem demais em pouco tempo.
    Freada,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MensagemDoSite {
    pub id: Uuid,
    pub caminho: String,
    pub campos: Vec<CampoRecebido>,
    pub criado_em: DateTime<Utc>,
}

/// Guarda a mensagem e põe na fila o aviso para o dono do site, na mesma
/// transação: mensagem guardada sem aviso ficaria esquecida.
pub async fn guardar_mensagem(
    pool: &PgPool,
    site_id: Uuid,
    caminho: &str,
    campos: &[CampoRecebido],
    avisar_para: Option<&str>,
) -> Result<Recebimento, ErroDeDados> {
    let mut transacao = pool.begin().await?;
    let recentes = sqlx::query_scalar!(
        r#"
        select count(*) as "total!" from mensagem
        where site_id = $1 and criado_em > now() - interval '10 minutes'
        "#,
        site_id
    )
    .fetch_one(&mut *transacao)
    .await?;
    if recentes >= MAXIMO_EM_DEZ_MINUTOS {
        return Ok(Recebimento::Freada);
    }
    let id = sqlx::query_scalar!(
        r#"
        insert into mensagem (site_id, documento_id, caminho, campos)
        values (
            $1,
            (select id from documento
             where site_id = $1 and caminho = $2 and situacao = 'publicado'),
            $2,
            $3
        )
        returning id
        "#,
        site_id,
        caminho,
        Json(campos) as _
    )
    .fetch_one(&mut *transacao)
    .await?;
    let evento = Evento::MensagemRecebida(MensagemRecebida {
        caminho: caminho.to_string(),
        campos: campos.to_vec(),
        avisar_para: avisar_para.map(str::to_string),
    });
    emitir(
        &mut transacao,
        site_id,
        &format!("mensagem.recebida:{id}"),
        &evento,
    )
    .await?;
    transacao.commit().await?;
    Ok(Recebimento::Guardada(id))
}

/// As mensagens mais recentes do site, da mais nova para a mais antiga.
pub async fn mensagens_do_site(
    pool: &PgPool,
    site_id: Uuid,
    limite: i64,
) -> Result<Vec<MensagemDoSite>, ErroDeDados> {
    let linhas = sqlx::query!(
        r#"
        select id, caminho, campos as "campos: Json<Vec<CampoRecebido>>", criado_em
        from mensagem where site_id = $1
        order by criado_em desc, id
        limit $2
        "#,
        site_id,
        limite
    )
    .fetch_all(pool)
    .await?;
    Ok(linhas
        .into_iter()
        .map(|linha| MensagemDoSite {
            id: linha.id,
            caminho: linha.caminho,
            campos: linha.campos.0,
            criado_em: linha.criado_em,
        })
        .collect())
}
