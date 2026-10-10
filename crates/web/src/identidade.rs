//! A identidade do site (nome, descrição, logo, organização e orientações
//! para IA) e o histórico do que foi feito nele.

use askama::Template;
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use cms_dominio::conta::pode_administrar;
use cms_dominio::{Conta, PerfilDoSite, datas};
use motor_web::tipos::{Endereco, Organizacao};
use serde::Deserialize;
use uuid::Uuid;

use crate::Estado;
use crate::acesso::{Acesso, ao_site};
use crate::resposta::{ErroWeb, html_privado, redirecionar, simples};

#[derive(Template)]
#[template(path = "identidade.html")]
struct PaginaDeIdentidade {
    site: String,
    slug: String,
    salvo: bool,
    p: PerfilDoSite,
    /// Identificador, descrição e se é a logo atual.
    logos: Vec<(String, String, bool)>,
    razao_social: String,
    telefone: String,
    email: String,
    logradouro: String,
    cidade: String,
    uf: String,
    cep: String,
    perfis: String,
    diretrizes: String,
}

async fn do_dono(estado: &Estado, conta: &Conta, slug: &str) -> Result<Acesso, ErroWeb> {
    Ok(match ao_site(estado, conta, slug).await? {
        Ok((_, ator)) if !pode_administrar(&ator) => Err(simples(
            StatusCode::FORBIDDEN,
            "Só o dono do site mexe na identidade.",
        )),
        acesso => acesso,
    })
}

pub async fn abrir(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let (site, _) = match do_dono(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let perfil = site.perfil;
    let organizacao = perfil.organizacao.clone();
    let endereco = organizacao.endereco.unwrap_or(Endereco {
        logradouro: String::new(),
        cidade: String::new(),
        uf: String::new(),
        cep: String::new(),
    });
    let corpo = PaginaDeIdentidade {
        site: perfil.nome.clone(),
        slug: site.slug,
        salvo: uri
            .query()
            .is_some_and(|consulta| consulta.contains("r=salvo")),
        logos: cms_dados::midias_do_site(&estado.pool, site.id)
            .await?
            .into_iter()
            .filter(|midia| midia.situacao != "falhou")
            .map(|midia| {
                let id = midia.id.to_string();
                let atual = id == perfil.logo.id;
                (id, midia.alt, atual)
            })
            .collect(),
        razao_social: organizacao.razao_social.unwrap_or_default(),
        telefone: organizacao.telefone.unwrap_or_default(),
        email: organizacao.email.unwrap_or_default(),
        logradouro: endereco.logradouro,
        cidade: endereco.cidade,
        uf: endereco.uf,
        cep: endereco.cep,
        perfis: organizacao.perfis.join("\n"),
        diretrizes: perfil.diretrizes_ia.join("\n"),
        p: perfil,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Formulario {
    nome: String,
    descricao: String,
    logo: String,
    razao_social: String,
    telefone: String,
    email: String,
    logradouro: String,
    cidade: String,
    uf: String,
    cep: String,
    perfis: String,
    diretrizes: String,
}

fn opcional(texto: &str) -> Option<String> {
    Some(texto.trim().to_string()).filter(|texto| !texto.is_empty())
}

fn linhas(texto: &str) -> Vec<String> {
    texto
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .map(str::to_string)
        .collect()
}

pub async fn gravar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match do_dono(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let formulario = serde_urlencoded::from_bytes::<Formulario>(corpo).unwrap_or_default();
    let nome = formulario.nome.trim();
    if nome.is_empty() {
        return Ok(simples(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Dê um nome ao site.",
        ));
    }
    // A logo é da biblioteca do site; qualquer outra coisa fica sem logo.
    let logo = match Uuid::parse_str(&formulario.logo) {
        Ok(id) => cms_dados::midia_para_conteudo(&estado.pool, site.id, id).await?,
        Err(_) => None,
    };
    // Endereço só entra inteiro: pela metade, o dado estruturado sairia errado.
    let endereco = match (
        opcional(&formulario.logradouro),
        opcional(&formulario.cidade),
        opcional(&formulario.uf),
        opcional(&formulario.cep),
    ) {
        (Some(logradouro), Some(cidade), Some(uf), Some(cep)) => Some(Endereco {
            logradouro,
            cidade,
            uf: uf.to_uppercase(),
            cep,
        }),
        _ => None,
    };
    let perfil = PerfilDoSite {
        nome: nome.to_string(),
        descricao: formulario.descricao.trim().to_string(),
        logo: logo.unwrap_or_else(cms_dados::midia_vazia),
        organizacao: Organizacao {
            razao_social: opcional(&formulario.razao_social),
            telefone: opcional(&formulario.telefone),
            email: opcional(&formulario.email),
            endereco,
            perfis: linhas(&formulario.perfis)
                .into_iter()
                .filter(|perfil| perfil.starts_with("https://"))
                .collect(),
        },
        diretrizes_ia: linhas(&formulario.diretrizes),
        ..site.perfil
    };
    cms_dados::atualizar_perfil(&estado.pool, site.id, &ator, &perfil).await?;
    // O nome e o rodapé aparecem em todas as páginas do site.
    estado.cache.invalidar_site(site.id);
    Ok(redirecionar(
        StatusCode::SEE_OTHER,
        &format!("/painel/sites/{}/identidade?r=salvo", site.slug),
    ))
}

#[derive(Template)]
#[template(path = "historico.html")]
struct PaginaDoHistorico {
    site: String,
    slug: String,
    /// O que aconteceu, e quem fez e quando.
    linhas: Vec<(String, String)>,
}

fn acao_por_extenso(acao: &str) -> &'static str {
    match acao {
        "rascunho.criado" => "Criou o rascunho de",
        "revisao.pedida" => "Enviou para revisão",
        "revisao.devolvida" => "Devolveu",
        "conteudo.publicado" => "Publicou",
        "conteudo.despublicado" => "Tirou do ar",
        "publicacao.agendada" => "Agendou a publicação de",
        _ => "Mexeu em",
    }
}

/// As últimas cem ações no site. Qualquer participante vê.
pub async fn historico(estado: &Estado, conta: &Conta, slug: &str) -> Result<Response, ErroWeb> {
    let (site, _) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let linhas = cms_dados::historico_do_site(&estado.pool, site.id, 100)
        .await?
        .into_iter()
        .map(|linha| {
            let titulo = linha
                .titulo
                .filter(|titulo| !titulo.trim().is_empty())
                .unwrap_or_else(|| "um documento".to_string());
            let quem = if linha.equipe {
                format!("{} (equipe Ávila Ops)", linha.conta)
            } else {
                linha.conta
            };
            (
                format!("{} “{titulo}”", acao_por_extenso(&linha.acao)),
                format!("{quem} · {}", datas::por_extenso(linha.criado_em)),
            )
        })
        .collect();
    let corpo = PaginaDoHistorico {
        site: site.perfil.nome,
        slug: site.slug,
        linhas,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}
