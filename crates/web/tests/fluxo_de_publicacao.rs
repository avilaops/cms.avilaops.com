//! O fluxo de publicação de ponta a ponta: as ações gravam no banco e o
//! resultado é conferido no site, como o visitante o vê.

mod comum;

use axum::http::StatusCode;
use chrono::{DateTime, TimeZone, Utc};
use cms_dados::fluxo::{self, ErroDeFluxo};
use cms_dominio::{Ator, Papel, PerfilDoSite, Situacao};
use motor_web::demonstracao;
use motor_web::tipos::{Conteudo, Pagina, Post};
use motor_web::validacao::{Problema, pode_publicar};
use sqlx::PgPool;
use uuid::Uuid;

use comum::{host, pedir};

const POST: &str = "/blog/como-escolher-a-madeira-da-mesa";

fn dia(dia: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, dia, 12, 0, 0)
        .single()
        .expect("data válida")
}

async fn criar_site(pool: &PgPool, slug: &str) -> Uuid {
    let perfil = PerfilDoSite::from(demonstracao::site());
    cms_dados::criar_site(pool, slug, Situacao::Ativo, true, &perfil)
        .await
        .expect("site criado")
}

fn exemplo(caminho: &str) -> Conteudo {
    demonstracao::documentos()
        .into_iter()
        .find(|documento| documento.caminho == caminho)
        .unwrap_or_else(|| panic!("o exemplo não tem {caminho}"))
        .conteudo
}

fn pagina(conteudo: &mut Conteudo) -> &mut Pagina {
    match conteudo {
        Conteudo::Pagina(pagina) => pagina,
        _ => panic!("o exemplo não é uma página"),
    }
}

fn post(conteudo: &mut Conteudo) -> &mut Post {
    match conteudo {
        Conteudo::Post(post) => post,
        _ => panic!("o exemplo não é um post"),
    }
}

fn editor() -> Ator {
    Ator::do_site("editor", Papel::Editor)
}

fn autor(conta: &str) -> Ator {
    Ator::do_site(conta, Papel::Autor)
}

fn codigos(problemas: &[Problema]) -> Vec<&'static str> {
    problemas.iter().map(|problema| problema.codigo).collect()
}

/// Os códigos do que barrou a ação.
fn recusa(resultado: Result<(), ErroDeFluxo>) -> Vec<&'static str> {
    match resultado {
        Err(ErroDeFluxo::Recusado(problemas)) => codigos(&problemas),
        outro => panic!("esperava recusa, veio {outro:?}"),
    }
}

/// Salva e publica como Editor.
async fn por_no_ar(
    pool: &PgPool,
    site_id: Uuid,
    documento_id: Option<Uuid>,
    conteudo: &Conteudo,
    agora: DateTime<Utc>,
) -> Uuid {
    let salvo = fluxo::salvar_rascunho(pool, site_id, &editor(), documento_id, conteudo)
        .await
        .expect("rascunho salvo");
    fluxo::publicar(pool, site_id, &editor(), salvo.documento_id, agora)
        .await
        .expect("publicado");
    salvo.documento_id
}

async fn versoes(pool: &PgPool, documento_id: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from versao where documento_id = $1")
        .bind(documento_id)
        .fetch_one(pool)
        .await
        .expect("contagem de versões")
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn rascunho_salva_com_problema_e_nao_vai_ao_ar(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let mut conteudo = exemplo("/sobre");
    pagina(&mut conteudo).seo.titulo = String::new();

    let salvo = fluxo::salvar_rascunho(&pool, site_id, &editor(), None, &conteudo)
        .await
        .expect("salvar rascunho nunca é barrado");
    assert!(codigos(&salvo.problemas).contains(&"seo.titulo.vazio"));

    let id = salvo.documento_id;
    assert!(
        recusa(fluxo::enviar_para_revisao(&pool, site_id, &editor(), id).await)
            .contains(&"seo.titulo.vazio")
    );
    assert!(
        recusa(fluxo::publicar(&pool, site_id, &editor(), id, dia(1)).await)
            .contains(&"seo.titulo.vazio")
    );
    assert_eq!(
        pedir(&pool, &host("oficina"), "/sobre").await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn autor_escreve_e_editor_publica(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let (helena, rafael) = (autor("helena"), autor("rafael"));

    let salvo = fluxo::salvar_rascunho(&pool, site_id, &helena, None, &exemplo(POST))
        .await
        .expect("rascunho salvo");
    assert!(pode_publicar(&salvo.problemas), "{:?}", salvo.problemas);
    let id = salvo.documento_id;

    // Outro autor não mexe no rascunho, e autor nenhum publica.
    assert!(matches!(
        fluxo::salvar_rascunho(&pool, site_id, &rafael, Some(id), &exemplo(POST)).await,
        Err(ErroDeFluxo::SemPermissao)
    ));
    assert!(matches!(
        fluxo::enviar_para_revisao(&pool, site_id, &rafael, id).await,
        Err(ErroDeFluxo::SemPermissao)
    ));
    assert!(matches!(
        fluxo::publicar(&pool, site_id, &helena, id, dia(5)).await,
        Err(ErroDeFluxo::SemPermissao)
    ));

    assert!(matches!(
        fluxo::devolver(&pool, site_id, &editor(), id).await,
        Err(ErroDeFluxo::ForaDeRevisao)
    ));
    fluxo::enviar_para_revisao(&pool, site_id, &helena, id)
        .await
        .expect("revisão pedida");
    assert!(matches!(
        fluxo::devolver(&pool, site_id, &helena, id).await,
        Err(ErroDeFluxo::SemPermissao)
    ));
    fluxo::devolver(&pool, site_id, &editor(), id)
        .await
        .expect("devolvido");
    fluxo::enviar_para_revisao(&pool, site_id, &helena, id)
        .await
        .expect("revisão pedida de novo");
    assert_eq!(
        pedir(&pool, &host("oficina"), POST).await.status,
        StatusCode::NOT_FOUND,
        "em revisão ainda não é no ar"
    );

    fluxo::publicar(&pool, site_id, &editor(), id, dia(5))
        .await
        .expect("publicado");
    let resposta = pedir(&pool, &host("oficina"), POST).await;
    assert_eq!(resposta.status, StatusCode::OK);
    // As datas são as do servidor, não as que vieram no conteúdo.
    assert!(
        resposta
            .corpo
            .contains(r#"<time datetime="2026-10-05">5 de outubro de 2026</time>"#)
    );
    assert!(!resposta.corpo.contains("Atualizado em"));
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn editar_o_que_esta_no_ar_so_muda_o_site_ao_publicar(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let id = por_no_ar(&pool, site_id, None, &exemplo("/contato"), dia(1)).await;

    let mut novo = exemplo("/contato");
    pagina(&mut novo).titulo = "Fale com a gente".into();
    // Dois salvamentos, como faz o salvamento automático: uma versão só.
    for _ in 0..2 {
        fluxo::salvar_rascunho(&pool, site_id, &editor(), Some(id), &novo)
            .await
            .expect("rascunho salvo");
    }
    assert_eq!(versoes(&pool, id).await, 2);
    assert!(
        !pedir(&pool, &host("oficina"), "/contato")
            .await
            .corpo
            .contains("Fale com a gente")
    );

    fluxo::publicar(&pool, site_id, &editor(), id, dia(2))
        .await
        .expect("publicado");
    assert!(
        pedir(&pool, &host("oficina"), "/contato")
            .await
            .corpo
            .contains("<h1>Fale com a gente</h1>")
    );
    assert_eq!(versoes(&pool, id).await, 2);
    assert!(matches!(
        fluxo::publicar(&pool, site_id, &editor(), id, dia(3)).await,
        Err(ErroDeFluxo::SemRascunho)
    ));
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn trocar_o_endereco_redireciona_o_antigo(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let h = host("oficina");
    let id = por_no_ar(&pool, site_id, None, &exemplo("/contato"), dia(1)).await;

    let com_slug = |slug: &str| {
        let mut conteudo = exemplo("/contato");
        pagina(&mut conteudo).slug = slug.into();
        conteudo
    };
    por_no_ar(&pool, site_id, Some(id), &com_slug("fale-conosco"), dia(2)).await;
    let antigo = pedir(&pool, &h, "/contato").await;
    assert_eq!(antigo.status, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(antigo.cabecalho("location"), "/fale-conosco");
    assert_eq!(
        pedir(&pool, &h, "/fale-conosco").await.status,
        StatusCode::OK
    );

    // Segunda troca: os dois endereços antigos vão direto ao atual.
    por_no_ar(&pool, site_id, Some(id), &com_slug("atendimento"), dia(3)).await;
    for caminho in ["/contato", "/fale-conosco"] {
        let resposta = pedir(&pool, &h, caminho).await;
        assert_eq!(resposta.status, StatusCode::MOVED_PERMANENTLY, "{caminho}");
        assert_eq!(resposta.cabecalho("location"), "/atendimento", "{caminho}");
    }

    // De volta ao primeiro endereço: ele serve a página, sem dar a volta.
    por_no_ar(&pool, site_id, Some(id), &com_slug("contato"), dia(4)).await;
    assert_eq!(pedir(&pool, &h, "/contato").await.status, StatusCode::OK);
    for caminho in ["/fale-conosco", "/atendimento"] {
        let resposta = pedir(&pool, &h, caminho).await;
        assert_eq!(resposta.status, StatusCode::MOVED_PERMANENTLY, "{caminho}");
        assert_eq!(resposta.cabecalho("location"), "/contato", "{caminho}");
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn despublicar_responde_410_e_voltar_ao_ar_guarda_as_datas(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let h = host("oficina");
    let id = por_no_ar(&pool, site_id, None, &exemplo(POST), dia(1)).await;

    assert!(matches!(
        fluxo::despublicar(&pool, site_id, &autor("helena"), id).await,
        Err(ErroDeFluxo::SemPermissao)
    ));
    fluxo::despublicar(&pool, site_id, &editor(), id)
        .await
        .expect("despublicado");

    let resposta = pedir(&pool, &h, POST).await;
    assert_eq!(resposta.status, StatusCode::GONE);
    assert_eq!(resposta.cabecalho("x-robots-tag"), "noindex");
    assert!(resposta.corpo.contains("Página removida"));
    assert!(!resposta.corpo.contains("madeira da mesa"));
    assert!(
        !pedir(&pool, &h, "/sitemap-posts.xml")
            .await
            .corpo
            .contains(POST)
    );
    assert!(matches!(
        fluxo::despublicar(&pool, site_id, &editor(), id).await,
        Err(ErroDeFluxo::ForaDoAr)
    ));

    // Volta como saiu: o conteúdo é o mesmo, então as datas também.
    fluxo::publicar(&pool, site_id, &editor(), id, dia(9))
        .await
        .expect("de volta ao ar");
    let resposta = pedir(&pool, &h, POST).await;
    assert_eq!(resposta.status, StatusCode::OK);
    assert!(resposta.corpo.contains(r#"<time datetime="2026-10-01">"#));
    assert!(!resposta.corpo.contains("Atualizado em"));
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn endereco_ocupado_ou_reservado_barra_a_publicacao(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    por_no_ar(&pool, site_id, None, &exemplo("/sobre"), dia(1)).await;

    // Um segundo documento igual: mesmo endereço, mesmo título de busca.
    let repetido = fluxo::salvar_rascunho(&pool, site_id, &editor(), None, &exemplo("/sobre"))
        .await
        .expect("rascunho salvo mesmo com o endereço ocupado");
    assert!(codigos(&repetido.problemas).contains(&"slug.em-uso"));
    let barrado =
        recusa(fluxo::publicar(&pool, site_id, &editor(), repetido.documento_id, dia(2)).await);
    for codigo in [
        "slug.em-uso",
        "seo.titulo.repetido",
        "seo.descricao.repetida",
    ] {
        assert!(barrado.contains(&codigo), "{codigo} em {barrado:?}");
    }

    let mut reservado = exemplo("/servicos");
    pagina(&mut reservado).slug = "blog".into();
    let salvo = fluxo::salvar_rascunho(&pool, site_id, &editor(), None, &reservado)
        .await
        .expect("rascunho salvo");
    assert!(
        recusa(fluxo::publicar(&pool, site_id, &editor(), salvo.documento_id, dia(2)).await)
            .contains(&"slug.reservado")
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn um_site_nao_alcanca_documento_de_outro(pool: PgPool) {
    let oficina = criar_site(&pool, "oficina").await;
    let vizinho = criar_site(&pool, "vizinho").await;
    let id = por_no_ar(&pool, oficina, None, &exemplo("/sobre"), dia(1)).await;
    let dono = Ator::do_site("dono-do-vizinho", Papel::Dono);

    assert!(matches!(
        fluxo::salvar_rascunho(&pool, vizinho, &dono, Some(id), &exemplo("/sobre")).await,
        Err(ErroDeFluxo::NaoEncontrado)
    ));
    assert!(matches!(
        fluxo::enviar_para_revisao(&pool, vizinho, &dono, id).await,
        Err(ErroDeFluxo::NaoEncontrado)
    ));
    assert!(matches!(
        fluxo::devolver(&pool, vizinho, &dono, id).await,
        Err(ErroDeFluxo::NaoEncontrado)
    ));
    assert!(matches!(
        fluxo::publicar(&pool, vizinho, &dono, id, dia(2)).await,
        Err(ErroDeFluxo::NaoEncontrado)
    ));
    assert!(matches!(
        fluxo::despublicar(&pool, vizinho, &dono, id).await,
        Err(ErroDeFluxo::NaoEncontrado)
    ));
    assert_eq!(
        pedir(&pool, &host("oficina"), "/sobre").await.status,
        StatusCode::OK
    );

    // Título e endereço só disputam dentro do próprio site.
    por_no_ar(&pool, vizinho, None, &exemplo("/sobre"), dia(2)).await;
    assert_eq!(
        pedir(&pool, &host("vizinho"), "/sobre").await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn atualizado_em_so_anda_quando_o_que_se_le_muda(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let id = por_no_ar(&pool, site_id, None, &exemplo(POST), dia(1)).await;

    // Só a descrição de busca mudou: o visitante lê o mesmo texto.
    let mut conteudo = exemplo(POST);
    post(&mut conteudo).seo.descricao =
        "Um roteiro curto para escolher a madeira certa antes de encomendar a mesa.".into();
    por_no_ar(&pool, site_id, Some(id), &conteudo, dia(2)).await;
    assert!(
        !pedir(&pool, &host("oficina"), POST)
            .await
            .corpo
            .contains("Atualizado em")
    );

    post(&mut conteudo).titulo = "Como escolher a madeira da sua mesa".into();
    por_no_ar(&pool, site_id, Some(id), &conteudo, dia(3)).await;
    let resposta = pedir(&pool, &host("oficina"), POST).await;
    assert!(resposta.corpo.contains(r#"<time datetime="2026-10-01">"#));
    assert!(
        resposta
            .corpo
            .contains(r#"Atualizado em <time datetime="2026-10-03">"#)
    );

    let datas: (Option<DateTime<Utc>>, Option<DateTime<Utc>>) =
        sqlx::query_as("select publicado_em, atualizado_em from documento where id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("datas do documento");
    assert_eq!(datas, (Some(dia(1)), Some(dia(3))));
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn historico_guarda_quem_fez_o_que(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let helena = autor("helena");
    let id = fluxo::salvar_rascunho(&pool, site_id, &helena, None, &exemplo(POST))
        .await
        .expect("rascunho salvo")
        .documento_id;
    fluxo::enviar_para_revisao(&pool, site_id, &helena, id)
        .await
        .expect("revisão pedida");
    fluxo::publicar(&pool, site_id, &Ator::da_equipe("suporte"), id, dia(1))
        .await
        .expect("publicado pela equipe");
    fluxo::despublicar(&pool, site_id, &editor(), id)
        .await
        .expect("despublicado");

    let linhas: Vec<(String, bool, String)> = sqlx::query_as(
        "select conta, equipe, acao from historico where documento_id = $1 order by id",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .expect("histórico");
    let esperado = [
        ("helena", false, "rascunho.criado"),
        ("helena", false, "revisao.pedida"),
        ("suporte", true, "conteudo.publicado"),
        ("editor", false, "conteudo.despublicado"),
    ];
    assert_eq!(linhas.len(), esperado.len(), "{linhas:?}");
    for (linha, (conta, equipe, acao)) in linhas.iter().zip(esperado) {
        assert_eq!(
            (linha.0.as_str(), linha.1, linha.2.as_str()),
            (conta, equipe, acao)
        );
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn apagar_o_site_leva_rascunhos_redirecionamentos_e_historico(pool: PgPool) {
    let site_id = criar_site(&pool, "oficina").await;
    let id = por_no_ar(&pool, site_id, None, &exemplo("/contato"), dia(1)).await;
    let mut novo = exemplo("/contato");
    pagina(&mut novo).slug = "fale-conosco".into();
    por_no_ar(&pool, site_id, Some(id), &novo, dia(2)).await;
    // Fica um rascunho em revisão sobre o documento no ar.
    fluxo::salvar_rascunho(&pool, site_id, &editor(), Some(id), &exemplo("/contato"))
        .await
        .expect("rascunho salvo");
    fluxo::enviar_para_revisao(&pool, site_id, &editor(), id)
        .await
        .expect("revisão pedida");

    assert!(
        cms_dados::apagar_site(&pool, "oficina")
            .await
            .expect("site apagado")
    );
    for tabela in ["documento", "versao", "redirecionamento", "historico"] {
        let sobra: i64 = sqlx::query_scalar(&format!("select count(*) from {tabela}"))
            .fetch_one(&pool)
            .await
            .expect("contagem");
        assert_eq!(sobra, 0, "{tabela}");
    }
}
