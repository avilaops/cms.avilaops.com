//! Cardápio nativo: conteúdo publicado, isolamento por site e CSP.

mod comum;

use chrono::Utc;
use cms_dominio::site::Tema;
use cms_dominio::{PerfilDoSite, Situacao};
use motor_web::demonstracao;
use motor_web::tipos::{Acao, Bloco, Conteudo, Documento, Plano};
use sqlx::PgPool;

use comum::{host, pedir};

fn pagina(caminho: &str, nome: &str, preco: &str) -> Documento {
    let mut documento = demonstracao::documentos()
        .into_iter()
        .find(|d| d.caminho == "/")
        .expect("início");
    documento.caminho = caminho.to_string();
    let Conteudo::Pagina(pagina) = &mut documento.conteudo else {
        panic!("página")
    };
    pagina.slug = caminho.trim_start_matches('/').to_string();
    pagina.titulo = nome.to_string();
    pagina.menu = Some(nome.to_string());
    pagina.abertura = None;
    pagina.corpo = vec![Bloco::Planos {
        titulo: nome.to_string(),
        itens: vec![Plano {
            nome: nome.to_string(),
            preco: preco.to_string(),
            descricao: Some("Queijo e tomate".to_string()),
            acao: Acao {
                rotulo: "Consultar".to_string(),
                url: "https://wa.me/5511999990000".to_string(),
            },
            ..Plano::default()
        }],
    }];
    documento
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn cardapio_le_precos_publicados_do_proprio_site(pool: PgPool) {
    let mut perfil = PerfilDoSite::from(demonstracao::site());
    perfil.tema = Tema::Cardapio;
    perfil.organizacao.telefone = Some("(11) 99999-0000".to_string());
    perfil.paginas_do_cardapio = vec![
        "pizza /pizzas".to_string(),
        "borda /bordas".to_string(),
        "produto /bebidas".to_string(),
    ];
    let site = cms_dados::criar_site(&pool, "cantina", Situacao::Ativo, true, &perfil)
        .await
        .expect("site");
    for documento in [
        pagina("/", "Início", "R$ 0,00"),
        pagina("/pizzas", "Pizza do forno", "R$ 35,90"),
        pagina("/bordas", "Borda de queijo", "R$ 12,00"),
    ] {
        cms_dados::publicar(&pool, site, &documento, Utc::now())
            .await
            .expect("publicação");
    }
    let outro = cms_dados::criar_site(&pool, "outra-cantina", Situacao::Ativo, true, &perfil)
        .await
        .expect("outro site");
    cms_dados::publicar(
        &pool,
        outro,
        &pagina("/bebidas", "Bebida de outro site", "R$ 8,00"),
        Utc::now(),
    )
    .await
    .expect("outro conteúdo");
    let home = pedir(&pool, &host("cantina"), "/").await;
    assert!(home.corpo.contains("data-cardapio-dados"));
    assert!(home.corpo.contains("Pizza do forno"));
    assert!(home.corpo.contains("3590"));
    assert!(!home.corpo.contains("Bebida de outro site"));
    assert!(home.corpo.contains("data-cardapio-alternativa"));
    assert!(
        home.cabecalho("content-security-policy")
            .contains("script-src 'self';")
    );
    let script = home
        .corpo
        .split("<script type=\"module\" src=\"")
        .nth(1)
        .expect("script")
        .split('"')
        .next()
        .expect("caminho");
    let recurso = pedir(&pool, &host("cantina"), script).await;
    assert!(
        recurso
            .cabecalho("content-type")
            .starts_with("text/javascript")
    );
    assert!(recurso.cabecalho("cache-control").contains("immutable"));
    assert!(!recurso.corpo.contains("fetch('./menu.json')"));
    // A publicação seguinte aparece sem reconstruir o site.
    cms_dados::publicar(
        &pool,
        site,
        &pagina("/pizzas", "Pizza do forno", "R$ 38,50"),
        Utc::now(),
    )
    .await
    .expect("preço novo");
    let atualizada = pedir(&pool, &host("cantina"), "/pizzas").await;
    assert!(atualizada.corpo.contains("3850"));
    assert!(!atualizada.corpo.contains("3590"));
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn preco_sob_consulta_mantem_pagina_estatica_sem_subtotal_inventado(pool: PgPool) {
    let mut perfil = PerfilDoSite::from(demonstracao::site());
    perfil.tema = Tema::Cardapio;
    perfil.organizacao.telefone = Some("(11) 99999-0000".to_string());
    perfil.paginas_do_cardapio = vec!["pizza /pizzas".to_string()];
    let site = cms_dados::criar_site(&pool, "restaurante", Situacao::Ativo, true, &perfil)
        .await
        .expect("site");
    cms_dados::publicar(
        &pool,
        site,
        &pagina("/pizzas", "Pizza especial", "Sob consulta"),
        Utc::now(),
    )
    .await
    .expect("publicação");
    let resposta = pedir(&pool, &host("restaurante"), "/pizzas").await;
    assert!(resposta.corpo.contains("Sob consulta"));
    assert!(!resposta.corpo.contains("data-cardapio-dados"));
    assert!(
        resposta
            .cabecalho("content-security-policy")
            .contains("script-src 'none'")
    );
}
