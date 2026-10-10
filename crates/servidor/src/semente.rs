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
use cms_dados::{DadosDoAutor, NovaMidia, VarianteGravada};
use cms_dominio::{Ator, PerfilDoSite, Situacao};
use cms_web::caminho_do_original;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use motor_web::demonstracao;
use motor_web::imagem::{ImagemProcessada, processar_imagem};
use motor_web::tipos::{Bloco, Conteudo, Documento, Midia, Post};
use sqlx::PgPool;
use uuid::Uuid;

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

/// Passa por todas as imagens de um conteúdo: capa, foto do autor e corpo.
fn para_cada_midia(conteudo: &mut Conteudo, visitar: &mut dyn FnMut(&mut Midia)) {
    let (capa, autor, corpo): (Option<&mut Midia>, Option<&mut Midia>, &mut [Bloco]) =
        match conteudo {
            Conteudo::Pagina(pagina) => (pagina.capa.as_mut(), None, &mut pagina.corpo),
            Conteudo::Post(post) => (
                Some(&mut post.capa),
                Some(&mut post.autor.foto),
                &mut post.corpo,
            ),
            Conteudo::Produto(_) => return,
        };
    capa.into_iter().chain(autor).for_each(&mut *visitar);
    for bloco in corpo {
        if let Bloco::Imagem { midia } = bloco {
            visitar(midia);
        }
    }
}

/// O conteúdo do exemplo que o CMS serve. Catálogo e checkout são do Lojas:
/// os produtos ficam de fora.
fn exemplo() -> (PerfilDoSite, Vec<Documento>) {
    let documentos = demonstracao::documentos()
        .into_iter()
        .filter(|d| !matches!(d.conteudo, Conteudo::Produto(_)))
        .collect();
    (PerfilDoSite::from(demonstracao::site()), documentos)
}

/// Uma imagem do exemplo, desenhada e já com as variantes.
struct Preparada {
    original: Midia,
    jpeg: Vec<u8>,
    processada: ImagemProcessada,
}

/// Desenha e processa cada imagem do exemplo uma vez. É trabalho pesado de
/// CPU, e por isso roda fora das threads que atendem pedidos.
fn preparar() -> Result<Vec<Preparada>, Erro> {
    let (mut perfil, mut documentos) = exemplo();
    let mut originais: BTreeMap<String, Midia> = BTreeMap::new();
    let mut coletar = |midia: &mut Midia| {
        originais
            .entry(midia.id.clone())
            .or_insert_with(|| midia.clone());
    };
    coletar(&mut perfil.logo);
    for documento in &mut documentos {
        para_cada_midia(&mut documento.conteudo, &mut coletar);
    }
    originais
        .into_values()
        .map(|original| {
            let jpeg = desenhar(&original)?;
            let processada =
                processar_imagem(&format!("{}.jpg", original.id), &jpeg).map_err(|problemas| {
                    let codigos: Vec<&str> = problemas.iter().map(|p| p.codigo).collect();
                    Erro::Semente(format!(
                        "imagem {} recusada: {}",
                        original.id,
                        codigos.join(", ")
                    ))
                })?;
            Ok(Preparada {
                original,
                jpeg,
                processada,
            })
        })
        .collect()
}

fn recusada(erro: impl std::fmt::Display) -> Erro {
    Erro::Semente(erro.to_string())
}

/// Põe uma imagem na biblioteca do site como o envio pelo painel faria:
/// registro, original no disco, variantes e a marca de pronta. Devolve a
/// imagem como o conteúdo passa a citá-la.
async fn guardar(
    pool: &PgPool,
    diretorio_de_midia: &Path,
    site_id: Uuid,
    equipe: &Ator,
    preparada: &Preparada,
) -> Result<Midia, Erro> {
    let nova = NovaMidia {
        nome: format!("{}.jpg", preparada.original.id),
        hash: cms_dados::hash_de_conteudo(&preparada.jpeg),
        largura: preparada.processada.largura,
        altura: preparada.processada.altura,
        alt: preparada.original.alt.clone(),
        legenda: preparada.original.legenda.clone(),
        credito: preparada.original.credito.clone(),
        direitos: preparada.original.direitos.clone(),
        bytes: preparada.jpeg.len() as u64,
    };
    let id = cms_dados::registrar_midia(pool, site_id, equipe, nova, u64::MAX / 4)
        .await
        .map_err(recusada)?;

    let original = caminho_do_original(diretorio_de_midia, site_id, id);
    if let Some(pasta) = original.parent() {
        tokio::fs::create_dir_all(pasta).await?;
    }
    tokio::fs::write(&original, &preparada.jpeg).await?;
    let pasta = diretorio_de_midia.join(site_id.to_string());
    let mut gravadas = Vec::with_capacity(preparada.processada.variantes.len());
    for variante in &preparada.processada.variantes {
        tokio::fs::write(pasta.join(&variante.arquivo), &variante.bytes).await?;
        gravadas.push(VarianteGravada {
            formato: variante.formato,
            largura: variante.largura,
            arquivo: variante.arquivo.clone(),
            bytes: variante.bytes.len() as u64,
        });
    }
    cms_dados::concluir_midia(
        pool,
        id,
        preparada.processada.largura,
        preparada.processada.altura,
        &gravadas,
    )
    .await?;
    cms_dados::midia_para_conteudo(pool, site_id, id)
        .await?
        .ok_or_else(|| {
            recusada(format!(
                "imagem {} sumiu da biblioteca",
                preparada.original.id
            ))
        })
}

/// Cadastra o autor e a categoria de um post no site e troca os do exemplo
/// pelos do cadastro, para o post abrir no editor com tudo escolhido.
async fn cadastrar_autor_e_categoria(
    pool: &PgPool,
    site_id: Uuid,
    equipe: &Ator,
    post: &mut Post,
) -> Result<(), Erro> {
    let dados = DadosDoAutor {
        nome: post.autor.nome.clone(),
        cargo: post.autor.cargo.clone(),
        bio: post.autor.bio.clone(),
        foto_id: Uuid::parse_str(&post.autor.foto.id).ok(),
        perfis: post.autor.perfis.join("\n"),
        credenciais: post.autor.credenciais.join("\n"),
    };
    let slug = cms_dados::salvar_autor(pool, site_id, equipe, &dados)
        .await
        .map_err(recusada)?;
    post.autor = cms_dados::autor_para_conteudo(pool, site_id, &slug)
        .await?
        .ok_or_else(|| recusada(format!("autor {slug} sumiu do cadastro")))?;
    post.categoria.slug = cms_dados::salvar_categoria(pool, site_id, equipe, &post.categoria.nome)
        .await
        .map_err(recusada)?;
    Ok(())
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
    let preparadas = tokio::task::spawn_blocking(preparar)
        .await
        .map_err(|erro| {
            Erro::Semente(format!("o preparo das imagens foi interrompido: {erro}"))
        })??;
    let (mut perfil, mut documentos) = exemplo();

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
    let equipe = Ator::da_equipe("semente");

    // As imagens entram na biblioteca do site, e o conteúdo passa a citá-las
    // por lá: é o que deixa a demonstração editável pelo painel.
    let mut biblioteca: BTreeMap<String, Midia> = BTreeMap::new();
    let mut arquivos = 0;
    for preparada in &preparadas {
        let guardada = guardar(pool, diretorio_de_midia, site_id, &equipe, preparada).await?;
        arquivos += preparada.processada.variantes.len();
        biblioteca.insert(preparada.original.id.clone(), guardada);
    }
    let mut trocar = |midia: &mut Midia| {
        if let Some(guardada) = biblioteca.get(&midia.id) {
            *midia = guardada.clone();
        }
    };
    trocar(&mut perfil.logo);
    cms_dados::atualizar_perfil(pool, site_id, &equipe, &perfil).await?;
    for documento in &mut documentos {
        para_cada_midia(&mut documento.conteudo, &mut trocar);
        if let Conteudo::Post(post) = &mut documento.conteudo {
            cadastrar_autor_e_categoria(pool, site_id, &equipe, post).await?;
        }
    }

    // Pelo mesmo caminho do painel: rascunho, validação do motor e publicação.
    // Cada documento vai ao ar na data em que o exemplo diz que foi publicado,
    // para o blog de demonstração não nascer com tudo no mesmo dia.
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
        arquivos,
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

        // As imagens estão na biblioteca, prontas e em uso, e os posts citam
        // autor e categoria do cadastro: a demonstração abre no editor inteira.
        let (imagens, pendentes, usos, autores, categorias): (i64, i64, i64, i64, i64) =
            sqlx::query_as(
                r#"
                select (select count(*) from midia),
                       (select count(*) from midia where situacao <> 'pronta'),
                       (select count(*) from uso_de_midia),
                       (select count(*) from autor where foto_id is not null),
                       (select count(*) from categoria)
                "#,
            )
            .fetch_one(&pool)
            .await
            .expect("contagens do cadastro");
        assert!(imagens > 0 && usos > 0, "{imagens} imagens, {usos} usos");
        assert_eq!((pendentes, autores, categorias), (0, 2, 2));
        let capa: String = sqlx::query_scalar(
            r#"
            select v.conteudo -> 'dados' -> 'capa' ->> 'id'
            from documento d join versao v on v.id = d.versao_publicada
            where d.caminho = '/blog/como-escolher-a-madeira-da-mesa'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("capa do post");
        assert!(uuid::Uuid::parse_str(&capa).is_ok(), "{capa}");

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
