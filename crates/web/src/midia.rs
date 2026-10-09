//! Entrega das variantes de imagem gravadas em disco.

use std::io::ErrorKind;
use std::path::Path;

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use crate::resposta::{ErroWeb, simples};

/// O nome leva o hash do conteúdo: o arquivo nunca muda, então o cache é longo.
const CACHE_DE_ARQUIVO: &str = "public, max-age=31536000, immutable";

/// Só o que o motor gera: letras minúsculas, números, hífen e a extensão.
/// Qualquer outra coisa nem chega ao disco.
fn tipo_do_arquivo(arquivo: &str) -> Option<&'static str> {
    let (nome, extensao) = arquivo.rsplit_once('.')?;
    let nome_valido = !nome.is_empty()
        && arquivo.len() <= 200
        && nome
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    match (nome_valido, extensao) {
        (true, "avif") => Some("image/avif"),
        (true, "webp") => Some("image/webp"),
        _ => None,
    }
}

pub async fn servir(diretorio: &Path, site_id: Uuid, arquivo: &str) -> Result<Response, ErroWeb> {
    let Some(tipo) = tipo_do_arquivo(arquivo) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    // A pasta é a do site do pedido: um site não lê arquivo de outro.
    let caminho = diretorio.join(site_id.to_string()).join(arquivo);
    match tokio::fs::read(&caminho).await {
        Ok(bytes) => {
            let mut resposta = bytes.into_response();
            let cabecalhos = resposta.headers_mut();
            cabecalhos.insert(CONTENT_TYPE, HeaderValue::from_static(tipo));
            cabecalhos.insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_DE_ARQUIVO));
            cabecalhos.insert(
                "x-content-type-options",
                HeaderValue::from_static("nosniff"),
            );
            Ok(resposta)
        }
        Err(erro) if erro.kind() == ErrorKind::NotFound => {
            Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."))
        }
        Err(erro) => Err(erro.into()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aceita_so_o_que_o_motor_gera() {
        assert_eq!(
            tipo_do_arquivo("bancada-0a1b2c3d-1200.webp"),
            Some("image/webp")
        );
        assert_eq!(
            tipo_do_arquivo("bancada-0a1b2c3d-480.avif"),
            Some("image/avif")
        );
    }

    #[test]
    fn recusa_caminho_e_extensao_estranhos() {
        for ruim in [
            "",
            ".webp",
            "../segredo.webp",
            "..%2fsegredo.webp",
            "pasta/arquivo.webp",
            "pasta\\arquivo.webp",
            "arquivo.svg",
            "arquivo.webp.exe",
            "Arquivo.webp",
            "arquivo",
        ] {
            assert_eq!(tipo_do_arquivo(ruim), None, "{ruim}");
        }
    }
}
