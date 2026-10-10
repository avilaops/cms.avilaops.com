//! Autores e categorias de um site, e a leitura de documentos para o painel.

use chrono::{DateTime, Utc};
use cms_dominio::{Ator, Papel};
use motor_web::tipos::{Autor, Categoria, Conteudo, Midia};
use motor_web::validacao::normalizar_slug;
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ErroDeDados;
use crate::midias::midia_para_conteudo;

/// As mensagens são para quem está na tela.
#[derive(Debug, thiserror::Error)]
pub enum ErroDeCatalogo {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("Só um editor ou o dono do site pode mexer em autores e categorias.")]
    SemPermissao,
    #[error("Informe o nome.")]
    SemNome,
    #[error("A foto escolhida não é da biblioteca deste site.")]
    FotoDesconhecida,
}

impl From<sqlx::Error> for ErroDeCatalogo {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

fn exigir_editor(ator: &Ator) -> Result<(), ErroDeCatalogo> {
    if ator.papel == Papel::Autor {
        Err(ErroDeCatalogo::SemPermissao)
    } else {
        Ok(())
    }
}

/// Uma lista digitada com um item por linha.
fn linhas(texto: &str) -> Vec<String> {
    texto
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .map(str::to_string)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutorDoSite {
    pub slug: String,
    pub nome: String,
    pub cargo: String,
    pub bio: String,
    pub foto_id: Option<Uuid>,
    pub perfis: Vec<String>,
    pub credenciais: Vec<String>,
}

pub async fn autores_do_site(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<AutorDoSite>, ErroDeDados> {
    let autores = sqlx::query_as!(
        AutorDoSite,
        r#"
        select slug, nome, cargo, bio, foto_id, perfis, credenciais
        from autor where site_id = $1 order by nome, slug
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(autores)
}

/// O que o painel manda ao salvar um autor. Listas vêm com um item por linha.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DadosDoAutor {
    pub nome: String,
    pub cargo: String,
    pub bio: String,
    pub foto_id: Option<Uuid>,
    pub perfis: String,
    pub credenciais: String,
}

/// Cria o autor ou regrava o que já existe com o mesmo nome. O endereço dele
/// (`/autor/<slug>`) sai do nome e não muda depois.
pub async fn salvar_autor(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    dados: &DadosDoAutor,
) -> Result<String, ErroDeCatalogo> {
    exigir_editor(ator)?;
    let nome = dados.nome.trim();
    let slug = normalizar_slug(nome);
    if slug.is_empty() {
        return Err(ErroDeCatalogo::SemNome);
    }
    if let Some(foto_id) = dados.foto_id {
        let do_site = sqlx::query_scalar!(
            r#"select exists(select 1 from midia where id = $1 and site_id = $2) as "existe!""#,
            foto_id,
            site_id
        )
        .fetch_one(pool)
        .await?;
        if !do_site {
            return Err(ErroDeCatalogo::FotoDesconhecida);
        }
    }
    sqlx::query!(
        r#"
        insert into autor (site_id, slug, nome, cargo, bio, foto_id, perfis, credenciais)
        values ($1, $2, $3, $4, $5, $6, $7, $8)
        on conflict (site_id, slug) do update
            set nome = excluded.nome, cargo = excluded.cargo, bio = excluded.bio,
                foto_id = excluded.foto_id, perfis = excluded.perfis,
                credenciais = excluded.credenciais
        "#,
        site_id,
        slug,
        nome,
        dados.cargo.trim(),
        dados.bio.trim(),
        dados.foto_id,
        &linhas(&dados.perfis),
        &linhas(&dados.credenciais)
    )
    .execute(pool)
    .await?;
    Ok(slug)
}

/// O autor no contrato do motor, para entrar em um post. Sem foto, a foto
/// vai vazia e o motor barra a publicação com a mensagem certa.
pub async fn autor_para_conteudo(
    pool: &PgPool,
    site_id: Uuid,
    slug: &str,
) -> Result<Option<Autor>, ErroDeDados> {
    let Some(autor) = sqlx::query_as!(
        AutorDoSite,
        r#"
        select slug, nome, cargo, bio, foto_id, perfis, credenciais
        from autor where site_id = $1 and slug = $2
        "#,
        site_id,
        slug
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let foto = match autor.foto_id {
        Some(foto_id) => midia_para_conteudo(pool, site_id, foto_id).await?,
        None => None,
    };
    Ok(Some(Autor {
        slug: autor.slug,
        nome: autor.nome,
        cargo: autor.cargo,
        bio: autor.bio,
        foto: foto.unwrap_or_else(midia_vazia),
        perfis: autor.perfis,
        credenciais: autor.credenciais,
    }))
}

/// O lugar de uma imagem que ainda não foi escolhida.
pub fn midia_vazia() -> Midia {
    Midia {
        id: String::new(),
        alt: String::new(),
        largura: 0,
        altura: 0,
        variantes: Vec::new(),
        legenda: None,
        credito: None,
    }
}

pub async fn categorias_do_site(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<Categoria>, ErroDeDados> {
    let linhas = sqlx::query!(
        "select slug, nome from categoria where site_id = $1 order by nome, slug",
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(linhas
        .into_iter()
        .map(|linha| Categoria {
            slug: linha.slug,
            nome: linha.nome,
        })
        .collect())
}

/// Cria a categoria ou regrava o nome da que tem o mesmo endereço.
pub async fn salvar_categoria(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    nome: &str,
) -> Result<String, ErroDeCatalogo> {
    exigir_editor(ator)?;
    let nome = nome.trim();
    let slug = normalizar_slug(nome);
    if slug.is_empty() {
        return Err(ErroDeCatalogo::SemNome);
    }
    sqlx::query!(
        r#"
        insert into categoria (site_id, slug, nome) values ($1, $2, $3)
        on conflict (site_id, slug) do update set nome = excluded.nome
        "#,
        site_id,
        slug,
        nome
    )
    .execute(pool)
    .await?;
    Ok(slug)
}

/// Um documento na lista do painel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentoDoSite {
    pub id: Uuid,
    pub especie: String,
    /// O título do rascunho, ou do que está no ar.
    pub titulo: String,
    /// 'rascunho', 'publicado' ou 'despublicado': o que o visitante vê.
    pub situacao: String,
    pub tem_rascunho: bool,
    pub em_revisao: bool,
    pub caminho: Option<String>,
}

/// Páginas e posts do site, do que foi mexido por último para trás.
pub async fn documentos_do_site(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<DocumentoDoSite>, ErroDeDados> {
    let documentos = sqlx::query_as!(
        DocumentoDoSite,
        r#"
        select d.id, d.especie,
               coalesce(v.conteudo -> 'dados' ->> 'titulo', '') as "titulo!",
               d.situacao,
               (d.versao_rascunho is not null) as "tem_rascunho!",
               (d.revisao_pedida_em is not null) as "em_revisao!",
               d.caminho
        from documento d
        left join versao v on v.id = coalesce(d.versao_rascunho, d.versao_publicada)
        where d.site_id = $1
        order by coalesce(v.criado_em, d.criado_em) desc, d.id
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(documentos)
}

/// Um documento aberto no editor: o rascunho, ou o que está no ar quando não
/// há rascunho.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentoAberto {
    pub id: Uuid,
    pub situacao: String,
    pub tem_rascunho: bool,
    pub em_revisao: bool,
    pub caminho: Option<String>,
    pub criado_por: Option<String>,
    /// A publicação marcada para depois, se houver.
    pub agendado_para: Option<DateTime<Utc>>,
    pub conteudo: Conteudo,
}

pub async fn abrir_documento(
    pool: &PgPool,
    site_id: Uuid,
    documento_id: Uuid,
) -> Result<Option<DocumentoAberto>, ErroDeDados> {
    let linha = sqlx::query!(
        r#"
        select d.id, d.situacao, d.caminho, d.criado_por, d.agendado_para,
               (d.versao_rascunho is not null) as "tem_rascunho!",
               (d.revisao_pedida_em is not null) as "em_revisao!",
               v.conteudo as "conteudo: Json<Conteudo>"
        from documento d
        join versao v on v.id = coalesce(d.versao_rascunho, d.versao_publicada)
        where d.id = $1 and d.site_id = $2
        "#,
        documento_id,
        site_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(linha.map(|linha| DocumentoAberto {
        id: linha.id,
        situacao: linha.situacao,
        tem_rascunho: linha.tem_rascunho,
        em_revisao: linha.em_revisao,
        caminho: linha.caminho,
        criado_por: linha.criado_por,
        agendado_para: linha.agendado_para,
        conteudo: linha.conteudo.0,
    }))
}
