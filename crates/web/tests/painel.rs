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
    /// Uma pasta de mídia só deste teste.
    diretorio: PathBuf,
    /// O cache de página, o mesmo em todos os pedidos do teste.
    cache: std::sync::Arc<cms_web::Cache>,
}

fn pasta_de_teste() -> PathBuf {
    std::env::temp_dir().join(format!("cms-painel-{}", Uuid::new_v4()))
}

impl Painel {
    async fn novo(pool: &PgPool) -> Self {
        Self {
            pool: pool.clone(),
            auth: subir_auth().await,
            diretorio: pasta_de_teste(),
            cache: cms_web::Cache::novo(),
        }
    }

    async fn enviar(&self, pedido: axum::http::request::Builder, corpo: Body) -> Resposta {
        let mut estado = estado(&self.pool, self.diretorio.clone());
        estado.auth = Some(self.auth.clone());
        estado.cache = self.cache.clone();
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
        diretorio: pasta_de_teste(),
        cache: cms_web::Cache::novo(),
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

const LIMITE: &str = "----cms-teste";

fn png(largura: u32, altura: u32, tom: u8) -> Vec<u8> {
    let imagem = image::RgbImage::from_pixel(largura, altura, image::Rgb([tom, 90, 60]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(imagem)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("png desenhado");
    bytes
}

/// O corpo de um formulário de envio, como o navegador monta.
fn formulario_de_envio(arquivo: &[u8], alt: &str) -> Vec<u8> {
    let mut corpo = Vec::new();
    let campo = |nome: &str, extra: &str| {
        format!("--{LIMITE}\r\ncontent-disposition: form-data; name=\"{nome}\"{extra}\r\n\r\n")
    };
    corpo.extend(campo("arquivo", "; filename=\"Vitrine da Padaria.png\"").into_bytes());
    corpo.extend_from_slice(arquivo);
    corpo.extend(format!("\r\n{}{alt}\r\n", campo("alt", "")).into_bytes());
    corpo.extend(format!("{}Foto: Ana\r\n--{LIMITE}--\r\n", campo("credito", "")).into_bytes());
    corpo
}

impl Painel {
    async fn enviar_imagem(&self, quem: &str, slug: &str, arquivo: &[u8], alt: &str) -> Resposta {
        let pedido = Request::builder()
            .method("POST")
            .uri(format!("/painel/sites/{slug}/midia"))
            .header("host", HOST_DO_PAINEL)
            .header("origin", ORIGEM)
            .header(
                "content-type",
                format!("multipart/form-data; boundary={LIMITE}"),
            )
            .header("cookie", format!("avila_sso={}", token(quem)));
        self.enviar(pedido, Body::from(formulario_de_envio(arquivo, alt)))
            .await
    }
}

async fn imagem_do_site(pool: &PgPool) -> (Uuid, String) {
    sqlx::query_as("select id, situacao from midia")
        .fetch_one(pool)
        .await
        .expect("uma imagem na biblioteca")
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn envio_de_imagem_grava_o_original_e_espera_as_variantes(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let id_do_site = site_id(&pool, "padaria").await;
    const MIDIA: &str = "/painel/sites/padaria/midia";
    let foto = png(1400, 800, 200);

    let vazia = painel.abrir(Some("ana"), MIDIA).await;
    assert_eq!(vazia.status, StatusCode::OK);
    assert!(vazia.corpo.contains("Nenhuma imagem ainda"));

    // Sem descrição, arquivo que não é imagem e imagem corrompida não entram.
    let sem_alt = painel.enviar_imagem("ana", "padaria", &foto, "  ").await;
    assert_eq!(sem_alt.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(sem_alt.corpo.contains("Escreva uma descrição da foto"));
    let nao_e_imagem = painel
        .enviar_imagem("ana", "padaria", b"%PDF-1.7 nada de imagem", "Um PDF")
        .await;
    assert_eq!(nao_e_imagem.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(nao_e_imagem.corpo.contains("JPG, PNG ou WebP"));
    assert!(nao_e_imagem.corpo.contains(r#"value="Um PDF""#));
    let corrompida = painel
        .enviar_imagem("ana", "padaria", &foto[..40], "Foto cortada")
        .await;
    assert_eq!(corrompida.status, StatusCode::UNPROCESSABLE_ENTITY);

    let enviada = painel
        .enviar_imagem("ana", "padaria", &foto, "Vitrine da padaria com pães")
        .await;
    assert_eq!(enviada.status, StatusCode::OK);
    assert!(enviada.corpo.contains("Imagem enviada"));
    assert!(enviada.corpo.contains("Vitrine da padaria com pães"));
    assert!(enviada.corpo.contains("1400 × 800 · preparando · sem uso"));
    let (id, situacao) = imagem_do_site(&pool).await;
    assert_eq!(situacao, "pendente");
    let original = cms_web::caminho_do_original(&painel.diretorio, id_do_site, id);
    assert_eq!(std::fs::read(&original).expect("original no disco"), foto);

    // O mesmo arquivo não entra duas vezes.
    let repetida = painel
        .enviar_imagem("ana", "padaria", &foto, "A mesma foto")
        .await;
    assert_eq!(repetida.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(repetida.corpo.contains("já está na biblioteca"));

    // Quem não é do site não vê nem envia; formulário de fora não vale.
    assert_eq!(
        painel.abrir(Some("caio"), MIDIA).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        painel
            .enviar_imagem("caio", "padaria", &png(10, 10, 1), "Outra")
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let apagada = painel
        .postar("ana", &format!("{MIDIA}/{id}/apagar"), "")
        .await;
    assert_eq!(apagada.status, StatusCode::OK);
    assert!(apagada.corpo.contains("Imagem apagada"));
    assert!(!original.exists());
    std::fs::remove_dir_all(&painel.diretorio).expect("pasta removida");
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn imagem_em_uso_nao_e_apagada_e_sem_variante_nao_vai_ao_ar(pool: PgPool) {
    use cms_dados::fluxo::{self, ErroDeFluxo};
    use motor_web::tipos::{Conteudo, Formato};

    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let id_do_site = site_id(&pool, "padaria").await;
    painel
        .enviar_imagem(
            "ana",
            "padaria",
            &png(1400, 800, 10),
            "Vitrine da padaria com pães",
        )
        .await;
    let (id, _) = imagem_do_site(&pool).await;
    let dona = Ator::do_site("sub-ana", Papel::Dono);

    // Uma página com a imagem da biblioteca como capa.
    let mut conteudo = motor_web::demonstracao::documentos()
        .into_iter()
        .find(|documento| documento.caminho == "/sobre")
        .expect("o exemplo tem /sobre")
        .conteudo;
    if let Conteudo::Pagina(pagina) = &mut conteudo {
        pagina.capa = cms_dados::midia_para_conteudo(&pool, id_do_site, id)
            .await
            .expect("consulta");
        assert!(pagina.capa.is_some());
    }
    let salvo = fluxo::salvar_rascunho(&pool, id_do_site, &dona, None, &conteudo)
        .await
        .expect("rascunho salvo");
    assert!(
        salvo
            .problemas
            .iter()
            .any(|problema| problema.codigo == "midia.sem-variante")
    );
    assert!(matches!(
        fluxo::publicar(
            &pool,
            id_do_site,
            &dona,
            salvo.documento_id,
            chrono::Utc::now()
        )
        .await,
        Err(ErroDeFluxo::Recusado(_))
    ));

    // Em uso, nem o Dono apaga, e a lista diz onde está.
    let lista = painel
        .abrir(Some("ana"), "/painel/sites/padaria/midia")
        .await;
    assert!(lista.corpo.contains("em 1 conteúdo"));
    let recusada = painel
        .postar(
            "ana",
            &format!("/painel/sites/padaria/midia/{id}/apagar"),
            "",
        )
        .await;
    assert_eq!(recusada.status, StatusCode::CONFLICT);
    assert!(recusada.corpo.contains("está em uso"));

    // Com as variantes prontas, a publicação passa e a página as usa, mesmo
    // tendo sido salva antes de elas existirem.
    let variante = cms_dados::VarianteGravada {
        formato: Formato::Webp,
        largura: 1400,
        arquivo: "vitrine-da-padaria-0a1b2c3d-1400.webp".into(),
        bytes: 1234,
    };
    cms_dados::concluir_midia(&pool, id, 1400, 800, &[variante])
        .await
        .expect("imagem pronta");
    fluxo::publicar(
        &pool,
        id_do_site,
        &dona,
        salvo.documento_id,
        chrono::Utc::now(),
    )
    .await
    .expect("publicado");
    let pagina = pedir(&pool, &host("padaria"), "/sobre").await;
    assert_eq!(pagina.status, StatusCode::OK);
    assert!(
        pagina
            .corpo
            .contains("/midia/vitrine-da-padaria-0a1b2c3d-1400.webp")
    );
    std::fs::remove_dir_all(&painel.diretorio).expect("pasta removida");
}

/// Um formulário do editor, codificado como o navegador manda.
fn formulario(campos: &[(&str, &str)]) -> String {
    serde_urlencoded::to_string(campos).expect("formulário codificado")
}

/// O endereço do documento para onde o editor redirecionou, sem o recado.
fn documento_de(resposta: &Resposta) -> String {
    assert_eq!(resposta.status, StatusCode::SEE_OTHER);
    let destino = resposta.cabecalho("location");
    destino.split('?').next().unwrap_or(destino).to_string()
}

const PAGINA: [(&str, &str); 8] = [
    ("especie", "pagina"),
    ("titulo", "Sobre a padaria"),
    ("slug", "sobre"),
    ("seo_titulo", "Sobre a Padaria da Ana"),
    (
        "seo_descricao",
        "Conheça a história e o jeito de trabalhar da Padaria da Ana, no bairro desde 1998.",
    ),
    ("indexar", "1"),
    ("n", "1"),
    ("b0_tipo", "paragrafo"),
];

fn pagina_com(extras: &[(&str, &str)]) -> String {
    let mut campos = PAGINA.to_vec();
    campos.extend_from_slice(extras);
    formulario(&campos)
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn escrever_rever_e_publicar_uma_pagina_pelo_painel(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    const SITE: &str = "/painel/sites/padaria";

    let vazio = painel.abrir(Some("ana"), SITE).await;
    assert_eq!(vazio.status, StatusCode::OK);
    assert!(vazio.corpo.contains("ainda não tem conteúdo"));
    let novo = painel
        .abrir(Some("ana"), &format!("{SITE}/novo/pagina"))
        .await;
    assert!(novo.corpo.contains("Nova página"));
    assert_eq!(
        painel
            .abrir(Some("ana"), &format!("{SITE}/novo/produto"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let texto = "A **Padaria** abre cedo. Veja [o cardápio](/cardapio).";
    let salvo = painel
        .postar(
            "ana",
            &format!("{SITE}/doc"),
            &pagina_com(&[("b0_texto", texto), ("acao", "salvar")]),
        )
        .await;
    assert!(salvo.cabecalho("location").ends_with("?r=salvo"));
    let documento = documento_de(&salvo);

    let editor = painel
        .abrir(Some("ana"), &format!("{documento}?r=salvo"))
        .await;
    assert_eq!(editor.status, StatusCode::OK);
    assert!(editor.corpo.contains("Rascunho salvo."));
    assert!(editor.corpo.contains(r#"value="Sobre a padaria""#));
    // O parágrafo volta na marcação em que foi digitado.
    assert!(
        editor
            .corpo
            .contains("A **Padaria** abre cedo. Veja [o cardápio](/cardapio).")
    );
    assert!(
        painel
            .abrir(Some("ana"), SITE)
            .await
            .corpo
            .contains("Página · Rascunho")
    );
    assert_eq!(
        pedir(&pool, &host("padaria"), "/sobre").await.status,
        StatusCode::NOT_FOUND
    );

    // A prévia mostra a página pelo template do site, sem pôr nada no ar.
    let previa = painel
        .abrir(Some("ana"), &format!("{documento}/previa"))
        .await;
    assert_eq!(previa.status, StatusCode::OK);
    assert!(previa.corpo.contains("<h1>Sobre a padaria</h1>"));
    assert!(previa.corpo.contains("<strong>Padaria</strong>"));
    assert!(
        previa
            .corpo
            .contains(r#"<a href="/cardapio">o cardápio</a>"#)
    );
    assert_eq!(previa.cabecalho("x-robots-tag"), "noindex");
    assert_eq!(previa.cabecalho("cache-control"), "private, no-store");

    // Acrescentar um bloco salva e devolve o editor com ele.
    let com_bloco = painel
        .postar(
            "ana",
            &documento,
            &pagina_com(&[
                ("b0_texto", texto),
                ("novo_tipo", "titulo"),
                ("acao", "adicionar"),
            ]),
        )
        .await;
    let editor = painel.abrir(Some("ana"), &documento_de(&com_bloco)).await;
    assert!(editor.corpo.contains(r#"name="b1_tipo" value="titulo""#));
    assert!(editor.corpo.contains(r#"name="n" value="2""#));

    let publicado = painel
        .postar(
            "ana",
            &documento,
            &pagina_com(&[
                ("b0_texto", texto),
                ("n", "2"),
                ("b1_tipo", "titulo"),
                ("b1_nivel", "2"),
                ("b1_texto", "Nossa história"),
                ("acao", "publicar"),
            ]),
        )
        .await;
    assert!(publicado.cabecalho("location").ends_with("?r=publicado"));
    let no_ar = pedir(&pool, &host("padaria"), "/sobre").await;
    assert_eq!(no_ar.status, StatusCode::OK);
    assert!(no_ar.corpo.contains("Nossa história</h2>"));
    assert!(
        no_ar
            .corpo
            .contains("<title>Sobre a Padaria da Ana</title>")
    );
    assert!(
        painel
            .abrir(Some("ana"), SITE)
            .await
            .corpo
            .contains("Página · No ar")
    );

    let fora = painel
        .postar("ana", &documento, &formulario(&[("acao", "despublicar")]))
        .await;
    assert!(fora.cabecalho("location").ends_with("?r=despublicado"));
    assert_eq!(
        pedir(&pool, &host("padaria"), "/sobre").await.status,
        StatusCode::GONE
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn publicar_incompleto_salva_e_mostra_o_que_falta(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let incompleto = formulario(&[
        ("especie", "pagina"),
        ("titulo", "Contato"),
        ("slug", "blog"),
        ("n", "0"),
        ("acao", "publicar"),
    ]);
    let recusado = painel
        .postar("ana", "/painel/sites/padaria/doc", &incompleto)
        .await;
    assert!(recusado.cabecalho("location").ends_with("?r=recusado"));

    let editor = painel
        .abrir(
            Some("ana"),
            &format!("{}?r=recusado", documento_de(&recusado)),
        )
        .await;
    assert!(editor.corpo.contains("corrija o que está em vermelho"));
    assert!(editor.corpo.contains(r#"class="bloqueia""#));
    assert!(
        editor
            .corpo
            .contains("Escreva o título que vai aparecer no Google.")
    );
    assert!(editor.corpo.contains("é usado pelo próprio site"));
    // O que foi digitado ficou salvo.
    assert!(editor.corpo.contains(r#"value="Contato""#));
    assert_eq!(
        pedir(&pool, &host("padaria"), "/blog").await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn autor_escreve_e_pede_revisao_e_o_dono_devolve(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let convite = painel
        .postar(
            "ana",
            "/painel/sites/padaria/convites",
            "email=bia%40exemplo.example&papel=autor",
        )
        .await;
    painel
        .abrir(Some("bia"), &caminho_do_convite(&convite.corpo))
        .await;

    let corpo = |acao: &str| pagina_com(&[("b0_texto", "Texto da Bia."), ("acao", acao)]);
    let salvo = painel
        .postar("bia", "/painel/sites/padaria/doc", &corpo("salvar"))
        .await;
    let documento = documento_de(&salvo);

    // Autor não vê o botão de publicar, e o pedido direto é recusado.
    let editor = painel.abrir(Some("bia"), &documento).await;
    assert!(editor.corpo.contains(r#"value="revisar""#));
    assert!(!editor.corpo.contains(r#"value="publicar""#));
    let negado = painel.postar("bia", &documento, &corpo("publicar")).await;
    assert!(negado.cabecalho("location").ends_with("?r=sem-permissao"));

    let pedido = painel.postar("bia", &documento, &corpo("revisar")).await;
    assert!(pedido.cabecalho("location").ends_with("?r=revisao"));
    let da_dona = painel.abrir(Some("ana"), &documento).await;
    assert!(da_dona.corpo.contains("Em revisão"));
    assert!(da_dona.corpo.contains(r#"value="devolver""#));
    let devolvido = painel
        .postar("ana", &documento, &formulario(&[("acao", "devolver")]))
        .await;
    assert!(devolvido.cabecalho("location").ends_with("?r=devolvido"));

    // Quem não é do site não abre o documento nem a prévia.
    for caminho in [documento.clone(), format!("{documento}/previa")] {
        assert_eq!(
            painel.abrir(Some("caio"), &caminho).await.status,
            StatusCode::NOT_FOUND,
            "{caminho}"
        );
    }
    assert_eq!(
        painel
            .postar("caio", &documento, &corpo("publicar"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn post_sai_com_autor_categoria_e_capa_do_cadastro(pool: PgPool) {
    use motor_web::tipos::Formato;

    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    const SITE: &str = "/painel/sites/padaria";

    // Uma imagem pronta na biblioteca, larga o bastante para capa.
    painel
        .enviar_imagem(
            "ana",
            "padaria",
            &png(1400, 800, 77),
            "Fornada de pães saindo do forno",
        )
        .await;
    let (imagem, _) = imagem_do_site(&pool).await;
    let variante = cms_dados::VarianteGravada {
        formato: Formato::Webp,
        largura: 1400,
        arquivo: "fornada-0a1b2c3d-1400.webp".into(),
        bytes: 999,
    };
    cms_dados::concluir_midia(&pool, imagem, 1400, 800, &[variante])
        .await
        .expect("imagem pronta");
    let imagem = imagem.to_string();

    let autor = formulario(&[
        ("nome", "Ana Souza"),
        ("cargo", "Padeira"),
        ("bio", "Faz pão de fermentação natural desde 1998."),
        ("foto", &imagem),
        ("credenciais", "Curso de panificação artesanal"),
    ]);
    let salvo = painel
        .postar("ana", &format!("{SITE}/autores"), &autor)
        .await;
    assert!(salvo.cabecalho("location").ends_with("/catalogo?r=autor"));
    let salva = painel
        .postar("ana", &format!("{SITE}/categorias"), "nome=Receitas")
        .await;
    assert!(
        salva
            .cabecalho("location")
            .ends_with("/catalogo?r=categoria")
    );
    let catalogo = painel.abrir(Some("ana"), &format!("{SITE}/catalogo")).await;
    assert!(catalogo.corpo.contains("Ana Souza"));
    assert!(catalogo.corpo.contains("/blog/categoria/receitas"));

    let post = formulario(&[
        ("especie", "post"),
        ("titulo", "Como fazer pão de fermentação natural"),
        ("slug", "pao-de-fermentacao-natural"),
        ("tipo_do_post", "guia-tecnico"),
        ("resumo", "O passo a passo do fermento ao forno."),
        ("autor", "ana-souza"),
        ("categoria", "receitas"),
        ("tags", "pão, fermento"),
        ("capa", &imagem),
        ("seo_titulo", "Pão de fermentação natural: passo a passo"),
        (
            "seo_descricao",
            "Aprenda a fazer pão de fermentação natural em casa, do fermento ao forno, com a Padaria da Ana.",
        ),
        ("indexar", "1"),
        ("n", "2"),
        ("b0_tipo", "paragrafo"),
        ("b0_texto", "Comece pelo fermento."),
        ("b1_tipo", "imagem"),
        ("b1_midia", &imagem),
        ("acao", "publicar"),
    ]);
    let publicado = painel.postar("ana", &format!("{SITE}/doc"), &post).await;
    assert!(
        publicado.cabecalho("location").ends_with("?r=publicado"),
        "{}",
        publicado.cabecalho("location")
    );

    let no_ar = pedir(&pool, &host("padaria"), "/blog/pao-de-fermentacao-natural").await;
    assert_eq!(no_ar.status, StatusCode::OK);
    assert!(no_ar.corpo.contains("Ana Souza"));
    assert!(no_ar.corpo.contains("Curso de panificação artesanal"));
    assert!(no_ar.corpo.contains("/midia/fornada-0a1b2c3d-1400.webp"));
    assert!(
        pedir(&pool, &host("padaria"), "/blog")
            .await
            .corpo
            .contains("Como fazer pão")
    );

    // O editor reabre o post com o que foi escolhido.
    let editor = painel.abrir(Some("ana"), &documento_de(&publicado)).await;
    assert!(
        editor
            .corpo
            .contains(r#"<option value="ana-souza" selected>"#)
    );
    assert!(
        editor
            .corpo
            .contains(r#"<option value="receitas" selected>"#)
    );
    assert!(
        editor
            .corpo
            .contains(r#"<option value="guia-tecnico" selected>"#)
    );
    // Em uso na capa, no corpo e na foto do autor: a imagem não é apagada.
    let recusada = painel
        .postar("ana", &format!("{SITE}/midia/{imagem}/apagar"), "")
        .await;
    assert_eq!(recusada.status, StatusCode::CONFLICT);
    std::fs::remove_dir_all(&painel.diretorio).expect("pasta removida");
}

const RETORNO: &str = "https://assistente.example/callback";
// O par de exemplo da RFC 7636.
const VERIFICADOR: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const DESAFIO: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

impl Painel {
    /// Um pedido do assistente: sem cookie, com corpo e, se houver, token.
    async fn do_assistente(
        &self,
        caminho: &str,
        tipo: &str,
        token: Option<&str>,
        corpo: String,
    ) -> Resposta {
        let mut pedido = Request::builder()
            .method("POST")
            .uri(caminho)
            .header("host", HOST_DO_PAINEL)
            .header("content-type", tipo);
        if let Some(token) = token {
            pedido = pedido.header("authorization", format!("Bearer {token}"));
        }
        self.enviar(pedido, Body::from(corpo)).await
    }

    async fn registrar_assistente(&self) -> String {
        let resposta = self
            .do_assistente(
                "/oauth/register",
                "application/json",
                None,
                json!({ "client_name": "Assistente de teste", "redirect_uris": [RETORNO] })
                    .to_string(),
            )
            .await;
        assert_eq!(resposta.status, StatusCode::CREATED, "{}", resposta.corpo);
        let corpo: Value = serde_json::from_str(&resposta.corpo).expect("json");
        corpo["client_id"].as_str().expect("client_id").to_string()
    }

    /// A pessoa autoriza na tela, com os escopos marcados. Devolve o código.
    async fn autorizar(&self, quem: &str, cliente: &str, escopos: &[&str]) -> String {
        let mut campos = vec![
            ("response_type", "code"),
            ("client_id", cliente),
            ("redirect_uri", RETORNO),
            ("code_challenge", DESAFIO),
            ("code_challenge_method", "S256"),
            ("state", "xyz"),
            ("decisao", "autorizar"),
        ];
        campos.extend(escopos.iter().map(|escopo| ("escopo", *escopo)));
        let resposta = self
            .postar(quem, "/oauth/authorize", &formulario(&campos))
            .await;
        assert_eq!(resposta.status, StatusCode::SEE_OTHER, "{}", resposta.corpo);
        let destino = resposta.cabecalho("location");
        assert!(
            destino.starts_with(&format!("{RETORNO}?code=")),
            "{destino}"
        );
        assert!(destino.ends_with("&state=xyz"), "{destino}");
        destino
            .trim_start_matches(&format!("{RETORNO}?code="))
            .trim_end_matches("&state=xyz")
            .to_string()
    }

    async fn trocar(&self, campos: &[(&str, &str)]) -> (StatusCode, Value) {
        let resposta = self
            .do_assistente(
                "/oauth/token",
                "application/x-www-form-urlencoded",
                None,
                formulario(campos),
            )
            .await;
        (
            resposta.status,
            serde_json::from_str(&resposta.corpo).expect("json"),
        )
    }

    /// Conecta um assistente à conta e devolve (cliente, acesso, renovação).
    async fn conectar(&self, quem: &str, escopos: &[&str]) -> (String, String, String) {
        let cliente = self.registrar_assistente().await;
        let codigo = self.autorizar(quem, &cliente, escopos).await;
        let (status, tokens) = self
            .trocar(&[
                ("grant_type", "authorization_code"),
                ("code", &codigo),
                ("code_verifier", VERIFICADOR),
                ("client_id", &cliente),
                ("redirect_uri", RETORNO),
            ])
            .await;
        assert_eq!(status, StatusCode::OK, "{tokens}");
        let texto = |campo: &str| tokens[campo].as_str().expect("token").to_string();
        (cliente, texto("access_token"), texto("refresh_token"))
    }

    async fn rpc(&self, token: &str, metodo: &str, parametros: Value) -> Value {
        let resposta = self
            .do_assistente(
                "/mcp",
                "application/json",
                Some(token),
                json!({ "jsonrpc": "2.0", "id": 1, "method": metodo, "params": parametros })
                    .to_string(),
            )
            .await;
        assert_eq!(resposta.status, StatusCode::OK, "{}", resposta.corpo);
        serde_json::from_str(&resposta.corpo).expect("json")
    }

    /// Chama uma ferramenta. Devolve se deu erro e o que ela respondeu.
    async fn ferramenta(&self, token: &str, nome: &str, argumentos: Value) -> (bool, Value) {
        let resposta = self
            .rpc(
                token,
                "tools/call",
                json!({ "name": nome, "arguments": argumentos }),
            )
            .await;
        let resultado = &resposta["result"];
        let texto = resultado["content"][0]["text"].as_str().expect("texto");
        (
            resultado["isError"].as_bool().expect("isError"),
            serde_json::from_str(texto).unwrap_or_else(|_| json!(texto)),
        )
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn assistente_so_se_conecta_com_registro_consentimento_e_pkce(pool: PgPool) {
    let painel = Painel::novo(&pool).await;

    let metadados = painel
        .abrir(None, "/.well-known/oauth-authorization-server")
        .await;
    assert_eq!(metadados.status, StatusCode::OK);
    let metadados: Value = serde_json::from_str(&metadados.corpo).expect("json");
    assert_eq!(
        metadados["authorization_endpoint"],
        "https://cms.teste/oauth/authorize"
    );
    assert_eq!(
        metadados["code_challenge_methods_supported"],
        json!(["S256"])
    );

    // Retorno em http fora da máquina de quem usa não é registrado.
    let ruim = painel
        .do_assistente(
            "/oauth/register",
            "application/json",
            None,
            json!({ "redirect_uris": ["http://golpe.example/cb"] }).to_string(),
        )
        .await;
    assert_eq!(ruim.status, StatusCode::BAD_REQUEST);

    let cliente = painel.registrar_assistente().await;
    let pedido = format!(
        "/oauth/authorize?response_type=code&client_id={cliente}&redirect_uri={}&code_challenge={DESAFIO}&code_challenge_method=S256&state=xyz",
        "https%3A%2F%2Fassistente.example%2Fcallback"
    );
    // Sem sessão, a pessoa entra e volta para o mesmo pedido.
    let sem_sessao = painel.abrir(None, &pedido).await;
    assert_eq!(sem_sessao.status, StatusCode::FOUND);
    assert!(sem_sessao.cabecalho("location").contains("code_challenge"));

    let tela = painel.abrir(Some("ana"), &pedido).await;
    assert_eq!(tela.status, StatusCode::OK);
    assert!(tela.corpo.contains("Assistente de teste"));
    assert!(tela.corpo.contains(r#"value="conteudo:escrever" checked"#));
    // Publicar nunca vem marcado.
    assert!(tela.corpo.contains(r#"value="conteudo:publicar">"#));

    // Cliente desconhecido, retorno não registrado e PKCE fraco param na tela.
    for errado in [
        pedido.replace(&cliente, &Uuid::new_v4().to_string()),
        pedido.replace("assistente.example", "outro.example"),
        pedido.replace("S256", "plain"),
    ] {
        assert_eq!(
            painel.abrir(Some("ana"), &errado).await.status,
            StatusCode::BAD_REQUEST
        );
    }

    // Recusar volta ao cliente com a recusa, sem código.
    let recusado = painel
        .postar(
            "ana",
            "/oauth/authorize",
            &formulario(&[
                ("response_type", "code"),
                ("client_id", &cliente),
                ("redirect_uri", RETORNO),
                ("code_challenge", DESAFIO),
                ("code_challenge_method", "S256"),
                ("state", "xyz"),
                ("decisao", "recusar"),
            ]),
        )
        .await;
    assert!(
        recusado
            .cabecalho("location")
            .contains("error=access_denied")
    );

    // Verificador errado gasta o código: nem o certo o troca depois.
    let codigo = painel.autorizar("ana", &cliente, &["sites:ler"]).await;
    let troca = |verificador: &'static str| {
        let (codigo, cliente) = (codigo.clone(), cliente.clone());
        let painel = &painel;
        async move {
            painel
                .trocar(&[
                    ("grant_type", "authorization_code"),
                    ("code", &codigo),
                    ("code_verifier", verificador),
                    ("client_id", &cliente),
                    ("redirect_uri", RETORNO),
                ])
                .await
        }
    };
    let (status, corpo) = troca("verificador-errado-verificador-errado-12345").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(corpo["error"], "invalid_grant");
    assert_eq!(troca(VERIFICADOR).await.0, StatusCode::BAD_REQUEST);

    // Sem token, o ponto MCP diz onde pedir acesso.
    let sem_token = painel
        .do_assistente("/mcp", "application/json", None, "{}".to_string())
        .await;
    assert_eq!(sem_token.status, StatusCode::UNAUTHORIZED);
    assert!(
        sem_token
            .cabecalho("www-authenticate")
            .contains("https://cms.teste/.well-known/oauth-protected-resource")
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn assistente_escreve_valida_e_envia_para_revisao_sem_publicar(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    painel
        .criar_site("caio", "oficina", "Oficina do Caio")
        .await;
    let (cliente, token, renovacao) = painel
        .conectar("ana", &["sites:ler", "conteudo:ler", "conteudo:escrever"])
        .await;

    let inicio = painel.rpc(&token, "initialize", json!({})).await;
    assert_eq!(inicio["result"]["serverInfo"]["name"], "cms-avila-ops");
    let lista = painel.rpc(&token, "tools/list", json!({})).await;
    assert_eq!(
        lista["result"]["tools"].as_array().expect("lista").len(),
        18
    );

    let (erro, sites) = painel.ferramenta(&token, "listar_sites", json!({})).await;
    assert!(!erro);
    assert_eq!(
        sites["sites"],
        json!([{ "site": "padaria", "nome": "Padaria da Ana", "situacao": "em-montagem", "papel": "dono" }])
    );

    // O site de outra conta não existe para esta conexão.
    let (erro, resposta) = painel
        .ferramenta(&token, "listar_documentos", json!({ "site": "oficina" }))
        .await;
    assert!(erro);
    assert!(resposta.as_str().expect("frase").contains("não encontrado"));

    // Um rascunho incompleto é salvo, e os problemas voltam com código e campo.
    let conteudo = |descricao: &str| {
        json!({
            "especie": "pagina",
            "dados": {
                "slug": "contato",
                "titulo": "Contato",
                "corpo": [{ "tipo": "paragrafo", "trechos": [{ "texto": "Fale com a gente." }] }],
                "seo": { "titulo": "Contato da Padaria da Ana", "descricao": descricao, "indexar": true }
            }
        })
    };
    let (erro, criado) = painel
        .ferramenta(
            &token,
            "criar_rascunho",
            json!({ "site": "padaria", "conteudo": conteudo("") }),
        )
        .await;
    assert!(!erro, "{criado}");
    let documento = criado["documento"].as_str().expect("documento").to_string();
    assert!(
        criado["problemas"]
            .as_array()
            .expect("problemas")
            .iter()
            .any(|p| p["codigo"] == "seo.descricao.vazia" && p["campo"] == "seo.descricao")
    );
    let alvo = json!({ "site": "padaria", "documento": documento });
    let (erro, recusa) = painel
        .ferramenta(&token, "enviar_para_revisao", alvo.clone())
        .await;
    assert!(erro);
    assert!(
        recusa
            .as_str()
            .expect("frase")
            .contains("seo.descricao.vazia")
    );

    // Corrige, confere e envia para revisão.
    let corrigido = conteudo("Telefone, endereço e horário de atendimento da Padaria da Ana.");
    let (erro, _) = painel
        .ferramenta(
            &token,
            "editar_rascunho",
            json!({ "site": "padaria", "documento": documento, "conteudo": corrigido }),
        )
        .await;
    assert!(!erro);
    let (_, validado) = painel
        .ferramenta(&token, "validar_documento", alvo.clone())
        .await;
    assert!(
        validado["problemas"]
            .as_array()
            .expect("problemas")
            .iter()
            .all(|p| p["gravidade"] != "bloqueia")
    );
    let (erro, enviado) = painel
        .ferramenta(&token, "enviar_para_revisao", alvo.clone())
        .await;
    assert!(!erro, "{enviado}");

    // A conexão não tem o escopo de publicar nem o de imagens: quem publica é gente.
    for ferramenta in ["publicar", "listar_midia"] {
        let (erro, resposta) = painel.ferramenta(&token, ferramenta, alvo.clone()).await;
        assert!(erro, "{ferramenta}");
        assert!(
            resposta
                .as_str()
                .expect("frase")
                .contains("não tem a permissão")
        );
    }
    assert_eq!(
        pedir(&pool, &host("padaria"), "/contato").await.status,
        StatusCode::NOT_FOUND
    );

    // O registro guarda a ferramenta e os identificadores, e se deu certo.
    let chamadas: Vec<(String, bool, bool)> = sqlx::query_as(
        "select ferramenta, deu_certo, documento_id is not null from chamada_mcp order by id",
    )
    .fetch_all(&pool)
    .await
    .expect("chamadas");
    assert_eq!(chamadas.len(), 9, "{chamadas:?}");
    assert_eq!(chamadas[2], ("criar_rascunho".to_string(), true, true));
    assert_eq!(chamadas[7], ("publicar".to_string(), false, false));

    // Renovar troca o par: o token antigo deixa de valer.
    let (status, novos) = painel
        .trocar(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &renovacao),
            ("client_id", &cliente),
        ])
        .await;
    assert_eq!(status, StatusCode::OK, "{novos}");
    let novo = novos["access_token"].as_str().expect("token").to_string();
    let antigo = painel
        .do_assistente("/mcp", "application/json", Some(&token), "{}".to_string())
        .await;
    assert_eq!(antigo.status, StatusCode::UNAUTHORIZED);
    assert!(!painel.ferramenta(&novo, "listar_sites", json!({})).await.0);

    // A pessoa vê a conexão no painel e a corta.
    let tela = painel.abrir(Some("ana"), "/painel/conector").await;
    assert!(tela.corpo.contains("Assistente de teste"));
    assert!(tela.corpo.contains("https://cms.teste/mcp"));
    let conexao: Uuid = sqlx::query_scalar("select id from conexao_mcp")
        .fetch_one(&pool)
        .await
        .expect("conexão");
    // Outra conta não desconecta o assistente de ninguém.
    painel
        .postar("caio", &format!("/painel/conector/{conexao}/revogar"), "")
        .await;
    assert!(!painel.ferramenta(&novo, "listar_sites", json!({})).await.0);
    painel
        .postar("ana", &format!("/painel/conector/{conexao}/revogar"), "")
        .await;
    let cortado = painel
        .do_assistente("/mcp", "application/json", Some(&novo), "{}".to_string())
        .await;
    assert_eq!(cortado.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn assistente_com_permissao_envia_imagem_e_publica(pool: PgPool) {
    use base64::Engine;

    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let (_, token, _) = painel
        .conectar(
            "ana",
            &[
                "conteudo:ler",
                "conteudo:escrever",
                "conteudo:publicar",
                "midia:escrever",
            ],
        )
        .await;

    let base64 = base64::engine::general_purpose::STANDARD.encode(png(1400, 800, 33));
    let (erro, sem_alt) = painel
        .ferramenta(
            &token,
            "enviar_midia",
            json!({ "site": "padaria", "nome": "fachada.png", "alt": " ", "base64": base64 }),
        )
        .await;
    assert!(erro, "{sem_alt}");
    let (erro, enviada) = painel
        .ferramenta(
            &token,
            "enviar_midia",
            json!({ "site": "padaria", "nome": "fachada.png", "alt": "Fachada da padaria", "base64": base64 }),
        )
        .await;
    assert!(!erro, "{enviada}");
    let (_, midias) = painel
        .ferramenta(&token, "listar_midia", json!({ "site": "padaria" }))
        .await;
    assert_eq!(midias["midias"][0]["alt"], "Fachada da padaria");

    let pagina = json!({
        "especie": "pagina",
        "dados": {
            "slug": "",
            "titulo": "Pão fresco todo dia",
            "corpo": [{ "tipo": "titulo", "nivel": 2, "texto": "Onde estamos" }],
            "seo": {
                "titulo": "Padaria da Ana: pão fresco todo dia",
                "descricao": "Padaria de bairro com pão de fermentação natural, aberta desde 1998.",
                "indexar": true
            }
        }
    });
    let (_, criado) = painel
        .ferramenta(
            &token,
            "criar_rascunho",
            json!({ "site": "padaria", "conteudo": pagina }),
        )
        .await;
    let alvo = json!({ "site": "padaria", "documento": criado["documento"] });
    let (erro, publicado) = painel.ferramenta(&token, "publicar", alvo.clone()).await;
    assert!(!erro, "{publicado}");
    let home = pedir(&pool, &host("padaria"), "/").await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(home.corpo.contains("<h1>Pão fresco todo dia</h1>"));

    let (_, visto) = painel
        .ferramenta(&token, "ver_documento", alvo.clone())
        .await;
    assert_eq!(visto["situacao"], "publicado");
    assert_eq!(visto["conteudo"]["dados"]["titulo"], "Pão fresco todo dia");
    let (erro, _) = painel.ferramenta(&token, "despublicar", alvo).await;
    assert!(!erro);
    assert_eq!(
        pedir(&pool, &host("padaria"), "/").await.status,
        StatusCode::GONE
    );
    std::fs::remove_dir_all(&painel.diretorio).expect("pasta removida");
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn assistente_cadastra_autor_categoria_e_identidade_e_escreve_um_post(pool: PgPool) {
    use base64::Engine;
    use motor_web::tipos::Formato;

    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let (_, token, _) = painel
        .conectar(
            "ana",
            &[
                "sites:ler",
                "sites:editar",
                "conteudo:ler",
                "conteudo:escrever",
                "midia:escrever",
            ],
        )
        .await;

    let base64 = base64::engine::general_purpose::STANDARD.encode(png(1400, 800, 51));
    let (erro, enviada) = painel
        .ferramenta(
            &token,
            "enviar_midia",
            json!({ "site": "padaria", "nome": "ana.png", "alt": "Ana Souza na padaria", "base64": base64 }),
        )
        .await;
    assert!(!erro, "{enviada}");
    let imagem = enviada["id"].as_str().expect("id").to_string();
    let variante = cms_dados::VarianteGravada {
        formato: Formato::Webp,
        largura: 1400,
        arquivo: "ana-0a1b2c3d-1400.webp".into(),
        bytes: 999,
    };
    let imagem_id = Uuid::parse_str(&imagem).expect("uuid");
    cms_dados::concluir_midia(&pool, imagem_id, 1400, 800, &[variante])
        .await
        .expect("imagem pronta");

    // Autor e categoria nascem pelo conector, com o endereço tirado do nome.
    let (erro, autor) = painel
        .ferramenta(
            &token,
            "salvar_autor",
            json!({
                "site": "padaria",
                "nome": "Ana Souza",
                "cargo": "Padeira",
                "bio": "Faz pão de fermentação natural desde 1998.",
                "foto": imagem,
                "credenciais": "Curso de panificação artesanal\nVinte anos de balcão",
            }),
        )
        .await;
    assert!(!erro, "{autor}");
    assert_eq!(autor["slug"], "ana-souza");
    let (erro, categoria) = painel
        .ferramenta(
            &token,
            "salvar_categoria",
            json!({ "site": "padaria", "nome": "Receitas" }),
        )
        .await;
    assert!(!erro, "{categoria}");
    assert_eq!(categoria["slug"], "receitas");
    let (_, autores) = painel
        .ferramenta(&token, "listar_autores", json!({ "site": "padaria" }))
        .await;
    assert_eq!(autores["autores"][0]["foto"], imagem);
    assert_eq!(
        autores["autores"][0]["credenciais"],
        json!(["Curso de panificação artesanal", "Vinte anos de balcão"])
    );
    let (erro, recusa) = painel
        .ferramenta(
            &token,
            "salvar_autor",
            json!({ "site": "padaria", "nome": "Outro", "foto": Uuid::new_v4() }),
        )
        .await;
    assert!(erro);
    assert!(recusa.as_str().expect("frase").contains("biblioteca"));

    // A identidade muda só no que foi enviado.
    let (erro, editada) = painel
        .ferramenta(
            &token,
            "editar_identidade",
            json!({
                "site": "padaria",
                "descricao": "Padaria de bairro com fermentação natural.",
                "telefone": "(11) 4000-0000",
                "logo": imagem,
            }),
        )
        .await;
    assert!(!erro, "{editada}");
    let (_, site) = painel
        .ferramenta(&token, "ver_site", json!({ "site": "padaria" }))
        .await;
    assert_eq!(site["identidade"]["nome"], "Padaria da Ana");
    assert_eq!(site["identidade"]["telefone"], "(11) 4000-0000");
    assert_eq!(site["identidade"]["logo"], imagem);
    let (erro, sem_nome) = painel
        .ferramenta(
            &token,
            "editar_identidade",
            json!({ "site": "padaria", "nome": " " }),
        )
        .await;
    assert!(erro, "{sem_nome}");

    // No post, autor, categoria e imagens vão só pela referência.
    let post = |autor: &str| {
        json!({
            "especie": "post",
            "dados": {
                "tipo": "guia-tecnico",
                "slug": "pao-de-fermentacao-natural",
                "titulo": "Como fazer pão de fermentação natural",
                "resumo": "O passo a passo do fermento ao forno.",
                "capa": imagem,
                "corpo": [
                    { "tipo": "paragrafo", "trechos": [{ "texto": "Comece pelo fermento." }] },
                    { "tipo": "imagem", "midia": imagem }
                ],
                "autor": autor,
                "categoria": "receitas",
                "seo": {
                    "titulo": "Pão de fermentação natural: passo a passo",
                    "descricao": "Aprenda a fazer pão de fermentação natural em casa, do fermento ao forno, com a Padaria da Ana.",
                    "indexar": true
                }
            }
        })
    };
    let (erro, recusa) = painel
        .ferramenta(
            &token,
            "criar_rascunho",
            json!({ "site": "padaria", "conteudo": post("ninguem") }),
        )
        .await;
    assert!(erro);
    assert!(recusa.as_str().expect("frase").contains("salvar_autor"));
    let (erro, criado) = painel
        .ferramenta(
            &token,
            "criar_rascunho",
            json!({ "site": "padaria", "conteudo": post("ana-souza") }),
        )
        .await;
    assert!(!erro, "{criado}");
    assert_eq!(criado["problemas"], json!([]), "{criado}");
    let alvo = json!({ "site": "padaria", "documento": criado["documento"] });
    let (_, visto) = painel.ferramenta(&token, "ver_documento", alvo).await;
    let dados = &visto["conteudo"]["dados"];
    assert_eq!(dados["autor"]["nome"], "Ana Souza");
    assert_eq!(dados["autor"]["foto"]["id"], imagem);
    assert_eq!(dados["categoria"]["nome"], "Receitas");
    assert_eq!(dados["capa"]["alt"], "Ana Souza na padaria");
    assert_eq!(dados["corpo"][1]["midia"]["largura"], 1400);

    // Sem a permissão de identidade, a conexão não mexe nela.
    let (_, outro, _) = painel.conectar("ana", &["sites:ler"]).await;
    let (erro, recusa) = painel
        .ferramenta(
            &outro,
            "editar_identidade",
            json!({ "site": "padaria", "nome": "Outro nome" }),
        )
        .await;
    assert!(erro);
    assert!(recusa.as_str().expect("frase").contains("sites:editar"));
    std::fs::remove_dir_all(&painel.diretorio).expect("pasta removida");
}

impl Painel {
    /// Um visitante abrindo o site, com o cache de página do teste.
    async fn visitar(&self, host: &str, caminho: &str) -> Resposta {
        let pedido = Request::builder().uri(caminho).header("host", host);
        self.enviar(pedido, Body::empty()).await
    }

    /// Publica a página inicial do site pelo painel e devolve o endereço do
    /// documento.
    async fn publicar_home(&self, quem: &str, slug: &str, titulo: &str) -> String {
        let home = formulario(&[
            ("especie", "pagina"),
            ("titulo", titulo),
            ("slug", ""),
            ("seo_titulo", "Padaria da Ana: pão fresco todo dia"),
            (
                "seo_descricao",
                "Padaria de bairro com pão de fermentação natural, aberta desde 1998.",
            ),
            ("indexar", "1"),
            ("n", "0"),
            ("acao", "publicar"),
        ]);
        let publicado = self
            .postar(quem, &format!("/painel/sites/{slug}/doc"), &home)
            .await;
        assert!(
            publicado.cabecalho("location").ends_with("?r=publicado"),
            "{}",
            publicado.cabecalho("location")
        );
        documento_de(&publicado)
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn pagina_sai_do_cache_ate_alguem_publicar(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let documento = painel
        .publicar_home("ana", "padaria", "Pão fresco todo dia")
        .await;
    let h = host("padaria");

    let primeira = painel.visitar(&h, "/").await;
    assert_eq!(primeira.status, StatusCode::OK);
    assert_eq!(primeira.cabecalho("x-cache"), "miss");
    let segunda = painel.visitar(&h, "/").await;
    assert_eq!(segunda.cabecalho("x-cache"), "hit");
    assert_eq!(segunda.corpo, primeira.corpo);
    assert!(segunda.cabecalho("content-type").starts_with("text/html"));
    // Página que não existe e arquivo de imagem não entram no cache.
    painel.visitar(&h, "/nao-existe").await;
    assert_eq!(
        painel.visitar(&h, "/nao-existe").await.cabecalho("x-cache"),
        ""
    );

    // Publicar derruba o cache do site na hora.
    let novo = formulario(&[
        ("especie", "pagina"),
        ("titulo", "Pão quentinho de hora em hora"),
        ("slug", ""),
        ("seo_titulo", "Padaria da Ana: pão fresco todo dia"),
        (
            "seo_descricao",
            "Padaria de bairro com pão de fermentação natural, aberta desde 1998.",
        ),
        ("indexar", "1"),
        ("n", "0"),
        ("acao", "publicar"),
    ]);
    painel.postar("ana", &documento, &novo).await;
    let depois = painel.visitar(&h, "/").await;
    assert_eq!(depois.cabecalho("x-cache"), "miss");
    assert!(
        depois
            .corpo
            .contains("<h1>Pão quentinho de hora em hora</h1>")
    );

    painel
        .postar("ana", &documento, &formulario(&[("acao", "despublicar")]))
        .await;
    assert_eq!(painel.visitar(&h, "/").await.status, StatusCode::GONE);
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn dominio_proprio_e_pedido_pelo_dono_e_vira_o_endereco(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let id = site_id(&pool, "padaria").await;
    const DOMINIO: &str = "/painel/sites/padaria/dominio";

    // Sem a página inicial no ar, o site ainda não aceita domínio.
    let cedo = painel.postar("ana", DOMINIO, "host=padaria.example").await;
    assert_eq!(cedo.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(cedo.corpo.contains("Publique a página inicial"));
    painel
        .publicar_home("ana", "padaria", "Pão fresco todo dia")
        .await;

    for (host, trecho) in [
        ("https://padaria.example", "Informe só o domínio"),
        ("loja.sites.teste", "da própria plataforma"),
        ("cms.teste", "da própria plataforma"),
    ] {
        let recusado = painel
            .postar("ana", DOMINIO, &formulario(&[("host", host)]))
            .await;
        assert_eq!(recusado.status, StatusCode::UNPROCESSABLE_ENTITY, "{host}");
        assert!(recusado.corpo.contains(trecho), "{host}");
    }

    let pedido = painel
        .postar("ana", DOMINIO, "host=WWW.Padaria.example")
        .await;
    assert!(pedido.cabecalho("location").ends_with("/dominio?r=pedido"));
    let tela = painel.abrir(Some("ana"), DOMINIO).await;
    assert!(tela.corpo.contains("padaria.example"));
    assert!(tela.corpo.contains("Esperando o DNS"));
    assert!(tela.corpo.contains("registro A apontando para 203.0.113.7"));

    // Pendente, o domínio ainda não serve o site, mas já pode ter certificado.
    assert_eq!(
        painel.visitar("padaria.example", "/").await.status,
        StatusCode::NOT_FOUND
    );
    let caddy = |dominio: &'static str| {
        let painel = &painel;
        async move {
            painel
                .visitar(
                    "qualquer.host",
                    &format!("/api/dominio-permitido?domain={dominio}"),
                )
                .await
                .status
        }
    };
    assert_eq!(caddy("padaria.example").await, StatusCode::OK);
    assert_eq!(caddy("padaria.sites.teste").await, StatusCode::OK);
    assert_eq!(caddy("cms.teste").await, StatusCode::OK);
    assert_eq!(caddy("golpe.example").await, StatusCode::NOT_FOUND);

    // Outro site não toma o domínio.
    painel
        .criar_site("caio", "oficina", "Oficina do Caio")
        .await;
    painel
        .publicar_home("caio", "oficina", "Oficina do Caio")
        .await;
    let tomado = painel
        .postar(
            "caio",
            "/painel/sites/oficina/dominio",
            "host=padaria.example",
        )
        .await;
    assert_eq!(tomado.status, StatusCode::CONFLICT);

    // Só o Dono mexe no endereço.
    assert_eq!(
        painel.abrir(Some("caio"), DOMINIO).await.status,
        StatusCode::NOT_FOUND
    );

    // A rotina conferiu o DNS: o domínio vira o endereço, e o provisório
    // passa a redirecionar para ele.
    let pendente = cms_dados::dominios_pendentes(&pool)
        .await
        .expect("pendentes")
        .into_iter()
        .find(|pendente| pendente.site_id == id)
        .expect("domínio pendente");
    cms_dados::confirmar_dominio(&pool, &pendente)
        .await
        .expect("domínio confirmado");
    painel.cache.invalidar_site(id);
    let proprio = painel.visitar("padaria.example", "/").await;
    assert_eq!(proprio.status, StatusCode::OK);
    assert_eq!(proprio.cabecalho("x-robots-tag"), "");
    let provisorio = painel.visitar(&host("padaria"), "/").await;
    assert_eq!(provisorio.status, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(provisorio.cabecalho("location"), "https://padaria.example/");

    let removido = painel
        .postar("ana", &format!("{DOMINIO}/remover"), "host=padaria.example")
        .await;
    assert!(
        removido
            .cabecalho("location")
            .ends_with("/dominio?r=removido")
    );
    // Sem domínio e sem assumir o provisório, o site volta a ficar fora da busca.
    assert_eq!(
        painel
            .visitar(&host("padaria"), "/")
            .await
            .cabecalho("x-robots-tag"),
        "noindex"
    );
    let assumido = painel
        .postar("ana", &format!("{DOMINIO}/provisorio"), "definitivo=1")
        .await;
    assert!(
        assumido
            .cabecalho("location")
            .ends_with("/dominio?r=provisorio")
    );
    assert_eq!(
        painel
            .visitar(&host("padaria"), "/")
            .await
            .cabecalho("x-robots-tag"),
        ""
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn publicacao_agendada_vai_ao_ar_na_hora_marcada(pool: PgPool) {
    use cms_dados::fluxo;

    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    let id = site_id(&pool, "padaria").await;
    let corpo = |extras: &[(&str, &str)]| {
        let mut campos = vec![("b0_texto", "Abrimos às seis.")];
        campos.extend_from_slice(extras);
        pagina_com(&campos)
    };
    let salvo = painel
        .postar(
            "ana",
            "/painel/sites/padaria/doc",
            &corpo(&[("acao", "salvar")]),
        )
        .await;
    let documento = documento_de(&salvo);

    // Data que já passou, ou que não é data, não agenda.
    for data in ["2020-01-01T10:00", "amanhã cedo"] {
        let recusado = painel
            .postar(
                "ana",
                &documento,
                &corpo(&[("agendar_para", data), ("acao", "agendar")]),
            )
            .await;
        assert!(
            recusado.cabecalho("location").ends_with("?r=data"),
            "{data}"
        );
    }
    let agendado = painel
        .postar(
            "ana",
            &documento,
            &corpo(&[("agendar_para", "2099-12-31T09:00"), ("acao", "agendar")]),
        )
        .await;
    assert!(agendado.cabecalho("location").ends_with("?r=agendado"));
    let editor = painel.abrir(Some("ana"), &documento).await;
    assert!(
        editor
            .corpo
            .contains("publicação agendada para 31 de dezembro de 2099")
    );
    assert!(editor.corpo.contains(r#"value="desagendar""#));

    // Antes da hora, a rotina não publica nada.
    let rodada = fluxo::publicar_agendados(&pool, chrono::Utc::now())
        .await
        .expect("rodada");
    assert!(rodada.sites_publicados.is_empty());
    assert_eq!(
        pedir(&pool, &host("padaria"), "/sobre").await.status,
        StatusCode::NOT_FOUND
    );

    // Chegou a hora.
    sqlx::query("update documento set agendado_para = now() - interval '1 minute'")
        .execute(&pool)
        .await
        .expect("relógio adiantado");
    let rodada = fluxo::publicar_agendados(&pool, chrono::Utc::now())
        .await
        .expect("rodada");
    assert_eq!(rodada.sites_publicados, vec![id]);
    assert_eq!(
        pedir(&pool, &host("padaria"), "/sobre").await.status,
        StatusCode::OK
    );
    // Publicado, o agendamento some, e a rotina não insiste.
    assert!(
        !painel
            .abrir(Some("ana"), &documento)
            .await
            .corpo
            .contains("publicação agendada")
    );
    let rodada = fluxo::publicar_agendados(&pool, chrono::Utc::now())
        .await
        .expect("rodada");
    assert_eq!(rodada, fluxo::RodadaDeAgendados::default());
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn dono_define_a_identidade_e_o_site_passa_a_mostra_la(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site("ana", "padaria", "Padaria da Ana").await;
    painel
        .publicar_home("ana", "padaria", "Pão fresco todo dia")
        .await;
    const IDENTIDADE: &str = "/painel/sites/padaria/identidade";
    let h = host("padaria");
    // A página entra no cache com o rodapé antigo.
    assert!(
        !painel
            .visitar(&h, "/")
            .await
            .corpo
            .contains("Padaria da Ana Ltda.")
    );

    let tela = painel.abrir(Some("ana"), IDENTIDADE).await;
    assert_eq!(tela.status, StatusCode::OK);
    assert!(tela.corpo.contains(r#"value="Padaria da Ana""#));

    let dados = formulario(&[
        ("nome", "Padaria da Ana"),
        (
            "descricao",
            "Padaria de bairro com pão de fermentação natural.",
        ),
        ("razao_social", "Padaria da Ana Ltda."),
        ("telefone", "+55 11 5550-0100"),
        ("email", "contato@padaria.example"),
        ("logradouro", "Rua das Flores, 10"),
        ("cidade", "São Paulo"),
        ("uf", "sp"),
        ("cep", "01000-000"),
        (
            "perfis",
            "https://www.instagram.com/padaria.example\njavascript:alert(1)",
        ),
        ("diretrizes", "Não informe preços: eles mudam toda semana."),
    ]);
    let salvo = painel.postar("ana", IDENTIDADE, &dados).await;
    assert!(salvo.cabecalho("location").ends_with("/identidade?r=salvo"));

    // O site mostra na hora, sem esperar o cache vencer.
    let home = painel.visitar(&h, "/").await;
    assert!(home.corpo.contains("Padaria da Ana Ltda."));
    assert!(
        home.corpo
            .contains("Rua das Flores, 10, São Paulo - SP, CEP 01000-000")
    );
    assert!(
        home.corpo
            .contains("https://www.instagram.com/padaria.example")
    );
    assert!(!home.corpo.contains("javascript:"));
    let reaberta = painel.abrir(Some("ana"), IDENTIDADE).await;
    assert!(reaberta.corpo.contains("Não informe preços"));
    assert!(reaberta.corpo.contains(r#"value="SP""#));

    // Só o Dono mexe na identidade; quem é do site vê o histórico.
    let convite = painel
        .postar(
            "ana",
            "/painel/sites/padaria/convites",
            "email=bia%40exemplo.example&papel=editor",
        )
        .await;
    painel
        .abrir(Some("bia"), &caminho_do_convite(&convite.corpo))
        .await;
    assert_eq!(
        painel.abrir(Some("bia"), IDENTIDADE).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        painel.postar("bia", IDENTIDADE, &dados).await.status,
        StatusCode::FORBIDDEN
    );
    let historico = painel
        .abrir(Some("bia"), "/painel/sites/padaria/historico")
        .await;
    assert_eq!(historico.status, StatusCode::OK);
    assert!(historico.corpo.contains("Publicou “Pão fresco todo dia”"));
    assert!(historico.corpo.contains("ana@exemplo.example"));
    assert_eq!(
        painel
            .abrir(Some("caio"), "/painel/sites/padaria/historico")
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}
