//! Os redirecionamentos de um site: os que nascem da troca de slug e os que o
//! Dono cadastra para trazer um site que já existia.

use cms_dominio::Ator;
use cms_dominio::conta::pode_administrar;
use cms_dominio::redirecionamento::{RegraRecusada, conferir, destino_por_prefixo};
use sqlx::PgPool;
use uuid::Uuid;

use crate::ErroDeDados;

/// As mensagens são para quem está na tela.
#[derive(Debug, thiserror::Error)]
pub enum ErroDeRedirecionamento {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("Só o dono do site mexe nos redirecionamentos.")]
    SemPermissao,
    #[error(transparent)]
    Regra(#[from] RegraRecusada),
    #[error("Não há redirecionamento saindo deste endereço.")]
    Inexistente,
}

impl From<sqlx::Error> for ErroDeRedirecionamento {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirecionamento {
    pub de: String,
    pub para: String,
}

pub async fn redirecionamentos_do_site(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<Redirecionamento>, ErroDeDados> {
    let regras = sqlx::query_as!(
        Redirecionamento,
        "select de, para from redirecionamento where site_id = $1 order by de",
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(regras)
}

/// Cria o redirecionamento ou troca o destino do que já sai daquele endereço.
/// Devolve a regra como ficou gravada.
pub async fn salvar_redirecionamento(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    de: &str,
    para: &str,
) -> Result<Redirecionamento, ErroDeRedirecionamento> {
    if !pode_administrar(ator) {
        return Err(ErroDeRedirecionamento::SemPermissao);
    }
    let (de, para) = conferir(de, para)?;
    sqlx::query!(
        r#"
        insert into redirecionamento (site_id, de, para) values ($1, $2, $3)
        on conflict (site_id, de) do update set para = excluded.para
        "#,
        site_id,
        de,
        para
    )
    .execute(pool)
    .await?;
    Ok(Redirecionamento { de, para })
}

pub async fn remover_redirecionamento(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    de: &str,
) -> Result<(), ErroDeRedirecionamento> {
    if !pode_administrar(ator) {
        return Err(ErroDeRedirecionamento::SemPermissao);
    }
    let removidos = sqlx::query!(
        "delete from redirecionamento where site_id = $1 and de = $2",
        site_id,
        de.trim()
    )
    .execute(pool)
    .await?
    .rows_affected();
    if removidos == 0 {
        return Err(ErroDeRedirecionamento::Inexistente);
    }
    Ok(())
}

/// Para onde um caminho sem página leva: a regra exata, se houver, ou a de
/// prefixo mais específica.
pub(crate) async fn destino_de(
    pool: &PgPool,
    site_id: Uuid,
    caminho: &str,
) -> Result<Option<String>, ErroDeDados> {
    let exato = sqlx::query_scalar!(
        "select para from redirecionamento where site_id = $1 and de = $2",
        site_id,
        caminho
    )
    .fetch_optional(pool)
    .await?;
    if exato.is_some() {
        return Ok(exato);
    }
    // Entre as regras de prefixo que cobrem o caminho, vale a mais longa.
    let regra = sqlx::query_as!(
        Redirecionamento,
        r#"
        select de, para from redirecionamento
        where site_id = $1 and right(de, 2) = '/*'
          and starts_with($2, left(de, length(de) - 1))
        order by length(de) desc
        limit 1
        "#,
        site_id,
        caminho
    )
    .fetch_optional(pool)
    .await?;
    Ok(regra.and_then(|regra| destino_por_prefixo(&regra.de, &regra.para, caminho)))
}
