//! Painel humano do App Builder. As mutações passam pela mesma API JSON usada
//! por clientes internos; esta camada só monta as páginas e o estado inicial.

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;
use cms_dados::{AppDaConta, ErroDeApp};
use cms_dominio::Conta;
use cms_dominio::app_registry::AppRole;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::Estado;
use crate::resposta::{ErroWeb, html_privado, redirecionar, simples, texto};

const SCRIPT: &str = include_str!("../recursos/app-builder.js");
const CAMINHO_DO_SCRIPT: &str = "/recursos/app-builder.js";

pub fn recurso(caminho: &str) -> Option<Response> {
    (caminho == CAMINHO_DO_SCRIPT).then(|| {
        texto(
            StatusCode::OK,
            "text/javascript; charset=utf-8",
            SCRIPT.into(),
        )
    })
}

struct AppNaLista {
    nome: String,
    slug: String,
    bundle_id: String,
    papel: &'static str,
    revisao: i64,
    abrir: String,
}

impl From<AppDaConta> for AppNaLista {
    fn from(app: AppDaConta) -> Self {
        Self {
            nome: app.nome,
            slug: app.slug,
            bundle_id: app.bundle_id,
            papel: rotulo_do_papel(app.papel),
            revisao: app.revisao_do_rascunho,
            abrir: format!("/painel/apps/{}", app.id),
        }
    }
}

#[derive(Template)]
#[template(path = "apps.html")]
struct PaginaDeApps {
    quem: String,
    apps: Vec<AppNaLista>,
    erro: Option<String>,
    nome: String,
    slug: String,
    bundle_id: String,
}

#[derive(Deserialize)]
struct NovoApp {
    #[serde(default)]
    nome: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    bundle_id: String,
}

fn quem(conta: &Conta) -> String {
    if conta.nome.is_empty() {
        conta.email.clone()
    } else {
        format!("{} · {}", conta.nome, conta.email)
    }
}

fn rotulo_do_papel(papel: AppRole) -> &'static str {
    match papel {
        AppRole::Owner => "Proprietário",
        AppRole::Editor => "Editor",
        AppRole::Viewer => "Leitor",
    }
}

async fn pagina_de_apps(
    estado: &Estado,
    conta: &Conta,
    status: StatusCode,
    erro: Option<String>,
    nome: String,
    slug: String,
    bundle_id: String,
) -> Result<Response, ErroWeb> {
    let apps = cms_dados::apps_da_conta(&estado.pool, conta)
        .await?
        .into_iter()
        .map(AppNaLista::from)
        .collect();
    let corpo = PaginaDeApps {
        quem: quem(conta),
        apps,
        erro,
        nome,
        slug,
        bundle_id,
    }
    .render()?;
    Ok(html_privado(status, corpo))
}

pub async fn listar(estado: &Estado, conta: &Conta) -> Result<Response, ErroWeb> {
    pagina_de_apps(
        estado,
        conta,
        StatusCode::OK,
        None,
        String::new(),
        String::new(),
        String::new(),
    )
    .await
}

pub async fn criar(estado: &Estado, conta: &Conta, corpo: &[u8]) -> Result<Response, ErroWeb> {
    let Ok(formulario) = serde_urlencoded::from_bytes::<NovoApp>(corpo) else {
        return Ok(simples(StatusCode::BAD_REQUEST, "Formulário inválido."));
    };
    let slug = formulario.slug.trim().to_lowercase();
    let bundle_id = formulario.bundle_id.trim().to_string();
    match cms_dados::criar_app(
        &estado.pool,
        conta,
        &slug,
        &formulario.nome,
        &bundle_id,
        Uuid::new_v4(),
    )
    .await
    {
        Ok(app) => Ok(redirecionar(
            StatusCode::SEE_OTHER,
            &format!("/painel/apps/{}", app.id),
        )),
        Err(ErroDeApp::Dados(erro)) => Err(erro.into()),
        Err(erro) => {
            pagina_de_apps(
                estado,
                conta,
                status_do_erro(&erro),
                Some(erro.to_string()),
                formulario.nome,
                slug,
                bundle_id,
            )
            .await
        }
    }
}

fn status_do_erro(erro: &ErroDeApp) -> StatusCode {
    match erro {
        ErroDeApp::EmUso | ErroDeApp::ConflitoDeRevisao => StatusCode::CONFLICT,
        ErroDeApp::NaoEncontrado | ErroDeApp::VersaoInexistente => StatusCode::NOT_FOUND,
        ErroDeApp::Dados(_) => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

struct VersaoNaTela {
    numero: i32,
    schema: String,
    runtime: String,
    criado_por: String,
    criado_em: String,
}

struct AmbienteNaTela {
    nome: &'static str,
    rotulo: &'static str,
    versao: Option<i32>,
    atualizado_por: Option<String>,
}

struct EventoNaTela {
    acao: String,
    conta: String,
    criado_em: String,
    correlation_id: Uuid,
}

struct ParticipanteNaTela {
    conta: String,
    papel: &'static str,
}

#[derive(Template)]
#[template(path = "app_editor.html")]
struct PaginaDoEditor {
    id: Uuid,
    nome: String,
    slug: String,
    bundle_id: String,
    papel: &'static str,
    pode_editar: bool,
    pode_publicar: bool,
    revisao: i64,
    definicao: String,
    versoes: Vec<VersaoNaTela>,
    ambientes: Vec<AmbienteNaTela>,
    auditoria: Vec<EventoNaTela>,
    participantes: Vec<ParticipanteNaTela>,
    script: &'static str,
}

fn definicao_inicial(slug: &str, nome: &str) -> serde_json::Value {
    json!({
        "schemaVersion": "1.0.0",
        "appId": slug,
        "minimumRuntimeVersion": "1.0.0",
        "navigation": { "initialRoute": "home" },
        "screens": [{
            "id": "home",
            "route": "home",
            "title": "Início",
            "components": [{
                "type": "text",
                "id": "titulo",
                "value": nome
            }]
        }]
    })
}

pub async fn editar(estado: &Estado, conta: &Conta, app_id: Uuid) -> Result<Response, ErroWeb> {
    let app = match cms_dados::app_da_conta(&estado.pool, conta, app_id).await {
        Ok(app) => app,
        Err(ErroDeApp::NaoEncontrado) => {
            return Ok(simples(StatusCode::NOT_FOUND, "Aplicativo não encontrado."));
        }
        Err(ErroDeApp::Dados(erro)) => return Err(erro.into()),
        Err(_) => {
            return Ok(simples(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Aplicativo inválido.",
            ));
        }
    };
    let (definicao, revisao) = match cms_dados::rascunho_do_app(&estado.pool, conta, app_id).await {
        Ok(rascunho) => (rascunho.definicao, rascunho.revisao),
        Err(ErroDeApp::SemRascunho) => (definicao_inicial(&app.slug, &app.nome), 0),
        Err(ErroDeApp::Dados(erro)) => return Err(erro.into()),
        Err(_) => return Ok(simples(StatusCode::NOT_FOUND, "Aplicativo não encontrado.")),
    };
    let versoes = cms_dados::versoes_do_app(&estado.pool, conta, app_id)
        .await
        .map_err(erro_web)?
        .into_iter()
        .map(|versao| VersaoNaTela {
            numero: versao.numero,
            schema: versao.schema_version,
            runtime: versao.minimum_runtime_version,
            criado_por: versao.criado_por,
            criado_em: versao.criado_em.format("%d/%m/%Y %H:%M UTC").to_string(),
        })
        .collect();
    let publicados = cms_dados::ambientes_do_app(&estado.pool, conta, app_id)
        .await
        .map_err(erro_web)?;
    let ambientes = [
        ("development", "Desenvolvimento"),
        ("staging", "Homologação"),
        ("production", "Produção"),
    ]
    .into_iter()
    .map(|(nome, rotulo)| {
        let publicado = publicados.iter().find(|ambiente| ambiente.nome == nome);
        AmbienteNaTela {
            nome,
            rotulo,
            versao: publicado.map(|ambiente| ambiente.numero_da_versao),
            atualizado_por: publicado.map(|ambiente| ambiente.atualizado_por.clone()),
        }
    })
    .collect();
    let auditoria = cms_dados::auditoria_do_app(&estado.pool, conta, app_id, 20)
        .await
        .map_err(erro_web)?
        .into_iter()
        .map(|evento| EventoNaTela {
            acao: evento.acao,
            conta: evento.conta,
            criado_em: evento.criado_em.format("%d/%m/%Y %H:%M UTC").to_string(),
            correlation_id: evento.correlation_id,
        })
        .collect();
    let participantes = if app.papel == AppRole::Owner {
        cms_dados::participantes_do_app(&estado.pool, conta, app_id)
            .await
            .map_err(erro_web)?
            .into_iter()
            .map(|participante| ParticipanteNaTela {
                conta: participante.conta,
                papel: rotulo_do_papel(participante.papel),
            })
            .collect()
    } else {
        Vec::new()
    };
    let corpo = PaginaDoEditor {
        id: app.id,
        nome: app.nome,
        slug: app.slug,
        bundle_id: app.bundle_id,
        papel: rotulo_do_papel(app.papel),
        pode_editar: matches!(app.papel, AppRole::Owner | AppRole::Editor),
        pode_publicar: app.papel == AppRole::Owner,
        revisao,
        definicao: serde_json::to_string_pretty(&definicao).expect("JSON do banco é serializável"),
        versoes,
        ambientes,
        auditoria,
        participantes,
        script: CAMINHO_DO_SCRIPT,
    }
    .render()?;
    let mut resposta = html_privado(StatusCode::OK, corpo);
    crate::resposta::liberar_script_do_painel(&mut resposta);
    Ok(resposta)
}

fn erro_web(erro: ErroDeApp) -> ErroWeb {
    match erro {
        ErroDeApp::Dados(erro) => erro.into(),
        _ => cms_dados::ErroDeDados::DadoInvalido(erro.to_string()).into(),
    }
}
