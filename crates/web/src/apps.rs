use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use cms_dados::{AppDaConta, ErroDeApp};
use cms_dominio::Conta;
use cms_dominio::app_builder::AppDefinition;
use cms_dominio::app_registry::AppRole;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::Estado;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NovoApp {
    slug: String,
    name: String,
    bundle_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppJson {
    id: Uuid,
    slug: String,
    name: String,
    bundle_id: String,
    draft_revision: i64,
    role: &'static str,
}

impl From<AppDaConta> for AppJson {
    fn from(app: AppDaConta) -> Self {
        Self {
            id: app.id,
            slug: app.slug,
            name: app.nome,
            bundle_id: app.bundle_id,
            draft_revision: app.revisao_do_rascunho,
            role: match app.papel {
                AppRole::Owner => "owner",
                AppRole::Editor => "editor",
                AppRole::Viewer => "viewer",
            },
        }
    }
}

fn json_response(status: StatusCode, valor: Value) -> Response {
    let mut resposta = (status, axum::Json(valor)).into_response();
    resposta.headers_mut().insert(
        "cache-control",
        HeaderValue::from_static("private, no-store"),
    );
    resposta
}

fn erro(status: StatusCode, codigo: &'static str, mensagem: impl ToString) -> Response {
    json_response(
        status,
        json!({ "error": { "code": codigo, "message": mensagem.to_string() } }),
    )
}

pub(crate) fn erro_de_autenticacao(
    status: StatusCode,
    codigo: &'static str,
    mensagem: &'static str,
) -> Response {
    let mut resposta = erro(status, codigo, mensagem);
    if let Ok(valor) = HeaderValue::from_str(&Uuid::new_v4().to_string()) {
        resposta.headers_mut().insert("x-correlation-id", valor);
    }
    resposta
}

fn resposta_de_erro(erro_app: ErroDeApp) -> Response {
    match erro_app {
        ErroDeApp::EmUso => erro(StatusCode::CONFLICT, "app_conflict", erro_app),
        ErroDeApp::ConflitoDeRevisao => erro(StatusCode::CONFLICT, "draft_conflict", erro_app),
        ErroDeApp::NaoEncontrado => erro(StatusCode::NOT_FOUND, "app_not_found", erro_app),
        ErroDeApp::SemRascunho => erro(StatusCode::UNPROCESSABLE_ENTITY, "draft_missing", erro_app),
        ErroDeApp::VersaoInexistente => erro(StatusCode::NOT_FOUND, "version_not_found", erro_app),
        ErroDeApp::SlugInvalido | ErroDeApp::NomeInvalido | ErroDeApp::BundleIdInvalido => {
            erro(StatusCode::UNPROCESSABLE_ENTITY, "invalid_app", erro_app)
        }
        ErroDeApp::ChaveIdempotenteInvalida => {
            erro(StatusCode::BAD_REQUEST, "invalid_idempotency_key", erro_app)
        }
        ErroDeApp::AmbienteInvalido => erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_environment",
            erro_app,
        ),
        ErroDeApp::Dados(erro_dados) => {
            tracing::error!(%erro_dados, "falha na API de aplicativos");
            erro(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Erro interno.",
            )
        }
    }
}

fn origem_valida(estado: &Estado, cabecalhos: &HeaderMap) -> bool {
    super::painel::veio_do_painel(
        &estado.configuracao,
        cabecalhos
            .get("host")
            .and_then(|valor| valor.to_str().ok())
            .unwrap_or_default(),
        cabecalhos,
    )
}

async fn raiz(
    estado: &Estado,
    conta: &Conta,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    corpo: &Bytes,
    correlation_id: Uuid,
) -> Response {
    match *metodo {
        Method::GET => match cms_dados::apps_da_conta(&estado.pool, &conta.sub).await {
            Ok(apps) => json_response(
                StatusCode::OK,
                serde_json::to_value(apps.into_iter().map(AppJson::from).collect::<Vec<_>>())
                    .expect("AppJson sempre serializa"),
            ),
            Err(erro_dados) => {
                tracing::error!(%erro_dados, "falha ao listar aplicativos");
                erro(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Erro interno.",
                )
            }
        },
        Method::POST => {
            if !origem_valida(estado, cabecalhos) {
                return erro(StatusCode::FORBIDDEN, "invalid_origin", "Pedido recusado.");
            }
            let Ok(pedido) = serde_json::from_slice::<NovoApp>(corpo) else {
                return erro(StatusCode::BAD_REQUEST, "invalid_json", "Corpo inválido.");
            };
            match cms_dados::criar_app(
                &estado.pool,
                conta,
                pedido.slug.trim(),
                &pedido.name,
                pedido.bundle_id.trim(),
                correlation_id,
            )
            .await
            {
                Ok(app) => json_response(
                    StatusCode::CREATED,
                    serde_json::to_value(AppJson::from(app)).expect("AppJson sempre serializa"),
                ),
                Err(erro_app) => resposta_de_erro(erro_app),
            }
        }
        _ => erro(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "Método não permitido.",
        ),
    }
}

async fn salvar_rascunho(
    estado: &Estado,
    conta: &Conta,
    app_id: Uuid,
    cabecalhos: &HeaderMap,
    corpo: &Bytes,
    correlation_id: Uuid,
) -> Response {
    if !origem_valida(estado, cabecalhos) {
        return erro(StatusCode::FORBIDDEN, "invalid_origin", "Pedido recusado.");
    }
    let revisao = cabecalhos
        .get("if-match")
        .and_then(|valor| valor.to_str().ok())
        .map(|valor| valor.trim_matches('"'))
        .and_then(|valor| valor.parse::<i64>().ok());
    let Some(revisao) = revisao else {
        return erro(
            StatusCode::PRECONDITION_REQUIRED,
            "if_match_required",
            "Envie a revisão atual em If-Match.",
        );
    };
    let Ok(definicao) = serde_json::from_slice(corpo) else {
        return erro(StatusCode::BAD_REQUEST, "invalid_json", "Corpo inválido.");
    };
    match cms_dados::salvar_rascunho_do_app(
        &estado.pool,
        conta,
        app_id,
        revisao,
        &definicao,
        correlation_id,
    )
    .await
    {
        Ok(revisao) => {
            let mut resposta = json_response(StatusCode::OK, json!({ "draftRevision": revisao }));
            resposta.headers_mut().insert(
                "etag",
                HeaderValue::from_str(&format!("\"{revisao}\"")).expect("revisão é cabeçalho"),
            );
            resposta
        }
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

fn definicao_valida(rascunho: &cms_dados::RascunhoDoApp) -> Result<AppDefinition, Box<Response>> {
    let definicao = serde_json::from_value::<AppDefinition>(rascunho.definicao.clone()).map_err(
        |erro_json| {
            Box::new(json_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({ "valid": false, "errors": [{ "code": "invalid_schema", "message": erro_json.to_string() }] }),
            ))
        },
    )?;
    if definicao.app_id != rascunho.slug {
        return Err(Box::new(json_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "valid": false, "errors": [{ "code": "app_id_mismatch", "message": "appId deve ser igual ao slug do aplicativo." }] }),
        )));
    }
    if let Err(erro_validacao) = definicao.validate() {
        return Err(Box::new(json_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "valid": false, "errors": [{ "code": "invalid_definition", "message": erro_validacao.to_string() }] }),
        )));
    }
    Ok(definicao)
}

async fn validar_rascunho(
    estado: &Estado,
    conta: &Conta,
    app_id: Uuid,
    cabecalhos: &HeaderMap,
    correlation_id: Uuid,
) -> Response {
    if !origem_valida(estado, cabecalhos) {
        return erro(StatusCode::FORBIDDEN, "invalid_origin", "Pedido recusado.");
    }
    let rascunho = match cms_dados::rascunho_do_app(&estado.pool, conta, app_id).await {
        Ok(rascunho) => rascunho,
        Err(erro_app) => return resposta_de_erro(erro_app),
    };
    if let Err(resposta) = definicao_valida(&rascunho) {
        return *resposta;
    }
    match cms_dados::auditar_validacao(&estado.pool, conta, app_id, correlation_id).await {
        Ok(()) => json_response(
            StatusCode::OK,
            json!({ "valid": true, "draftRevision": rascunho.revisao, "errors": [] }),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

async fn criar_versao(
    estado: &Estado,
    conta: &Conta,
    app_id: Uuid,
    cabecalhos: &HeaderMap,
    correlation_id: Uuid,
) -> Response {
    if !origem_valida(estado, cabecalhos) {
        return erro(StatusCode::FORBIDDEN, "invalid_origin", "Pedido recusado.");
    }
    let chave = cabecalhos
        .get("idempotency-key")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or_default();
    let rascunho = match cms_dados::rascunho_do_app(&estado.pool, conta, app_id).await {
        Ok(rascunho) => rascunho,
        Err(erro_app) => return resposta_de_erro(erro_app),
    };
    if definicao_valida(&rascunho).is_err() {
        return erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_definition",
            "O rascunho precisa ser válido antes de gerar uma versão.",
        );
    }
    match cms_dados::criar_versao_do_app(&estado.pool, conta, app_id, chave, correlation_id).await {
        Ok(versao) => json_response(
            if versao.criada {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            json!({ "id": versao.id, "number": versao.numero }),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publicacao {
    version: i32,
}

async fn publicar(
    estado: &Estado,
    conta: &Conta,
    app_id: Uuid,
    ambiente: &str,
    cabecalhos: &HeaderMap,
    corpo: &Bytes,
    correlation_id: Uuid,
) -> Response {
    if !origem_valida(estado, cabecalhos) {
        return erro(StatusCode::FORBIDDEN, "invalid_origin", "Pedido recusado.");
    }
    let Ok(publicacao) = serde_json::from_slice::<Publicacao>(corpo) else {
        return erro(StatusCode::BAD_REQUEST, "invalid_json", "Corpo inválido.");
    };
    match cms_dados::publicar_versao_no_ambiente(
        &estado.pool,
        conta,
        app_id,
        ambiente,
        publicacao.version,
        correlation_id,
    )
    .await
    {
        Ok(versao_id) => json_response(
            StatusCode::OK,
            json!({ "environment": ambiente, "version": publicacao.version, "versionId": versao_id }),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

pub async fn atender(
    estado: &Estado,
    conta: &Conta,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    caminho: &str,
    corpo: &Bytes,
) -> Response {
    let correlation_id = Uuid::new_v4();
    let partes: Vec<&str> = caminho.split('/').skip(1).collect();
    let resposta = match (metodo, partes.as_slice()) {
        (_, ["api", "v1", "apps"]) => {
            raiz(estado, conta, metodo, cabecalhos, corpo, correlation_id).await
        }
        (&Method::PUT, ["api", "v1", "apps", app_id, "draft"]) => match app_id.parse() {
            Ok(app_id) => {
                salvar_rascunho(estado, conta, app_id, cabecalhos, corpo, correlation_id).await
            }
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
        (&Method::POST, ["api", "v1", "apps", app_id, "validate"]) => match app_id.parse() {
            Ok(app_id) => validar_rascunho(estado, conta, app_id, cabecalhos, correlation_id).await,
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
        (&Method::POST, ["api", "v1", "apps", app_id, "versions"]) => match app_id.parse() {
            Ok(app_id) => criar_versao(estado, conta, app_id, cabecalhos, correlation_id).await,
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
        (&Method::PUT, ["api", "v1", "apps", app_id, "environments", ambiente]) => {
            match app_id.parse() {
                Ok(app_id) => {
                    publicar(
                        estado,
                        conta,
                        app_id,
                        ambiente,
                        cabecalhos,
                        corpo,
                        correlation_id,
                    )
                    .await
                }
                Err(_) => erro(
                    StatusCode::NOT_FOUND,
                    "app_not_found",
                    "Aplicativo não encontrado.",
                ),
            }
        }
        _ => erro(StatusCode::NOT_FOUND, "not_found", "Não encontrado."),
    };
    let mut resposta = resposta;
    if let Ok(valor) = HeaderValue::from_str(&correlation_id.to_string()) {
        resposta.headers_mut().insert("x-correlation-id", valor);
    }
    resposta
}
