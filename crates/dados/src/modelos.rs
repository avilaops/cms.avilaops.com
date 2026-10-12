//! A biblioteca de modelos 3D: registro do envio, lista, remoção e o modelo
//! no formato que o motor espera.

use cms_dominio::{Ator, Papel};
use motor_web::tipos::{Bloco, Conteudo, Modelo3d};
use motor_web::validacao::CAMINHO_DOS_MODELOS;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::ErroDeDados;

/// As mensagens são para quem está na tela.
#[derive(Debug, thiserror::Error)]
pub enum ErroDeModelo {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("Descreva o que o modelo 3D mostra, para quem não o enxerga.")]
    SemDescricao,
    #[error("Este modelo 3D já está na biblioteca do site.")]
    Repetido,
    #[error(
        "O site chegou ao limite de espaço para arquivos. Apague o que não usa ou fale com a Ávila Ops."
    )]
    SemEspaco,
    #[error("Este modelo 3D está em uso em uma página ou post. Tire-o de lá antes de apagar.")]
    EmUso,
    #[error("Só quem enviou o modelo 3D, um editor ou o dono pode apagá-lo.")]
    SemPermissao,
    #[error("Modelo 3D não encontrado.")]
    Inexistente,
}

impl From<sqlx::Error> for ErroDeModelo {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

fn inteiro(valor: u64) -> i64 {
    i64::try_from(valor).unwrap_or(i64::MAX)
}

/// Quanto o site já guarda, em bytes: os originais das imagens e os modelos
/// 3D. É uma conta só, contra o mesmo limite.
pub(crate) async fn espaco_em_uso(
    conexao: &mut PgConnection,
    site_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        select (
            (select coalesce(sum(bytes), 0) from midia where site_id = $1)
            + (select coalesce(sum(glb_bytes + coalesce(usdz_bytes, 0)), 0)
               from modelo_3d where site_id = $1)
        )::bigint as "total!"
        "#,
        site_id
    )
    .fetch_one(conexao)
    .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoModelo {
    pub nome: String,
    pub descricao: String,
    /// SHA-256 do GLB, como em [`crate::hash_de_conteudo`].
    pub hash: String,
    pub glb_bytes: u64,
    pub usdz_bytes: Option<u64>,
}

/// Registra o modelo enviado. Qualquer papel do site pode enviar.
pub async fn registrar_modelo(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    novo: NovoModelo,
    limite_de_bytes_do_site: u64,
) -> Result<Uuid, ErroDeModelo> {
    let descricao = novo.descricao.trim();
    if descricao.is_empty() {
        return Err(ErroDeModelo::SemDescricao);
    }
    let mut transacao = pool.begin().await?;
    let em_uso = espaco_em_uso(&mut transacao, site_id).await?;
    let chegando = inteiro(novo.glb_bytes).saturating_add(inteiro(novo.usdz_bytes.unwrap_or(0)));
    if em_uso.saturating_add(chegando) > inteiro(limite_de_bytes_do_site) {
        return Err(ErroDeModelo::SemEspaco);
    }
    let gravado = sqlx::query_scalar!(
        r#"
        insert into modelo_3d (site_id, nome, descricao, hash, glb_bytes, usdz_bytes, enviado_por)
        values ($1, $2, $3, $4, $5, $6, $7)
        returning id
        "#,
        site_id,
        novo.nome,
        descricao,
        novo.hash,
        inteiro(novo.glb_bytes),
        novo.usdz_bytes.map(inteiro),
        ator.conta
    )
    .fetch_one(&mut *transacao)
    .await;
    let id = match gravado {
        Ok(id) => id,
        Err(sqlx::Error::Database(erro)) if erro.is_unique_violation() => {
            return Err(ErroDeModelo::Repetido);
        }
        Err(erro) => return Err(erro.into()),
    };
    transacao.commit().await?;
    Ok(id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeloDoSite {
    pub id: Uuid,
    pub nome: String,
    pub descricao: String,
    pub glb_bytes: i64,
    pub usdz_bytes: Option<i64>,
    pub enviado_por: String,
}

/// Os modelos 3D do site, do mais novo para o mais antigo.
pub async fn modelos_do_site(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<ModeloDoSite>, ErroDeDados> {
    let modelos = sqlx::query_as!(
        ModeloDoSite,
        r#"
        select id, nome, descricao, glb_bytes, usdz_bytes, enviado_por
        from modelo_3d
        where site_id = $1
        order by criado_em desc, id
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(modelos)
}

/// Onde o site serve o arquivo de um modelo.
fn endereco(id: Uuid, extensao: &str) -> String {
    format!("{CAMINHO_DOS_MODELOS}{id}.{extensao}")
}

async fn modelo_gravado(
    conexao: &mut PgConnection,
    site_id: Uuid,
    modelo_id: Uuid,
) -> Result<Option<Modelo3d>, sqlx::Error> {
    let linha = sqlx::query!(
        "select descricao, usdz_bytes from modelo_3d where id = $1 and site_id = $2",
        modelo_id,
        site_id
    )
    .fetch_optional(conexao)
    .await?;
    Ok(linha.map(|linha| Modelo3d {
        id: modelo_id.to_string(),
        glb: endereco(modelo_id, "glb"),
        usdz: linha.usdz_bytes.map(|_| endereco(modelo_id, "usdz")),
        descricao: linha.descricao,
    }))
}

/// O modelo da biblioteca no contrato do motor, para entrar em um documento.
pub async fn modelo_para_conteudo(
    pool: &PgPool,
    site_id: Uuid,
    modelo_id: Uuid,
) -> Result<Option<Modelo3d>, ErroDeDados> {
    let mut conexao = pool.acquire().await?;
    Ok(modelo_gravado(&mut conexao, site_id, modelo_id).await?)
}

/// Troca cada modelo 3D do corpo pelo que está na biblioteca do site: os
/// endereços e a descrição. Modelo que não é deste site fica sem arquivo, e a
/// validação do motor não deixa publicar.
pub(crate) async fn hidratar(
    conexao: &mut PgConnection,
    site_id: Uuid,
    conteudo: &mut Conteudo,
) -> Result<(), sqlx::Error> {
    let corpo = match conteudo {
        Conteudo::Pagina(pagina) => &mut pagina.corpo,
        Conteudo::Post(post) => &mut post.corpo,
        Conteudo::Produto(_) => return Ok(()),
    };
    for bloco in corpo {
        let Bloco::Modelo3d { modelo, .. } = bloco else {
            continue;
        };
        let gravado = match Uuid::parse_str(&modelo.id) {
            Ok(id) => modelo_gravado(&mut *conexao, site_id, id).await?,
            Err(_) => None,
        };
        match gravado {
            Some(gravado) => *modelo = gravado,
            None => {
                modelo.glb = String::new();
                modelo.usdz = None;
            }
        }
    }
    Ok(())
}

/// Apaga o modelo e diz se ele tinha USDZ, para quem chama tirar os arquivos
/// do disco. Em uso no rascunho ou no que está no ar, ninguém apaga.
pub async fn apagar_modelo(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    modelo_id: Uuid,
) -> Result<bool, ErroDeModelo> {
    let mut transacao = pool.begin().await?;
    let linha = sqlx::query!(
        "select enviado_por, usdz_bytes from modelo_3d where id = $1 and site_id = $2 for update",
        modelo_id,
        site_id
    )
    .fetch_optional(&mut *transacao)
    .await?
    .ok_or(ErroDeModelo::Inexistente)?;
    if ator.papel == Papel::Autor && linha.enviado_por != ator.conta {
        return Err(ErroDeModelo::SemPermissao);
    }
    // O bloco guarda o identificador do modelo: basta procurá-lo no conteúdo
    // das versões que valem (o rascunho e a publicada) de cada documento.
    let em_uso = sqlx::query_scalar!(
        r#"
        select exists(
            select 1
            from documento d
            join versao v on v.id in (d.versao_rascunho, d.versao_publicada)
            where d.site_id = $1 and v.conteudo::text like '%' || $2 || '%'
        ) as "em_uso!"
        "#,
        site_id,
        modelo_id.to_string()
    )
    .fetch_one(&mut *transacao)
    .await?;
    if em_uso {
        return Err(ErroDeModelo::EmUso);
    }
    sqlx::query!("delete from modelo_3d where id = $1", modelo_id)
        .execute(&mut *transacao)
        .await?;
    transacao.commit().await?;
    Ok(linha.usdz_bytes.is_some())
}
