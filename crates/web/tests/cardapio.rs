//! Tema optativo: os dados vêm das versões publicadas do próprio site.
mod comum;

use chrono::Utc;
use cms_dominio::site::Tema;
use cms_dominio::{PerfilDoSite, Situacao};
use comum::{host, pedir};
use motor_web::demonstracao;
use motor_web::tipos::{Bloco, Conteudo, Documento};
use sqlx::PgPool;

fn pagina(caminho: &str, preco: &str) -> Documento {
    let mut documento = demonstracao::documentos()
        .into_iter()
        .find(|d| d.caminho == "/")
        .expect("home do exemplo");
    documento.caminho = caminho.to_string();
    if let Conteudo::Pagina(pagina) = &mut documento.conteudo {
        pagina.slug = caminho.trim_start_matches('/').to_string();
        pagina.titulo = if caminho == "/" {
            "Sua noite começa aqui"
        } else {
            "Pizzas"
        }
        .to_string();
        pagina.menu = Some("Pizzas".to_string());
        pagina.corpo = serde_json::from_value(serde_json::json!([{
            "tipo": "planos",
            "titulo": "Sabores do exemplo",
            "itens": [{
                "nome": "Pizza de exemplo",
                "preco": preco,
                "descricao": "Queijo e tomate",
                "acao": {"rotulo": "Consultar", "url": "https://wa.me/5511999990000"}
            }]
        }]))
        .expect("planos válidos");
    }
    documento
}

async fn semear(pool: &PgPool, slug: &str, tema: Tema, preco: &str) -> uuid::Uuid {
    let mut perfil = PerfilDoSite::from(demonstracao::site());
    perfil.tema = tema;
    perfil.organizacao.telefone = Some("(11) 99999-0000".to_string());
    perfil.paginas_do_cardapio = vec!["pizza /pizzas".to_string()];
    let id = cms_dados::criar_site(pool, slug, Situacao::Ativo, true, &perfil)
        .await
        .expect("site criado");
    for documento in [pagina("/", preco), pagina("/pizzas", preco)] {
        cms_dados::publicar(pool, id, &documento, Utc::now())
            .await
            .expect("publicado");
    }
    id
}

fn dados_do_menu(html: &str) -> serde_json::Value {
    let atributo = html
        .split("data-menu=\"")
        .nth(1)
        .expect("menu")
        .split('"')
        .next()
        .expect("atributo");
    let json = atributo
        .replace("&#34;", "\"")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    serde_json::from_str(&json).expect("JSON do menu")
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn cardapio_usa_preco_atual_e_script_do_cms(pool: PgPool) {
    let id = semear(&pool, "restaurante", Tema::Cardapio, "R$ 35,90").await;
    semear(&pool, "outro", Tema::Cardapio, "R$ 99,00").await;
    let resposta = pedir(&pool, &host("restaurante"), "/").await;
    assert_eq!(resposta.status, axum::http::StatusCode::OK);
    assert!(
        resposta
            .cabecalho("content-security-policy")
            .contains("script-src 'self';")
    );
    assert!(resposta.corpo.contains("data-cardapio-dados"));
    assert_eq!(dados_do_menu(&resposta.corpo)[0]["items"][0]["price"], 3590);
    assert_eq!(resposta.corpo.matches("<h1").count(), 1);
    let script = resposta
        .corpo
        .split("<script src=\"")
        .last()
        .expect("script")
        .split('"')
        .next()
        .expect("endereço");
    assert!(script.starts_with("/recursos/cardapio-"));
    let recurso = pedir(&pool, &host("restaurante"), script).await;
    assert_eq!(recurso.status, axum::http::StatusCode::OK);
    assert!(
        recurso
            .cabecalho("content-type")
            .starts_with("text/javascript")
    );
    assert!(recurso.cabecalho("cache-control").contains("immutable"));
    assert!(recurso.corpo.contains("function renderProducts"));
    assert!(!recurso.corpo.contains("export const"));
    if let Ok(destino) = std::env::var("CMS_CARDAPIO_PREVIEW") {
        let pasta = std::path::Path::new(&destino);
        let recurso_destino = pasta.join(script.trim_start_matches('/'));
        std::fs::create_dir_all(recurso_destino.parent().expect("pasta")).expect("criar pasta");
        std::fs::write(pasta.join("index.html"), &resposta.corpo).expect("salvar HTML");
        std::fs::write(recurso_destino, &recurso.corpo).expect("salvar script");
    }
    let alterado = pagina("/pizzas", "R$ 42,00");
    cms_dados::publicar(&pool, id, &alterado, Utc::now())
        .await
        .expect("preço publicado");
    let atual = pedir(&pool, &host("restaurante"), "/").await;
    assert_eq!(dados_do_menu(&atual.corpo)[0]["items"][0]["price"], 4200);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn tema_comum_e_preco_ambiguo_preservam_conteudo_sem_interacao(pool: PgPool) {
    semear(&pool, "comum", Tema::Classico, "R$ 35,90").await;
    semear(&pool, "consulta", Tema::Cardapio, "Sob consulta").await;
    for slug in ["comum", "consulta"] {
        let resposta = pedir(&pool, &host(slug), "/pizzas").await;
        assert!(!resposta.corpo.contains("data-cardapio-dados"), "{slug}");
        assert!(resposta.corpo.contains("Pizza de exemplo"), "{slug}");
        assert!(
            resposta
                .cabecalho("content-security-policy")
                .contains("script-src 'none'"),
            "{slug}"
        );
    }
}

#[test]
fn identidade_antiga_continua_sem_cardapio() {
    let perfil = PerfilDoSite::from(demonstracao::site());
    let mut json = serde_json::to_value(&perfil).expect("perfil");
    json.as_object_mut()
        .expect("objeto")
        .remove("paginasDoCardapio");
    let antigo: PerfilDoSite = serde_json::from_value(json).expect("perfil antigo");
    assert!(antigo.paginas_do_cardapio.is_empty());
    // Garante também que o fixture usado pelo teste tem os blocos previstos.
    assert!(matches!(
        pagina("/pizzas", "R$ 1,00").corpo().first(),
        Some(Bloco::Planos { .. })
    ));
}
