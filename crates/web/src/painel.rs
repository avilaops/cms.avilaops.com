//! O painel: o que responde no host do CMS para quem entrou pelo Auth.
//!
//! Aqui só há leitura de formulário e montagem de tela. Permissão, limite e
//! validação são de `cms_dados` e `cms_dominio`.

use askama::Template;
use axum::body::Bytes;
use axum::http::header::{COOKIE, ORIGIN};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use cms_dados::{ErroDeConta, SiteDaConta};
use cms_dominio::eventos::origem_do_site;
use cms_dominio::{Conta, Papel, Situacao};
use cms_integracoes::auth::{COOKIE_DE_SESSAO, RespostaDoAuth};
use serde::Deserialize;

use crate::resposta::{ErroWeb, html_privado, redirecionar, simples};
use crate::{Configuracao, Estado};
use crate::{biblioteca, conector, conteudo, equipe};

const CAMINHO: &str = "/painel";

struct LinhaDeSite {
    nome: String,
    url: String,
    endereco: String,
    situacao: &'static str,
    papel: &'static str,
    /// A tela de equipe, para quem é Dono.
    equipe: Option<String>,
    midia: String,
    /// A página do site no painel.
    abrir: String,
}

impl LinhaDeSite {
    fn nova(site: SiteDaConta, configuracao: &Configuracao) -> Self {
        let url = origem_do_site(
            &configuracao.esquema,
            &configuracao.dominio_base,
            &site.slug,
            None,
        );
        Self {
            nome: site.nome,
            endereco: format!("{}.{}", site.slug, configuracao.dominio_base),
            url,
            situacao: match site.situacao {
                Situacao::EmMontagem => "Em montagem",
                Situacao::Ativo => "No ar",
                Situacao::Suspenso => "Suspenso",
            },
            papel: site.papel.rotulo(),
            midia: format!("/painel/sites/{}/midia", site.slug),
            abrir: format!("/painel/sites/{}", site.slug),
            equipe: (site.papel == Papel::Dono)
                .then(|| format!("/painel/sites/{}/equipe", site.slug)),
        }
    }
}

#[derive(Template)]
#[template(path = "painel.html")]
struct PaginaDoPainel {
    quem: String,
    erro: Option<String>,
    aviso: Option<String>,
    sites: Vec<LinhaDeSite>,
    dominio_base: String,
    /// O que a pessoa digitou, de volta no formulário quando algo é recusado.
    nome: String,
    slug: String,
}

#[derive(Default)]
struct Recado {
    erro: Option<String>,
    aviso: Option<String>,
    nome: String,
    slug: String,
}

#[derive(Deserialize)]
struct NovoSite {
    #[serde(default)]
    nome: String,
    #[serde(default)]
    slug: String,
}

/// O valor do cookie de sessão do Auth, se veio.
fn token_de_sessao(cabecalhos: &HeaderMap) -> Option<&str> {
    cabecalhos
        .get_all(COOKIE)
        .iter()
        .filter_map(|valor| valor.to_str().ok())
        .flat_map(|valor| valor.split(';'))
        .filter_map(|par| par.trim().split_once('='))
        .find(|(nome, _)| *nome == COOKIE_DE_SESSAO)
        .map(|(_, valor)| valor)
}

fn origem_do_painel(configuracao: &Configuracao, host: &str) -> String {
    format!("{}://{host}", configuracao.esquema)
}

/// Quem está pedindo, ou a resposta que o manda entrar.
///
/// `volta=1` marca que a pessoa já foi ao Auth uma vez. Se voltar e a sessão
/// continuar não chegando, a rota para: o Auth devolve na hora quem já está
/// logado, e sem a marca os dois lados ficariam se mandando um ao outro.
/// A resposta que barra a entrada vai em caixa: é bem maior que a conta.
fn recusa(resposta: Response) -> Box<Response> {
    Box::new(resposta)
}

async fn entrar(
    estado: &Estado,
    host: &str,
    cabecalhos: &HeaderMap,
    uri: &Uri,
) -> Result<Conta, Box<Response>> {
    let Some(auth) = &estado.auth else {
        return Err(recusa(simples(
            StatusCode::SERVICE_UNAVAILABLE,
            "O login do painel não está configurado.",
        )));
    };
    match auth.consultar(token_de_sessao(cabecalhos)).await {
        RespostaDoAuth::Entrou(conta) => Ok(conta),
        RespostaDoAuth::SemSessao => {
            let ja_voltou = uri
                .query()
                .is_some_and(|consulta| consulta.split('&').any(|par| par == "volta=1"));
            if ja_voltou {
                return Err(recusa(simples(
                    StatusCode::UNAUTHORIZED,
                    "Não foi possível confirmar o seu login. Feche esta aba e entre de novo.",
                )));
            }
            // O pedido volta inteiro: a tela de autorização do conector vive
            // nos parâmetros da consulta.
            let consulta = uri
                .query()
                .map(|consulta| format!("{consulta}&"))
                .unwrap_or_default();
            let retorno = format!(
                "{}{}?{consulta}volta=1",
                origem_do_painel(&estado.configuracao, host),
                uri.path()
            );
            Err(recusa(redirecionar(
                StatusCode::FOUND,
                &auth.url_de_login(&retorno),
            )))
        }
        RespostaDoAuth::SemAcesso => Err(recusa(simples(
            StatusCode::FORBIDDEN,
            "Sua conta Ávila Ops ainda não foi liberada para o CMS.",
        ))),
        RespostaDoAuth::Indisponivel => Err(recusa(simples(
            StatusCode::SERVICE_UNAVAILABLE,
            "Não foi possível falar com o login da Ávila Ops agora. Tente de novo em instantes.",
        ))),
    }
}

/// Formulário só vale vindo do próprio painel. O cookie do Auth chega de
/// qualquer `*.avilaops.com`, inclusive de sites servidos por esta aplicação.
fn veio_do_painel(configuracao: &Configuracao, host: &str, cabecalhos: &HeaderMap) -> bool {
    cabecalhos
        .get(ORIGIN)
        .and_then(|valor| valor.to_str().ok())
        .is_some_and(|origem| origem == origem_do_painel(configuracao, host))
}

async fn pagina(
    estado: &Estado,
    conta: &Conta,
    status: StatusCode,
    recado: Recado,
) -> Result<Response, ErroWeb> {
    let sites = cms_dados::sites_da_conta(&estado.pool, &conta.sub)
        .await?
        .into_iter()
        .map(|site| LinhaDeSite::nova(site, &estado.configuracao))
        .collect();
    let quem = if conta.nome.is_empty() {
        conta.email.clone()
    } else {
        format!("{} · {}", conta.nome, conta.email)
    };
    let corpo = PaginaDoPainel {
        quem,
        erro: recado.erro,
        aviso: recado.aviso,
        sites,
        dominio_base: estado.configuracao.dominio_base.clone(),
        nome: recado.nome,
        slug: recado.slug,
    }
    .render()?;
    Ok(html_privado(status, corpo))
}

fn status_do_erro(erro: &ErroDeConta) -> StatusCode {
    match erro {
        ErroDeConta::Dados(_) => StatusCode::INTERNAL_SERVER_ERROR,
        ErroDeConta::SemPermissao | ErroDeConta::ConviteDeOutroEmail => StatusCode::FORBIDDEN,
        ErroDeConta::ConviteInexistente => StatusCode::NOT_FOUND,
        ErroDeConta::ConviteExpirado | ErroDeConta::ConviteUsado => StatusCode::GONE,
        ErroDeConta::LimiteDeSites | ErroDeConta::LimiteDiario => StatusCode::TOO_MANY_REQUESTS,
        ErroDeConta::SlugEmUso => StatusCode::CONFLICT,
        ErroDeConta::Slug(_) | ErroDeConta::NomeInvalido | ErroDeConta::EmailInvalido => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
    }
}

/// O que o banco recusou vira recado na tela; falha de banco segue como erro.
async fn recusar(
    estado: &Estado,
    conta: &Conta,
    erro: ErroDeConta,
    mut recado: Recado,
) -> Result<Response, ErroWeb> {
    if let ErroDeConta::Dados(erro) = erro {
        return Err(erro.into());
    }
    let status = status_do_erro(&erro);
    recado.erro = Some(erro.to_string());
    pagina(estado, conta, status, recado).await
}

async fn criar_site(estado: &Estado, conta: &Conta, corpo: &[u8]) -> Result<Response, ErroWeb> {
    let Ok(formulario) = serde_urlencoded::from_bytes::<NovoSite>(corpo) else {
        return Ok(simples(StatusCode::BAD_REQUEST, "Formulário inválido."));
    };
    let slug = formulario.slug.trim().to_lowercase();
    let criado = cms_dados::criar_site_para(
        &estado.pool,
        conta,
        &slug,
        &formulario.nome,
        estado.configuracao.limites_de_criacao,
    )
    .await;
    match criado {
        Ok(_) => Ok(redirecionar(
            StatusCode::SEE_OTHER,
            &format!("{CAMINHO}?criado=1"),
        )),
        Err(erro) => {
            let recado = Recado {
                nome: formulario.nome,
                slug,
                ..Recado::default()
            };
            recusar(estado, conta, erro, recado).await
        }
    }
}

async fn aceitar_convite(estado: &Estado, conta: &Conta, token: &str) -> Result<Response, ErroWeb> {
    match cms_dados::aceitar_convite(&estado.pool, token, conta).await {
        Ok(_) => Ok(redirecionar(
            StatusCode::SEE_OTHER,
            &format!("{CAMINHO}?convite=1"),
        )),
        Err(erro) => recusar(estado, conta, erro, Recado::default()).await,
    }
}

fn aviso_da_consulta(uri: &Uri) -> Option<String> {
    let consulta = uri.query()?;
    let tem = |par: &str| consulta.split('&').any(|p| p == par);
    if tem("criado=1") {
        Some("Site criado. Ele já responde no endereço abaixo, ainda fora da busca.".to_string())
    } else if tem("convite=1") {
        Some("Convite aceito. O site já aparece na sua lista.".to_string())
    } else {
        None
    }
}

/// O que o painel atende. Qualquer outra coisa não existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rota<'a> {
    Raiz,
    Sites,
    CriarSite,
    Convite(&'a str),
    Equipe(&'a str),
    Convidar(&'a str),
    Midia(&'a str),
    EnviarMidia(&'a str),
    ArquivoDeMidia(&'a str, &'a str),
    ApagarMidia(&'a str, &'a str),
    Site(&'a str),
    NovoDocumento(&'a str, &'a str),
    GravarNovo(&'a str),
    Documento(&'a str, &'a str),
    Gravar(&'a str, &'a str),
    Previa(&'a str, &'a str),
    Catalogo(&'a str),
    SalvarAutor(&'a str),
    SalvarCategoria(&'a str),
    Autorizar,
    Decidir,
    Conector,
    RevogarConexao(&'a str),
}

impl<'a> Rota<'a> {
    fn ler(metodo: &Method, caminho: &'a str) -> Option<Self> {
        let leitura = metodo == Method::GET;
        if !leitura && metodo != Method::POST {
            return None;
        }
        let partes: Vec<&'a str> = caminho.split('/').skip(1).collect();
        match (leitura, partes.as_slice()) {
            (true, [""]) => Some(Rota::Raiz),
            (true, ["painel"]) => Some(Rota::Sites),
            (true, ["convite", token]) => Some(Rota::Convite(token)),
            (false, ["painel", "sites"]) => Some(Rota::CriarSite),
            (true, ["painel", "sites", slug, "equipe"]) => Some(Rota::Equipe(slug)),
            (false, ["painel", "sites", slug, "convites"]) => Some(Rota::Convidar(slug)),
            (true, ["painel", "sites", slug, "midia"]) => Some(Rota::Midia(slug)),
            (false, ["painel", "sites", slug, "midia"]) => Some(Rota::EnviarMidia(slug)),
            (true, ["painel", "sites", slug, "midia", "arquivo", arquivo]) => {
                Some(Rota::ArquivoDeMidia(slug, arquivo))
            }
            (false, ["painel", "sites", slug, "midia", id, "apagar"]) => {
                Some(Rota::ApagarMidia(slug, id))
            }
            (true, ["oauth", "authorize"]) => Some(Rota::Autorizar),
            (false, ["oauth", "authorize"]) => Some(Rota::Decidir),
            (true, ["painel", "conector"]) => Some(Rota::Conector),
            (false, ["painel", "conector", id, "revogar"]) => Some(Rota::RevogarConexao(id)),
            (true, ["painel", "sites", slug]) => Some(Rota::Site(slug)),
            (true, ["painel", "sites", slug, "novo", especie]) => {
                Some(Rota::NovoDocumento(slug, especie))
            }
            (false, ["painel", "sites", slug, "doc"]) => Some(Rota::GravarNovo(slug)),
            (true, ["painel", "sites", slug, "doc", id]) => Some(Rota::Documento(slug, id)),
            (false, ["painel", "sites", slug, "doc", id]) => Some(Rota::Gravar(slug, id)),
            (true, ["painel", "sites", slug, "doc", id, "previa"]) => Some(Rota::Previa(slug, id)),
            (true, ["painel", "sites", slug, "catalogo"]) => Some(Rota::Catalogo(slug)),
            (false, ["painel", "sites", slug, "autores"]) => Some(Rota::SalvarAutor(slug)),
            (false, ["painel", "sites", slug, "categorias"]) => Some(Rota::SalvarCategoria(slug)),
            _ => None,
        }
    }

    /// Rota que muda alguma coisa: só vale vinda do próprio painel.
    fn escreve(self) -> bool {
        matches!(
            self,
            Rota::CriarSite
                | Rota::Convidar(_)
                | Rota::EnviarMidia(_)
                | Rota::ApagarMidia(..)
                | Rota::GravarNovo(_)
                | Rota::Gravar(..)
                | Rota::SalvarAutor(_)
                | Rota::SalvarCategoria(_)
                | Rota::Decidir
                | Rota::RevogarConexao(_)
        )
    }
}

async fn responder(
    estado: &Estado,
    host: &str,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    uri: &Uri,
    corpo: &Bytes,
) -> Result<Response, ErroWeb> {
    let caminho = uri.path();
    // O que o assistente chama sem sessão de navegador: quem se identifica
    // ali é o cliente registrado e o token, não o cookie do Auth.
    let origem = origem_do_painel(&estado.configuracao, host);
    let leitura = metodo == Method::GET;
    match caminho {
        "/.well-known/oauth-authorization-server" if leitura => {
            return Ok(conector::metadados(&origem));
        }
        "/.well-known/oauth-protected-resource" if leitura => {
            return Ok(conector::recurso_protegido(&origem));
        }
        "/oauth/register" if metodo == Method::POST => {
            return conector::registrar(estado, corpo).await;
        }
        "/oauth/token" if metodo == Method::POST => return conector::token(estado, corpo).await,
        "/mcp" => return conector::mcp(estado, &origem, metodo, cabecalhos, corpo).await,
        _ => {}
    }
    let Some(rota) = Rota::ler(metodo, caminho) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    if rota == Rota::Raiz {
        return Ok(redirecionar(StatusCode::FOUND, CAMINHO));
    }
    if rota.escreve() && !veio_do_painel(&estado.configuracao, host, cabecalhos) {
        return Ok(simples(
            StatusCode::FORBIDDEN,
            "Pedido recusado: não veio do painel.",
        ));
    }
    let conta = match entrar(estado, host, cabecalhos, uri).await {
        Ok(conta) => conta,
        Err(resposta) => return Ok(*resposta),
    };

    match rota {
        Rota::Raiz | Rota::Sites => {
            let recado = Recado {
                aviso: aviso_da_consulta(uri),
                ..Recado::default()
            };
            pagina(estado, &conta, StatusCode::OK, recado).await
        }
        Rota::CriarSite => criar_site(estado, &conta, corpo).await,
        Rota::Convite(token) => aceitar_convite(estado, &conta, token).await,
        Rota::Equipe(slug) => equipe::abrir(estado, &conta, slug).await,
        Rota::Convidar(slug) => {
            let origem = origem_do_painel(&estado.configuracao, host);
            equipe::convidar(estado, &conta, slug, &origem, corpo).await
        }
        Rota::Midia(slug) => biblioteca::abrir(estado, &conta, slug).await,
        Rota::EnviarMidia(slug) => {
            biblioteca::enviar(estado, &conta, slug, cabecalhos, corpo).await
        }
        Rota::ArquivoDeMidia(slug, arquivo) => {
            biblioteca::arquivo(estado, &conta, slug, arquivo).await
        }
        Rota::ApagarMidia(slug, id) => biblioteca::apagar(estado, &conta, slug, id).await,
        Rota::Site(slug) => conteudo::inicio(estado, &conta, slug).await,
        Rota::NovoDocumento(slug, especie) => {
            conteudo::novo(estado, &conta, slug, especie, uri).await
        }
        Rota::GravarNovo(slug) => conteudo::gravar(estado, &conta, slug, None, corpo).await,
        Rota::Documento(slug, id) => conteudo::abrir(estado, &conta, slug, id, uri).await,
        Rota::Gravar(slug, id) => conteudo::gravar(estado, &conta, slug, Some(id), corpo).await,
        Rota::Previa(slug, id) => conteudo::previa(estado, &conta, slug, id).await,
        Rota::Catalogo(slug) => conteudo::catalogo(estado, &conta, slug, uri).await,
        Rota::SalvarAutor(slug) => {
            conteudo::salvar_no_catalogo(estado, &conta, slug, true, corpo).await
        }
        Rota::SalvarCategoria(slug) => {
            conteudo::salvar_no_catalogo(estado, &conta, slug, false, corpo).await
        }
        Rota::Autorizar => conector::tela_de_autorizacao(estado, &conta, uri).await,
        Rota::Decidir => conector::autorizar(estado, &conta, corpo).await,
        Rota::Conector => {
            let origem = origem_do_painel(&estado.configuracao, host);
            conector::tela(estado, &conta, &origem).await
        }
        Rota::RevogarConexao(id) => conector::revogar(estado, &conta, id).await,
    }
}

pub async fn atender(
    estado: &Estado,
    host: &str,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    uri: &Uri,
    corpo: &Bytes,
) -> Response {
    match responder(estado, host, metodo, cabecalhos, uri, corpo).await {
        Ok(resposta) => resposta,
        Err(erro) => {
            tracing::error!(%erro, caminho = uri.path(), "falha no painel");
            simples(StatusCode::INTERNAL_SERVER_ERROR, "Erro interno.")
        }
    }
}
