//! Persistência do estúdio. Toda consulta filtra por `site_id`.

use chrono::{DateTime, Utc};
use cms_dominio::estudio::{
    DefinicaoDeArquivo, LIMITE_DE_ARQUIVOS, LIMITE_DE_NOS, Medida, NoDaDefinicao,
    PaginaDaDefinicao, PropsDeComponente, prender,
};
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ErroDeDados;

#[derive(Debug, thiserror::Error)]
pub enum ErroDeEstudio {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("arquivo não encontrado")]
    NaoEncontrado,
    #[error("nó não encontrado")]
    NoNaoEncontrado,
    #[error("posição inválida")]
    PosicaoInvalida,
    #[error("limite de arquivos")]
    LimiteDeArquivos,
    #[error("limite de nós")]
    LimiteDeNos,
    #[error("biblioteca inativa")]
    BibliotecaInativa,
}

impl From<sqlx::Error> for ErroDeEstudio {
    fn from(erro: sqlx::Error) -> Self {
        if let sqlx::Error::Database(banco) = &erro
            && matches!(banco.code().as_deref(), Some("23514" | "23503"))
        {
            return Self::PosicaoInvalida;
        }
        Self::Dados(ErroDeDados::from(erro))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoArquivo {
    pub site_id: Uuid,
    pub nome: String,
    pub pedido: String,
    pub criado_por: String,
    pub sistema: String,
    pub largura: i32,
    pub altura: i32,
    pub texto: String,
    pub texto_x: i32,
    pub texto_y: i32,
    pub texto_largura: i32,
    pub texto_altura: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArquivoListado {
    pub id: Uuid,
    pub nome: String,
    pub favorito: bool,
    pub sistema: String,
    pub criado_por: String,
    pub atualizado_em: DateTime<Utc>,
    pub compartilhado: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArquivoAberto {
    pub id: Uuid,
    pub nome: String,
    pub pedido: String,
    pub favorito: bool,
    pub sistema: String,
    pub criado_por: String,
    pub paginas: Vec<PaginaAberta>,
    pub token_da_previa: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaginaAberta {
    pub id: Uuid,
    pub nome: String,
    pub largura: i32,
    pub altura: i32,
    pub nos: Vec<NoAberto>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoAberto {
    pub id: Uuid,
    pub tipo: String,
    pub x: i32,
    pub y: i32,
    pub largura: i32,
    pub altura: i32,
    pub texto: String,
    pub preenchimento: Option<String>,
    pub biblioteca: Option<String>,
    pub variante: Option<String>,
    pub catalogo_versao: Option<String>,
    pub props: PropsDeComponente,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoNo {
    pub site_id: Uuid,
    pub arquivo_id: Uuid,
    pub tipo: String,
    pub x: i32,
    pub y: i32,
    pub largura: i32,
    pub altura: i32,
    pub texto: String,
    pub preenchimento: Option<String>,
    pub biblioteca: Option<String>,
    pub variante: Option<String>,
    pub catalogo_versao: Option<String>,
    pub props: PropsDeComponente,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BibliotecaDoSite {
    pub versao: String,
    pub ativa: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviaPublicada {
    pub token: String,
    pub numero: i32,
}

pub async fn listar_arquivos(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<ArquivoListado>, ErroDeEstudio> {
    let linhas = sqlx::query!(
        r#"
        select id, nome, favorito, sistema, criado_por, atualizado_em
        from estudio_arquivo
        where site_id = $1
        order by atualizado_em desc, id
        limit 200
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    let publicados = sqlx::query_scalar!(
        r#"select arquivo_id from estudio_publicacao where site_id = $1"#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(linhas
        .into_iter()
        .map(|linha| ArquivoListado {
            compartilhado: publicados.contains(&linha.id),
            id: linha.id,
            nome: linha.nome,
            favorito: linha.favorito,
            sistema: linha.sistema,
            criado_por: linha.criado_por,
            atualizado_em: linha.atualizado_em,
        })
        .collect())
}

pub async fn criar_arquivo(pool: &PgPool, novo: NovoArquivo) -> Result<Uuid, ErroDeEstudio> {
    let mut tx = pool.begin().await?;
    let total = sqlx::query_scalar!(
        r#"select count(*) as "total!" from estudio_arquivo where site_id = $1"#,
        novo.site_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if total >= LIMITE_DE_ARQUIVOS {
        return Err(ErroDeEstudio::LimiteDeArquivos);
    }
    let id = sqlx::query_scalar!(
        r#"
        insert into estudio_arquivo (site_id, nome, pedido, criado_por, sistema)
        values ($1, $2, $3, $4, $5)
        returning id
        "#,
        novo.site_id,
        novo.nome,
        novo.pedido,
        novo.criado_por,
        novo.sistema
    )
    .fetch_one(&mut *tx)
    .await?;
    let pagina_id = sqlx::query_scalar!(
        r#"
        insert into estudio_pagina (site_id, arquivo_id, nome, ordem, largura, altura)
        values ($1, $2, 'Página 1', 0, $3, $4)
        returning id
        "#,
        novo.site_id,
        id,
        novo.largura,
        novo.altura
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        r#"
        insert into estudio_no (
            site_id, arquivo_id, pagina_id, tipo, x, y, largura, altura, texto, ordem
        )
        values ($1, $2, $3, 'texto', $4, $5, $6, $7, $8, 0)
        "#,
        novo.site_id,
        id,
        pagina_id,
        novo.texto_x,
        novo.texto_y,
        novo.texto_largura,
        novo.texto_altura,
        novo.texto
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn abrir_arquivo(
    pool: &PgPool,
    site_id: Uuid,
    arquivo_id: Uuid,
) -> Result<Option<ArquivoAberto>, ErroDeEstudio> {
    let arquivo = sqlx::query!(
        r#"
        select id, nome, pedido, favorito, sistema, criado_por
        from estudio_arquivo
        where site_id = $1 and id = $2
        "#,
        site_id,
        arquivo_id
    )
    .fetch_optional(pool)
    .await?;
    let Some(arquivo) = arquivo else {
        return Ok(None);
    };
    let paginas = sqlx::query!(
        r#"
        select id, nome, largura, altura
        from estudio_pagina
        where site_id = $1 and arquivo_id = $2
        order by ordem, id
        "#,
        site_id,
        arquivo_id
    )
    .fetch_all(pool)
    .await?;
    let nos = sqlx::query!(
        r#"
        select id, pagina_id, tipo, x, y, largura, altura, texto, preenchimento,
               biblioteca, variante, catalogo_versao,
               props as "props: Json<PropsDeComponente>"
        from estudio_no
        where site_id = $1 and arquivo_id = $2
        order by ordem, id
        "#,
        site_id,
        arquivo_id
    )
    .fetch_all(pool)
    .await?;
    let token = sqlx::query_scalar!(
        r#"
        select token from estudio_publicacao
        where site_id = $1 and arquivo_id = $2
        "#,
        site_id,
        arquivo_id
    )
    .fetch_optional(pool)
    .await?;
    let paginas = paginas
        .into_iter()
        .map(|pagina| PaginaAberta {
            nos: nos
                .iter()
                .filter(|no| no.pagina_id == pagina.id)
                .map(|no| NoAberto {
                    id: no.id,
                    tipo: no.tipo.clone(),
                    x: no.x,
                    y: no.y,
                    largura: no.largura,
                    altura: no.altura,
                    texto: no.texto.clone(),
                    preenchimento: no.preenchimento.clone(),
                    biblioteca: no.biblioteca.clone(),
                    variante: no.variante.clone(),
                    catalogo_versao: no.catalogo_versao.clone(),
                    props: no.props.0.clone(),
                })
                .collect(),
            id: pagina.id,
            nome: pagina.nome,
            largura: pagina.largura,
            altura: pagina.altura,
        })
        .collect();
    Ok(Some(ArquivoAberto {
        id: arquivo.id,
        nome: arquivo.nome,
        pedido: arquivo.pedido,
        favorito: arquivo.favorito,
        sistema: arquivo.sistema,
        criado_por: arquivo.criado_por,
        paginas,
        token_da_previa: token,
    }))
}

pub async fn guardar_pedido_e_sistema(
    pool: &PgPool,
    site_id: Uuid,
    arquivo_id: Uuid,
    pedido: &str,
    sistema: &str,
) -> Result<(), ErroDeEstudio> {
    let atualizado = sqlx::query!(
        r#"
        update estudio_arquivo
        set pedido = $3, sistema = $4, atualizado_em = now()
        where site_id = $1 and id = $2
        "#,
        site_id,
        arquivo_id,
        pedido,
        sistema
    )
    .execute(pool)
    .await?;
    if atualizado.rows_affected() == 0 {
        return Err(ErroDeEstudio::NaoEncontrado);
    }
    Ok(())
}

pub async fn definir_sistema(
    pool: &PgPool,
    site_id: Uuid,
    arquivo_id: Uuid,
    sistema: &str,
) -> Result<(), ErroDeEstudio> {
    let atualizado = sqlx::query!(
        r#"
        update estudio_arquivo
        set sistema = $3, atualizado_em = now()
        where site_id = $1 and id = $2
        "#,
        site_id,
        arquivo_id,
        sistema
    )
    .execute(pool)
    .await?;
    if atualizado.rows_affected() == 0 {
        return Err(ErroDeEstudio::NaoEncontrado);
    }
    Ok(())
}

pub async fn alternar_favorito(
    pool: &PgPool,
    site_id: Uuid,
    arquivo_id: Uuid,
) -> Result<bool, ErroDeEstudio> {
    let favorito = sqlx::query_scalar!(
        r#"
        update estudio_arquivo
        set favorito = not favorito, atualizado_em = now()
        where site_id = $1 and id = $2
        returning favorito
        "#,
        site_id,
        arquivo_id
    )
    .fetch_optional(pool)
    .await?;
    favorito.ok_or(ErroDeEstudio::NaoEncontrado)
}

pub async fn biblioteca_docs(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Option<BibliotecaDoSite>, ErroDeEstudio> {
    let linha = sqlx::query!(
        r#"
        select versao, ativa from estudio_biblioteca
        where site_id = $1 and slug = 'docs'
        "#,
        site_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(linha.map(|linha| BibliotecaDoSite {
        versao: linha.versao,
        ativa: linha.ativa,
    }))
}

pub async fn definir_biblioteca_docs(
    pool: &PgPool,
    site_id: Uuid,
    ativa: bool,
    versao: &str,
    conta: &str,
) -> Result<(), ErroDeEstudio> {
    sqlx::query!(
        r#"
        insert into estudio_biblioteca (site_id, slug, versao, ativa, ativada_por)
        values ($1, 'docs', $2, $3, $4)
        on conflict (site_id, slug) do update
        set versao = excluded.versao,
            ativa = excluded.ativa,
            ativada_por = excluded.ativada_por,
            ativada_em = now()
        "#,
        site_id,
        versao,
        ativa,
        conta
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn criar_no(pool: &PgPool, novo: NovoNo) -> Result<Uuid, ErroDeEstudio> {
    let biblioteca = biblioteca_docs(pool, novo.site_id).await?;
    if novo.biblioteca.is_some() && !biblioteca.is_some_and(|item| item.ativa) {
        return Err(ErroDeEstudio::BibliotecaInativa);
    }
    let mut tx = pool.begin().await?;
    let pagina = sqlx::query!(
        r#"
        select id, largura, altura
        from estudio_pagina
        where site_id = $1 and arquivo_id = $2
        order by ordem, id
        limit 1
        "#,
        novo.site_id,
        novo.arquivo_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(pagina) = pagina else {
        return Err(ErroDeEstudio::NaoEncontrado);
    };
    let (x, y) = prender(
        novo.x,
        novo.y,
        Medida {
            largura: novo.largura,
            altura: novo.altura,
        },
        Medida {
            largura: pagina.largura,
            altura: pagina.altura,
        },
    )
    .ok_or(ErroDeEstudio::PosicaoInvalida)?;
    let total = sqlx::query_scalar!(
        r#"
        select count(*) as "total!" from estudio_no
        where site_id = $1 and arquivo_id = $2
        "#,
        novo.site_id,
        novo.arquivo_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if total >= LIMITE_DE_NOS {
        return Err(ErroDeEstudio::LimiteDeNos);
    }
    let ordem = i32::try_from(total).unwrap_or(i32::MAX);
    let id = sqlx::query_scalar!(
        r#"
        insert into estudio_no (
            site_id, arquivo_id, pagina_id, tipo, x, y, largura, altura, texto,
            preenchimento, biblioteca, variante, catalogo_versao, props, ordem
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        returning id
        "#,
        novo.site_id,
        novo.arquivo_id,
        pagina.id,
        novo.tipo,
        x,
        y,
        novo.largura,
        novo.altura,
        novo.texto,
        novo.preenchimento,
        novo.biblioteca,
        novo.variante,
        novo.catalogo_versao,
        Json(&novo.props) as _,
        ordem
    )
    .fetch_one(&mut *tx)
    .await?;
    tocar(&mut tx, novo.site_id, novo.arquivo_id).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn mover_no(
    pool: &PgPool,
    site_id: Uuid,
    arquivo_id: Uuid,
    no_id: Uuid,
    x: i32,
    y: i32,
) -> Result<(), ErroDeEstudio> {
    let mut tx = pool.begin().await?;
    let no = sqlx::query!(
        r#"
        select n.largura, n.altura, p.largura as pagina_largura, p.altura as pagina_altura
        from estudio_no n
        join estudio_pagina p
          on p.id = n.pagina_id and p.site_id = n.site_id and p.arquivo_id = n.arquivo_id
        where n.site_id = $1 and n.arquivo_id = $2 and n.id = $3
        for update of n
        "#,
        site_id,
        arquivo_id,
        no_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(no) = no else {
        return Err(ErroDeEstudio::NoNaoEncontrado);
    };
    let Some((x, y)) = prender(
        x,
        y,
        Medida {
            largura: no.largura,
            altura: no.altura,
        },
        Medida {
            largura: no.pagina_largura,
            altura: no.pagina_altura,
        },
    ) else {
        return Err(ErroDeEstudio::PosicaoInvalida);
    };
    sqlx::query!(
        r#"
        update estudio_no set x = $4, y = $5
        where site_id = $1 and arquivo_id = $2 and id = $3
        "#,
        site_id,
        arquivo_id,
        no_id,
        x,
        y
    )
    .execute(&mut *tx)
    .await?;
    tocar(&mut tx, site_id, arquivo_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn publicar_previa(
    pool: &PgPool,
    site_id: Uuid,
    arquivo_id: Uuid,
    conta: &str,
) -> Result<PreviaPublicada, ErroDeEstudio> {
    let mut tx = pool.begin().await?;
    let arquivo = sqlx::query!(
        r#"
        select id, nome, pedido, sistema
        from estudio_arquivo
        where site_id = $1 and id = $2
        for update
        "#,
        site_id,
        arquivo_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(arquivo) = arquivo else {
        return Err(ErroDeEstudio::NaoEncontrado);
    };
    let paginas = sqlx::query!(
        r#"
        select id, nome, largura, altura
        from estudio_pagina
        where site_id = $1 and arquivo_id = $2
        order by ordem, id
        "#,
        site_id,
        arquivo_id
    )
    .fetch_all(&mut *tx)
    .await?;
    let nos = sqlx::query!(
        r#"
        select pagina_id, tipo, x, y, largura, altura, texto, preenchimento,
               biblioteca, variante, catalogo_versao,
               props as "props: Json<PropsDeComponente>"
        from estudio_no
        where site_id = $1 and arquivo_id = $2
        order by ordem, id
        "#,
        site_id,
        arquivo_id
    )
    .fetch_all(&mut *tx)
    .await?;
    let sistema = cms_dominio::estudio::sistema_ou_padrao(&arquivo.sistema);
    let definicao = DefinicaoDeArquivo {
        nome: arquivo.nome,
        pedido: arquivo.pedido,
        sistema: sistema.slug.to_string(),
        tokens: sistema.tokens.clone(),
        paginas: paginas
            .iter()
            .map(|pagina| PaginaDaDefinicao {
                nome: pagina.nome.clone(),
                largura: pagina.largura,
                altura: pagina.altura,
                nos: nos
                    .iter()
                    .filter(|no| no.pagina_id == pagina.id)
                    .map(|no| NoDaDefinicao {
                        tipo: no.tipo.clone(),
                        x: no.x,
                        y: no.y,
                        largura: no.largura,
                        altura: no.altura,
                        texto: no.texto.clone(),
                        preenchimento: no.preenchimento.clone(),
                        biblioteca: no.biblioteca.clone(),
                        variante: no.variante.clone(),
                        catalogo_versao: no.catalogo_versao.clone(),
                        props: no.props.0.clone(),
                    })
                    .collect(),
            })
            .collect(),
    };
    let numero = sqlx::query_scalar!(
        r#"
        select coalesce(max(numero), 0) as "numero!"
        from estudio_versao
        where site_id = $1 and arquivo_id = $2
        "#,
        site_id,
        arquivo_id
    )
    .fetch_one(&mut *tx)
    .await?
        + 1;
    let versao_id = sqlx::query_scalar!(
        r#"
        insert into estudio_versao (site_id, arquivo_id, numero, definicao, criado_por)
        values ($1, $2, $3, $4, $5)
        returning id
        "#,
        site_id,
        arquivo_id,
        numero,
        Json(&definicao) as _,
        conta
    )
    .fetch_one(&mut *tx)
    .await?;
    let token = sqlx::query_scalar!(
        r#"
        insert into estudio_publicacao (arquivo_id, site_id, versao_id, token, publicado_por)
        values ($1, $2, $3, $4, $5)
        on conflict (arquivo_id) do update
        set versao_id = excluded.versao_id,
            publicado_por = excluded.publicado_por,
            publicado_em = now()
        returning token
        "#,
        arquivo_id,
        site_id,
        versao_id,
        token_novo(),
        conta
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(PreviaPublicada { token, numero })
}

pub async fn previa_por_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<DefinicaoDeArquivo>, ErroDeEstudio> {
    let definicao = sqlx::query_scalar!(
        r#"
        select v.definicao as "definicao: Json<DefinicaoDeArquivo>"
        from estudio_publicacao p
        join estudio_versao v
          on v.id = p.versao_id and v.site_id = p.site_id and v.arquivo_id = p.arquivo_id
        where p.token = $1
        "#,
        token
    )
    .fetch_optional(pool)
    .await?;
    Ok(definicao.map(|json| json.0))
}

async fn tocar(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    site_id: Uuid,
    arquivo_id: Uuid,
) -> Result<(), ErroDeEstudio> {
    sqlx::query!(
        r#"
        update estudio_arquivo set atualizado_em = now()
        where site_id = $1 and id = $2
        "#,
        site_id,
        arquivo_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn token_novo() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
