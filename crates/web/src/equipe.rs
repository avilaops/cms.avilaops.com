//! A tela de equipe de um site: quem participa, os convites em aberto e o
//! formulário de convite. Só o Dono chega aqui.

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;
use cms_dados::{ErroDeConta, SiteGravado};
use cms_dominio::conta::pode_administrar;
use cms_dominio::{Conta, Papel, datas};
use serde::Deserialize;

use crate::Estado;
use crate::acesso::{Acesso, ao_site};
use crate::resposta::{ErroWeb, html_privado, simples};

struct Linha {
    email: String,
    papel: &'static str,
    validade: String,
}

struct ConviteNovo {
    email: String,
    link: String,
    validade: String,
}

#[derive(Template)]
#[template(path = "equipe.html")]
struct PaginaDaEquipe {
    site: String,
    slug: String,
    erro: Option<String>,
    criado: Option<ConviteNovo>,
    membros: Vec<Linha>,
    pendentes: Vec<Linha>,
    /// O e-mail digitado, de volta no formulário quando o convite é recusado.
    email: String,
}

#[derive(Default)]
struct Recado {
    erro: Option<String>,
    criado: Option<ConviteNovo>,
    email: String,
}

#[derive(Deserialize)]
struct NovoConvite {
    #[serde(default)]
    email: String,
    #[serde(default)]
    papel: String,
}

/// Só o Dono chega à equipe.
async fn do_dono(estado: &Estado, conta: &Conta, slug: &str) -> Result<Acesso, ErroWeb> {
    Ok(match ao_site(estado, conta, slug).await? {
        Ok((_, ator)) if !pode_administrar(&ator) => Err(simples(
            StatusCode::FORBIDDEN,
            "Só o dono do site vê a equipe.",
        )),
        acesso => acesso,
    })
}

async fn pagina(
    estado: &Estado,
    site: &SiteGravado,
    status: StatusCode,
    recado: Recado,
) -> Result<Response, ErroWeb> {
    let membros = cms_dados::equipe_do_site(&estado.pool, site.id)
        .await?
        .into_iter()
        .map(|membro| Linha {
            email: membro.email,
            papel: membro.papel.rotulo(),
            validade: String::new(),
        })
        .collect();
    let pendentes = cms_dados::convites_pendentes(&estado.pool, site.id)
        .await?
        .into_iter()
        .map(|convite| Linha {
            email: convite.email,
            papel: convite.papel.rotulo(),
            validade: datas::por_extenso(convite.expira_em),
        })
        .collect();
    let corpo = PaginaDaEquipe {
        site: site.perfil.nome.clone(),
        slug: site.slug.clone(),
        erro: recado.erro,
        criado: recado.criado,
        membros,
        pendentes,
        email: recado.email,
    }
    .render()?;
    Ok(html_privado(status, corpo))
}

pub async fn abrir(estado: &Estado, conta: &Conta, slug: &str) -> Result<Response, ErroWeb> {
    match do_dono(estado, conta, slug).await? {
        Ok((site, _)) => pagina(estado, &site, StatusCode::OK, Recado::default()).await,
        Err(resposta) => Ok(resposta),
    }
}

/// Cria o convite e mostra o link uma vez. O e-mail sai pelo n8n; o link na
/// tela é o que garante que o convite funciona sem ele.
pub async fn convidar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    origem_do_painel: &str,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match do_dono(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let formulario = serde_urlencoded::from_bytes::<NovoConvite>(corpo).ok();
    let Some((formulario, papel)) = formulario.and_then(|formulario| {
        let papel = Papel::try_from(formulario.papel.as_str()).ok()?;
        Some((formulario, papel))
    }) else {
        return Ok(simples(StatusCode::BAD_REQUEST, "Formulário inválido."));
    };

    let criado = cms_dados::criar_convite(
        &estado.pool,
        site.id,
        &ator,
        &formulario.email,
        papel,
        origem_do_painel,
    )
    .await;
    match criado {
        Ok(convite) => {
            let recado = Recado {
                criado: Some(ConviteNovo {
                    email: formulario.email.trim().to_lowercase(),
                    link: convite.link,
                    validade: datas::por_extenso(convite.expira_em),
                }),
                ..Recado::default()
            };
            pagina(estado, &site, StatusCode::OK, recado).await
        }
        Err(ErroDeConta::Dados(erro)) => Err(erro.into()),
        Err(erro) => {
            let status = match erro {
                ErroDeConta::SemPermissao => StatusCode::FORBIDDEN,
                _ => StatusCode::UNPROCESSABLE_ENTITY,
            };
            let recado = Recado {
                erro: Some(erro.to_string()),
                email: formulario.email,
                ..Recado::default()
            };
            pagina(estado, &site, status, recado).await
        }
    }
}
