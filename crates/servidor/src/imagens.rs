//! A rotina `tarefas.imagens`: gera as variantes das imagens enviadas.
//!
//! O envio só grava o original. Aqui ele vira AVIF e WebP nas larguras do
//! motor, fora de qualquer pedido, e a imagem fica pronta para ir ao ar.

use std::path::{Path, PathBuf};
use std::time::Duration;

use cms_dados::{ErroDeDados, MidiaAProcessar, VarianteGravada};
use cms_web::caminho_do_original;
use motor_web::imagem::{ImagemProcessada, processar_imagem};
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;

const CADENCIA: Duration = Duration::from_secs(60);
/// O codificador AVIF leva segundos por imagem: a rodada tem teto.
const IMAGENS_POR_RODADA: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rodada {
    pub prontas: usize,
    pub falhas: usize,
}

#[derive(Debug, thiserror::Error)]
enum Falha {
    #[error("original ilegível no disco: {0}")]
    Arquivo(#[from] std::io::Error),
    #[error("imagem recusada pelo motor: {0}")]
    Recusada(String),
    #[error("o processamento foi interrompido")]
    Interrompido,
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
}

/// Lê o original, gera as variantes fora das threads que atendem pedidos e
/// grava cada uma na pasta do site.
async fn gerar(pool: &PgPool, diretorio: &Path, midia: &MidiaAProcessar) -> Result<(), Falha> {
    let original = tokio::fs::read(caminho_do_original(diretorio, midia.site_id, midia.id)).await?;
    let nome = midia.nome.clone();
    let processada: ImagemProcessada =
        tokio::task::spawn_blocking(move || processar_imagem(&nome, &original))
            .await
            .map_err(|_| Falha::Interrompido)?
            .map_err(|problemas| {
                let codigos: Vec<&str> = problemas.iter().map(|p| p.codigo).collect();
                Falha::Recusada(codigos.join(", "))
            })?;

    let pasta = diretorio.join(midia.site_id.to_string());
    let mut gravadas = Vec::with_capacity(processada.variantes.len());
    for variante in &processada.variantes {
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
        midia.id,
        processada.largura,
        processada.altura,
        &gravadas,
    )
    .await?;
    Ok(())
}

/// Uma rodada: processa as imagens pendentes, uma de cada vez. A que falha
/// volta para a fila até a terceira tentativa.
pub async fn processar_pendentes(pool: &PgPool, diretorio: &Path) -> Result<Rodada, ErroDeDados> {
    let mut rodada = Rodada::default();
    while rodada.prontas + rodada.falhas < IMAGENS_POR_RODADA {
        let Some(midia) = cms_dados::reivindicar_midia(pool).await? else {
            break;
        };
        match gerar(pool, diretorio, &midia).await {
            Ok(()) => rodada.prontas += 1,
            Err(erro) => {
                tracing::warn!(%erro, midia = %midia.id, "variantes não geradas");
                cms_dados::falhar_midia(pool, midia.id).await?;
                rodada.falhas += 1;
            }
        }
    }
    Ok(rodada)
}

/// Dispara a rotina a cada minuto, enquanto o servidor estiver no ar.
pub fn agendar(pool: PgPool, diretorio: PathBuf) {
    tokio::spawn(async move {
        let mut relogio = tokio::time::interval(CADENCIA);
        relogio.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            relogio.tick().await;
            match processar_pendentes(&pool, &diretorio).await {
                Ok(rodada) if rodada.prontas + rodada.falhas > 0 => {
                    tracing::info!(
                        prontas = rodada.prontas,
                        falhas = rodada.falhas,
                        "tarefas.imagens"
                    );
                }
                Ok(_) => {}
                Err(erro) => tracing::error!(%erro, "tarefas.imagens falhou"),
            }
        }
    });
}

#[cfg(test)]
mod testes {
    use std::io::Cursor;

    use cms_dados::NovaMidia;
    use cms_dominio::{Ator, PerfilDoSite, Situacao};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use motor_web::demonstracao;
    use uuid::Uuid;

    use super::*;

    fn foto(largura: u32, altura: u32) -> Vec<u8> {
        let imagem = RgbImage::from_fn(largura, altura, |x, y| {
            Rgb([(x % 255) as u8, (y % 255) as u8, 120])
        });
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(imagem)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("jpeg desenhado");
        bytes
    }

    struct Cenario {
        site_id: Uuid,
        diretorio: PathBuf,
    }

    async fn cenario(pool: &PgPool) -> Cenario {
        let perfil = PerfilDoSite::from(demonstracao::site());
        let site_id = cms_dados::criar_site(pool, "oficina", Situacao::Ativo, true, &perfil)
            .await
            .expect("site criado");
        let diretorio = std::env::temp_dir().join(format!("cms-imagens-{}", Uuid::new_v4()));
        Cenario { site_id, diretorio }
    }

    /// Registra a imagem e grava o original, como o envio pelo painel faz.
    async fn enviar(pool: &PgPool, cenario: &Cenario, nome: &str, bytes: &[u8]) -> Uuid {
        let nova = NovaMidia {
            nome: nome.into(),
            hash: cms_dados::hash_de_conteudo(bytes),
            largura: 1300,
            altura: 700,
            alt: "Bancada da oficina com ferramentas".into(),
            legenda: None,
            credito: None,
            direitos: Default::default(),
            bytes: bytes.len() as u64,
        };
        let id = cms_dados::registrar_midia(
            pool,
            cenario.site_id,
            &Ator::da_equipe("suporte"),
            nova,
            u64::MAX / 4,
        )
        .await
        .expect("imagem registrada");
        let destino = caminho_do_original(&cenario.diretorio, cenario.site_id, id);
        std::fs::create_dir_all(destino.parent().expect("pasta")).expect("pasta criada");
        std::fs::write(destino, bytes).expect("original gravado");
        id
    }

    async fn situacao(pool: &PgPool, id: Uuid) -> String {
        sqlx::query_scalar("select situacao from midia where id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("situação da imagem")
    }

    #[sqlx::test(migrator = "cms_dados::MIGRADOR")]
    async fn gera_as_variantes_e_deixa_a_imagem_pronta(pool: PgPool) {
        let cenario = cenario(&pool).await;
        let id = enviar(&pool, &cenario, "Bancada da Oficina.jpg", &foto(1300, 700)).await;
        assert_eq!(situacao(&pool, id).await, "pendente");
        // Sem variante, a imagem ainda não tem o que mostrar.
        let antes = cms_dados::midia_para_conteudo(&pool, cenario.site_id, id)
            .await
            .expect("consulta")
            .expect("imagem do site");
        assert!(antes.variantes.is_empty());

        let rodada = processar_pendentes(&pool, &cenario.diretorio)
            .await
            .expect("rodada");
        assert_eq!(
            rodada,
            Rodada {
                prontas: 1,
                falhas: 0
            }
        );
        assert_eq!(situacao(&pool, id).await, "pronta");

        let pronta = cms_dados::midia_para_conteudo(&pool, cenario.site_id, id)
            .await
            .expect("consulta")
            .expect("imagem do site");
        assert_eq!((pronta.largura, pronta.altura), (1300, 700));
        assert!(pronta.variantes.len() >= 4, "{:?}", pronta.variantes);
        let pasta = cenario.diretorio.join(cenario.site_id.to_string());
        for variante in &pronta.variantes {
            let arquivo = variante
                .url
                .strip_prefix("/midia/")
                .expect("endereço servido pelo site");
            assert!(arquivo.starts_with("bancada-da-oficina-"), "{arquivo}");
            assert!(pasta.join(arquivo).is_file(), "{arquivo}");
        }

        // Pronta, não volta para a fila.
        let rodada = processar_pendentes(&pool, &cenario.diretorio)
            .await
            .expect("rodada");
        assert_eq!(rodada, Rodada::default());
        std::fs::remove_dir_all(&cenario.diretorio).expect("pasta removida");
    }

    #[sqlx::test(migrator = "cms_dados::MIGRADOR")]
    async fn imagem_que_nao_abre_falha_na_terceira_tentativa(pool: PgPool) {
        let cenario = cenario(&pool).await;
        // Começa como JPEG e não é uma imagem.
        let id = enviar(
            &pool,
            &cenario,
            "quebrada.jpg",
            &[0xFF, 0xD8, 0xFF, 0, 1, 2, 3],
        )
        .await;

        for tentativa in 1..=3 {
            let rodada = processar_pendentes(&pool, &cenario.diretorio)
                .await
                .expect("rodada");
            assert_eq!(rodada.falhas, 1, "tentativa {tentativa}");
            let esperada = if tentativa < 3 { "pendente" } else { "falhou" };
            assert_eq!(situacao(&pool, id).await, esperada, "tentativa {tentativa}");
            sqlx::query("update midia set proxima_tentativa_em = now() where id = $1")
                .bind(id)
                .execute(&pool)
                .await
                .expect("fila adiantada");
        }
        let rodada = processar_pendentes(&pool, &cenario.diretorio)
            .await
            .expect("rodada");
        assert_eq!(rodada, Rodada::default());
        std::fs::remove_dir_all(&cenario.diretorio).expect("pasta removida");
    }
}
