//! As mensagens que os visitantes mandaram pelos formulários de um site.
//! É dado pessoal: só o dono e os editores do site as veem.

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;
use cms_dominio::{Ator, Conta, Papel, datas};

use crate::Estado;
use crate::acesso::ao_site;
use crate::resposta::{ErroWeb, html_privado, simples};

/// Quantas mensagens a tela e o conector mostram: as mais recentes.
pub const MENSAGENS_NA_LISTA: i64 = 100;

/// Quem escreve mas não revisa não lê o que os visitantes mandam.
pub fn pode_ler(ator: &Ator) -> bool {
    ator.papel != Papel::Autor
}

struct Linha {
    quando: String,
    caminho: String,
    /// O rótulo e o que foi preenchido em cada campo.
    campos: Vec<(String, String)>,
}

#[derive(Template)]
#[template(path = "mensagens.html")]
struct PaginaDeMensagens {
    site: String,
    slug: String,
    linhas: Vec<Linha>,
}

pub async fn abrir(estado: &Estado, conta: &Conta, slug: &str) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    if !pode_ler(&ator) {
        return Ok(simples(
            StatusCode::FORBIDDEN,
            "Só o dono e os editores do site leem as mensagens.",
        ));
    }
    let linhas = cms_dados::mensagens_do_site(&estado.pool, site.id, MENSAGENS_NA_LISTA)
        .await?
        .into_iter()
        .map(|mensagem| Linha {
            quando: datas::por_extenso(mensagem.criado_em),
            caminho: mensagem.caminho,
            campos: mensagem
                .campos
                .into_iter()
                .map(|campo| (campo.rotulo, campo.valor))
                .collect(),
        })
        .collect();
    let corpo = PaginaDeMensagens {
        site: site.perfil.nome,
        slug: site.slug,
        linhas,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}
