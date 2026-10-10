use chrono::{DateTime, Utc};
use motor_web::tipos::{Conteudo, Documento};
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ErroDeDados;

/// O documento que está no ar em um caminho de um site.
pub async fn documento_publicado(
    pool: &PgPool,
    site_id: Uuid,
    caminho: &str,
) -> Result<Option<Documento>, ErroDeDados> {
    let linha = sqlx::query!(
        r#"
        select d.caminho as "caminho!", v.conteudo as "conteudo: Json<Conteudo>"
        from documento d
        join versao v on v.id = d.versao_publicada
        where d.site_id = $1 and d.caminho = $2 and d.situacao = 'publicado'
        "#,
        site_id,
        caminho
    )
    .fetch_optional(pool)
    .await?;
    Ok(linha.map(|l| Documento {
        caminho: l.caminho,
        conteudo: l.conteudo.0,
    }))
}

/// Tudo o que está no ar em um site, em ordem de caminho.
pub async fn documentos_publicados(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<Documento>, ErroDeDados> {
    let linhas = sqlx::query!(
        r#"
        select d.caminho as "caminho!", v.conteudo as "conteudo: Json<Conteudo>"
        from documento d
        join versao v on v.id = d.versao_publicada
        where d.site_id = $1 and d.situacao = 'publicado'
        order by d.caminho
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(linhas
        .into_iter()
        .map(|l| Documento {
            caminho: l.caminho,
            conteudo: l.conteudo.0,
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDeNavegacao {
    pub caminho: String,
    pub titulo: String,
    pub eh_post: bool,
}

/// Caminho e nome no menu do que está no ar, sem carregar o corpo. O nome é
/// o que a página definiu para o menu ou, sem ele, o título.
pub async fn navegacao(pool: &PgPool, site_id: Uuid) -> Result<Vec<ItemDeNavegacao>, ErroDeDados> {
    let linhas = sqlx::query!(
        r#"
        select d.caminho as "caminho!", d.especie,
               coalesce(
                   nullif(btrim(v.conteudo -> 'dados' ->> 'menu'), ''),
                   v.conteudo -> 'dados' ->> 'titulo',
                   ''
               ) as "titulo!"
        from documento d
        join versao v on v.id = d.versao_publicada
        where d.site_id = $1 and d.situacao = 'publicado'
        order by d.caminho
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(linhas
        .into_iter()
        .map(|l| ItemDeNavegacao {
            caminho: l.caminho,
            titulo: l.titulo,
            eh_post: l.especie == "post",
        })
        .collect())
}

/// Por que um caminho não tem documento no ar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ausencia {
    /// O endereço mudou: o visitante é levado ao novo.
    Redirecionado(String),
    /// Já esteve no ar e foi tirado.
    Despublicado,
    Inexistente,
}

pub async fn ausencia(
    pool: &PgPool,
    site_id: Uuid,
    caminho: &str,
) -> Result<Ausencia, ErroDeDados> {
    let destino = sqlx::query_scalar!(
        "select para from redirecionamento where site_id = $1 and de = $2",
        site_id,
        caminho
    )
    .fetch_optional(pool)
    .await?;
    if let Some(destino) = destino {
        return Ok(Ausencia::Redirecionado(destino));
    }
    let despublicado = sqlx::query_scalar!(
        r#"
        select exists(
            select 1 from documento
            where site_id = $1 and caminho = $2 and situacao = 'despublicado'
        ) as "despublicado!"
        "#,
        site_id,
        caminho
    )
    .fetch_one(pool)
    .await?;
    Ok(if despublicado {
        Ausencia::Despublicado
    } else {
        Ausencia::Inexistente
    })
}

pub(crate) fn especie_e_slug(conteudo: &Conteudo) -> Result<(&'static str, &str), ErroDeDados> {
    match conteudo {
        Conteudo::Pagina(pagina) => Ok(("pagina", &pagina.slug)),
        Conteudo::Post(post) => Ok(("post", &post.slug)),
        // Catálogo e checkout são do Lojas.
        Conteudo::Produto(_) => Err(ErroDeDados::DadoInvalido("o CMS não guarda produto".into())),
    }
}

/// Grava uma versão do documento e a põe no ar. É a camada de baixo: quem
/// chama já validou com o motor. Devolve o identificador do documento.
pub async fn publicar(
    pool: &PgPool,
    site_id: Uuid,
    documento: &Documento,
    agora: DateTime<Utc>,
) -> Result<Uuid, ErroDeDados> {
    let (especie, slug) = especie_e_slug(&documento.conteudo)?;
    let mut transacao = pool.begin().await?;

    let documento_id = sqlx::query_scalar!(
        r#"
        insert into documento (site_id, especie, slug, caminho)
        values ($1, $2, $3, $4)
        on conflict (site_id, especie, slug) do update set caminho = excluded.caminho
        returning id
        "#,
        site_id,
        especie,
        slug,
        documento.caminho
    )
    .fetch_one(&mut *transacao)
    .await?;

    let versao_id = sqlx::query_scalar!(
        r#"
        insert into versao (documento_id, numero, conteudo)
        values ($1, (select coalesce(max(numero), 0) + 1 from versao where documento_id = $1), $2)
        returning id
        "#,
        documento_id,
        Json(&documento.conteudo) as _
    )
    .fetch_one(&mut *transacao)
    .await?;

    // "Publicado em" é a primeira publicação e não muda mais.
    sqlx::query!(
        r#"
        update documento
        set situacao = 'publicado',
            versao_publicada = $2,
            publicado_em = coalesce(publicado_em, $3),
            atualizado_em = $4
        where id = $1
        "#,
        documento_id,
        versao_id,
        agora,
        documento.atualizado_em()
    )
    .execute(&mut *transacao)
    .await?;

    transacao.commit().await?;
    Ok(documento_id)
}
