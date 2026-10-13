//! A identidade do site (nome, descrição, logo, organização e orientações
//! para IA) e o histórico do que foi feito nele.

use askama::Template;
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use cms_dados::SiteGravado;
use cms_dominio::conta::pode_administrar;
use cms_dominio::eventos::origem_do_site;
use cms_dominio::site::{
    CAMINHO_DA_LICENCA_DE_IMAGENS, Tema, conferir_cor, conferir_tag_do_google,
};
use cms_dominio::{Ator, Conta, EntradaDoMenu, PerfilDoSite, datas, ler_entrada_do_menu};
use motor_web::tipos::{Endereco, Organizacao};
use serde::Deserialize;
use uuid::Uuid;

use crate::Estado;
use crate::acesso::{Acesso, ao_site};
use crate::painel::miniatura;
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
    /// O tema do site, pelo nome gravado (`classico`, `industrial`...).
    tema: String,
    /// A ordem das páginas no menu: um caminho por linha, como `/servicos`.
    menu: String,
    /// A tag do Google, como `GTM-ABC1234`; vazio deixa o site sem script.
    tag: String,
    /// Os termos de uso das imagens do site, um parágrafo por linha; vazio
    /// usa o texto padrão da página de licença.
    licenca: String,
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
            tema: perfil.tema.slug().to_string(),
            menu: perfil.menu.join("\n"),
            tag: perfil.tag_do_google.clone().unwrap_or_default(),
            licenca: perfil.licenca_de_imagens.clone().unwrap_or_default(),
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
            "tema" => &mut self.tema,
            "menu" => &mut self.menu,
            "tagDoGoogle" => &mut self.tag,
            "licencaDeImagens" => &mut self.licenca,
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
    /// Os temas: nome gravado, como aparece para escolher e se é o atual.
    temas: Vec<(&'static str, String, bool)>,
    /// A logo de agora, para a pessoa ver qual é antes de trocar.
    logo_atual: Option<String>,
    /// As páginas que podem ir para o menu do topo, com a posição de cada uma.
    paginas: Vec<PaginaDoMenu>,
    /// O endereço da página de licença de imagens do site.
    pagina_da_licenca: String,
}

/// Uma página na lista do menu do topo.
struct PaginaDoMenu {
    caminho: String,
    nome: String,
    /// A posição no menu, a partir de 1; vazia quando a página está no rodapé.
    posicao: String,
}

/// As páginas publicadas que podem aparecer no menu, as do menu primeiro e na
/// ordem dele. Os posts entram juntos, como "Blog".
async fn paginas_do_menu(
    estado: &Estado,
    site: &SiteGravado,
) -> Result<Vec<PaginaDoMenu>, ErroWeb> {
    let itens = cms_dados::navegacao(&estado.pool, site.id).await?;
    // Os grupos (submenus) aparecem como uma linha só, com as páginas deles.
    let grupos: Vec<(String, String, Vec<&str>)> = site
        .perfil
        .menu
        .iter()
        .filter_map(|linha| match ler_entrada_do_menu(linha)? {
            EntradaDoMenu::Grupo { rotulo, caminhos } => {
                Some((linha.clone(), format!("Grupo {rotulo}"), caminhos))
            }
            EntradaDoMenu::Pagina(_) => None,
        })
        .collect();
    let no_grupo = |caminho: &str| {
        grupos
            .iter()
            .any(|(_, _, caminhos)| caminhos.iter().any(|c| c.trim_end_matches('/') == caminho))
    };
    let mut paginas: Vec<(String, String)> = itens
        .iter()
        .filter(|item| !item.eh_post && item.caminho != "/" && !no_grupo(&item.caminho))
        .map(|item| (item.caminho.clone(), item.titulo.clone()))
        .collect();
    if itens.iter().any(|item| item.eh_post) && !no_grupo("/blog") {
        paginas.push(("/blog".to_string(), "Blog".to_string()));
    }
    paginas.extend(
        grupos
            .iter()
            .map(|(linha, nome, _)| (linha.clone(), nome.clone())),
    );
    let posicao = |caminho: &str| {
        site.perfil
            .menu
            .iter()
            .position(|no_menu| no_menu.trim_end_matches('/') == caminho)
    };
    paginas.sort_by(|a, b| {
        let chave = |pagina: &(String, String)| {
            (
                posicao(&pagina.0).unwrap_or(usize::MAX),
                pagina.1.to_lowercase(),
            )
        };
        chave(a).cmp(&chave(b))
    });
    Ok(paginas
        .into_iter()
        .map(|(caminho, nome)| PaginaDoMenu {
            posicao: posicao(&caminho).map_or(String::new(), |i| (i + 1).to_string()),
            caminho,
            nome,
        })
        .collect())
}

/// O menu como a tela o manda: cada página com a posição digitada. Devolve os
/// caminhos numerados, na ordem dos números; `None` se a tela não mandou
/// página nenhuma (site sem página no ar), para o menu gravado não se perder.
fn menu_da_tela(corpo: &[u8]) -> Option<String> {
    let pares = serde_urlencoded::from_bytes::<Vec<(String, String)>>(corpo).unwrap_or_default();
    // A tela manda uma `posicao` e uma `pagina` por linha, na mesma ordem.
    let mut paginas: Vec<&str> = Vec::new();
    let mut posicoes: Vec<&str> = Vec::new();
    for (chave, valor) in &pares {
        match chave.as_str() {
            "pagina" => paginas.push(valor.trim()),
            "posicao" => posicoes.push(valor.trim()),
            _ => {}
        }
    }
    if paginas.is_empty() {
        return None;
    }
    let mut numeradas: Vec<(u32, usize, &str)> = paginas
        .iter()
        .zip(&posicoes)
        .enumerate()
        .filter(|(_, (linha, _))| ler_entrada_do_menu(linha).is_some())
        .filter_map(|(ordem, (caminho, posicao))| {
            let posicao = posicao.parse::<u32>().ok().filter(|posicao| *posicao > 0)?;
            Some((posicao, ordem, *caminho))
        })
        .collect();
    // Dois números iguais: vale a ordem em que estavam na tela.
    numeradas.sort_unstable();
    let caminhos: Vec<&str> = numeradas
        .into_iter()
        .map(|(_, _, caminho)| caminho)
        .collect();
    Some(caminhos.join("\n"))
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
    let paginas = paginas_do_menu(estado, &site).await?;
    let configuracao = &estado.configuracao;
    let origem = origem_do_site(
        &configuracao.esquema,
        &configuracao.dominio_base,
        &site.slug,
        site.dominio_ativo.as_deref(),
    );
    let corpo = PaginaDeIdentidade {
        pagina_da_licenca: format!("{origem}{CAMINHO_DA_LICENCA_DE_IMAGENS}"),
        logo_atual: miniatura(&site.perfil.logo, &site.slug),
        temas: Tema::TODOS
            .into_iter()
            .map(|tema| {
                let rotulo = format!("{}: {}", tema.nome(), tema.descricao());
                (tema.slug(), rotulo, dados.tema.trim() == tema.slug())
            })
            .collect(),
        paginas,
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
    let Some(tema) = Tema::do_slug(&dados.tema) else {
        let nomes: Vec<&str> = Tema::TODOS.into_iter().map(Tema::slug).collect();
        return Ok(Err(format!(
            "Este tema não existe. Os temas são: {}.",
            nomes.join(", ")
        )));
    };
    dados.tema = tema.slug().to_string();
    // Quem chamou recebe de volta a cor como ficou gravada.
    dados.cor = cor_de_destaque.clone().unwrap_or_default();
    let tag_do_google = match opcional(&dados.tag) {
        Some(tag) => match conferir_tag_do_google(&tag) {
            Ok(tag) => Some(tag),
            Err(recusa) => return Ok(Err(recusa.to_string())),
        },
        None => None,
    };
    dados.tag = tag_do_google.clone().unwrap_or_default();
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
        tema,
        tag_do_google,
        licenca_de_imagens: opcional(&dados.licenca),
        // Só caminho do site ou grupo de caminhos entra na ordem do menu.
        menu: linhas(&dados.menu)
            .iter()
            .filter_map(|linha| ler_entrada_do_menu(linha).map(|entrada| entrada.como_linha()))
            .collect(),
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
    // Na tela o menu vem como posições; no conector, como lista de caminhos.
    dados.menu = menu_da_tela(corpo).unwrap_or_else(|| site.perfil.menu.join("\n"));
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn menu_da_tela_segue_os_numeros_e_ignora_pagina_sem_numero() {
        let corpo = b"nome=Site&posicao=2&pagina=%2Fservicos&posicao=&pagina=%2Ffaq&posicao=1&pagina=%2Fblog&posicao=2&pagina=%2Fcontato";
        assert_eq!(
            menu_da_tela(corpo).as_deref(),
            Some("/blog\n/servicos\n/contato")
        );
        // Sem número em nenhuma: menu vazio, e todas as páginas ficam no topo.
        assert_eq!(
            menu_da_tela(b"posicao=&pagina=%2Fservicos").as_deref(),
            Some("")
        );
        // Site sem página no ar não manda linha: o menu gravado é mantido.
        assert_eq!(menu_da_tela(b"nome=Site"), None);
    }
}
