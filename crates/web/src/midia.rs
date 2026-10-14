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
        (true, "jpg") => Some("image/jpeg"),
        _ => None,
    }
}

pub async fn servir(diretorio: &Path, site_id: Uuid, arquivo: &str) -> Result<Response, ErroWeb> {
    let Some(tipo) = tipo_do_arquivo(arquivo) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    // A pasta é a do site do pedido: um site não lê arquivo de outro.
    let caminho = diretorio.join(site_id.to_string()).join(arquivo);
    let lido = match tokio::fs::read(&caminho).await {
        Err(erro) if erro.kind() == ErrorKind::NotFound && tipo == "image/jpeg" => {
            jpeg_do_webp(&caminho).await
        }
        lido => lido,
    };
    match lido {
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

/// Qualidade do JPEG da prévia de link: perto do WebP, sem pesar demais.
const QUALIDADE_JPEG: u8 = 85;

/// WhatsApp e Telegram não abrem WebP na prévia de link: o `og:image` aponta
/// para o JPEG com o mesmo nome. Ele nasce do WebP no primeiro pedido e fica
/// gravado ao lado; serve também para as imagens enviadas antes desta regra.
async fn jpeg_do_webp(caminho: &Path) -> std::io::Result<Vec<u8>> {
    let webp = tokio::fs::read(caminho.with_extension("webp")).await?;
    let jpeg = tokio::task::spawn_blocking(move || webp_para_jpeg(&webp))
        .await
        .map_err(std::io::Error::other)??;
    // Grava num temporário e renomeia: dois pedidos juntos não deixam meio arquivo.
    let temporario = caminho.with_extension(format!("jpg.{}", Uuid::new_v4()));
    tokio::fs::write(&temporario, &jpeg).await?;
    if let Err(erro) = tokio::fs::rename(&temporario, caminho).await {
        let _ = tokio::fs::remove_file(&temporario).await;
        tracing::warn!(%erro, "jpeg da prévia não ficou gravado");
    }
    Ok(jpeg)
}

/// O JPEG não tem transparência: o que era transparente vira branco.
fn webp_para_jpeg(webp: &[u8]) -> std::io::Result<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageFormat, Rgb, RgbImage};

    let imagem = image::load_from_memory_with_format(webp, ImageFormat::WebP)
        .map_err(std::io::Error::other)?
        .to_rgba8();
    let rgb = RgbImage::from_fn(imagem.width(), imagem.height(), |x, y| {
        let [r, g, b, a] = imagem.get_pixel(x, y).0;
        let sobre_branco = |canal: u8| {
            ((u16::from(canal) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8
        };
        Rgb([sobre_branco(r), sobre_branco(g), sobre_branco(b)])
    });
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, QUALIDADE_JPEG)
        .encode_image(&rgb)
        .map_err(std::io::Error::other)?;
    Ok(jpeg)
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
        assert_eq!(
            tipo_do_arquivo("bancada-0a1b2c3d-1024.jpg"),
            Some("image/jpeg")
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
            "arquivo.jpeg",
            "../segredo.jpg",
            "Arquivo.webp",
            "arquivo",
        ] {
            assert_eq!(tipo_do_arquivo(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn jpeg_sai_do_webp_com_o_transparente_em_branco() {
        use image::codecs::webp::WebPEncoder;
        use image::{ExtendedColorType, Rgba, RgbaImage};

        let imagem = RgbaImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([200, 80, 20, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let mut webp = Vec::new();
        WebPEncoder::new_lossless(&mut webp)
            .encode(imagem.as_raw(), 64, 32, ExtendedColorType::Rgba8)
            .expect("webp de teste");

        let jpeg = webp_para_jpeg(&webp).expect("jpeg gerado");
        assert_eq!(&jpeg[..3], [0xFF, 0xD8, 0xFF]);
        let lido = image::load_from_memory(&jpeg)
            .expect("jpeg legível")
            .to_rgb8();
        assert_eq!(lido.dimensions(), (64, 32));
        let [r, g, b] = lido.get_pixel(60, 16).0;
        assert!(r > 245 && g > 245 && b > 245, "transparente virou branco");

        assert!(webp_para_jpeg(b"RIFF0000WEBP").is_err());
    }
}
