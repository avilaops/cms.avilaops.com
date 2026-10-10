//! O servidor HTTP: resolve o site pelo host e serve o que está publicado.

mod acesso;
mod admin;
mod biblioteca;
mod cache;
mod conector;
mod conteudo;
mod dominio;
mod editor;
mod equipe;
mod midia;
mod painel;
mod publico;
mod resposta;
mod visao;

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::StatusCode;
use axum::http::{HeaderMap, Method, Uri};
use axum::response::Response;
use axum::routing::{get, post};
use cms_dominio::LimitesDeCriacao;
use cms_integracoes::auth::ClienteAuth;
use serde::Deserialize;
use sqlx::PgPool;

pub use biblioteca::caminho_do_original;
pub use cache::Cache;

#[derive(Debug, Clone)]
pub struct Configuracao {
    /// Todo site nasce em `<slug>.<dominio_base>`.
    pub dominio_base: String,
    /// `https` em produção.
    pub esquema: String,
    /// Uma pasta por site, com as variantes de imagem.
    pub diretorio_de_midia: PathBuf,
    /// O token com que o n8n encerra eventos. Sem ele, a rota de volta
    /// recusa todo pedido.
    pub token_do_n8n: Option<Segredo>,
    /// A chave do IndexNow. Cada site a serve em `/<chave>.txt`, que é como o
    /// buscador confere que o aviso veio de quem responde pelo endereço.
    pub chave_do_indexnow: Option<String>,
    /// O host em que o painel responde, como `cms.avilaops.com`. Sem ele, a
    /// aplicação só serve sites.
    pub host_do_painel: Option<String>,
    pub limites_de_criacao: LimitesDeCriacao,
    /// Quanto de imagem original um site pode guardar, em bytes.
    pub limite_de_midia_por_site: u64,
    /// Os endereços IP deste servidor. É para eles que o domínio próprio de
    /// um cliente precisa apontar.
    pub ips_do_servidor: Vec<IpAddr>,
}

/// O maior corpo de pedido aceito: o limite de envio do motor e a folga do
/// formulário em volta.
const CORPO_MAXIMO: usize = motor_web::validacao::UPLOAD_MAXIMO_BYTES + 1024 * 1024;

/// Um segredo de configuração. Não aparece em registro nem em `Debug`.
#[derive(Clone)]
pub struct Segredo(String);

impl Segredo {
    pub fn novo(valor: impl Into<String>) -> Self {
        Self(valor.into())
    }

    pub fn expor(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segredo(***)")
    }
}

#[derive(Debug, Clone)]
pub struct Estado {
    pub pool: PgPool,
    pub configuracao: Arc<Configuracao>,
    /// O login único. Sem ele, o painel não deixa ninguém entrar.
    pub auth: Option<ClienteAuth>,
    /// As páginas prontas, em memória.
    pub cache: Arc<Cache>,
}

pub fn roteador(estado: Estado) -> Router {
    Router::new()
        .route("/api/saude", get(saude))
        .route("/api/dominio-permitido", get(dominio_permitido))
        .route(
            "/api/admin/eventos/{id}/encerrar",
            post(admin::encerrar_evento),
        )
        .fallback(despachar)
        .layer(DefaultBodyLimit::max(CORPO_MAXIMO))
        .with_state(estado)
}

#[derive(Deserialize)]
struct PerguntaDoCaddy {
    #[serde(default)]
    domain: String,
}

/// O que o Caddy pergunta antes de emitir certificado sob demanda. 200 só
/// para o host do painel e para endereço de site que existe.
async fn dominio_permitido(
    State(estado): State<Estado>,
    Query(pergunta): Query<PerguntaDoCaddy>,
) -> StatusCode {
    let configuracao = &estado.configuracao;
    if configuracao.host_do_painel.as_deref() == Some(pergunta.domain.as_str()) {
        return StatusCode::OK;
    }
    match cms_dados::dominio_permitido(&estado.pool, &pergunta.domain, &configuracao.dominio_base)
        .await
    {
        Ok(true) => StatusCode::OK,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(erro) => {
            tracing::error!(%erro, "falha ao conferir domínio para o certificado");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

/// O host decide: o do painel vai para o painel, qualquer outro é um site.
async fn despachar(
    State(estado): State<Estado>,
    metodo: Method,
    cabecalhos: HeaderMap,
    uri: Uri,
    corpo: Bytes,
) -> Response {
    let host = publico::host_do_pedido(&cabecalhos);
    let host_do_painel = estado.configuracao.host_do_painel.as_deref();
    match host {
        Some(host) if Some(host.nome.as_str()) == host_do_painel => {
            painel::atender(&estado, &host.com_porta, &metodo, &cabecalhos, &uri, &corpo).await
        }
        _ => publico::atender(&estado, &metodo, &cabecalhos, &uri).await,
    }
}

/// 200 quando o banco responde, 503 quando não. É o que o despachante de
/// deploy consulta antes de dar a troca de versão por boa.
async fn saude(State(estado): State<Estado>) -> (StatusCode, &'static str) {
    if cms_dados::banco_responde(&estado.pool).await {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "banco indisponível")
    }
}
