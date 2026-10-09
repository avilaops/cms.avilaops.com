use cms_dominio::{PerfilDoSite, Situacao};
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ErroDeDados;

#[derive(Debug, Clone, PartialEq)]
pub struct SiteGravado {
    pub id: Uuid,
    pub slug: String,
    pub situacao: Situacao,
    pub provisorio_definitivo: bool,
    pub perfil: PerfilDoSite,
    /// O domínio próprio em uso, se houver.
    pub dominio_ativo: Option<String>,
}

fn montar(
    id: Uuid,
    slug: String,
    situacao: String,
    provisorio_definitivo: bool,
    perfil: Json<PerfilDoSite>,
    dominio_ativo: Option<String>,
) -> Result<SiteGravado, ErroDeDados> {
    let situacao = Situacao::try_from(situacao.as_str())
        .map_err(|erro| ErroDeDados::DadoInvalido(erro.to_string()))?;
    Ok(SiteGravado {
        id,
        slug,
        situacao,
        provisorio_definitivo,
        perfil: perfil.0,
        dominio_ativo,
    })
}

/// O site do endereço provisório `<slug>.<domínio-base>`.
pub async fn site_por_slug(pool: &PgPool, slug: &str) -> Result<Option<SiteGravado>, ErroDeDados> {
    let linha = sqlx::query!(
        r#"
        select s.id, s.slug, s.situacao, s.provisorio_definitivo,
               s.perfil as "perfil: Json<PerfilDoSite>",
               (select d.host from dominio d where d.site_id = s.id and d.situacao = 'ativo') as dominio_ativo
        from site s
        where s.slug = $1
        "#,
        slug
    )
    .fetch_optional(pool)
    .await?;
    linha
        .map(|l| {
            montar(
                l.id,
                l.slug,
                l.situacao,
                l.provisorio_definitivo,
                l.perfil,
                l.dominio_ativo,
            )
        })
        .transpose()
}

/// O site de um domínio próprio. Domínio pendente não resolve site nenhum.
pub async fn site_por_dominio(
    pool: &PgPool,
    host: &str,
) -> Result<Option<SiteGravado>, ErroDeDados> {
    let linha = sqlx::query!(
        r#"
        select s.id, s.slug, s.situacao, s.provisorio_definitivo,
               s.perfil as "perfil: Json<PerfilDoSite>",
               d.host as dominio_ativo
        from dominio d
        join site s on s.id = d.site_id
        where d.host = $1 and d.situacao = 'ativo'
        "#,
        host
    )
    .fetch_optional(pool)
    .await?;
    linha
        .map(|l| {
            montar(
                l.id,
                l.slug,
                l.situacao,
                l.provisorio_definitivo,
                l.perfil,
                Some(l.dominio_ativo),
            )
        })
        .transpose()
}

pub async fn criar_site(
    pool: &PgPool,
    slug: &str,
    situacao: Situacao,
    provisorio_definitivo: bool,
    perfil: &PerfilDoSite,
) -> Result<Uuid, ErroDeDados> {
    let id = sqlx::query_scalar!(
        r#"
        insert into site (slug, situacao, provisorio_definitivo, perfil)
        values ($1, $2, $3, $4)
        returning id
        "#,
        slug,
        situacao.como_texto(),
        provisorio_definitivo,
        Json(perfil) as _
    )
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn mudar_situacao(
    pool: &PgPool,
    site_id: Uuid,
    situacao: Situacao,
) -> Result<(), ErroDeDados> {
    sqlx::query!(
        "update site set situacao = $2, atualizado_em = now() where id = $1",
        site_id,
        situacao.como_texto()
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Põe um domínio próprio em uso. O anterior do mesmo site, se houver, volta
/// a pendente: um site tem um endereço canônico só.
pub async fn ativar_dominio(pool: &PgPool, site_id: Uuid, host: &str) -> Result<(), ErroDeDados> {
    let mut transacao = pool.begin().await?;
    sqlx::query!(
        "update dominio set situacao = 'pendente' where site_id = $1 and situacao = 'ativo'",
        site_id
    )
    .execute(&mut *transacao)
    .await?;
    sqlx::query!(
        r#"
        insert into dominio (host, site_id, situacao, conferido_em)
        values ($1, $2, 'ativo', now())
        on conflict (host) do update
            set situacao = 'ativo', conferido_em = now()
            where dominio.site_id = excluded.site_id
        "#,
        host,
        site_id
    )
    .execute(&mut *transacao)
    .await?;
    transacao.commit().await?;
    Ok(())
}

/// Apaga o site e tudo o que é dele. Devolve se havia o que apagar.
pub async fn apagar_site(pool: &PgPool, slug: &str) -> Result<bool, ErroDeDados> {
    let mut transacao = pool.begin().await?;
    // A versão publicada é referenciada pelo documento; solta antes de apagar.
    sqlx::query!(
        r#"
        update documento set situacao = 'despublicado', versao_publicada = null
        where site_id = (select id from site where slug = $1)
        "#,
        slug
    )
    .execute(&mut *transacao)
    .await?;
    let apagados = sqlx::query!("delete from site where slug = $1", slug)
        .execute(&mut *transacao)
        .await?;
    transacao.commit().await?;
    Ok(apagados.rows_affected() > 0)
}
