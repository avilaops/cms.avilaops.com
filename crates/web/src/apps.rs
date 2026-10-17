use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use cms_dados::{
    AmbienteDoApp, AppDaConta, ErroDeApp, EventoDeAuditoriaDoApp, ParticipanteDoApp,
    VersaoListadaDoApp,
};
use cms_dominio::Conta;
use cms_dominio::app_builder::{AppDefinition, MAX_DEFINITION_BYTES};
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersaoJson {
    id: Uuid,
    number: i32,
    schema_version: String,
    minimum_runtime_version: String,
    created_by: String,
    created_at: String,
}

impl From<VersaoListadaDoApp> for VersaoJson {
    fn from(versao: VersaoListadaDoApp) -> Self {
        Self {
            id: versao.id,
            number: versao.numero,
            schema_version: versao.schema_version,
            minimum_runtime_version: versao.minimum_runtime_version,
            created_by: versao.criado_por,
            created_at: versao.criado_em.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AmbienteJson {
    name: String,
    version_id: Uuid,
    version: i32,
    updated_by: String,
    updated_at: String,
}

impl From<AmbienteDoApp> for AmbienteJson {
    fn from(ambiente: AmbienteDoApp) -> Self {
        Self {
            name: ambiente.nome,
            version_id: ambiente.versao_id,
            version: ambiente.numero_da_versao,
            updated_by: ambiente.atualizado_por,
            updated_at: ambiente.atualizado_em.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditoriaJson {
    id: i64,
    account: String,
    action: String,
    data: Value,
    correlation_id: Uuid,
    created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParticipanteJson {
    account: String,
    role: &'static str,
    created_at: String,
}

impl From<ParticipanteDoApp> for ParticipanteJson {
    fn from(participante: ParticipanteDoApp) -> Self {
        Self {
            account: participante.conta,
            role: match participante.papel {
                AppRole::Owner => "owner",
                AppRole::Editor => "editor",
                AppRole::Viewer => "viewer",
            },
            created_at: participante.criado_em.to_rfc3339(),
        }
    }
}

impl From<EventoDeAuditoriaDoApp> for AuditoriaJson {
    fn from(evento: EventoDeAuditoriaDoApp) -> Self {
        Self {
            id: evento.id,
            account: evento.conta,
            action: evento.acao,
            data: evento.dados,
            correlation_id: evento.correlation_id,
            created_at: evento.criado_em.to_rfc3339(),
        }
    }
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
        ErroDeApp::DefinicaoInvalida => erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_definition",
            erro_app,
        ),
        ErroDeApp::ContaInvalida => erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_account",
            erro_app,
        ),
        ErroDeApp::UltimoProprietario => erro(StatusCode::CONFLICT, "last_owner", erro_app),
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
        Method::GET => match cms_dados::apps_da_conta(&estado.pool, conta).await {
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

async fn obter_app(estado: &Estado, conta: &Conta, app_id: Uuid) -> Response {
    match cms_dados::app_da_conta(&estado.pool, conta, app_id).await {
        Ok(app) => json_response(
            StatusCode::OK,
            serde_json::to_value(AppJson::from(app)).expect("AppJson sempre serializa"),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

async fn obter_rascunho(estado: &Estado, conta: &Conta, app_id: Uuid) -> Response {
    match cms_dados::rascunho_do_app(&estado.pool, conta, app_id).await {
        Ok(rascunho) => {
            let mut resposta = json_response(
                StatusCode::OK,
                json!({
                    "draftRevision": rascunho.revisao,
                    "definition": rascunho.definicao,
                }),
            );
            resposta.headers_mut().insert(
                "etag",
                HeaderValue::from_str(&format!("\"{}\"", rascunho.revisao))
                    .expect("revisão é cabeçalho"),
            );
            resposta
        }
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

async fn listar_versoes(estado: &Estado, conta: &Conta, app_id: Uuid) -> Response {
    match cms_dados::versoes_do_app(&estado.pool, conta, app_id).await {
        Ok(versoes) => json_response(
            StatusCode::OK,
            serde_json::to_value(
                versoes
                    .into_iter()
                    .map(VersaoJson::from)
                    .collect::<Vec<_>>(),
            )
            .expect("VersaoJson sempre serializa"),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

async fn obter_versao(estado: &Estado, conta: &Conta, app_id: Uuid, numero: i32) -> Response {
    match cms_dados::definicao_da_versao(&estado.pool, conta, app_id, numero).await {
        Ok(definicao) => json_response(
            StatusCode::OK,
            json!({ "number": numero, "definition": definicao }),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

async fn listar_ambientes(estado: &Estado, conta: &Conta, app_id: Uuid) -> Response {
    match cms_dados::ambientes_do_app(&estado.pool, conta, app_id).await {
        Ok(ambientes) => json_response(
            StatusCode::OK,
            serde_json::to_value(
                ambientes
                    .into_iter()
                    .map(AmbienteJson::from)
                    .collect::<Vec<_>>(),
            )
            .expect("AmbienteJson sempre serializa"),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

async fn listar_auditoria(estado: &Estado, conta: &Conta, app_id: Uuid) -> Response {
    match cms_dados::auditoria_do_app(&estado.pool, conta, app_id, 50).await {
        Ok(eventos) => json_response(
            StatusCode::OK,
            serde_json::to_value(
                eventos
                    .into_iter()
                    .map(AuditoriaJson::from)
                    .collect::<Vec<_>>(),
            )
            .expect("AuditoriaJson sempre serializa"),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AlteracaoDeParticipante {
    account: String,
    role: Option<AppRole>,
}

async fn membros(
    estado: &Estado,
    conta: &Conta,
    app_id: Uuid,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    corpo: &Bytes,
    correlation_id: Uuid,
) -> Response {
    match *metodo {
        Method::GET => match cms_dados::participantes_do_app(&estado.pool, conta, app_id).await {
            Ok(participantes) => json_response(
                StatusCode::OK,
                serde_json::to_value(
                    participantes
                        .into_iter()
                        .map(ParticipanteJson::from)
                        .collect::<Vec<_>>(),
                )
                .expect("ParticipanteJson sempre serializa"),
            ),
            Err(erro_app) => resposta_de_erro(erro_app),
        },
        Method::PUT | Method::DELETE => {
            if !origem_valida(estado, cabecalhos) {
                return erro(StatusCode::FORBIDDEN, "invalid_origin", "Pedido recusado.");
            }
            let Ok(pedido) = serde_json::from_slice::<AlteracaoDeParticipante>(corpo) else {
                return erro(StatusCode::BAD_REQUEST, "invalid_json", "Corpo inválido.");
            };
            let resultado = if *metodo == Method::PUT {
                match pedido.role {
                    Some(papel) => {
                        cms_dados::definir_participante_do_app(
                            &estado.pool,
                            conta,
                            app_id,
                            &pedido.account,
                            papel,
                            correlation_id,
                        )
                        .await
                    }
                    None => {
                        return erro(
                            StatusCode::BAD_REQUEST,
                            "role_required",
                            "Papel obrigatório.",
                        );
                    }
                }
            } else {
                cms_dados::remover_participante_do_app(
                    &estado.pool,
                    conta,
                    app_id,
                    &pedido.account,
                    correlation_id,
                )
                .await
            };
            match resultado {
                Ok(()) => json_response(StatusCode::OK, json!({ "updated": true })),
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
    if corpo.len() > MAX_DEFINITION_BYTES {
        return erro(
            StatusCode::PAYLOAD_TOO_LARGE,
            "definition_too_large",
            "A definição excede 512 KiB.",
        );
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
    corpo: &Bytes,
    correlation_id: Uuid,
) -> Response {
    if !origem_valida(estado, cabecalhos) {
        return erro(StatusCode::FORBIDDEN, "invalid_origin", "Pedido recusado.");
    }
    if corpo.len() > MAX_DEFINITION_BYTES {
        return erro(
            StatusCode::PAYLOAD_TOO_LARGE,
            "definition_too_large",
            "A definição excede 512 KiB.",
        );
    }
    let mut rascunho = match cms_dados::rascunho_do_app(&estado.pool, conta, app_id).await {
        Ok(rascunho) => rascunho,
        Err(erro_app) => return resposta_de_erro(erro_app),
    };
    if let Ok(definicao) = serde_json::from_slice::<Value>(corpo)
        && !definicao.is_null()
    {
        rascunho.definicao = definicao;
    }
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
        (&Method::GET, ["api", "v1", "apps", app_id]) => match app_id.parse() {
            Ok(app_id) => obter_app(estado, conta, app_id).await,
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
        (&Method::GET, ["api", "v1", "apps", app_id, "draft"]) => match app_id.parse() {
            Ok(app_id) => obter_rascunho(estado, conta, app_id).await,
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
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
            Ok(app_id) => {
                validar_rascunho(estado, conta, app_id, cabecalhos, corpo, correlation_id).await
            }
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
        (&Method::GET, ["api", "v1", "apps", app_id, "versions"]) => match app_id.parse() {
            Ok(app_id) => listar_versoes(estado, conta, app_id).await,
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
        (&Method::GET, ["api", "v1", "apps", app_id, "versions", numero]) => {
            match (app_id.parse(), numero.parse()) {
                (Ok(app_id), Ok(numero)) => obter_versao(estado, conta, app_id, numero).await,
                _ => erro(
                    StatusCode::NOT_FOUND,
                    "version_not_found",
                    "Versão não encontrada.",
                ),
            }
        }
        (&Method::GET, ["api", "v1", "apps", app_id, "environments"]) => match app_id.parse() {
            Ok(app_id) => listar_ambientes(estado, conta, app_id).await,
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
        (&Method::GET, ["api", "v1", "apps", app_id, "audit"]) => match app_id.parse() {
            Ok(app_id) => listar_auditoria(estado, conta, app_id).await,
            Err(_) => erro(
                StatusCode::NOT_FOUND,
                "app_not_found",
                "Aplicativo não encontrado.",
            ),
        },
        (_, ["api", "v1", "apps", app_id, "members"]) => match app_id.parse() {
            Ok(app_id) => {
                membros(
                    estado,
                    conta,
                    app_id,
                    metodo,
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
