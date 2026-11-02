//! O estúdio no painel: login, isolamento por site, canvas e prévia.

mod comum;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use cms_integracoes::auth::ClienteAuth;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::HashMap;

use comum::{HOST_DO_PAINEL, Resposta, estado, responder};

const ORIGEM: &str = "https://cms.teste";

fn token(quem: &str) -> String {
    format!("a.b.{quem}")
}

async fn sessao(
    axum::extract::Query(consulta): axum::extract::Query<HashMap<String, String>>,
    cabecalhos: axum::http::HeaderMap,
) -> (StatusCode, Json<Value>) {
    let quem = cabecalhos
        .get("cookie")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.strip_prefix("avila_sso=a.b."))
        .unwrap_or("");
    let conta = |papel: &str| json!({ "sub": format!("sub-{quem}"), "email": format!("{quem}@exemplo.example"), "nome": quem, "papel": papel });
    let do_cms = consulta.get("app").is_some_and(|app| app == "cms");
    match quem {
        "ana" | "bia" | "caio" => (
            StatusCode::OK,
            Json(json!({ "autenticado": true, "permitido": do_cms, "sessao": conta("CLIENTE") })),
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
    tokio::spawn(async move {
        axum::serve(escuta, Router::new().route("/api/session", get(sessao)))
            .await
            .expect("auth de teste");
    });
    ClienteAuth::novo(&format!("http://{endereco}"), "cms").expect("cliente")
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
        let mut estado = estado(&self.pool, std::path::PathBuf::from("midia-que-nao-existe"));
        estado.auth = Some(self.auth.clone());
        responder(estado, pedido.body(corpo).expect("pedido")).await
    }

    async fn abrir(&self, quem: Option<&str>, caminho: &str) -> Resposta {
        let mut pedido = Request::builder()
            .uri(caminho)
            .header("host", HOST_DO_PAINEL);
        if let Some(quem) = quem {
            pedido = pedido.header("cookie", format!("avila_sso={}", token(quem)));
        }
        self.enviar(pedido, Body::empty()).await
    }

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
}

fn sem_consulta(caminho: &str) -> &str {
    caminho.split('?').next().unwrap_or(caminho)
}

fn id_do_no(corpo: &str) -> String {
    let marca = "data-id=\"";
    let inicio = corpo.find(marca).expect("nó no canvas") + marca.len();
    corpo[inicio..].split('"').next().expect("id").to_string()
}

fn caminho_do_convite(corpo: &str) -> String {
    let inicio = corpo.find("/convite/").expect("link do convite");
    corpo[inicio..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '/')
        .collect()
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn estudio_cria_canvas_publica_e_isola(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    assert_eq!(painel.criar_site().await.status, StatusCode::SEE_OTHER);
    let anonimo = painel.abrir(None, "/painel/sites/oficina/estudio").await;
    assert_eq!(anonimo.status, StatusCode::FOUND);
    assert!(anonimo.cabecalho("location").contains("login"));

    assert_eq!(
        painel
            .abrir(Some("caio"), "/painel/sites/oficina/estudio")
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let home = painel
        .abrir(Some("ana"), "/painel/sites/oficina/estudio")
        .await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(home.corpo.contains("O que você quer desenhar?"));
    assert!(home.corpo.contains("Tinta"));
    assert!(home.corpo.contains("Bibliotecas"));
    assert!(!home.corpo.contains("MagicPath"));
    assert!(!home.corpo.contains("OpenAI"));
    assert!(home.corpo.contains("htmx"));

    let sem_origem = painel
        .enviar(
            Request::builder()
                .method("POST")
                .uri("/painel/sites/oficina/estudio")
                .header("host", HOST_DO_PAINEL)
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", format!("avila_sso={}", token("ana"))),
            Body::from("pedido=Casa&sistema=tinta&atalho=aplicativo".to_string()),
        )
        .await;
    assert_eq!(sem_origem.status, StatusCode::FORBIDDEN);

    let criado = painel
        .postar(
            "ana",
            "/painel/sites/oficina/estudio",
            "pedido=Habit+tracker&sistema=tinta&atalho=aplicativo",
        )
        .await;
    assert_eq!(criado.status, StatusCode::SEE_OTHER);
    let editor_url = sem_consulta(criado.cabecalho("location"));
    assert!(editor_url.contains("/painel/sites/oficina/estudio/"));
    let editor = painel.abrir(Some("ana"), editor_url).await;
    assert_eq!(editor.status, StatusCode::OK);
    assert!(editor.corpo.contains("Habit tracker"));
    assert!(editor.corpo.contains("data-sistema=\"tinta\""));
    assert!(editor.corpo.contains("#e11d2e"));
    assert!(editor.corpo.contains("width:390px"));
    assert!(editor.corpo.contains("function pontoNoQuadro") || editor.corpo.contains("estudio-"));
    assert!(editor.corpo.contains(">100%<"));
    assert!(editor.corpo.contains("Mão"));
    assert!(editor.corpo.contains("Ainda não disponível nesta versão"));

    let no = id_do_no(&editor.corpo);
    let movido = painel
        .postar("ana", &format!("{editor_url}/nos/{no}"), "x=64&y=72")
        .await;
    assert_eq!(movido.status, StatusCode::NO_CONTENT);
    let depois = painel.abrir(Some("ana"), editor_url).await;
    // A página do aplicativo tem 390 px e o texto ocupa quase tudo: o x
    // encosta na borda, o y pedido fica.
    assert!(depois.corpo.contains("left:16px;top:72px"));

    let publicado = painel
        .postar("ana", &format!("{editor_url}/publicar"), "")
        .await;
    assert_eq!(publicado.status, StatusCode::SEE_OTHER);
    let com_link = painel
        .abrir(Some("ana"), publicado.cabecalho("location"))
        .await;
    let marca = "value=\"/estudio/p/";
    let inicio = com_link.corpo.find(marca).expect("link da prévia") + marca.len();
    let token = com_link.corpo[inicio..].split('"').next().expect("token");
    let previa = painel.abrir(None, &format!("/estudio/p/{token}")).await;
    assert_eq!(previa.status, StatusCode::OK);
    assert!(previa.corpo.contains("Prévia somente leitura"));
    assert!(previa.corpo.contains("Habit tracker"));
    assert!(!previa.corpo.contains("data-tipo=\"retangulo\""));
    assert!(!previa.corpo.contains("/painel/sites/oficina"));

    let retangulo = painel
        .postar("ana", &format!("{editor_url}/nos"), "tipo=retangulo")
        .await;
    assert_eq!(retangulo.status, StatusCode::SEE_OTHER);
    let rascunho = painel.abrir(Some("ana"), editor_url).await;
    assert!(rascunho.corpo.contains("data-tipo=\"retangulo\""));
    let previa_antiga = painel.abrir(None, &format!("/estudio/p/{token}")).await;
    assert!(!previa_antiga.corpo.contains("data-tipo=\"retangulo\""));
    painel
        .postar("ana", &format!("{editor_url}/publicar"), "")
        .await;
    let previa_nova = painel.abrir(None, &format!("/estudio/p/{token}")).await;
    assert!(previa_nova.corpo.contains("data-tipo=\"retangulo\""));

    let convite = painel
        .postar(
            "ana",
            "/painel/sites/oficina/convites",
            "email=bia%40exemplo.example&papel=autor",
        )
        .await;
    painel
        .abrir(Some("bia"), &caminho_do_convite(&convite.corpo))
        .await;
    let da_bia = painel.abrir(Some("bia"), editor_url).await;
    assert!(!da_bia.corpo.contains("Publicar prévia"));
    assert!(!da_bia.corpo.contains("name=\"tipo\""));
    assert_eq!(
        painel
            .postar("bia", &format!("{editor_url}/publicar"), "")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        painel
            .postar("bia", &format!("{editor_url}/nos"), "tipo=retangulo")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

impl Painel {
    async fn criar_site(&self) -> Resposta {
        self.postar("ana", "/painel/sites", "nome=Oficina&slug=oficina")
            .await
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn biblioteca_docs_entra_no_canvas(pool: PgPool) {
    let painel = Painel::novo(&pool).await;
    painel.criar_site().await;
    let criado = painel
        .postar(
            "ana",
            "/painel/sites/oficina/estudio",
            "pedido=Guia&sistema=padrao&atalho=",
        )
        .await;
    let editor_url = sem_consulta(criado.cabecalho("location"));
    let sem_biblioteca = painel.abrir(Some("ana"), editor_url).await;
    assert!(sem_biblioteca.corpo.contains("Nenhuma biblioteca ativa"));
    let ativada = painel
        .postar(
            "ana",
            "/painel/sites/oficina/estudio/bibliotecas/docs",
            "ativa=1",
        )
        .await;
    assert_eq!(ativada.status, StatusCode::SEE_OTHER);
    let lista = painel
        .abrir(Some("ana"), ativada.cabecalho("location"))
        .await;
    assert!(lista.corpo.contains("Ativa neste site"));
    assert!(lista.corpo.contains("2026-11-02"));
    assert!(lista.corpo.contains("Fumadocs"));
    let editor = painel.abrir(Some("ana"), editor_url).await;
    assert!(editor.corpo.contains("Inserir Chamada"));
    let inserido = painel
        .postar(
            "ana",
            &format!("{editor_url}/nos"),
            "tipo=componente&variante=chamada",
        )
        .await;
    assert_eq!(inserido.status, StatusCode::SEE_OTHER);
    let canvas = painel.abrir(Some("ana"), editor_url).await;
    assert!(canvas.corpo.contains("class=\"docs docs-chamada\""));
    assert!(
        canvas
            .corpo
            .contains("Um aviso curto para quem lê a página.")
    );
}
