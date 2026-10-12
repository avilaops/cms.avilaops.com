//! O ícone do site (favicon), gerado da logo: quem cuida do site escolhe a
//! logo e o ícone da aba, do atalho no celular e dos favoritos sai sozinho.
//!
//! A logo entra inteira num quadrado, sem cortar nem esticar. Logo quadrada dá
//! o melhor ícone; logo larga fica pequena, porque precisa caber.

use std::io::{Cursor, ErrorKind};
use std::path::Path;

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use image::imageops::{FilterType, overlay};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use uuid::Uuid;

use crate::biblioteca::caminho_do_original;
use crate::resposta::{ErroWeb, simples};

/// O ícone muda quando a logo muda, e o endereço leva a versão: um dia de
/// cache basta para o navegador não pedir a cada página.
const CACHE_DO_ICONE: &str = "public, max-age=86400";

/// Como o ícone é montado para um endereço.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icone {
    lado: u32,
    /// O atalho do iPhone não aceita transparência: o fundo vira preto.
    fundo_branco: bool,
}

impl Icone {
    /// O ícone que responde em um endereço do site, se for um deles.
    /// `/favicon.ico` entrega PNG, que todo navegador atual aceita ali.
    pub fn do_caminho(caminho: &str) -> Option<Self> {
        let (lado, fundo_branco) = match caminho {
            "/favicon.ico" | "/favicon-48.png" => (48, false),
            "/favicon-32.png" => (32, false),
            "/favicon-192.png" => (192, false),
            "/favicon-512.png" => (512, false),
            "/apple-touch-icon.png" | "/apple-touch-icon-precomposed.png" => (180, true),
            _ => return None,
        };
        Some(Self { lado, fundo_branco })
    }

    /// A logo dentro de um quadrado do tamanho do ícone, em PNG.
    fn montar(self, original: &[u8]) -> Option<Vec<u8>> {
        let logo = image::load_from_memory(original).ok()?;
        // `resize` mantém a proporção: a logo cabe no quadrado sem deformar.
        let reduzida = logo
            .resize(self.lado, self.lado, FilterType::Lanczos3)
            .to_rgba8();
        let fundo = if self.fundo_branco {
            Rgba([255, 255, 255, 255])
        } else {
            Rgba([0, 0, 0, 0])
        };
        let mut quadro = RgbaImage::from_pixel(self.lado, self.lado, fundo);
        let x = i64::from((self.lado - reduzida.width()) / 2);
        let y = i64::from((self.lado - reduzida.height()) / 2);
        overlay(&mut quadro, &reduzida, x, y);
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(quadro)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .ok()?;
        Some(png)
    }
}

/// Entrega o ícone do site. Sem logo, ou com logo que não abre, não há ícone.
pub async fn servir(
    diretorio: &Path,
    site_id: Uuid,
    logo: &str,
    icone: Icone,
) -> Result<Response, ErroWeb> {
    let nao_tem = || Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    let Ok(logo_id) = Uuid::parse_str(logo) else {
        return nao_tem();
    };
    let original = match tokio::fs::read(caminho_do_original(diretorio, site_id, logo_id)).await {
        Ok(bytes) => bytes,
        Err(erro) if erro.kind() == ErrorKind::NotFound => return nao_tem(),
        Err(erro) => return Err(erro.into()),
    };
    // Reduzir imagem ocupa processador: fora das threads que atendem pedidos.
    let png = tokio::task::spawn_blocking(move || icone.montar(&original))
        .await
        .ok()
        .flatten();
    let Some(png) = png else {
        return nao_tem();
    };
    let mut resposta = png.into_response();
    let cabecalhos = resposta.headers_mut();
    cabecalhos.insert(CONTENT_TYPE, HeaderValue::from_static("image/png"));
    cabecalhos.insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_DO_ICONE));
    cabecalhos.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(resposta)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn logo(largura: u32, altura: u32) -> Vec<u8> {
        let imagem = RgbaImage::from_pixel(largura, altura, Rgba([34, 92, 242, 255]));
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(imagem)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .expect("png");
        png
    }

    #[test]
    fn so_os_enderecos_de_icone_respondem() {
        assert!(Icone::do_caminho("/favicon.ico").is_some());
        assert!(Icone::do_caminho("/apple-touch-icon.png").is_some());
        assert!(Icone::do_caminho("/favicon-192.png").is_some());
        assert!(Icone::do_caminho("/favicon-64.png").is_none());
        assert!(Icone::do_caminho("/blog/favicon.ico").is_none());
    }

    #[test]
    fn logo_larga_cabe_inteira_no_quadrado_sem_deformar() {
        let icone = Icone::do_caminho("/favicon-32.png").expect("ícone");
        let png = icone.montar(&logo(400, 100)).expect("png");
        let pronto = image::load_from_memory(&png).expect("abre").to_rgba8();
        assert_eq!((pronto.width(), pronto.height()), (32, 32));
        // No meio está a logo; em cima sobra o fundo, transparente.
        assert_eq!(pronto.get_pixel(16, 16), &Rgba([34, 92, 242, 255]));
        assert_eq!(pronto.get_pixel(16, 2)[3], 0);
    }

    #[test]
    fn atalho_do_iphone_tem_fundo_branco() {
        let icone = Icone::do_caminho("/apple-touch-icon.png").expect("ícone");
        let png = icone.montar(&logo(400, 100)).expect("png");
        let pronto = image::load_from_memory(&png).expect("abre").to_rgba8();
        assert_eq!((pronto.width(), pronto.height()), (180, 180));
        assert_eq!(pronto.get_pixel(90, 5), &Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn arquivo_que_nao_e_imagem_nao_vira_icone() {
        let icone = Icone::do_caminho("/favicon.ico").expect("ícone");
        assert!(icone.montar(b"isto nao e imagem").is_none());
    }
}
