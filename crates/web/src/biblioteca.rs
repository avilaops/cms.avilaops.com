//! A tela de imagens de um site: envio, lista e remoção.
//!
//! O envio grava o original e responde. As variantes saem depois, pela rotina
//! de imagens do servidor.

use std::io::Cursor;
use std::path::PathBuf;

use askama::Template;
use axum::body::Bytes;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use cms_dados::{ErroDeMidia, MidiaDoSite, NovaMidia, SiteGravado};
use cms_dominio::{Ator, Conta, Papel};
use motor_web::validacao::validar_upload;
use uuid::Uuid;

use crate::acesso::ao_site;
use crate::resposta::{ErroWeb, html_privado, simples};
use crate::{Estado, midia};

struct Linha {
    id: Uuid,
    alt: String,
    miniatura: Option<String>,
    dimensoes: String,
    situacao: &'static str,
    uso: String,
    pode_apagar: bool,
}

impl Linha {
    fn nova(midia: MidiaDoSite, slug: &str, ator: &Ator) -> Self {
        Self {
            id: midia.id,
            alt: midia.alt,
            miniatura: midia
                .miniatura
                .map(|arquivo| format!("/painel/sites/{slug}/midia/arquivo/{arquivo}")),
            dimensoes: format!("{} × {}", midia.largura, midia.altura),
            situacao: match midia.situacao.as_str() {
                "pronta" => "pronta",
                "falhou" => "falhou: envie de novo",
                _ => "preparando",
            },
            uso: match midia.usos {
                0 => "sem uso".to_string(),
                1 => "em 1 conteúdo".to_string(),
                n => format!("em {n} conteúdos"),
            },
            // Quem enviou não vem na lista: o Autor tenta, e o banco decide.
            pode_apagar: midia.usos == 0 && ator.papel != Papel::Autor,
        }
    }
}

#[derive(Template)]
#[template(path = "midia.html")]
struct PaginaDeMidia {
    site: String,
    slug: String,
    erro: Option<String>,
    aviso: Option<String>,
    midias: Vec<Linha>,
    alt: String,
}

#[derive(Default)]
struct Recado {
    erro: Option<String>,
    aviso: Option<String>,
    alt: String,
}

async fn pagina(
    estado: &Estado,
    site: &SiteGravado,
    ator: &Ator,
    status: StatusCode,
    recado: Recado,
) -> Result<Response, ErroWeb> {
    let midias = cms_dados::midias_do_site(&estado.pool, site.id)
        .await?
        .into_iter()
        .map(|midia| Linha::nova(midia, &site.slug, ator))
        .collect();
    let corpo = PaginaDeMidia {
        site: site.perfil.nome.clone(),
        slug: site.slug.clone(),
        erro: recado.erro,
        aviso: recado.aviso,
        midias,
        alt: recado.alt,
    }
    .render()?;
    Ok(html_privado(status, corpo))
}

pub async fn abrir(estado: &Estado, conta: &Conta, slug: &str) -> Result<Response, ErroWeb> {
    match ao_site(estado, conta, slug).await? {
        Ok((site, ator)) => pagina(estado, &site, &ator, StatusCode::OK, Recado::default()).await,
        Err(resposta) => Ok(resposta),
    }
}

/// A miniatura na lista. Sai pelo painel porque a tela não carrega imagem de
/// outro host.
pub async fn arquivo(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo: &str,
) -> Result<Response, ErroWeb> {
    match ao_site(estado, conta, slug).await? {
        Ok((site, _)) => {
            midia::servir(&estado.configuracao.diretorio_de_midia, site.id, arquivo).await
        }
        Err(resposta) => Ok(resposta),
    }
}

#[derive(Default)]
struct Envio {
    nome: String,
    bytes: Bytes,
    alt: String,
    credito: String,
}

/// Lê o formulário de envio. `None` para o que não é um formulário válido.
async fn ler_envio(cabecalhos: &HeaderMap, corpo: Bytes) -> Option<Envio> {
    let tipo = cabecalhos.get(CONTENT_TYPE)?.to_str().ok()?;
    let limite = multer::parse_boundary(tipo).ok()?;
    let fluxo = futures_util::stream::once(async move { Ok::<Bytes, std::io::Error>(corpo) });
    let mut partes = multer::Multipart::new(fluxo, limite);
    let mut envio = Envio::default();
    while let Some(campo) = partes.next_field().await.ok()? {
        let nome_do_campo = campo.name().map(str::to_string);
        let nome_do_arquivo = campo.file_name().map(str::to_string);
        let dados = campo.bytes().await.ok()?;
        match nome_do_campo.as_deref() {
            Some("arquivo") => {
                envio.nome = nome_do_arquivo.unwrap_or_default();
                envio.bytes = dados;
            }
            Some("alt") => envio.alt = String::from_utf8_lossy(&dados).trim().to_string(),
            Some("credito") => envio.credito = String::from_utf8_lossy(&dados).trim().to_string(),
            _ => {}
        }
    }
    Some(envio)
}

fn dimensoes(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
        .filter(|(largura, altura)| *largura > 0 && *altura > 0)
}

/// Onde fica o original de uma imagem. Fora do que o site serve.
pub fn caminho_do_original(diretorio: &std::path::Path, site_id: Uuid, midia_id: Uuid) -> PathBuf {
    diretorio
        .join(site_id.to_string())
        .join("originais")
        .join(midia_id.to_string())
}

pub async fn enviar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    cabecalhos: &HeaderMap,
    corpo: &Bytes,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Some(envio) = ler_envio(cabecalhos, corpo.clone()).await else {
        return Ok(simples(StatusCode::BAD_REQUEST, "Formulário inválido."));
    };
    let recusar = |mensagem: String, alt: String| Recado {
        erro: Some(mensagem),
        alt,
        ..Recado::default()
    };
    const RECUSADO: StatusCode = StatusCode::UNPROCESSABLE_ENTITY;

    if let Some(problema) = validar_upload(&envio.nome, &envio.bytes).into_iter().next() {
        let recado = recusar(problema.mensagem, envio.alt);
        return pagina(estado, &site, &ator, RECUSADO, recado).await;
    }
    let Some((largura, altura)) = dimensoes(&envio.bytes) else {
        let recado = recusar(
            "Não foi possível abrir esta imagem. O arquivo pode estar corrompido: envie outro."
                .to_string(),
            envio.alt,
        );
        return pagina(estado, &site, &ator, RECUSADO, recado).await;
    };

    let nova = NovaMidia {
        nome: envio.nome,
        hash: cms_dados::hash_de_conteudo(&envio.bytes),
        largura,
        altura,
        alt: envio.alt.clone(),
        legenda: None,
        credito: Some(envio.credito),
        bytes: envio.bytes.len() as u64,
    };
    let limite = estado.configuracao.limite_de_midia_por_site;
    let midia_id =
        match cms_dados::registrar_midia(&estado.pool, site.id, &ator, nova, limite).await {
            Ok(id) => id,
            Err(ErroDeMidia::Dados(erro)) => return Err(erro.into()),
            Err(erro) => {
                let recado = recusar(erro.to_string(), envio.alt);
                return pagina(estado, &site, &ator, RECUSADO, recado).await;
            }
        };

    let destino = caminho_do_original(&estado.configuracao.diretorio_de_midia, site.id, midia_id);
    let gravado = async {
        if let Some(pasta) = destino.parent() {
            tokio::fs::create_dir_all(pasta).await?;
        }
        tokio::fs::write(&destino, &envio.bytes).await
    }
    .await;
    if let Err(erro) = gravado {
        // Sem o original, a linha só deixaria uma imagem que nunca fica pronta.
        if let Err(falha) = cms_dados::apagar_midia(&estado.pool, site.id, &ator, midia_id).await {
            tracing::error!(%falha, midia = %midia_id, "imagem sem original ficou registrada");
        }
        return Err(erro.into());
    }

    let recado = Recado {
        aviso: Some("Imagem enviada. Ela fica pronta em instantes.".to_string()),
        ..Recado::default()
    };
    pagina(estado, &site, &ator, StatusCode::OK, recado).await
}

pub async fn apagar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    midia_id: &str,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Ok(midia_id) = Uuid::parse_str(midia_id) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    let (status, recado) = match cms_dados::apagar_midia(&estado.pool, site.id, &ator, midia_id)
        .await
    {
        Ok(arquivos) => {
            let diretorio = &estado.configuracao.diretorio_de_midia;
            let pasta = diretorio.join(site.id.to_string());
            let original = caminho_do_original(diretorio, site.id, midia_id);
            for caminho in arquivos
                .iter()
                .map(|arquivo| pasta.join(arquivo))
                .chain([original])
            {
                // Arquivo que já não existe não impede a remoção.
                if let Err(erro) = tokio::fs::remove_file(&caminho).await {
                    tracing::warn!(%erro, caminho = %caminho.display(), "arquivo de imagem não removido");
                }
            }
            (
                StatusCode::OK,
                Recado {
                    aviso: Some("Imagem apagada.".to_string()),
                    ..Recado::default()
                },
            )
        }
        Err(ErroDeMidia::Dados(erro)) => return Err(erro.into()),
        Err(erro) => {
            let status = match erro {
                ErroDeMidia::Inexistente => StatusCode::NOT_FOUND,
                ErroDeMidia::SemPermissao => StatusCode::FORBIDDEN,
                _ => StatusCode::CONFLICT,
            };
            (
                status,
                Recado {
                    erro: Some(erro.to_string()),
                    ..Recado::default()
                },
            )
        }
    };
    pagina(estado, &site, &ator, status, recado).await
}
