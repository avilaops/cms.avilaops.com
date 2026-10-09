//! Os eventos que o fluxo grava e a rota com que o n8n os encerra.

mod comum;

use axum::http::StatusCode;
use chrono::Utc;
use cms_dados::fluxo;
use cms_dominio::{Ator, Papel, PerfilDoSite, Situacao};
use motor_web::demonstracao;
use motor_web::tipos::Conteudo;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use comum::{TOKEN_DO_N8N, postar};

fn exemplo(caminho: &str) -> Conteudo {
    demonstracao::documentos()
        .into_iter()
        .find(|documento| documento.caminho == caminho)
        .unwrap_or_else(|| panic!("o exemplo não tem {caminho}"))
        .conteudo
}

fn editor() -> Ator {
    Ator::do_site("editor", Papel::Editor)
}

async fn criar_site(pool: &PgPool) -> Uuid {
    let perfil = PerfilDoSite::from(demonstracao::site());
    cms_dados::criar_site(pool, "oficina", Situacao::Ativo, true, &perfil)
        .await
        .expect("site criado")
}

/// Tipo, situação e dados de cada evento, na ordem em que aconteceram.
async fn eventos(pool: &PgPool) -> Vec<(String, String, Value)> {
    sqlx::query_as("select tipo, situacao, dados from evento order by ocorrido_em, chave")
        .fetch_all(pool)
        .await
        .expect("eventos")
}

/// Publica a página de contato e devolve o identificador do evento gerado.
async fn evento_de_publicacao(pool: &PgPool) -> Uuid {
    let site_id = criar_site(pool).await;
    let salvo = fluxo::salvar_rascunho(pool, site_id, &editor(), None, &exemplo("/contato"))
        .await
        .expect("rascunho salvo");
    fluxo::publicar(pool, site_id, &editor(), salvo.documento_id, Utc::now())
        .await
        .expect("publicado");
    sqlx::query_scalar("select id from evento")
        .fetch_one(pool)
        .await
        .expect("um evento na fila")
}

fn rota(id: Uuid) -> String {
    format!("/api/admin/eventos/{id}/encerrar")
}

fn portador() -> String {
    format!("Bearer {TOKEN_DO_N8N}")
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn cada_fato_do_fluxo_grava_um_evento_com_dados_escolhidos(pool: PgPool) {
    let site_id = criar_site(&pool).await;
    let autora = Ator::do_site("helena", Papel::Autor);
    let mut conteudo = exemplo("/contato");

    // Salvar rascunho não é fato para automação.
    let id = fluxo::salvar_rascunho(&pool, site_id, &autora, None, &conteudo)
        .await
        .expect("rascunho salvo")
        .documento_id;
    assert!(eventos(&pool).await.is_empty());

    fluxo::enviar_para_revisao(&pool, site_id, &autora, id)
        .await
        .expect("revisão pedida");
    fluxo::publicar(&pool, site_id, &editor(), id, Utc::now())
        .await
        .expect("publicado");
    if let Conteudo::Pagina(pagina) = &mut conteudo {
        pagina.slug = "fale-conosco".into();
    }
    fluxo::salvar_rascunho(&pool, site_id, &editor(), Some(id), &conteudo)
        .await
        .expect("rascunho salvo");
    fluxo::publicar(&pool, site_id, &editor(), id, Utc::now())
        .await
        .expect("publicado no endereço novo");
    fluxo::despublicar(&pool, site_id, &editor(), id)
        .await
        .expect("despublicado");

    let titulo = match &conteudo {
        Conteudo::Pagina(pagina) => pagina.titulo.clone(),
        _ => panic!("o exemplo é uma página"),
    };
    let documento = id.to_string();
    let esperado = [
        (
            "conteudo.enviado_para_revisao",
            json!({ "documentoId": documento, "especie": "pagina", "titulo": titulo, "pedidoPor": "helena" }),
        ),
        (
            "conteudo.publicado",
            json!({ "documentoId": documento, "especie": "pagina", "caminho": "/contato", "titulo": titulo }),
        ),
        (
            "conteudo.publicado",
            json!({
                "documentoId": documento,
                "especie": "pagina",
                "caminho": "/fale-conosco",
                "titulo": titulo,
                "caminhoAnterior": "/contato"
            }),
        ),
        (
            "conteudo.despublicado",
            json!({ "documentoId": documento, "especie": "pagina", "caminho": "/fale-conosco" }),
        ),
    ];
    let gravados = eventos(&pool).await;
    assert_eq!(gravados.len(), esperado.len(), "{gravados:?}");
    for ((tipo, situacao, dados), (tipo_esperado, dados_esperados)) in gravados.iter().zip(esperado)
    {
        assert_eq!(tipo, tipo_esperado);
        assert_eq!(situacao, "pendente");
        assert_eq!(dados, &dados_esperados, "{tipo}");
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn acao_recusada_nao_grava_evento(pool: PgPool) {
    let site_id = criar_site(&pool).await;
    let mut conteudo = exemplo("/contato");
    if let Conteudo::Pagina(pagina) = &mut conteudo {
        pagina.seo.titulo = String::new();
    }
    let id = fluxo::salvar_rascunho(&pool, site_id, &editor(), None, &conteudo)
        .await
        .expect("rascunho salvo")
        .documento_id;
    assert!(
        fluxo::publicar(&pool, site_id, &editor(), id, Utc::now())
            .await
            .is_err()
    );
    assert!(eventos(&pool).await.is_empty());
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn so_o_token_do_n8n_encerra_evento(pool: PgPool) {
    let id = evento_de_publicacao(&pool).await;
    let corpo = r#"{"resultado":{"indexnow":202}}"#;

    for autorizacao in [
        None,
        Some("Bearer token-errado"),
        Some(TOKEN_DO_N8N),
        Some("Bearer "),
    ] {
        let resposta = postar(&pool, &rota(id), autorizacao, corpo).await;
        assert_eq!(resposta.status, StatusCode::UNAUTHORIZED, "{autorizacao:?}");
    }
    assert_eq!(eventos(&pool).await[0].1, "pendente");
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn encerrar_grava_o_resultado_uma_vez_so(pool: PgPool) {
    let id = evento_de_publicacao(&pool).await;
    let portador = portador();

    let invalido = postar(&pool, &rota(id), Some(&portador), r#"{"outro":1}"#).await;
    assert_eq!(invalido.status, StatusCode::BAD_REQUEST);
    let inexistente = postar(
        &pool,
        &rota(Uuid::new_v4()),
        Some(&portador),
        r#"{"resultado":{}}"#,
    )
    .await;
    assert_eq!(inexistente.status, StatusCode::NOT_FOUND);

    let primeiro = postar(
        &pool,
        &rota(id),
        Some(&portador),
        r#"{"resultado":{"indexnow":202}}"#,
    )
    .await;
    assert_eq!(primeiro.status, StatusCode::OK);
    let repetido = postar(
        &pool,
        &rota(id),
        Some(&portador),
        r#"{"resultado":{"indexnow":500}}"#,
    )
    .await;
    assert_eq!(repetido.status, StatusCode::CONFLICT);

    let (situacao, resultado): (String, Value) =
        sqlx::query_as("select situacao, resultado from evento where id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("evento");
    assert_eq!(situacao, "encerrado");
    assert_eq!(resultado, json!({ "indexnow": 202 }));

    // Evento encerrado não volta para a fila de entrega.
    assert!(
        cms_dados::reivindicar_eventos(&pool, 10)
            .await
            .expect("fila")
            .is_empty()
    );
}
