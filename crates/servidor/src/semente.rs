//! O site de demonstração: a marcenaria fictícia do motor, gravada pelo mesmo
//! caminho de publicação, com imagens de verdade.
//!
//! As fotos são desenhadas aqui, porque o repositório é público e não guarda
//! foto de ninguém. O que importa é que passem pelo processamento real: AVIF e
//! WebP nas larguras certas, com dimensões declaradas.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

use chrono::Utc;
use cms_dominio::{PerfilDoSite, Situacao};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use motor_web::demonstracao;
use motor_web::imagem::{VarianteGerada, processar_imagem};
use motor_web::tipos::{Bloco, Conteudo, Documento, Midia, Variante};
use motor_web::validacao::{Contexto, pode_publicar, validar};
use sqlx::PgPool;

use crate::Erro;

const SLUG: &str = "demonstracao";

#[derive(Debug)]
pub struct Resumo {
    pub slug: &'static str,
    pub documentos: usize,
    pub arquivos: usize,
}

/// Uma foto de mentira: um degradê cuja cor sai do identificador da mídia.
fn desenhar(midia: &Midia) -> Result<Vec<u8>, Erro> {
    let semente = midia.id.bytes().fold(0u32, |soma, byte| {
        soma.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    let (r, g, b) = (
        (semente % 120) as u8 + 60,
        ((semente / 7) % 120) as u8 + 60,
        ((semente / 53) % 120) as u8 + 60,
    );
    let (largura, altura) = (midia.largura.max(1), midia.altura.max(1));
    let imagem = RgbImage::from_fn(largura, altura, |x, y| {
        let claro = (x * 60 / largura + y * 40 / altura) as u8;
        Rgb([
            r.saturating_add(claro),
            g.saturating_add(claro),
            b.saturating_add(claro),
        ])
    });
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(imagem)
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
        .map_err(|erro| Erro::Semente(format!("não foi possível desenhar {}: {erro}", midia.id)))?;
    Ok(bytes)
}

/// As mídias já processadas, por identificador, e os arquivos a gravar.
#[derive(Default)]
struct Acervo {
    midias: BTreeMap<String, Midia>,
    arquivos: Vec<VarianteGerada>,
}

impl Acervo {
    /// Troca a mídia de exemplo pela processada, gerando-a na primeira vez.
    fn trocar(&mut self, midia: &mut Midia) -> Result<(), Erro> {
        if let Some(pronta) = self.midias.get(&midia.id) {
            *midia = pronta.clone();
            return Ok(());
        }
        let processada = processar_imagem(&format!("{}.jpg", midia.id), &desenhar(midia)?)
            .map_err(|problemas| {
                let codigos: Vec<&str> = problemas.iter().map(|p| p.codigo).collect();
                Erro::Semente(format!(
                    "imagem {} recusada: {}",
                    midia.id,
                    codigos.join(", ")
                ))
            })?;
        midia.largura = processada.largura;
        midia.altura = processada.altura;
        midia.variantes = processada
            .variantes
            .iter()
            .map(|v| Variante {
                formato: v.formato,
                largura: v.largura,
                url: format!("/midia/{}", v.arquivo),
            })
            .collect();
        self.midias.insert(midia.id.clone(), midia.clone());
        self.arquivos.extend(processada.variantes);
        Ok(())
    }

    fn trocar_no_corpo(&mut self, corpo: &mut [Bloco]) -> Result<(), Erro> {
        for bloco in corpo {
            if let Bloco::Imagem { midia } = bloco {
                self.trocar(midia)?;
            }
        }
        Ok(())
    }

    fn trocar_no_documento(&mut self, documento: &mut Documento) -> Result<(), Erro> {
        match &mut documento.conteudo {
            Conteudo::Pagina(pagina) => {
                if let Some(capa) = &mut pagina.capa {
                    self.trocar(capa)?;
                }
                self.trocar_no_corpo(&mut pagina.corpo)
            }
            Conteudo::Post(post) => {
                self.trocar(&mut post.capa)?;
                self.trocar(&mut post.autor.foto)?;
                self.trocar_no_corpo(&mut post.corpo)
            }
            Conteudo::Produto(_) => Ok(()),
        }
    }
}

/// Desenha e processa todas as imagens. É trabalho pesado de CPU, e por isso
/// roda fora das threads que atendem pedidos.
fn preparar() -> Result<(PerfilDoSite, Vec<Documento>, Vec<VarianteGerada>), Erro> {
    let mut acervo = Acervo::default();
    let mut perfil = PerfilDoSite::from(demonstracao::site());
    acervo.trocar(&mut perfil.logo)?;

    // Catálogo e checkout são do Lojas: os produtos do exemplo ficam de fora.
    let mut documentos: Vec<Documento> = demonstracao::documentos()
        .into_iter()
        .filter(|d| !matches!(d.conteudo, Conteudo::Produto(_)))
        .collect();
    for documento in &mut documentos {
        acervo.trocar_no_documento(documento)?;
    }
    Ok((perfil, documentos, acervo.arquivos))
}

/// A mesma trava de qualquer publicação: o que o motor barra não vai ao ar.
fn conferir(documentos: &[Documento]) -> Result<(), Erro> {
    for (i, documento) in documentos.iter().enumerate() {
        let outros = || {
            documentos
                .iter()
                .enumerate()
                .filter(move |(j, _)| *j != i)
                .map(|(_, d)| d)
        };
        let titulos: Vec<String> = outros().map(|d| d.seo().titulo.clone()).collect();
        let descricoes: Vec<String> = outros().map(|d| d.seo().descricao.clone()).collect();
        let contexto = Contexto {
            titulos_em_uso: &titulos,
            descricoes_em_uso: &descricoes,
            ..Contexto::vazio()
        };
        let problemas = validar(documento, &contexto);
        if !pode_publicar(&problemas) {
            let codigos: Vec<&str> = problemas.iter().map(|p| p.codigo).collect();
            return Err(Erro::Semente(format!(
                "{} não pode ser publicado: {}",
                documento.caminho,
                codigos.join(", ")
            )));
        }
    }
    Ok(())
}

pub async fn semear_demonstracao(pool: &PgPool, diretorio_de_midia: &Path) -> Result<Resumo, Erro> {
    let (perfil, documentos, arquivos) =
        tokio::task::spawn_blocking(preparar)
            .await
            .map_err(|erro| {
                Erro::Semente(format!("o preparo das imagens foi interrompido: {erro}"))
            })??;
    conferir(&documentos)?;

    // Recriar o site troca o identificador dele; as imagens antigas saem junto.
    if let Some(anterior) = cms_dados::site_por_slug(pool, SLUG).await? {
        match tokio::fs::remove_dir_all(diretorio_de_midia.join(anterior.id.to_string())).await {
            Err(erro) if erro.kind() != std::io::ErrorKind::NotFound => return Err(erro.into()),
            _ => {}
        }
    }
    cms_dados::apagar_site(pool, SLUG).await?;
    // Endereço provisório assumido como definitivo: o site de demonstração
    // precisa aparecer na busca para ser medido.
    let site_id = cms_dados::criar_site(pool, SLUG, Situacao::Ativo, true, &perfil).await?;

    let pasta = diretorio_de_midia.join(site_id.to_string());
    tokio::fs::create_dir_all(&pasta).await?;
    for arquivo in &arquivos {
        tokio::fs::write(pasta.join(&arquivo.arquivo), &arquivo.bytes).await?;
    }

    let agora = Utc::now();
    for documento in &documentos {
        cms_dados::publicar(pool, site_id, documento, agora).await?;
    }
    Ok(Resumo {
        slug: SLUG,
        documentos: documentos.len(),
        arquivos: arquivos.len(),
    })
}
