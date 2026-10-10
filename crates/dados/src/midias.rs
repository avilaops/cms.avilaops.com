//! A biblioteca de mídia: registro do envio, fila de variantes, uso por
//! documento e a mídia no formato que o motor espera.

use cms_dominio::{Ator, Papel};
use motor_web::tipos::{Bloco, Conteudo, Direitos, Formato, Midia, Variante};
use motor_web::validacao::{Gravidade, Problema};
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::ErroDeDados;

/// As mensagens são para quem está na tela.
#[derive(Debug, thiserror::Error)]
pub enum ErroDeMidia {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("Escreva uma descrição da foto para quem não enxerga a imagem.")]
    SemDescricao,
    #[error("Esta imagem já está na biblioteca do site.")]
    Repetida,
    #[error(
        "O site chegou ao limite de espaço para imagens. Apague o que não usa ou fale com a Ávila Ops."
    )]
    SemEspaco,
    #[error("Esta imagem está em uso em uma página ou post. Tire-a de lá antes de apagar.")]
    EmUso,
    #[error("Só quem enviou a imagem, um editor ou o dono pode apagá-la.")]
    SemPermissao,
    #[error("Imagem não encontrada.")]
    Inexistente,
    #[error(
        "O endereço da licença precisa começar com https:// ou ser um caminho do site, como /licenca."
    )]
    EnderecoInvalido,
}

impl From<sqlx::Error> for ErroDeMidia {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

/// SHA-256 em hexadecimal. É a identidade do arquivo dentro do site.
pub fn hash_de_conteudo(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaMidia {
    pub nome: String,
    pub hash: String,
    pub largura: u32,
    pub altura: u32,
    pub alt: String,
    pub legenda: Option<String>,
    pub credito: Option<String>,
    pub direitos: Direitos,
    pub bytes: u64,
}

/// Os direitos como vão para o banco: sem sobra de espaço, vazio vira
/// ausente, e endereço que não é endereço é recusado.
fn direitos_conferidos(direitos: &Direitos) -> Result<Direitos, ErroDeMidia> {
    let endereco = |valor: &Option<String>| match texto_opcional(valor.clone()) {
        Some(e)
            if !(e.starts_with("https://") || e.starts_with('/'))
                || e.starts_with("//")
                || e.contains(char::is_whitespace) =>
        {
            Err(ErroDeMidia::EnderecoInvalido)
        }
        outro => Ok(outro),
    };
    Ok(Direitos {
        autoria: texto_opcional(direitos.autoria.clone()),
        aviso: texto_opcional(direitos.aviso.clone()),
        licenca: endereco(&direitos.licenca)?,
        aquisicao: endereco(&direitos.aquisicao)?,
    })
}

fn inteiro(valor: u64) -> i64 {
    i64::try_from(valor).unwrap_or(i64::MAX)
}

fn texto_opcional(texto: Option<String>) -> Option<String> {
    texto
        .map(|texto| texto.trim().to_string())
        .filter(|texto| !texto.is_empty())
}

/// Registra a imagem enviada. Ela nasce pendente: a rotina de imagens gera as
/// variantes depois. Qualquer papel do site pode enviar.
pub async fn registrar_midia(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    nova: NovaMidia,
    limite_de_bytes_do_site: u64,
) -> Result<Uuid, ErroDeMidia> {
    let alt = nova.alt.trim();
    if alt.is_empty() {
        return Err(ErroDeMidia::SemDescricao);
    }
    let direitos = direitos_conferidos(&nova.direitos)?;
    let mut transacao = pool.begin().await?;
    let em_uso = sqlx::query_scalar!(
        r#"select coalesce(sum(bytes), 0)::bigint as "total!" from midia where site_id = $1"#,
        site_id
    )
    .fetch_one(&mut *transacao)
    .await?;
    if em_uso.saturating_add(inteiro(nova.bytes)) > inteiro(limite_de_bytes_do_site) {
        return Err(ErroDeMidia::SemEspaco);
    }
    let gravada = sqlx::query_scalar!(
        r#"
        insert into midia (site_id, nome, hash, largura, altura, alt, legenda, credito, bytes,
                           enviado_por, autoria, aviso_de_direitos, licenca, aquisicao_de_licenca)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        returning id
        "#,
        site_id,
        nova.nome,
        nova.hash,
        i32::try_from(nova.largura).unwrap_or(i32::MAX),
        i32::try_from(nova.altura).unwrap_or(i32::MAX),
        alt,
        texto_opcional(nova.legenda),
        texto_opcional(nova.credito),
        inteiro(nova.bytes),
        ator.conta,
        direitos.autoria,
        direitos.aviso,
        direitos.licenca,
        direitos.aquisicao
    )
    .fetch_one(&mut *transacao)
    .await;
    let id = match gravada {
        Ok(id) => id,
        Err(sqlx::Error::Database(erro)) if erro.is_unique_violation() => {
            return Err(ErroDeMidia::Repetida);
        }
        Err(erro) => return Err(erro.into()),
    };
    transacao.commit().await?;
    Ok(id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiaDoSite {
    pub id: Uuid,
    pub nome: String,
    pub alt: String,
    pub largura: i32,
    pub altura: i32,
    /// 'pendente', 'pronta' ou 'falhou'.
    pub situacao: String,
    /// A menor variante WebP, para mostrar na lista.
    pub miniatura: Option<String>,
    /// Em quantos documentos a imagem aparece.
    pub usos: i64,
    pub legenda: Option<String>,
    pub credito: Option<String>,
    pub autoria: Option<String>,
    pub aviso: Option<String>,
    pub licenca: Option<String>,
    pub aquisicao: Option<String>,
}

/// A biblioteca do site, da imagem mais nova para a mais antiga.
pub async fn midias_do_site(pool: &PgPool, site_id: Uuid) -> Result<Vec<MidiaDoSite>, ErroDeDados> {
    let midias = sqlx::query_as!(
        MidiaDoSite,
        r#"
        select m.id, m.nome, m.alt, m.largura, m.altura, m.situacao,
               (select v.arquivo from variante v
                where v.midia_id = m.id and v.formato = 'webp'
                order by v.largura limit 1) as miniatura,
               (select count(*) from uso_de_midia u where u.midia_id = m.id) as "usos!",
               m.legenda, m.credito, m.autoria, m.aviso_de_direitos as aviso, m.licenca,
               m.aquisicao_de_licenca as aquisicao
        from midia m
        where m.site_id = $1
        order by m.criado_em desc, m.id
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(midias)
}

fn formato_lido(texto: &str) -> Option<Formato> {
    match texto {
        "avif" => Some(Formato::Avif),
        "webp" => Some(Formato::Webp),
        _ => None,
    }
}

/// As variantes de uma mídia, como o motor as lê.
async fn variantes_de(
    conexao: &mut PgConnection,
    midia_id: Uuid,
) -> Result<Vec<Variante>, sqlx::Error> {
    let linhas = sqlx::query!(
        "select formato, largura, arquivo from variante where midia_id = $1 order by formato, largura",
        midia_id
    )
    .fetch_all(&mut *conexao)
    .await?;
    Ok(linhas
        .into_iter()
        .filter_map(|linha| {
            Some(Variante {
                formato: formato_lido(&linha.formato)?,
                largura: u32::try_from(linha.largura).ok()?,
                url: format!("/midia/{}", linha.arquivo),
            })
        })
        .collect())
}

/// A imagem da biblioteca no contrato do motor, para entrar em um documento.
pub async fn midia_para_conteudo(
    pool: &PgPool,
    site_id: Uuid,
    midia_id: Uuid,
) -> Result<Option<Midia>, ErroDeDados> {
    let mut conexao = pool.acquire().await.map_err(ErroDeDados::Banco)?;
    let Some(linha) = sqlx::query!(
        r#"
        select largura, altura, alt, legenda, credito, autoria, aviso_de_direitos, licenca,
               aquisicao_de_licenca
        from midia where id = $1 and site_id = $2
        "#,
        midia_id,
        site_id
    )
    .fetch_optional(&mut *conexao)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(Midia {
        id: midia_id.to_string(),
        alt: linha.alt,
        largura: u32::try_from(linha.largura).unwrap_or(0),
        altura: u32::try_from(linha.altura).unwrap_or(0),
        variantes: variantes_de(&mut conexao, midia_id).await?,
        legenda: linha.legenda,
        credito: linha.credito,
        direitos: Direitos {
            autoria: linha.autoria,
            aviso: linha.aviso_de_direitos,
            licenca: linha.licenca,
            aquisicao: linha.aquisicao_de_licenca,
        },
    }))
}

/// O que se corrige em uma imagem depois do envio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DescricaoDaMidia {
    pub alt: String,
    pub legenda: Option<String>,
    pub credito: Option<String>,
    pub direitos: Direitos,
}

/// Regrava a descrição, o crédito e os direitos de uma imagem. Quem pode é
/// quem pode apagar: quem enviou, um editor ou o dono. O que já está no ar
/// muda na próxima publicação de cada conteúdo.
pub async fn atualizar_midia(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    midia_id: Uuid,
    descricao: &DescricaoDaMidia,
) -> Result<(), ErroDeMidia> {
    let alt = descricao.alt.trim();
    if alt.is_empty() {
        return Err(ErroDeMidia::SemDescricao);
    }
    let direitos = direitos_conferidos(&descricao.direitos)?;
    let mut transacao = pool.begin().await?;
    let enviado_por = sqlx::query_scalar!(
        "select enviado_por from midia where id = $1 and site_id = $2 for update",
        midia_id,
        site_id
    )
    .fetch_optional(&mut *transacao)
    .await?
    .ok_or(ErroDeMidia::Inexistente)?;
    if ator.papel == Papel::Autor && enviado_por != ator.conta {
        return Err(ErroDeMidia::SemPermissao);
    }
    sqlx::query!(
        r#"
        update midia
        set alt = $2, legenda = $3, credito = $4, autoria = $5, aviso_de_direitos = $6,
            licenca = $7, aquisicao_de_licenca = $8
        where id = $1
        "#,
        midia_id,
        alt,
        texto_opcional(descricao.legenda.clone()),
        texto_opcional(descricao.credito.clone()),
        direitos.autoria,
        direitos.aviso,
        direitos.licenca,
        direitos.aquisicao
    )
    .execute(&mut *transacao)
    .await?;
    transacao.commit().await?;
    Ok(())
}

/// Apaga a imagem e devolve os arquivos das variantes, para quem chama tirar
/// do disco. Em uso, ninguém apaga.
pub async fn apagar_midia(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    midia_id: Uuid,
) -> Result<Vec<String>, ErroDeMidia> {
    let mut transacao = pool.begin().await?;
    let enviado_por = sqlx::query_scalar!(
        "select enviado_por from midia where id = $1 and site_id = $2 for update",
        midia_id,
        site_id
    )
    .fetch_optional(&mut *transacao)
    .await?
    .ok_or(ErroDeMidia::Inexistente)?;
    if ator.papel == Papel::Autor && enviado_por != ator.conta {
        return Err(ErroDeMidia::SemPermissao);
    }
    let arquivos =
        sqlx::query_scalar!("select arquivo from variante where midia_id = $1", midia_id)
            .fetch_all(&mut *transacao)
            .await?;
    let apagada = sqlx::query!("delete from midia where id = $1", midia_id)
        .execute(&mut *transacao)
        .await;
    match apagada {
        Ok(_) => {}
        Err(sqlx::Error::Database(erro)) if erro.is_foreign_key_violation() => {
            return Err(ErroDeMidia::EmUso);
        }
        Err(erro) => return Err(erro.into()),
    }
    transacao.commit().await?;
    Ok(arquivos)
}

/// Uma imagem separada para gerar variantes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiaAProcessar {
    pub id: Uuid,
    pub site_id: Uuid,
    pub nome: String,
}

/// Separa uma imagem pendente para esta instância processar. A reivindicação
/// já conta a tentativa e marca a próxima para dali a cinco minutos: se o
/// processo cair no meio, a imagem volta sozinha.
pub async fn reivindicar_midia(pool: &PgPool) -> Result<Option<MidiaAProcessar>, ErroDeDados> {
    let midia = sqlx::query_as!(
        MidiaAProcessar,
        r#"
        update midia
        set tentativas = tentativas + 1, proxima_tentativa_em = now() + interval '5 minutes'
        where id = (
            select id from midia
            where situacao = 'pendente' and proxima_tentativa_em <= now()
            order by criado_em
            limit 1
            for update skip locked
        )
        returning id, site_id, nome
        "#
    )
    .fetch_optional(pool)
    .await?;
    Ok(midia)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarianteGravada {
    pub formato: Formato,
    pub largura: u32,
    pub arquivo: String,
    pub bytes: u64,
}

/// As variantes estão no disco: a imagem fica pronta para ir ao ar.
pub async fn concluir_midia(
    pool: &PgPool,
    midia_id: Uuid,
    largura: u32,
    altura: u32,
    variantes: &[VarianteGravada],
) -> Result<(), ErroDeDados> {
    let mut transacao = pool.begin().await?;
    for variante in variantes {
        sqlx::query!(
            r#"
            insert into variante (midia_id, formato, largura, arquivo, bytes)
            values ($1, $2, $3, $4, $5)
            on conflict (midia_id, formato, largura)
                do update set arquivo = excluded.arquivo, bytes = excluded.bytes
            "#,
            midia_id,
            variante.formato.extensao(),
            i32::try_from(variante.largura).unwrap_or(i32::MAX),
            variante.arquivo,
            inteiro(variante.bytes)
        )
        .execute(&mut *transacao)
        .await?;
    }
    sqlx::query!(
        "update midia set situacao = 'pronta', largura = $2, altura = $3 where id = $1",
        midia_id,
        i32::try_from(largura).unwrap_or(i32::MAX),
        i32::try_from(altura).unwrap_or(i32::MAX)
    )
    .execute(&mut *transacao)
    .await?;
    transacao.commit().await?;
    Ok(())
}

/// A geração falhou. Na terceira tentativa a imagem fica marcada, e o painel
/// pede novo envio.
pub async fn falhar_midia(pool: &PgPool, midia_id: Uuid) -> Result<(), ErroDeDados> {
    sqlx::query!(
        "update midia set situacao = 'falhou' where id = $1 and tentativas >= 3",
        midia_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Passa por todas as imagens de um conteúdo: capa, corpo, foto do autor e
/// imagem social.
fn para_cada_midia(conteudo: &mut Conteudo, mut visitar: impl FnMut(&mut Midia)) {
    let no_corpo = |corpo: &mut [Bloco], visitar: &mut dyn FnMut(&mut Midia)| {
        for bloco in corpo {
            bloco.midias_mut().into_iter().for_each(&mut *visitar);
        }
    };
    match conteudo {
        Conteudo::Pagina(pagina) => {
            pagina.capa.iter_mut().for_each(&mut visitar);
            pagina.seo.imagem_social.iter_mut().for_each(&mut visitar);
            no_corpo(&mut pagina.corpo, &mut visitar);
        }
        Conteudo::Post(post) => {
            visitar(&mut post.capa);
            visitar(&mut post.autor.foto);
            post.seo.imagem_social.iter_mut().for_each(&mut visitar);
            no_corpo(&mut post.corpo, &mut visitar);
        }
        Conteudo::Produto(_) => {}
    }
}

/// Os identificadores das imagens que vêm da biblioteca. Identificador que
/// não é da biblioteca (conteúdo de carga direta) fica de fora.
fn ids_da_biblioteca(conteudo: &Conteudo) -> Vec<Uuid> {
    let mut ids = Vec::new();
    para_cada_midia(&mut conteudo.clone(), |midia| {
        ids.extend(Uuid::parse_str(&midia.id).ok());
    });
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// A trava que só o CMS conhece: imagem da biblioteca sem variante, ou que
/// não é deste site, não vai ao ar.
pub(crate) async fn problemas_de_midia(
    conexao: &mut PgConnection,
    site_id: Uuid,
    conteudo: &Conteudo,
) -> Result<Vec<Problema>, sqlx::Error> {
    let ids = ids_da_biblioteca(conteudo);
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let prontas = sqlx::query_scalar!(
        "select id from midia where site_id = $1 and id = any($2) and situacao = 'pronta'",
        site_id,
        &ids
    )
    .fetch_all(&mut *conexao)
    .await?;
    if prontas.len() == ids.len() {
        return Ok(Vec::new());
    }
    Ok(vec![Problema {
        codigo: "midia.sem-variante",
        campo: "midia".to_string(),
        gravidade: Gravidade::Bloqueia,
        mensagem: "Uma imagem deste conteúdo ainda está sendo preparada, falhou ou não é deste site. Aguarde um instante ou escolha outra.".to_string(),
    }])
}

/// Troca dimensões, variantes e direitos de cada imagem da biblioteca pelo que
/// está gravado agora. O conteúdo pode ter sido salvo antes de as variantes
/// existirem.
pub(crate) async fn hidratar(
    conexao: &mut PgConnection,
    site_id: Uuid,
    conteudo: &mut Conteudo,
) -> Result<(), sqlx::Error> {
    for id in ids_da_biblioteca(conteudo) {
        let Some(dimensoes) = sqlx::query!(
            r#"
            select largura, altura, autoria, aviso_de_direitos, licenca, aquisicao_de_licenca
            from midia where id = $1 and site_id = $2
            "#,
            id,
            site_id
        )
        .fetch_optional(&mut *conexao)
        .await?
        else {
            continue;
        };
        let variantes = variantes_de(conexao, id).await?;
        let direitos = Direitos {
            autoria: dimensoes.autoria,
            aviso: dimensoes.aviso_de_direitos,
            licenca: dimensoes.licenca,
            aquisicao: dimensoes.aquisicao_de_licenca,
        };
        let texto = id.to_string();
        para_cada_midia(conteudo, |midia| {
            if midia.id == texto {
                midia.largura = u32::try_from(dimensoes.largura).unwrap_or(midia.largura);
                midia.altura = u32::try_from(dimensoes.altura).unwrap_or(midia.altura);
                midia.variantes = variantes.clone();
                midia.direitos = direitos.clone();
            }
        });
    }
    Ok(())
}

/// O mesmo que a publicação faz, para quem recebe conteúdo de fora: o que o
/// conector manda sobre uma imagem não vale, vale a biblioteca.
pub async fn hidratar_conteudo(
    pool: &PgPool,
    site_id: Uuid,
    conteudo: &mut Conteudo,
) -> Result<(), ErroDeDados> {
    let mut conexao = pool.acquire().await?;
    hidratar(&mut conexao, site_id, conteudo).await?;
    Ok(())
}

/// Regrava de onde as imagens são usadas: o rascunho e o que está no ar.
pub(crate) async fn atualizar_usos(
    conexao: &mut PgConnection,
    site_id: Uuid,
    documento_id: Uuid,
) -> Result<(), sqlx::Error> {
    let conteudos = sqlx::query_scalar!(
        r#"
        select v.conteudo as "conteudo: Json<Conteudo>"
        from documento d
        join versao v on v.id in (d.versao_rascunho, d.versao_publicada)
        where d.id = $1
        "#,
        documento_id
    )
    .fetch_all(&mut *conexao)
    .await?;
    let mut ids: Vec<Uuid> = conteudos
        .iter()
        .flat_map(|conteudo| ids_da_biblioteca(&conteudo.0))
        .collect();
    ids.sort_unstable();
    ids.dedup();

    sqlx::query!(
        "delete from uso_de_midia where documento_id = $1",
        documento_id
    )
    .execute(&mut *conexao)
    .await?;
    sqlx::query!(
        r#"
        insert into uso_de_midia (midia_id, documento_id)
        select id, $1 from midia where site_id = $2 and id = any($3)
        "#,
        documento_id,
        site_id,
        &ids
    )
    .execute(&mut *conexao)
    .await?;
    Ok(())
}
