mod comum;

use std::collections::HashMap;
use std::path::PathBuf;

use axum::body::Body;
use axum::extract::Query;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use cms_integracoes::auth::ClienteAuth;
use serde_json::{Value, json};
use sqlx::PgPool;

use comum::{HOST_DO_PAINEL, Resposta, estado, responder};

const ORIGEM: &str = "https://cms.teste";

async fn sessao(
    Query(consulta): Query<HashMap<String, String>>,
    cabecalhos: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let quem = cabecalhos
        .get("cookie")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.strip_prefix("avila_sso=a.b."))
        .unwrap_or("");
    let permitido = consulta.get("app").is_some_and(|app| app == "cms");
    match quem {
        "ana" | "bia" => (
            StatusCode::OK,
            Json(json!({
                "autenticado": true,
                "permitido": permitido,
                "sessao": {
                    "sub": format!("sub-{quem}"),
                    "email": format!("{quem}@example.test"),
                    "nome": quem,
                    "papel": "CLIENTE"
                }
            })),
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
    ClienteAuth::novo(&format!("http://{endereco}"), "cms").expect("cliente do auth")
}

async fn enviar(
    pool: &PgPool,
    quem: &str,
    pedido: axum::http::request::Builder,
    corpo: Value,
) -> Resposta {
    let mut estado = estado(pool, PathBuf::from("midia-que-nao-existe"));
    estado.auth = Some(subir_auth().await);
    let pedido = pedido
        .header("host", HOST_DO_PAINEL)
        .header("origin", ORIGEM)
        .header("content-type", "application/json")
        .header("cookie", format!("avila_sso=a.b.{quem}"))
        .body(Body::from(corpo.to_string()))
        .expect("pedido válido");
    responder(estado, pedido).await
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn api_sem_sessao_responde_erro_json_em_vez_de_redirecionar(pool: PgPool) {
    let mut estado = estado(&pool, PathBuf::from("midia-que-nao-existe"));
    estado.auth = Some(subir_auth().await);
    let pedido = Request::builder()
        .uri("/api/v1/apps")
        .header("host", HOST_DO_PAINEL)
        .body(Body::empty())
        .expect("pedido válido");
    let resposta = responder(estado, pedido).await;
    assert_eq!(resposta.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        serde_json::from_str::<Value>(&resposta.corpo).expect("erro JSON")["error"]["code"],
        "unauthenticated"
    );
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn cria_e_lista_so_os_apps_da_conta(pool: PgPool) {
    let criada = enviar(
        &pool,
        "ana",
        Request::builder().method("POST").uri("/api/v1/apps"),
        json!({
            "slug": "app-da-ana",
            "name": "App da Ana",
            "bundleId": "com.avilaops.app-da-ana"
        }),
    )
    .await;
    assert_eq!(criada.status, StatusCode::CREATED, "{}", criada.corpo);
    let app: Value = serde_json::from_str(&criada.corpo).expect("resposta JSON");
    assert_eq!(app["slug"], "app-da-ana");
    assert_eq!(app["draftRevision"], 0);

    let da_ana = enviar(
        &pool,
        "ana",
        Request::builder().uri("/api/v1/apps"),
        Value::Null,
    )
    .await;
    assert_eq!(da_ana.status, StatusCode::OK, "{}", da_ana.corpo);
    let apps: Value = serde_json::from_str(&da_ana.corpo).expect("resposta JSON");
    assert_eq!(apps.as_array().map(Vec::len), Some(1));
    assert_eq!(apps[0]["id"], app["id"]);

    let da_bia = enviar(
        &pool,
        "bia",
        Request::builder().uri("/api/v1/apps"),
        Value::Null,
    )
    .await;
    assert_eq!(da_bia.status, StatusCode::OK, "{}", da_bia.corpo);
    assert_eq!(da_bia.corpo, "[]");
}

fn aad(slug: &str) -> Value {
    json!({
        "schemaVersion": "1.0.0",
        "appId": slug,
        "minimumRuntimeVersion": "1.0.0",
        "navigation": { "initialRoute": "home" },
        "screens": [{
            "id": "home",
            "route": "home",
            "title": "Home",
            "components": [{ "type": "text", "id": "titulo", "value": "Olá" }]
        }]
    })
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn rascunho_valida_versiona_e_publica_com_concorrencia_e_idempotencia(pool: PgPool) {
    let criada = enviar(
        &pool,
        "ana",
        Request::builder().method("POST").uri("/api/v1/apps"),
        json!({
            "slug": "app-publicavel",
            "name": "App publicável",
            "bundleId": "com.avilaops.app-publicavel"
        }),
    )
    .await;
    let criada: Value = serde_json::from_str(&criada.corpo).expect("app criado");
    let id = criada["id"].as_str().expect("id");

    let salvo = enviar(
        &pool,
        "ana",
        Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/apps/{id}/draft"))
            .header("if-match", "0"),
        aad("app-publicavel"),
    )
    .await;
    assert_eq!(salvo.status, StatusCode::OK, "{}", salvo.corpo);
    assert_eq!(salvo.cabecalho("etag"), "\"1\"");

    let desatualizado = enviar(
        &pool,
        "ana",
        Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/apps/{id}/draft"))
            .header("if-match", "0"),
        aad("app-publicavel"),
    )
    .await;
    assert_eq!(desatualizado.status, StatusCode::CONFLICT);

    let validado = enviar(
        &pool,
        "ana",
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/apps/{id}/validate")),
        Value::Null,
    )
    .await;
    assert_eq!(validado.status, StatusCode::OK, "{}", validado.corpo);
    assert_eq!(
        serde_json::from_str::<Value>(&validado.corpo).expect("JSON")["valid"],
        true
    );

    let primeira = enviar(
        &pool,
        "ana",
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/apps/{id}/versions"))
            .header("idempotency-key", "release-1"),
        Value::Null,
    )
    .await;
    assert_eq!(primeira.status, StatusCode::CREATED, "{}", primeira.corpo);
    let primeira: Value = serde_json::from_str(&primeira.corpo).expect("versão JSON");
    assert_eq!(primeira["number"], 1);

    let repetida = enviar(
        &pool,
        "ana",
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/apps/{id}/versions"))
            .header("idempotency-key", "release-1"),
        Value::Null,
    )
    .await;
    assert_eq!(repetida.status, StatusCode::OK, "{}", repetida.corpo);
    assert_eq!(
        serde_json::from_str::<Value>(&repetida.corpo).expect("versão JSON")["id"],
        primeira["id"]
    );

    let publicada = enviar(
        &pool,
        "ana",
        Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/apps/{id}/environments/production")),
        json!({ "version": 1 }),
    )
    .await;
    assert_eq!(publicada.status, StatusCode::OK, "{}", publicada.corpo);

    let barrada = enviar(
        &pool,
        "bia",
        Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/apps/{id}/draft"))
            .header("if-match", "1"),
        aad("app-publicavel"),
    )
    .await;
    assert_eq!(barrada.status, StatusCode::NOT_FOUND);

    let auditoria: i64 = sqlx::query_scalar("select count(*) from app_auditoria where app_id = $1")
        .bind(id.parse::<uuid::Uuid>().expect("uuid"))
        .fetch_one(&pool)
        .await
        .expect("auditoria");
    assert_eq!(auditoria, 5);
}
