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

/// A origem (`esquema://host[:porta]`) de um endereço, se ela só tem o que
/// cabe em uma política de conteúdo.
fn origem_de(endereco: &str) -> Option<&str> {
    let depois_do_esquema = endereco.find("://")? + 3;
    let fim = endereco[depois_do_esquema..]
        .find(['/', '?', '#'])
        .map_or(endereco.len(), |i| depois_do_esquema + i);
    let origem = &endereco[..fim];
    let limpa = fim > depois_do_esquema
        && origem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '/' | '[' | ']'));
    limpa.then_some(origem)
}

/// Tela do painel cujo formulário termina em um redirecionamento para outro
/// site: o consentimento do conector, que devolve a pessoa ao assistente. O
/// navegador aplica `form-action` também ao redirecionamento que vem depois
/// do envio, então a origem do destino precisa estar na política, e só ela.
pub fn html_privado_com_retorno(status: StatusCode, corpo: String, retorno: &str) -> Response {
    let mut resposta = html_privado(status, corpo);
    let politica = origem_de(retorno)
        .map(|origem| {
            POLITICA_DE_CONTEUDO.replace(
                "form-action 'self'",
                &format!("form-action 'self' {origem}"),
            )
        })
        .and_then(|politica| HeaderValue::from_str(&politica).ok());
    if let Some(politica) = politica {
        resposta
            .headers_mut()
            .insert("content-security-policy", politica);
    }
    resposta
}

/// Tela do painel: é de uma pessoa só. Ninguém no caminho guarda, e buscador
/// nenhum indexa.
pub fn html_privado(status: StatusCode, corpo: String) -> Response {
    let mut resposta = html(status, corpo, false);
    resposta
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    resposta
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn origem_sai_sem_caminho_e_recusa_o_que_nao_cabe_na_politica() {
        assert_eq!(
            origem_de("https://claude.ai/api/mcp/auth_callback"),
            Some("https://claude.ai")
        );
        assert_eq!(
            origem_de("http://localhost:51234/callback?x=1"),
            Some("http://localhost:51234")
        );
        assert_eq!(origem_de("https://a.example;script-src *"), None);
        assert_eq!(origem_de("https:///caminho"), None);
        assert_eq!(origem_de("sem-esquema"), None);
    }

    #[test]
    fn consentimento_libera_o_envio_so_para_a_origem_do_retorno() {
        let resposta = html_privado_com_retorno(
            StatusCode::OK,
            String::new(),
            "https://claude.ai/api/mcp/auth_callback",
        );
        let politica = resposta.headers()["content-security-policy"]
            .to_str()
            .expect("texto");
        assert!(politica.contains("form-action 'self' https://claude.ai;"));
        assert!(politica.contains("script-src 'none'"));
        // As outras telas continuam só com o próprio painel.
        let comum = html_privado(StatusCode::OK, String::new());
        assert!(
            comum.headers()["content-security-policy"]
                .to_str()
                .expect("texto")
                .contains("form-action 'self';")
        );
    }
}
