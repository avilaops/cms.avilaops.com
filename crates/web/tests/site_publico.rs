//! O site público contra um Postgres de verdade. Cada teste ganha um banco
//! próprio, já migrado, e fala com o roteador como um navegador falaria.

mod comum;

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use cms_dominio::{PerfilDoSite, Situacao};
use cms_web::roteador;
use motor_web::demonstracao;
use motor_web::tipos::{Conteudo, Documento};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use comum::{CHAVE_DO_INDEXNOW, estado, host, pedir, pedir_em};

fn documentos_do_exemplo() -> Vec<Documento> {
    demonstracao::documentos()
        .into_iter()
        .filter(|d| !matches!(d.conteudo, Conteudo::Produto(_)))
        .collect()
}

/// Um site com o conteúdo de demonstração publicado.
async fn semear(
    pool: &PgPool,
    slug: &str,
    situacao: Situacao,
    provisorio_definitivo: bool,
) -> Uuid {
    let perfil = PerfilDoSite::from(demonstracao::site());
    let site_id = cms_dados::criar_site(pool, slug, situacao, provisorio_definitivo, &perfil)
        .await
        .expect("site criado");
    for documento in documentos_do_exemplo() {
        cms_dados::publicar(pool, site_id, &documento, Utc::now())
            .await
            .expect("documento publicado");
    }
    site_id
}

const POST: &str = "/blog/como-escolher-a-madeira-da-mesa";

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn saude_responde_com_o_banco_no_ar(pool: PgPool) {
    let resposta = pedir(&pool, "qualquer.host", "/api/saude").await;
    assert_eq!(resposta.status, StatusCode::OK);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn host_desconhecido_ou_invalido_nao_acha_site(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    for host in [
        "outro.sites.teste",
        "oficina.com.br",
        "sites.teste",
        "a b",
        "oficina.sites.teste.golpe.com",
    ] {
        let resposta = pedir(&pool, host, "/").await;
        assert_eq!(resposta.status, StatusCode::NOT_FOUND, "{host}");
        assert!(
            !resposta.corpo.contains("Oficina Exemplo"),
            "{host} vazou conteúdo"
        );
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn home_sai_com_cabecalho_e_dados_estruturados_do_motor(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let resposta = pedir(&pool, &host("oficina"), "/").await;

    assert_eq!(resposta.status, StatusCode::OK);
    assert!(resposta.cabecalho("content-type").starts_with("text/html"));
    // O navegador não pede um favicon que o site não tem.
    assert!(resposta.corpo.contains(r#"<link rel="icon" href="data:,">"#));
    assert_eq!(resposta.corpo.matches("<h1").count(), 1);
    assert!(
        resposta
            .corpo
            .contains(r#"<link rel="canonical" href="https://oficina.sites.teste/">"#)
    );
    assert!(resposta.corpo.contains(r#""@type":"Organization""#));
    assert!(
        resposta
            .corpo
            .contains(r#"<meta name="robots" content="index, follow">"#)
    );
    assert_eq!(resposta.cabecalho("x-robots-tag"), "");
    assert!(
        resposta
            .cabecalho("content-security-policy")
            .contains("script-src 'none'")
    );
    assert!(resposta.cabecalho("cache-control").contains("s-maxage=60"));
    // Navegação: início, as páginas e o blog. Post não entra no menu.
    assert!(
        resposta
            .corpo
            .contains(r#"<a href="/sobre">Sobre a oficina</a>"#)
    );
    assert!(resposta.corpo.contains(r#"<a href="/blog">Blog</a>"#));
    assert!(!resposta.corpo.contains(&format!(r#"<li><a href="{POST}""#)));
    // Rodapé com quem está por trás do site.
    assert!(resposta.corpo.contains("Oficina Exemplo Marcenaria Ltda."));
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn post_mostra_datas_autor_e_uma_imagem_com_prioridade(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let resposta = pedir(&pool, &host("oficina"), POST).await;

    assert_eq!(resposta.status, StatusCode::OK);
    assert_eq!(resposta.corpo.matches("<h1").count(), 1);
    assert!(resposta.corpo.contains(r#""@type":"Article""#));
    assert!(
        resposta
            .corpo
            .contains(r#"<time datetime="2026-08-12">12 de agosto de 2026</time>"#)
    );
    assert!(
        resposta
            .corpo
            .contains(r#"Atualizado em <time datetime="2026-09-03">"#)
    );
    assert!(resposta.corpo.contains("Helena Prado"));
    assert!(resposta.corpo.contains("Técnica em Design de Móveis"));
    assert_eq!(resposta.corpo.matches(r#"fetchpriority="high""#).count(), 1);
    assert!(
        resposta
            .corpo
            .contains(r#"aria-current="page">Como escolher a madeira da mesa de jantar</span>"#)
    );
    for imagem in resposta.corpo.split("<img ").skip(1) {
        let tag = imagem.split('>').next().unwrap_or("");
        assert!(
            tag.contains("width=\"") && tag.contains("height=\"") && tag.contains("alt=\""),
            "{tag}"
        );
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn caminho_que_nao_existe_e_404_fora_da_busca(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let resposta = pedir(&pool, &host("oficina"), "/nao-existe").await;
    assert_eq!(resposta.status, StatusCode::NOT_FOUND);
    assert_eq!(resposta.cabecalho("x-robots-tag"), "noindex");
    assert!(resposta.corpo.contains("Página não encontrada"));
    assert_eq!(resposta.cabecalho("cache-control"), "");
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn barra_final_redireciona_para_o_endereco_unico(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let resposta = pedir(&pool, &host("oficina"), "/sobre/").await;
    assert_eq!(resposta.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(resposta.cabecalho("location"), "/sobre");
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn site_suspenso_sai_do_ar(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Suspenso, true).await;
    for caminho in ["/", POST, "/sitemap.xml", "/robots.txt"] {
        let resposta = pedir(&pool, &host("oficina"), caminho).await;
        assert_eq!(resposta.status, StatusCode::GONE, "{caminho}");
        assert!(
            !resposta.corpo.contains("madeira"),
            "{caminho} vazou conteúdo"
        );
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn site_sem_endereco_definitivo_responde_fora_da_busca(pool: PgPool) {
    semear(&pool, "montagem", Situacao::EmMontagem, true).await;
    semear(&pool, "provisorio", Situacao::Ativo, false).await;

    for slug in ["montagem", "provisorio"] {
        let pagina = pedir(&pool, &host(slug), POST).await;
        assert_eq!(pagina.status, StatusCode::OK, "{slug}");
        assert_eq!(pagina.cabecalho("x-robots-tag"), "noindex", "{slug}");
        assert!(
            pagina
                .corpo
                .contains(r#"<meta name="robots" content="noindex, follow">"#),
            "{slug}"
        );

        let robots = pedir(&pool, &host(slug), "/robots.txt").await;
        assert_eq!(robots.corpo, "User-agent: *\nDisallow: /\n", "{slug}");
        for arquivo in ["/sitemap.xml", "/llms.txt", "/llms-full.txt"] {
            assert_eq!(
                pedir(&pool, &host(slug), arquivo).await.status,
                StatusCode::NOT_FOUND,
                "{slug}{arquivo}"
            );
        }
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn um_site_nao_enxerga_o_conteudo_do_outro(pool: PgPool) {
    let oficina = semear(&pool, "oficina", Situacao::Ativo, true).await;
    let perfil = PerfilDoSite::from(demonstracao::site());
    cms_dados::criar_site(&pool, "vazio", Situacao::Ativo, true, &perfil)
        .await
        .expect("site criado");

    assert_eq!(
        pedir(&pool, &host("oficina"), POST).await.status,
        StatusCode::OK
    );
    for caminho in ["/", POST, "/sobre", "/blog"] {
        let resposta = pedir(&pool, &host("vazio"), caminho).await;
        assert_eq!(resposta.status, StatusCode::NOT_FOUND, "{caminho}");
        assert!(
            !resposta.corpo.contains("madeira"),
            "{caminho} vazou conteúdo"
        );
    }
    assert_eq!(
        pedir(&pool, &host("vazio"), "/sitemap.xml").await.status,
        StatusCode::NOT_FOUND
    );

    // A mídia é procurada na pasta do site do pedido, nunca na de outro.
    let pasta = std::env::temp_dir().join(format!("cms-teste-{}", Uuid::new_v4()));
    std::fs::create_dir_all(pasta.join(oficina.to_string())).expect("pasta criada");
    std::fs::write(
        pasta
            .join(oficina.to_string())
            .join("foto-0a1b2c3d-480.webp"),
        b"RIFF0000WEBP",
    )
    .expect("arquivo");

    let do_dono = pedir_em(
        estado(&pool, pasta.clone()),
        &host("oficina"),
        "/midia/foto-0a1b2c3d-480.webp",
    )
    .await;
    assert_eq!(do_dono.status, StatusCode::OK);
    assert_eq!(do_dono.cabecalho("content-type"), "image/webp");
    assert!(do_dono.cabecalho("cache-control").contains("immutable"));
    let de_outro = pedir_em(
        estado(&pool, pasta.clone()),
        &host("vazio"),
        "/midia/foto-0a1b2c3d-480.webp",
    )
    .await;
    assert_eq!(de_outro.status, StatusCode::NOT_FOUND);
    let fuga = format!("/midia/..%2f{oficina}%2ffoto-0a1b2c3d-480.webp");
    assert_eq!(
        pedir_em(estado(&pool, pasta.clone()), &host("vazio"), &fuga)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    std::fs::remove_dir_all(pasta).expect("pasta removida");
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn sitemaps_robots_e_llms_saem_do_motor(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let h = host("oficina");

    let indice = pedir(&pool, &h, "/sitemap.xml").await;
    assert_eq!(indice.status, StatusCode::OK);
    assert!(
        indice
            .cabecalho("content-type")
            .starts_with("application/xml")
    );
    for arquivo in [
        "sitemap-paginas.xml",
        "sitemap-posts.xml",
        "sitemap-categorias.xml",
        "sitemap-imagens.xml",
    ] {
        assert!(
            indice.corpo.contains(&format!("https://{h}/{arquivo}")),
            "{arquivo}"
        );
    }
    assert!(!indice.corpo.contains("sitemap-produtos.xml"));

    let posts = pedir(&pool, &h, "/sitemap-posts.xml").await;
    assert_eq!(posts.corpo.matches("<url>").count(), 5);
    assert!(
        posts
            .corpo
            .contains(&format!("<loc>https://{h}{POST}</loc>"))
    );
    let listagens = pedir(&pool, &h, "/sitemap-categorias.xml").await;
    assert!(
        listagens
            .corpo
            .contains(&format!("<loc>https://{h}/blog</loc>"))
    );
    assert_eq!(
        pedir(&pool, &h, "/sitemap-inexistente.xml").await.status,
        StatusCode::NOT_FOUND
    );

    let robots = pedir(&pool, &h, "/robots.txt").await;
    assert!(robots.corpo.contains("Disallow: /painel"));
    assert!(
        robots
            .corpo
            .ends_with(&format!("Sitemap: https://{h}/sitemap.xml\n"))
    );

    let llms = pedir(&pool, &h, "/llms.txt").await;
    assert!(llms.corpo.starts_with("# Oficina Exemplo"));
    assert!(llms.corpo.contains(&format!("https://{h}{POST}")));
    assert!(
        pedir(&pool, &h, "/llms-full.txt")
            .await
            .corpo
            .contains("O que olhar na madeira")
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn blog_lista_os_posts_do_mais_novo_para_o_mais_antigo(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let resposta = pedir(&pool, &host("oficina"), "/blog").await;

    assert_eq!(resposta.status, StatusCode::OK);
    assert_eq!(
        resposta
            .corpo
            .matches(r#"<article class="cartao">"#)
            .count(),
        5
    );
    assert!(
        resposta
            .corpo
            .contains("<title>Blog | Oficina Exemplo</title>")
    );
    assert!(
        resposta
            .corpo
            .contains(r#"<link rel="canonical" href="https://oficina.sites.teste/blog">"#)
    );
    let mais_novo = resposta
        .corpo
        .find("Cuidados com móveis de madeira no dia a dia")
        .expect("post mais novo");
    let mais_antigo = resposta
        .corpo
        .find("Como escolher a madeira da mesa de jantar</a>")
        .expect("post mais antigo");
    assert!(mais_novo < mais_antigo);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn dominio_proprio_vira_o_endereco_do_site(pool: PgPool) {
    let site_id = semear(&pool, "oficina", Situacao::Ativo, false).await;
    cms_dados::ativar_dominio(&pool, site_id, "oficina.com.br")
        .await
        .expect("domínio ativado");

    // O domínio próprio serve o site, com a canônica nele, e entra na busca.
    let proprio = pedir(&pool, "oficina.com.br", POST).await;
    assert_eq!(proprio.status, StatusCode::OK);
    assert!(proprio.corpo.contains(&format!(
        r#"<link rel="canonical" href="https://oficina.com.br{POST}">"#
    )));
    assert_eq!(proprio.cabecalho("x-robots-tag"), "");

    // O provisório passa a redirecionar, guardando caminho e consulta.
    let provisorio = pedir(&pool, &host("oficina"), &format!("{POST}?origem=email")).await;
    assert_eq!(provisorio.status, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        provisorio.cabecalho("location"),
        format!("https://oficina.com.br{POST}?origem=email")
    );

    // `www.` do domínio conhecido vai para o endereço sem `www.`.
    let www = pedir(&pool, "www.oficina.com.br", "/sobre").await;
    assert_eq!(www.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(www.cabecalho("location"), "https://oficina.com.br/sobre");
    assert_eq!(
        pedir(&pool, "www.desconhecido.com.br", "/").await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn dominio_pendente_nao_resolve_site(pool: PgPool) {
    let site_id = semear(&pool, "oficina", Situacao::Ativo, true).await;
    sqlx::query("insert into dominio (host, site_id) values ('pendente.com.br', $1)")
        .bind(site_id)
        .execute(&pool)
        .await
        .expect("domínio pendente gravado");
    assert_eq!(
        pedir(&pool, "pendente.com.br", "/").await.status,
        StatusCode::NOT_FOUND
    );
    // E não derruba o endereço provisório.
    assert_eq!(
        pedir(&pool, &host("oficina"), "/").await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn rascunho_nao_vai_ao_ar(pool: PgPool) {
    let site_id = semear(&pool, "oficina", Situacao::Ativo, true).await;
    sqlx::query(
        "update documento set situacao = 'rascunho' where site_id = $1 and caminho = '/sobre'",
    )
    .bind(site_id)
    .execute(&pool)
    .await
    .expect("documento despublicado");

    assert_eq!(
        pedir(&pool, &host("oficina"), "/sobre").await.status,
        StatusCode::NOT_FOUND
    );
    assert!(
        !pedir(&pool, &host("oficina"), "/sitemap-paginas.xml")
            .await
            .corpo
            .contains("/sobre")
    );
    assert!(
        !pedir(&pool, &host("oficina"), "/")
            .await
            .corpo
            .contains(r#"href="/sobre""#)
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn publicar_de_novo_cria_versao_e_troca_o_que_esta_no_ar(pool: PgPool) {
    let site_id = semear(&pool, "oficina", Situacao::Ativo, true).await;
    let mut documento = documentos_do_exemplo()
        .into_iter()
        .find(|d| d.caminho == "/contato")
        .expect("contato");
    if let Conteudo::Pagina(pagina) = &mut documento.conteudo {
        pagina.titulo = "Fale com a gente".into();
    }
    cms_dados::publicar(&pool, site_id, &documento, Utc::now())
        .await
        .expect("republicado");

    let resposta = pedir(&pool, &host("oficina"), "/contato").await;
    assert!(resposta.corpo.contains("<h1>Fale com a gente</h1>"));
    let versoes: i64 = sqlx::query_scalar(
        "select count(*) from versao v join documento d on d.id = v.documento_id where d.site_id = $1 and d.caminho = '/contato'",
    )
    .bind(site_id)
    .fetch_one(&pool)
    .await
    .expect("contagem");
    assert_eq!(versoes, 2);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn so_leitura_e_aceita_no_site_publico(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let pedido = Request::builder()
        .method("POST")
        .uri("/")
        .header("host", host("oficina"))
        .body(Body::empty())
        .expect("pedido válido");
    let resposta = roteador(estado(&pool, PathBuf::from("x")))
        .oneshot(pedido)
        .await
        .expect("o roteador responde");
    assert_eq!(resposta.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn cada_site_serve_a_chave_do_indexnow(pool: PgPool) {
    semear(&pool, "oficina", Situacao::Ativo, true).await;
    let resposta = pedir(
        &pool,
        &host("oficina"),
        &format!("/{CHAVE_DO_INDEXNOW}.txt"),
    )
    .await;
    assert_eq!(resposta.status, StatusCode::OK);
    assert!(resposta.cabecalho("content-type").starts_with("text/plain"));
    assert_eq!(resposta.corpo, CHAVE_DO_INDEXNOW);

    // Outro nome de arquivo não é a chave, e host desconhecido não a entrega.
    assert_eq!(
        pedir(&pool, &host("oficina"), "/outra-chave.txt")
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        pedir(
            &pool,
            "desconhecido.com.br",
            &format!("/{CHAVE_DO_INDEXNOW}.txt")
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}
