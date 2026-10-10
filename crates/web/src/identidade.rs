//! A identidade do site (nome, descrição, logo, organização e orientações
//! para IA) e o histórico do que foi feito nele.

use askama::Template;
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use cms_dados::SiteGravado;
use cms_dominio::conta::pode_administrar;
use cms_dominio::site::conferir_cor;
use cms_dominio::{Ator, Conta, PerfilDoSite, datas};
use motor_web::tipos::{Endereco, Organizacao};
use serde::Deserialize;
use uuid::Uuid;

use crate::Estado;
use crate::acesso::{Acesso, ao_site};
use crate::resposta::{ErroWeb, html_privado, redirecionar, simples, texto};

/// A identidade como é digitada, no formulário do painel e nos argumentos do
/// conector. Listas vêm com um item por linha.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub(crate) struct DadosDaIdentidade {
    nome: String,
    descricao: String,
    /// O identificador da imagem na biblioteca; vazio é sem logo.
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
    /// A cor da marca, como `#225cf2`; vazio usa a cor do tema.
    cor: String,
}

impl DadosDaIdentidade {
    /// O que está gravado: para mostrar na tela e para completar o que o
    /// conector não mandou.
    pub(crate) fn do_perfil(perfil: &PerfilDoSite) -> Self {
        let organizacao = perfil.organizacao.clone();
        let endereco = organizacao.endereco.unwrap_or(Endereco {
            logradouro: String::new(),
            cidade: String::new(),
            uf: String::new(),
            cep: String::new(),
        });
        Self {
            nome: perfil.nome.clone(),
            descricao: perfil.descricao.clone(),
            logo: perfil.logo.id.clone(),
            razao_social: organizacao.razao_social.unwrap_or_default(),
            telefone: organizacao.telefone.unwrap_or_default(),
            email: organizacao.email.unwrap_or_default(),
            logradouro: endereco.logradouro,
            cidade: endereco.cidade,
            uf: endereco.uf,
            cep: endereco.cep,
            perfis: organizacao.perfis.join("\n"),
            diretrizes: perfil.diretrizes_ia.join("\n"),
            cor: perfil.cor_de_destaque.clone().unwrap_or_default(),
        }
    }

    /// Um campo pelo nome que o conector usa, para ler ou trocar só ele.
    pub(crate) fn campo(&mut self, nome: &str) -> Option<&mut String> {
        Some(match nome {
            "nome" => &mut self.nome,
            "descricao" => &mut self.descricao,
            "logo" => &mut self.logo,
            "razaoSocial" => &mut self.razao_social,
            "telefone" => &mut self.telefone,
            "email" => &mut self.email,
            "logradouro" => &mut self.logradouro,
            "cidade" => &mut self.cidade,
            "uf" => &mut self.uf,
            "cep" => &mut self.cep,
            "perfis" => &mut self.perfis,
            "diretrizes" => &mut self.diretrizes,
            "corDeDestaque" => &mut self.cor,
            _ => return None,
        })
    }
}

#[derive(Template)]
#[template(path = "identidade.html")]
struct PaginaDeIdentidade {
    site: String,
    slug: String,
    salvo: bool,
    d: DadosDaIdentidade,
    /// Identificador, descrição e se é a logo atual.
    logos: Vec<(String, String, bool)>,
}

async fn do_dono(estado: &Estado, conta: &Conta, slug: &str) -> Result<Acesso, ErroWeb> {
    Ok(match ao_site(estado, conta, slug).await? {
        Ok((_, ator)) if !pode_administrar(&ator) => Err(simples(StatusCode::FORBIDDEN, SO_O_DONO)),
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
    let dados = DadosDaIdentidade::do_perfil(&site.perfil);
    let corpo = PaginaDeIdentidade {
        site: site.perfil.nome,
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
                let atual = id == dados.logo;
                (id, midia.alt, atual)
            })
            .collect(),
        d: dados,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}

const SO_O_DONO: &str = "Só o dono do site mexe na identidade.";

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

/// Grava a identidade digitada. A recusa é uma frase para quem pediu.
pub(crate) async fn aplicar(
    estado: &Estado,
    site: &SiteGravado,
    ator: &Ator,
    dados: &mut DadosDaIdentidade,
) -> Result<Result<(), String>, ErroWeb> {
    let nome = dados.nome.trim().to_string();
    if nome.is_empty() {
        return Ok(Err("Dê um nome ao site.".to_string()));
    }
    let cor_de_destaque = match opcional(&dados.cor) {
        Some(cor) => match conferir_cor(&cor) {
            Ok(cor) => Some(cor),
            Err(recusa) => return Ok(Err(recusa.to_string())),
        },
        None => None,
    };
    // Quem chamou recebe de volta a cor como ficou gravada.
    dados.cor = cor_de_destaque.clone().unwrap_or_default();
    // A logo é da biblioteca do site; qualquer outra coisa fica sem logo.
    let logo = match Uuid::parse_str(dados.logo.trim()) {
        Ok(id) => cms_dados::midia_para_conteudo(&estado.pool, site.id, id).await?,
        Err(_) => None,
    };
    // Endereço só entra inteiro: pela metade, o dado estruturado sairia errado.
    let endereco = match (
        opcional(&dados.logradouro),
        opcional(&dados.cidade),
        opcional(&dados.uf),
        opcional(&dados.cep),
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
        descricao: dados.descricao.trim().to_string(),
        logo: logo.unwrap_or_else(cms_dados::midia_vazia),
        organizacao: Organizacao {
            razao_social: opcional(&dados.razao_social),
            telefone: opcional(&dados.telefone),
            email: opcional(&dados.email),
            endereco,
            perfis: linhas(&dados.perfis)
                .into_iter()
                .filter(|perfil| perfil.starts_with("https://"))
                .collect(),
        },
        diretrizes_ia: linhas(&dados.diretrizes),
        cor_de_destaque,
        ..site.perfil.clone()
    };
    if !cms_dados::atualizar_perfil(&estado.pool, site.id, ator, &perfil).await? {
        return Ok(Err(SO_O_DONO.to_string()));
    }
    // O nome e o rodapé aparecem em todas as páginas do site.
    estado.cache.invalidar_site(site.id);
    Ok(Ok(()))
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
    let mut dados = serde_urlencoded::from_bytes::<DadosDaIdentidade>(corpo).unwrap_or_default();
    Ok(match aplicar(estado, &site, &ator, &mut dados).await? {
        Ok(()) => redirecionar(
            StatusCode::SEE_OTHER,
            &format!("/painel/sites/{}/identidade?r=salvo", site.slug),
        ),
        Err(motivo) => texto(
            StatusCode::UNPROCESSABLE_ENTITY,
            "text/plain; charset=utf-8",
            motivo,
        ),
    })
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
