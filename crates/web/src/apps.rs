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
use serde_json::Value;
use uuid::Uuid;

use crate::Estado;
use crate::app_contract_types::*;

#[derive(Deserialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NovoApp {
    slug: String,
    name: String,
    bundle_id: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppJson {
    id: Uuid,
    slug: String,
    name: String,
    bundle_id: String,
    draft_revision: i64,
    role: AppRole,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct VersaoJson {
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
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct AmbienteJson {
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
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuditoriaJson {
    id: i64,
    account: String,
    action: String,
    data: Value,
    correlation_id: Uuid,
    created_at: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParticipanteJson {
    account: String,
    role: AppRole,
    created_at: String,
}

impl From<ParticipanteDoApp> for ParticipanteJson {
    fn from(participante: ParticipanteDoApp) -> Self {
        Self {
            account: participante.conta,
            role: participante.papel,
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
            role: app.papel,
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
        valor(ErroJson {
            error: DetalheErro {
                code: codigo.to_string(),
                message: mensagem.to_string(),
            },
        }),
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
                valor(RascunhoJson {
                    draft_revision: rascunho.revisao,
                    definition: rascunho.definicao,
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
            valor(DefinicaoVersionadaJson {
                number: numero,
                definition: definicao,
            }),
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
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AlteracaoDeParticipante {
    account: String,
    role: Option<AppRole>,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(crate) struct DefinicaoDeParticipante {
    account: String,
    role: AppRole,
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
                        let pedido = DefinicaoDeParticipante {
                            account: pedido.account,
                            role: papel,
                        };
                        cms_dados::definir_participante_do_app(
                            &estado.pool,
                            conta,
                            app_id,
                            &pedido.account,
                            pedido.role,
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
                Ok(()) => json_response(StatusCode::OK, valor(AtualizadoJson { updated: true })),
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
            let mut resposta = json_response(
                StatusCode::OK,
                valor(RevisaoJson {
                    draft_revision: revisao,
                }),
            );
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
                validacao_invalida("invalid_schema", erro_json),
            ))
        },
    )?;
    if definicao.app_id != rascunho.slug {
        return Err(Box::new(json_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            validacao_invalida(
                "app_id_mismatch",
                "appId deve ser igual ao slug do aplicativo.",
            ),
        )));
    }
    if let Err(erro_validacao) = definicao.validate() {
        return Err(Box::new(json_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            validacao_invalida("invalid_definition", erro_validacao),
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
            valor(ValidacaoValidaJson {
                valid: true,
                draft_revision: rascunho.revisao,
                errors: vec![],
            }),
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
            valor(VersaoCriadaJson {
                id: versao.id,
                number: versao.numero,
            }),
        ),
        Err(erro_app) => resposta_de_erro(erro_app),
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "contracts", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(crate) struct Publicacao {
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
            valor(PublicadoJson {
                environment: ambiente.to_string(),
                version: publicacao.version,
                version_id: versao_id,
            }),
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
    use crate::app_routes::{Operacao, localizar};
    let resposta = if let Some(rota) =
        localizar(metodo, caminho).filter(|rota| rota.endpoint.operation != Operacao::GetRuntime)
    {
        tracing::debug!(operation = rota.endpoint.id, "operação da API de apps");
        match rota.endpoint.operation {
            Operacao::ListApps | Operacao::CreateApp => {
                raiz(estado, conta, metodo, cabecalhos, corpo, correlation_id).await
            }
            operation => match rota
                .parametro("appId")
                .and_then(|id| id.parse::<Uuid>().ok())
            {
                Some(app_id) => match operation {
                    Operacao::GetApp => obter_app(estado, conta, app_id).await,
                    Operacao::GetDraft => obter_rascunho(estado, conta, app_id).await,
                    Operacao::SaveDraft => {
                        salvar_rascunho(estado, conta, app_id, cabecalhos, corpo, correlation_id)
                            .await
                    }
                    Operacao::ValidateDraft => {
                        validar_rascunho(estado, conta, app_id, cabecalhos, corpo, correlation_id)
                            .await
                    }
                    Operacao::ListVersions => listar_versoes(estado, conta, app_id).await,
                    Operacao::CreateVersion => {
                        criar_versao(estado, conta, app_id, cabecalhos, correlation_id).await
                    }
                    Operacao::GetVersion => match rota
                        .parametro("number")
                        .and_then(|number| number.parse().ok())
                    {
                        Some(number) => obter_versao(estado, conta, app_id, number).await,
                        None => erro(
                            StatusCode::NOT_FOUND,
                            "version_not_found",
                            "Versão não encontrada.",
                        ),
                    },
                    Operacao::ListEnvironments => listar_ambientes(estado, conta, app_id).await,
                    Operacao::PublishVersion => {
                        publicar(
                            estado,
                            conta,
                            app_id,
                            rota.parametro("environment").expect("ambiente da rota"),
                            cabecalhos,
                            corpo,
                            correlation_id,
                        )
                        .await
                    }
                    Operacao::ListMembers | Operacao::PutMember | Operacao::DeleteMember => {
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
                    Operacao::ListAudit => listar_auditoria(estado, conta, app_id).await,
                    Operacao::ListApps | Operacao::CreateApp | Operacao::GetRuntime => {
                        unreachable!("operações tratadas antes do identificador")
                    }
                },
                None if operation == Operacao::GetVersion => erro(
                    StatusCode::NOT_FOUND,
                    "version_not_found",
                    "Versão não encontrada.",
                ),
                None => erro(
                    StatusCode::NOT_FOUND,
                    "app_not_found",
                    "Aplicativo não encontrado.",
                ),
            },
        }
    } else {
        // Preserva 405 nas duas famílias que já distinguiam método de caminho.
        let partes: Vec<&str> = caminho.split('/').skip(1).collect();
        match partes.as_slice() {
            ["api", "v1", "apps"] => {
                raiz(estado, conta, metodo, cabecalhos, corpo, correlation_id).await
            }
            ["api", "v1", "apps", app_id, "members"] => match app_id.parse() {
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
            _ => erro(StatusCode::NOT_FOUND, "not_found", "Não encontrado."),
        }
    };
    let mut resposta = resposta;
    if let Ok(valor) = HeaderValue::from_str(&correlation_id.to_string()) {
        resposta.headers_mut().insert("x-correlation-id", valor);
    }
    resposta
}
