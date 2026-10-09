//! A rotina `eventos.entregar`: leva ao n8n o que está na fila.
//!
//! Nada no caminho de quem edita ou de quem visita espera por ela. Com o n8n
//! fora do ar, os eventos ficam pendentes e voltam na próxima tentativa.

use std::time::Duration;

use cms_dados::{ErroDeDados, EventoAEntregar};
use cms_dominio::Situacao;
use cms_dominio::eventos::{CorpoDoEvento, SiteDoEvento, origem_do_site};
use cms_dominio::site::aparece_na_busca;
use cms_integracoes::ClienteN8n;
use cms_web::Configuracao;
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;

const CADENCIA: Duration = Duration::from_secs(60);
const EVENTOS_POR_RODADA: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rodada {
    pub entregues: usize,
    pub falhas: usize,
}

fn corpo(evento: EventoAEntregar, configuracao: &Configuracao) -> CorpoDoEvento {
    let origem = origem_do_site(
        &configuracao.esquema,
        &configuracao.dominio_base,
        &evento.site_slug,
        evento.dominio_ativo.as_deref(),
    );
    // Situação que o servidor não conhece não põe site na busca.
    let na_busca = Situacao::try_from(evento.site_situacao.as_str()).is_ok_and(|situacao| {
        aparece_na_busca(
            situacao,
            evento.dominio_ativo.is_some(),
            evento.site_provisorio_definitivo,
        )
    });
    CorpoDoEvento {
        id: evento.id,
        tipo: evento.tipo,
        chave: evento.chave,
        ocorrido_em: evento.ocorrido_em,
        site: SiteDoEvento {
            id: evento.site_id,
            slug: evento.site_slug,
            nome: evento.site_nome,
            origem,
            na_busca,
            indexnow_chave: configuracao.chave_do_indexnow.clone(),
        },
        dados: evento.dados,
    }
}

/// Uma rodada: reivindica os pendentes e entrega um a um. O que falha fica
/// pendente, com a próxima tentativa já marcada pela reivindicação.
pub async fn entregar_pendentes(
    pool: &PgPool,
    cliente: &ClienteN8n,
    configuracao: &Configuracao,
) -> Result<Rodada, ErroDeDados> {
    let mut rodada = Rodada {
        entregues: 0,
        falhas: 0,
    };
    for evento in cms_dados::reivindicar_eventos(pool, EVENTOS_POR_RODADA).await? {
        let corpo = corpo(evento, configuracao);
        match cliente.entregar(&corpo).await {
            Ok(()) => {
                cms_dados::marcar_evento_entregue(pool, corpo.id).await?;
                rodada.entregues += 1;
            }
            Err(erro) => {
                tracing::warn!(%erro, evento = %corpo.id, tipo = %corpo.tipo, "evento não entregue");
                rodada.falhas += 1;
            }
        }
    }
    Ok(rodada)
}

/// Dispara a rotina a cada minuto, enquanto o servidor estiver no ar.
pub fn agendar(pool: PgPool, cliente: ClienteN8n, configuracao: Configuracao) {
    tokio::spawn(async move {
        let mut relogio = tokio::time::interval(CADENCIA);
        relogio.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            relogio.tick().await;
            match entregar_pendentes(&pool, &cliente, &configuracao).await {
                Ok(rodada) if rodada.entregues + rodada.falhas > 0 => {
                    tracing::info!(
                        entregues = rodada.entregues,
                        falhas = rodada.falhas,
                        "eventos.entregar"
                    );
                }
                Ok(_) => {}
                Err(erro) => tracing::error!(%erro, "eventos.entregar falhou"),
            }
        }
    });
}

#[cfg(test)]
mod testes {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use axum::Json;
    use axum::Router;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use chrono::Utc;
    use cms_dados::fluxo;
    use cms_dominio::{Ator, PerfilDoSite, Situacao};
    use motor_web::demonstracao;
    use serde_json::Value;

    use super::*;

    type Recebidos = Arc<Mutex<Vec<(String, Value)>>>;

    fn configuracao() -> Configuracao {
        Configuracao {
            dominio_base: "sites.teste".into(),
            esquema: "https".into(),
            diretorio_de_midia: PathBuf::from("midia-que-nao-existe"),
            token_do_n8n: None,
            chave_do_indexnow: Some("0123456789abcdef".into()),
            host_do_painel: None,
            limites_de_criacao: Default::default(),
            limite_de_midia_por_site: 0,
        }
    }

    async fn receber(
        State(recebidos): State<Recebidos>,
        cabecalhos: HeaderMap,
        Json(corpo): Json<Value>,
    ) -> StatusCode {
        let autorizacao = cabecalhos
            .get("authorization")
            .and_then(|valor| valor.to_str().ok())
            .unwrap_or("")
            .to_string();
        recebidos
            .lock()
            .expect("lista de recebidos")
            .push((autorizacao, corpo));
        StatusCode::OK
    }

    /// Um n8n de mentira em uma porta livre. Devolve o endereço do webhook.
    async fn subir_n8n(recebidos: Recebidos) -> String {
        let escuta = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("porta livre");
        let endereco = escuta.local_addr().expect("endereço");
        let rotas = Router::new()
            .route("/webhook/cms-eventos", post(receber))
            .with_state(recebidos);
        tokio::spawn(async move {
            axum::serve(escuta, rotas).await.expect("n8n de teste");
        });
        format!("http://{endereco}/webhook/cms-eventos")
    }

    /// Publica a página "Sobre" do exemplo: um evento na fila.
    async fn publicar_uma_pagina(pool: &PgPool) {
        let perfil = PerfilDoSite::from(demonstracao::site());
        let site_id = cms_dados::criar_site(pool, "oficina", Situacao::Ativo, true, &perfil)
            .await
            .expect("site criado");
        let conteudo = demonstracao::documentos()
            .into_iter()
            .find(|documento| documento.caminho == "/sobre")
            .expect("o exemplo tem /sobre")
            .conteudo;
        let equipe = Ator::da_equipe("suporte");
        let salvo = fluxo::salvar_rascunho(pool, site_id, &equipe, None, &conteudo)
            .await
            .expect("rascunho salvo");
        fluxo::publicar(pool, site_id, &equipe, salvo.documento_id, Utc::now())
            .await
            .expect("publicado");
    }

    async fn fila(pool: &PgPool) -> Vec<(String, i32)> {
        sqlx::query_as("select situacao, tentativas from evento order by ocorrido_em")
            .fetch_all(pool)
            .await
            .expect("fila de eventos")
    }

    #[sqlx::test(migrator = "cms_dados::MIGRADOR")]
    async fn com_o_n8n_fora_do_ar_o_site_publica_e_o_evento_espera(pool: PgPool) {
        // Publicar não depende do n8n: já aconteceu quando a rotina roda.
        publicar_uma_pagina(&pool).await;

        // Porta 9 (discard) em 127.0.0.1: ninguém escuta.
        let cliente = ClienteN8n::novo("http://127.0.0.1:9/webhook/cms-eventos", "Bearer x")
            .expect("cliente");
        let rodada = entregar_pendentes(&pool, &cliente, &configuracao())
            .await
            .expect("a rodada não falha por causa do n8n");
        assert_eq!(
            rodada,
            Rodada {
                entregues: 0,
                falhas: 1
            }
        );
        assert_eq!(fila(&pool).await, [("pendente".to_string(), 1)]);

        // A próxima tentativa ficou para depois: a rodada seguinte não insiste.
        let rodada = entregar_pendentes(&pool, &cliente, &configuracao())
            .await
            .expect("rodada");
        assert_eq!(rodada.entregues + rodada.falhas, 0);
    }

    #[sqlx::test(migrator = "cms_dados::MIGRADOR")]
    async fn entrega_o_corpo_com_autorizacao_e_marca_como_entregue(pool: PgPool) {
        publicar_uma_pagina(&pool).await;
        let recebidos = Recebidos::default();
        let url = subir_n8n(recebidos.clone()).await;
        let cliente = ClienteN8n::novo(url, "Bearer segredo-de-saida").expect("cliente");

        let rodada = entregar_pendentes(&pool, &cliente, &configuracao())
            .await
            .expect("rodada");
        assert_eq!(
            rodada,
            Rodada {
                entregues: 1,
                falhas: 0
            }
        );
        assert_eq!(fila(&pool).await, [("entregue".to_string(), 1)]);

        let recebidos = recebidos.lock().expect("lista de recebidos").clone();
        assert_eq!(recebidos.len(), 1);
        let (autorizacao, corpo) = &recebidos[0];
        assert_eq!(autorizacao, "Bearer segredo-de-saida");
        assert_eq!(corpo["tipo"], "conteudo.publicado");
        assert_eq!(corpo["site"]["slug"], "oficina");
        assert_eq!(corpo["site"]["origem"], "https://oficina.sites.teste");
        assert_eq!(corpo["site"]["naBusca"], true);
        assert_eq!(corpo["site"]["indexnowChave"], "0123456789abcdef");
        assert_eq!(corpo["dados"]["caminho"], "/sobre");
        assert_eq!(corpo["dados"]["especie"], "pagina");
        assert!(
            corpo["chave"]
                .as_str()
                .is_some_and(|c| c.starts_with("historico:"))
        );

        // Entregue não é reenviado.
        let rodada = entregar_pendentes(&pool, &cliente, &configuracao())
            .await
            .expect("rodada");
        assert_eq!(rodada.entregues + rodada.falhas, 0);
    }
}
