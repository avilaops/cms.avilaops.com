//! Respostas e cabeçalhos comuns.

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, LOCATION};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum ErroWeb {
    #[error(transparent)]
    Dados(#[from] cms_dados::ErroDeDados),
    #[error("falha ao montar a página: {0}")]
    Modelo(#[from] askama::Error),
    #[error(transparent)]
    Documento(#[from] motor_web::validacao::DocumentoInvalido),
    #[error("falha ao ler arquivo: {0}")]
    Arquivo(#[from] std::io::Error),
}

/// Sem script próprio nem de terceiros; quadro só de YouTube e Vimeo, que são
/// as origens de vídeo que o motor aceita.
const POLITICA_DE_CONTEUDO: &str = "default-src 'self'; script-src 'none'; style-src 'self' 'unsafe-inline'; \
    img-src 'self' data:; frame-src https://www.youtube-nocookie.com https://player.vimeo.com; \
    base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Página pública sem sessão: a borda pode guardar por um minuto e servir a
/// antiga enquanto busca a nova.
const CACHE_DE_PAGINA: &str = "public, max-age=0, s-maxage=60, stale-while-revalidate=600";

fn com_cabecalho(mut resposta: Response, nome: HeaderName, valor: &'static str) -> Response {
    resposta
        .headers_mut()
        .insert(nome, HeaderValue::from_static(valor));
    resposta
}

pub fn html(status: StatusCode, corpo: String, indexavel: bool) -> Response {
    let mut resposta = (status, corpo).into_response();
    let cabecalhos = resposta.headers_mut();
    cabecalhos.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    cabecalhos.insert(
        "content-security-policy",
        HeaderValue::from_static(POLITICA_DE_CONTEUDO),
    );
    cabecalhos.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    cabecalhos.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    if status == StatusCode::OK {
        cabecalhos.insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_DE_PAGINA));
    }
    if !indexavel {
        cabecalhos.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    }
    resposta
}

pub fn texto(status: StatusCode, tipo: &'static str, corpo: String) -> Response {
    let resposta = (status, corpo).into_response();
    let resposta = com_cabecalho(resposta, CONTENT_TYPE, tipo);
    com_cabecalho(
        resposta,
        HeaderName::from_static("x-content-type-options"),
        "nosniff",
    )
}

pub fn simples(status: StatusCode, mensagem: &'static str) -> Response {
    texto(status, "text/plain; charset=utf-8", mensagem.to_string())
}

pub fn redirecionar(status: StatusCode, destino: &str) -> Response {
    match HeaderValue::from_str(destino) {
        Ok(valor) => {
            let mut resposta = status.into_response();
            resposta.headers_mut().insert(LOCATION, valor);
            resposta
        }
        Err(_) => simples(StatusCode::BAD_REQUEST, "Endereço inválido."),
    }
}
