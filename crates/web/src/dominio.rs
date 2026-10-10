//! A tela de endereço de um site: o provisório, o domínio próprio e o que
//! criar no DNS. Só o Dono.

use askama::Template;
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use cms_dados::{DominioDoSite, ErroDeDominio, SiteGravado};
use cms_dominio::Conta;
use cms_dominio::conta::pode_administrar;
use cms_dominio::eventos::origem_do_site;
use serde::Deserialize;

use crate::Estado;
use crate::acesso::{Acesso, ao_site};
use crate::resposta::{ErroWeb, html_privado, redirecionar, simples};

#[derive(Template)]
#[template(path = "dominio.html")]
struct PaginaDeDominio {
    site: String,
    slug: String,
    erro: Option<String>,
    aviso: Option<String>,
    provisorio: String,
    url_provisoria: String,
    definitivo: bool,
    dominios: Vec<DominioDoSite>,
    instrucao: String,
    digitado: String,
}

async fn do_dono(estado: &Estado, conta: &Conta, slug: &str) -> Result<Acesso, ErroWeb> {
    Ok(match ao_site(estado, conta, slug).await? {
        Ok((_, ator)) if !pode_administrar(&ator) => Err(simples(
            StatusCode::FORBIDDEN,
            "Só o dono do site mexe no endereço.",
        )),
        acesso => acesso,
    })
}

fn aviso_da(uri: &Uri) -> Option<String> {
    let consulta = uri.query()?;
    let tem = |par: &str| consulta.split('&').any(|p| p == par);
    if tem("r=pedido") {
        Some(
            "Domínio registrado. Assim que o DNS apontar para cá, ele vira o endereço do site."
                .to_string(),
        )
    } else if tem("r=removido") {
        Some("Domínio removido.".to_string())
    } else if tem("r=provisorio") {
        Some("Endereço provisório atualizado.".to_string())
    } else {
        None
    }
}

async fn pagina(
    estado: &Estado,
    site: &SiteGravado,
    status: StatusCode,
    erro: Option<String>,
    aviso: Option<String>,
    digitado: String,
) -> Result<Response, ErroWeb> {
    let configuracao = &estado.configuracao;
    let instrucao = match configuracao.ips_do_servidor.first() {
        Some(ip) => format!("No painel do seu domínio, crie um registro A apontando para {ip}."),
        None => "Peça à Ávila Ops o endereço para onde apontar o domínio.".to_string(),
    };
    let corpo = PaginaDeDominio {
        site: site.perfil.nome.clone(),
        slug: site.slug.clone(),
        erro,
        aviso,
        provisorio: format!("{}.{}", site.slug, configuracao.dominio_base),
        url_provisoria: origem_do_site(
            &configuracao.esquema,
            &configuracao.dominio_base,
            &site.slug,
            None,
        ),
        definitivo: site.provisorio_definitivo,
        dominios: cms_dados::dominios_do_site(&estado.pool, site.id).await?,
        instrucao,
        digitado,
    }
    .render()?;
    Ok(html_privado(status, corpo))
}

pub async fn abrir(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    match do_dono(estado, conta, slug).await? {
        Ok((site, _)) => {
            pagina(
                estado,
                &site,
                StatusCode::OK,
                None,
                aviso_da(uri),
                String::new(),
            )
            .await
        }
        Err(resposta) => Ok(resposta),
    }
}

/// O que o formulário pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pedido {
    Apontar,
    Remover,
    Provisorio,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Formulario {
    host: String,
    definitivo: String,
}

pub async fn gravar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    pedido: Pedido,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match do_dono(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let formulario = serde_urlencoded::from_bytes::<Formulario>(corpo).unwrap_or_default();
    let pool = &estado.pool;
    let configuracao = &estado.configuracao;
    let feito = match pedido {
        Pedido::Apontar => {
            let mut reservados = vec![configuracao.dominio_base.as_str()];
            reservados.extend(configuracao.host_do_painel.as_deref());
            cms_dados::pedir_dominio(pool, site.id, &ator, &formulario.host, &reservados)
                .await
                .map(|_| "pedido")
        }
        Pedido::Remover => cms_dados::remover_dominio(pool, site.id, &ator, &formulario.host)
            .await
            .map(|_| "removido"),
        Pedido::Provisorio => cms_dados::definir_provisorio_definitivo(
            pool,
            site.id,
            &ator,
            formulario.definitivo == "1",
        )
        .await
        .map(|()| "provisorio"),
    };
    match feito {
        Ok(codigo) => {
            // O endereço e a indexação do site mudaram: o que estava guardado
            // não vale mais.
            estado.cache.invalidar_site(site.id);
            Ok(redirecionar(
                StatusCode::SEE_OTHER,
                &format!("/painel/sites/{}/dominio?r={codigo}", site.slug),
            ))
        }
        Err(ErroDeDominio::Dados(erro)) => Err(erro.into()),
        Err(erro) => {
            let status = match erro {
                ErroDeDominio::SemPermissao => StatusCode::FORBIDDEN,
                ErroDeDominio::EmUso => StatusCode::CONFLICT,
                _ => StatusCode::UNPROCESSABLE_ENTITY,
            };
            pagina(
                estado,
                &site,
                status,
                Some(erro.to_string()),
                None,
                formulario.host,
            )
            .await
        }
    }
}
