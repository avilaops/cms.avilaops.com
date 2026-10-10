//! O site de demonstração: a marcenaria fictícia do motor, gravada pelo mesmo
//! caminho de publicação, com imagens de verdade.
//!
//! As fotos são desenhadas aqui, porque o repositório é público e não guarda
//! foto de ninguém. O que importa é que passem pelo processamento real: AVIF e
//! WebP nas larguras certas, com dimensões declaradas.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

use chrono::{DateTime, Utc};
use cms_dados::fluxo::{self, ErroDeFluxo};
use cms_dominio::{Ator, PerfilDoSite, Situacao};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use motor_web::demonstracao;
use motor_web::imagem::{VarianteGerada, processar_imagem};
use motor_web::tipos::{Bloco, Conteudo, Documento, Midia, Variante};
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

/// A data em que o exemplo diz que o documento foi ao ar.
fn data_de_publicacao(documento: &Documento) -> DateTime<Utc> {
    match &documento.conteudo {
        Conteudo::Pagina(pagina) => pagina.publicado_em,
        Conteudo::Post(post) => post.publicado_em,
        Conteudo::Produto(produto) => produto.atualizado_em,
    }
}

/// O que o fluxo barrou, com os códigos do motor quando for o caso.
fn recusa(documento: &Documento, erro: &ErroDeFluxo) -> Erro {
    let motivo = match erro {
        ErroDeFluxo::Recusado(problemas) => problemas
            .iter()
            .map(|problema| problema.codigo)
            .collect::<Vec<_>>()
            .join(", "),
        outro => outro.to_string(),
    };
    Erro::Semente(format!(
        "{} não pode ser publicado: {motivo}",
        documento.caminho
    ))
}

pub async fn semear_demonstracao(pool: &PgPool, diretorio_de_midia: &Path) -> Result<Resumo, Erro> {
    let (perfil, documentos, arquivos) =
        tokio::task::spawn_blocking(preparar)
            .await
            .map_err(|erro| {
                Erro::Semente(format!("o preparo das imagens foi interrompido: {erro}"))
            })??;
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

    // Pelo mesmo caminho do painel: rascunho, validação do motor e publicação.
    // Cada documento vai ao ar na data em que o exemplo diz que foi publicado,
    // para o blog de demonstração não nascer com tudo no mesmo dia.
    let equipe = Ator::da_equipe("semente");
    for documento in &documentos {
        let salvo = fluxo::salvar_rascunho(pool, site_id, &equipe, None, &documento.conteudo)
            .await
            .map_err(|erro| recusa(documento, &erro))?;
        fluxo::publicar(
            pool,
            site_id,
            &equipe,
            salvo.documento_id,
            data_de_publicacao(documento),
        )
        .await
        .map_err(|erro| recusa(documento, &erro))?;
    }
    Ok(Resumo {
        slug: SLUG,
        documentos: documentos.len(),
        arquivos: arquivos.len(),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[sqlx::test(migrator = "cms_dados::MIGRADOR")]
    async fn a_demonstracao_vai_ao_ar_pelo_fluxo_de_publicacao(pool: PgPool) {
        let diretorio = std::env::temp_dir().join(format!("cms-semente-{}", uuid::Uuid::new_v4()));
        let resumo = semear_demonstracao(&pool, &diretorio)
            .await
            .expect("demonstração semeada");
        assert_eq!(resumo.documentos, 9);
        assert!(resumo.arquivos > 0);

        // Tudo no ar, e cada publicação passou pelo histórico e virou evento.
        let (no_ar, publicacoes, eventos): (i64, i64, i64) = sqlx::query_as(
            r#"
            select (select count(*) from documento where situacao = 'publicado'),
                   (select count(*) from historico where acao = 'conteudo.publicado' and equipe),
                   (select count(*) from evento where tipo = 'conteudo.publicado')
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("contagens");
        assert_eq!((no_ar, publicacoes, eventos), (9, 9, 9));

        // A data de publicação é a do exemplo, gravada pelo servidor.
        let publicado_em: DateTime<Utc> = sqlx::query_scalar(
            "select publicado_em from documento where caminho = '/blog/como-escolher-a-madeira-da-mesa'",
        )
        .fetch_one(&pool)
        .await
        .expect("post do exemplo");
        assert_eq!(publicado_em.date_naive().to_string(), "2026-08-12");

        // Semear de novo recria o site, sem sobrar nada do anterior.
        semear_demonstracao(&pool, &diretorio)
            .await
            .expect("demonstração recriada");
        let sites: i64 = sqlx::query_scalar("select count(*) from site")
            .fetch_one(&pool)
            .await
            .expect("contagem");
        assert_eq!(sites, 1);
        std::fs::remove_dir_all(&diretorio).expect("pasta removida");
    }
}
