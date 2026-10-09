//! O painel de ponta a ponta: login pelo Auth, criação de site com os freios
//! e convites. O Auth aqui é um servidor de mentira em uma porta livre.

mod comum;

use std::collections::HashMap;
use std::path::PathBuf;

use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::Query;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::get;
use cms_dados::ErroDeConta;
use cms_dominio::{Ator, Papel};
use cms_integracoes::auth::ClienteAuth;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use comum::{HOST_DO_PAINEL, Resposta, estado, host, pedir, responder};

const ORIGEM: &str = "https://cms.teste";

/// O token de teste é `a.b.<quem>`: o Auth de mentira responde por quem.
fn token(quem: &str) -> String {
    format!("a.b.{quem}")
}

async fn sessao(
    Query(consulta): Query<HashMap<String, String>>,
    cabecalhos: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let quem = cabecalhos
        .get("cookie")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.strip_prefix("avila_sso=a.b."))
        .unwrap_or("");
    let conta = |papel: &str| json!({ "sub": format!("sub-{quem}"), "email": format!("{quem}@exemplo.example"), "nome": quem, "papel": papel });
    // Só o aplicativo `cms` é reconhecido, como no cadastro do Auth.
    let do_cms = consulta.get("app").is_some_and(|app| app == "cms");
    match quem {
        "ana" | "bia" | "caio" => (
            StatusCode::OK,
            Json(json!({ "autenticado": true, "permitido": do_cms, "sessao": conta("CLIENTE") })),
        ),
        "equipe" => (
            StatusCode::OK,
            Json(json!({ "autenticado": true, "permitido": do_cms, "sessao": conta("ADMIN") })),
        ),
        "bloqueada" => (
            StatusCode::OK,
            Json(json!({ "autenticado": true, "permitido": false, "sessao": conta("CLIENTE") })),
        ),
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "autenticado": false })),
        ),
    }
}

async fn subir_auth() -> ClienteAuth {
    let escuta = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("porta livre");
    let endereco = escuta.local_addr().expect("endereço");
    let rotas = Router::new().route("/api/session", get(sessao));
    tokio::spawn(async move {
        axum::serve(escuta, rotas).await.expect("auth de teste");
    });
    ClienteAuth::novo(&format!("http://{endereco}"), "cms").expect("cliente do auth")
}

struct Painel {
    pool: PgPool,
    auth: ClienteAuth,
}

impl Painel {
    async fn novo(pool: &PgPool) -> Self {
        Self {
            pool: pool.clone(),
            auth: subir_auth().await,
        }
    }

    async fn enviar(&self, pedido: axum::http::request::Builder, corpo: Body) -> Resposta {
        let mut estado = estado(&self.pool, PathBuf::from("midia-que-nao-existe"));
        estado.auth = Some(self.auth.clone());
        responder(estado, pedido.body(corpo).expect("pedido válido")).await
    }

    async fn abrir(&self, quem: Option<&str>, caminho: &str) -> Resposta {
        let mut pedido = Request::builder()
            .uri(caminho)
            .header("host", HOST_DO_PAINEL);
        if let Some(quem) = quem {
            pedido = pedido.header("cookie", format!("outro=1; avila_sso={}", token(quem)));
        }
        self.enviar(pedido, Body::empty()).await
    }

    /// Um formulário enviado de dentro do painel.
    async fn postar(&self, quem: &str, caminho: &str, corpo: &str) -> Resposta {
        let pedido = Request::builder()
            .method("POST")
            .uri(caminho)
            .header("host", HOST_DO_PAINEL)
            .header("origin", ORIGEM)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", format!("avila_sso={}", token(quem)));
        self.enviar(pedido, Body::from(corpo.to_string())).await
    }

    async fn criar_site(&self, quem: &str, slug: &str, nome: &str) -> Resposta {
        let corpo = format!("nome={}&slug={slug}", nome.replace(' ', "+"));
        self.postar(quem, "/painel/sites", &corpo).await
    }
}

async fn site_id(pool: &PgPool, slug: &str) -> Uuid {
    sqlx::query_scalar("select id from site where slug = $1")
        .bind(slug)
        .fetch_one(pool)
        .await
        .expect("site gravado")
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn sem_sessao_vai_ao_login_do_auth_uma_vez_so(pool: PgPool) {
    let painel = Painel::novo(&pool).await;

    let raiz = painel.abrir(None, "/").await;
    assert_eq!(raiz.status, StatusCode::FOUND);
    assert_eq!(raiz.cabecalho("location"), "/painel");

    for quem in [None, Some("desconhecida")] {
        let resposta = painel.abrir(quem, "/painel").await;
        assert_eq!(resposta.status, StatusCode::FOUND, "{quem:?}");
        let destino = resposta.cabecalho("location");
        assert!(destino.contains("/login?app=cms&returnTo="), "{destino}");
        assert!(
            destino.ends_with("https%3A%2F%2Fcms.teste%2Fpainel%3Fvolta%3D1"),
            "{destino}"
        );
    }

    // Voltou do Auth e a sessão não chegou: para, em vez de mandar de novo.
    let de_volta = painel.abrir(None, "/painel?volta=1").await;
    assert_eq!(de_volta.status, StatusCode::UNAUTHORIZED);
    assert_eq!(de_volta.cabecalho("location"), "");
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn so_entra_quem_o_auth_liberou(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    assert_eq!(
        painel.abrir(Some("bloqueada"), "/painel").await.status,
        StatusCode::FORBIDDEN
    );

    let entrou = painel.abrir(Some("ana"), "/painel").await;
    assert_eq!(entrou.status, StatusCode::OK);
    assert!(entrou.corpo.contains("ana@exemplo.example"));
    assert!(entrou.corpo.contains("Você ainda não tem nenhum site"));
    assert_eq!(entrou.cabecalho("cache-control"), "private, no-store");
    assert_eq!(entrou.cabecalho("x-robots-tag"), "noindex");

    // Auth fora do ar: ninguém entra, e a tela diz por quê.
    let fora_do_ar = Painel {
        pool: pool.clone(),
        auth: ClienteAuth::novo("http://127.0.0.1:9", "cms").expect("cliente"),
    };
    assert_eq!(
        fora_do_ar.abrir(Some("ana"), "/painel").await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );

    // Sem login configurado, o painel não abre para ninguém.
    let sem_login = pedir(&pool, HOST_DO_PAINEL, "/painel").await;
    assert_eq!(sem_login.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn quem_cria_o_site_vira_dono_e_o_site_nasce_em_montagem(pool: PgPool) {
    let painel = Painel::novo(&pool).await;

    let criado = painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    assert_eq!(criado.status, StatusCode::SEE_OTHER);
    assert_eq!(criado.cabecalho("location"), "/painel?criado=1");

    let lista = painel.abrir(Some("ana"), "/painel?criado=1").await;
    assert!(lista.corpo.contains("Padaria da Ana"));
    assert!(
        lista
            .corpo
            .contains(r#"href="https://padaria.sites.teste""#)
    );
    assert!(lista.corpo.contains("Em montagem · Dono"));
    assert!(lista.corpo.contains("Site criado."));
    // O site de uma conta não aparece para outra.
    assert!(
        !painel
            .abrir(Some("bia"), "/painel")
            .await
            .corpo
            .contains("Padaria")
    );

    let id = site_id(&pool, "padaria").await;
    assert_eq!(
        cms_dados::papel_no_site(&pool, id, "sub-ana")
            .await
            .expect("papel"),
        Some(Papel::Dono)
    );
    assert_eq!(
        cms_dados::papel_no_site(&pool, id, "sub-bia")
            .await
            .expect("papel"),
        None
    );
    let (tipo, dados): (String, Value) =
        sqlx::query_as("select tipo, dados from evento where site_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("evento do site");
    assert_eq!(tipo, "site.criado");
    assert_eq!(dados, json!({ "criadoPor": "ana@exemplo.example" }));

    // O endereço provisório já responde, vazio e fora da busca.
    let no_ar = pedir(&pool, &host("padaria"), "/").await;
    assert_eq!(no_ar.status, StatusCode::NOT_FOUND);
    assert_eq!(no_ar.cabecalho("x-robots-tag"), "noindex");
    assert!(no_ar.corpo.contains("Padaria da Ana"));
    assert_eq!(
        pedir(&pool, &host("padaria"), "/robots.txt").await.corpo,
        "User-agent: *\nDisallow: /\n"
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn criacao_recusada_volta_para_a_tela_com_o_motivo(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    assert_eq!(
        painel
            .criar_site("ana", "padaria", "Padaria da Ana")
            .await
            .status,
        StatusCode::SEE_OTHER
    );

    let repetido = painel.criar_site("bia", "padaria", "Padaria da Bia").await;
    assert_eq!(repetido.status, StatusCode::CONFLICT);
    assert!(
        repetido
            .corpo
            .contains("Já existe um site com este endereço")
    );
    // O que foi digitado volta no formulário.
    assert!(repetido.corpo.contains(r#"value="Padaria da Bia""#));
    assert!(repetido.corpo.contains(r#"value="padaria""#));

    for (slug, nome, status) in [
        ("cms", "Meu site", StatusCode::UNPROCESSABLE_ENTITY),
        ("-x-", "Meu site", StatusCode::UNPROCESSABLE_ENTITY),
        ("meu-site", "", StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let resposta = painel.criar_site("bia", slug, nome).await;
        assert_eq!(resposta.status, status, "{slug} {nome}");
        assert!(resposta.corpo.contains(r#"role="alert""#), "{slug} {nome}");
    }
    let total: i64 = sqlx::query_scalar("select count(*) from site")
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(total, 1);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn formulario_so_vale_vindo_do_proprio_painel(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    for origem in [
        None,
        Some("https://golpe.sites.teste"),
        Some("http://cms.teste"),
    ] {
        let mut pedido = Request::builder()
            .method("POST")
            .uri("/painel/sites")
            .header("host", HOST_DO_PAINEL)
            .header("cookie", format!("avila_sso={}", token("ana")));
        if let Some(origem) = origem {
            pedido = pedido.header("origin", origem);
        }
        let resposta = painel
            .enviar(pedido, Body::from("nome=Golpe&slug=golpe"))
            .await;
        assert_eq!(resposta.status, StatusCode::FORBIDDEN, "{origem:?}");
    }
    let total: i64 = sqlx::query_scalar("select count(*) from site")
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(total, 0);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn a_criacao_aberta_tem_freio_e_a_equipe_nao(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    // O padrão: duas criações por dia.
    for slug in ["primeiro", "segundo"] {
        assert_eq!(
            painel.criar_site("ana", slug, "Site").await.status,
            StatusCode::SEE_OTHER,
            "{slug}"
        );
    }
    let barrado = painel.criar_site("ana", "terceiro", "Site").await;
    assert_eq!(barrado.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(barrado.corpo.contains("Tente de novo amanhã"));

    // No dia seguinte o freio diário solta, e o de sites por conta segura.
    sqlx::query("update site set criado_em = criado_em - interval '2 days'")
        .execute(&pool)
        .await
        .expect("sites envelhecidos");
    assert_eq!(
        painel.criar_site("ana", "terceiro", "Site").await.status,
        StatusCode::SEE_OTHER
    );
    sqlx::query("update site set criado_em = criado_em - interval '2 days'")
        .execute(&pool)
        .await
        .expect("sites envelhecidos");
    let no_teto = painel.criar_site("ana", "quarto", "Site").await;
    assert_eq!(no_teto.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(no_teto.corpo.contains("número máximo de sites"));

    for slug in ["equipe-um", "equipe-dois", "equipe-tres", "equipe-quatro"] {
        assert_eq!(
            painel.criar_site("equipe", slug, "Site").await.status,
            StatusCode::SEE_OTHER,
            "{slug}"
        );
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn convite_so_serve_ao_email_convidado_e_uma_vez(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let id = site_id(&pool, "padaria").await;
    let dona = Ator::do_site("sub-ana", Papel::Dono);

    // Só o Dono convida.
    let editor = Ator::do_site("sub-x", Papel::Editor);
    assert!(matches!(
        cms_dados::criar_convite(
            &pool,
            id,
            &editor,
            "bia@exemplo.example",
            Papel::Autor,
            ORIGEM
        )
        .await,
        Err(ErroDeConta::SemPermissao)
    ));
    assert!(matches!(
        cms_dados::criar_convite(&pool, id, &dona, "sem-arroba", Papel::Autor, ORIGEM).await,
        Err(ErroDeConta::EmailInvalido)
    ));

    let convite = cms_dados::criar_convite(
        &pool,
        id,
        &dona,
        " Bia@Exemplo.example ",
        Papel::Editor,
        ORIGEM,
    )
    .await
    .expect("convite criado");
    let caminho = convite
        .link
        .strip_prefix(ORIGEM)
        .expect("o link é do painel")
        .to_string();
    assert!(caminho.starts_with("/convite/"));

    // O evento leva o link pronto; o banco, só o hash do token.
    let dados: Value = sqlx::query_scalar("select dados from evento where tipo = 'convite.criado'")
        .fetch_one(&pool)
        .await
        .expect("evento do convite");
    assert_eq!(dados["link"], convite.link);
    assert_eq!(dados["email"], "bia@exemplo.example");
    assert_eq!(dados["papel"], "Editor");
    let hash: String = sqlx::query_scalar("select token_hash from convite")
        .fetch_one(&pool)
        .await
        .expect("convite gravado");
    assert!(!convite.link.contains(&hash));

    // Link repassado não abre o site para outra conta.
    let de_outro = painel.abrir(Some("caio"), &caminho).await;
    assert_eq!(de_outro.status, StatusCode::FORBIDDEN);
    assert!(de_outro.corpo.contains("outro e-mail"));
    // Sem sessão, o link leva ao login e volta para o convite.
    let sem_sessao = painel.abrir(None, &caminho).await;
    assert_eq!(sem_sessao.status, StatusCode::FOUND);
    assert!(sem_sessao.cabecalho("location").contains("%2Fconvite%2F"));

    let aceito = painel.abrir(Some("bia"), &caminho).await;
    assert_eq!(aceito.status, StatusCode::SEE_OTHER);
    assert_eq!(aceito.cabecalho("location"), "/painel?convite=1");
    assert_eq!(
        cms_dados::papel_no_site(&pool, id, "sub-bia")
            .await
            .expect("papel"),
        Some(Papel::Editor)
    );
    assert!(
        painel
            .abrir(Some("bia"), "/painel")
            .await
            .corpo
            .contains("Em montagem · Editor")
    );

    assert_eq!(
        painel.abrir(Some("bia"), &caminho).await.status,
        StatusCode::GONE
    );
    assert_eq!(
        painel
            .abrir(Some("bia"), "/convite/token-que-nao-existe")
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let vencido = cms_dados::criar_convite(
        &pool,
        id,
        &dona,
        "caio@exemplo.example",
        Papel::Autor,
        ORIGEM,
    )
    .await
    .expect("convite criado");
    sqlx::query("update convite set expira_em = now() - interval '1 minute' where id = $1")
        .bind(vencido.id)
        .execute(&pool)
        .await
        .expect("convite vencido");
    let caminho = vencido.link.strip_prefix(ORIGEM).expect("link do painel");
    assert_eq!(
        painel.abrir(Some("caio"), caminho).await.status,
        StatusCode::GONE
    );
    assert_eq!(
        cms_dados::papel_no_site(&pool, id, "sub-caio")
            .await
            .expect("papel"),
        None
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn painel_e_sites_nao_se_misturam(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;

    // No host de um site, `/painel` é só um caminho que não existe.
    let no_site = Request::builder()
        .uri("/painel")
        .header("host", host("padaria"))
        .header("cookie", format!("avila_sso={}", token("ana")));
    let resposta = painel.enviar(no_site, Body::empty()).await;
    assert_eq!(resposta.status, StatusCode::NOT_FOUND);
    assert!(!resposta.corpo.contains("Meus sites"));

    // No host do painel não há site: caminho de site não existe ali.
    for caminho in ["/blog", "/sitemap.xml", "/robots.txt"] {
        assert_eq!(
            painel.abrir(Some("ana"), caminho).await.status,
            StatusCode::NOT_FOUND,
            "{caminho}"
        );
    }
}

/// O caminho do convite, tirado do link que a tela mostra.
fn caminho_do_convite(corpo: &str) -> String {
    let inicio = corpo.find("/convite/").expect("a tela mostra o link");
    corpo[inicio..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '/')
        .collect()
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn dono_convida_pela_tela_de_equipe(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    const EQUIPE: &str = "/painel/sites/padaria/equipe";
    const CONVITES: &str = "/painel/sites/padaria/convites";

    // A lista de sites leva o Dono à equipe.
    assert!(
        painel
            .abrir(Some("ana"), "/painel")
            .await
            .corpo
            .contains(&format!(r#"href="{EQUIPE}""#))
    );
    let equipe = painel.abrir(Some("ana"), EQUIPE).await;
    assert_eq!(equipe.status, StatusCode::OK);
    assert!(equipe.corpo.contains("Padaria da Ana"));
    assert!(equipe.corpo.contains("ana@exemplo.example"));
    assert_eq!(equipe.cabecalho("cache-control"), "private, no-store");

    let recusado = painel
        .postar("ana", CONVITES, "email=sem-arroba&papel=editor")
        .await;
    assert_eq!(recusado.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(recusado.corpo.contains("Informe um e-mail válido"));
    assert!(recusado.corpo.contains(r#"value="sem-arroba""#));
    assert_eq!(
        painel
            .postar("ana", CONVITES, "email=bia%40exemplo.example&papel=chefe")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    let criado = painel
        .postar("ana", CONVITES, "email=Bia%40exemplo.example&papel=editor")
        .await;
    assert_eq!(criado.status, StatusCode::OK);
    assert!(
        criado
            .corpo
            .contains("Convite criado para bia@exemplo.example")
    );
    // Em aberto, o convite aparece na equipe com o papel e a validade.
    assert!(criado.corpo.contains("Editor · convite até"));
    let caminho = caminho_do_convite(&criado.corpo);
    assert!(criado.corpo.contains(&format!("{ORIGEM}{caminho}")));
    // O link só aparece na criação: reabrir a tela não o mostra de novo.
    assert!(
        !painel
            .abrir(Some("ana"), EQUIPE)
            .await
            .corpo
            .contains("/convite/")
    );

    assert_eq!(
        painel.abrir(Some("bia"), &caminho).await.status,
        StatusCode::SEE_OTHER
    );
    let depois = painel.abrir(Some("ana"), EQUIPE).await;
    assert!(depois.corpo.contains("bia@exemplo.example"));
    assert!(!depois.corpo.contains("convite até"));
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn equipe_e_so_do_dono_e_de_quem_e_do_site(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    const EQUIPE: &str = "/painel/sites/padaria/equipe";
    const CONVITES: &str = "/painel/sites/padaria/convites";
    let corpo = "email=caio%40exemplo.example&papel=dono";

    // Bia entra como Editora.
    let convite = painel
        .postar("ana", CONVITES, "email=bia%40exemplo.example&papel=editor")
        .await;
    painel
        .abrir(Some("bia"), &caminho_do_convite(&convite.corpo))
        .await;

    // Editor participa, mas não administra: nem vê, nem convida.
    assert_eq!(
        painel.abrir(Some("bia"), EQUIPE).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        painel.postar("bia", CONVITES, corpo).await.status,
        StatusCode::FORBIDDEN
    );
    assert!(
        !painel
            .abrir(Some("bia"), "/painel")
            .await
            .corpo
            .contains(EQUIPE)
    );

    // Quem não é do site recebe o mesmo que para um site que não existe.
    for caminho in [EQUIPE, "/painel/sites/nao-existe/equipe"] {
        let resposta = painel.abrir(Some("caio"), caminho).await;
        assert_eq!(resposta.status, StatusCode::NOT_FOUND, "{caminho}");
        assert!(!resposta.corpo.contains("Padaria"), "{caminho}");
    }
    assert_eq!(
        painel.postar("caio", CONVITES, corpo).await.status,
        StatusCode::NOT_FOUND
    );

    // Formulário de fora do painel não cria convite.
    let de_fora = Request::builder()
        .method("POST")
        .uri(CONVITES)
        .header("host", HOST_DO_PAINEL)
        .header("origin", "https://padaria.sites.teste")
        .header("cookie", format!("avila_sso={}", token("ana")));
    assert_eq!(
        painel.enviar(de_fora, Body::from(corpo)).await.status,
        StatusCode::FORBIDDEN
    );
    let convites: i64 = sqlx::query_scalar("select count(*) from convite")
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(convites, 1);

    // A equipe da Ávila Ops entra em qualquer site com poder de Dono.
    assert_eq!(
        painel.abrir(Some("equipe"), EQUIPE).await.status,
        StatusCode::OK
    );
}
