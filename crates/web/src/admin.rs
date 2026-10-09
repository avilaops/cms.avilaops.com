//! Rotas de volta das automações. Quem chama é o n8n, com token próprio.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use cms_dados::Encerramento;
use serde::Deserialize;
use serde_json::Value;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::resposta::simples;
use crate::{Estado, Segredo};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PedidoDeEncerramento {
    /// O que a automação fez. É o que o painel mostra: "buscadores avisados"
    /// só aparece com a resposta de verdade gravada aqui.
    resultado: Value,
}

/// Compara em tempo constante. Sem token configurado, ninguém entra.
fn autorizado(cabecalhos: &HeaderMap, token: Option<&Segredo>) -> bool {
    let Some(token) = token else {
        return false;
    };
    let recebido = cabecalhos
        .get(AUTHORIZATION)
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.strip_prefix("Bearer "))
        .unwrap_or("");
    recebido.as_bytes().ct_eq(token.expor().as_bytes()).into()
}

/// `POST /api/admin/eventos/{id}/encerrar`
///
/// O corpo só é lido depois do token: quem não é o n8n não descobre nada
/// sobre o formato.
pub async fn encerrar_evento(
    State(estado): State<Estado>,
    Path(id): Path<Uuid>,
    cabecalhos: HeaderMap,
    corpo: Bytes,
) -> Response {
    if !autorizado(&cabecalhos, estado.configuracao.token_do_n8n.as_ref()) {
        return simples(StatusCode::UNAUTHORIZED, "Não autorizado.");
    }
    let Ok(pedido) = serde_json::from_slice::<PedidoDeEncerramento>(&corpo) else {
        return simples(StatusCode::BAD_REQUEST, "Corpo inválido.");
    };
    match cms_dados::encerrar_evento(&estado.pool, id, &pedido.resultado).await {
        Ok(Encerramento::Encerrado) => simples(StatusCode::OK, "Encerrado."),
        Ok(Encerramento::JaEncerrado) => simples(StatusCode::CONFLICT, "Evento já encerrado."),
        Ok(Encerramento::Inexistente) => simples(StatusCode::NOT_FOUND, "Evento não encontrado."),
        Err(erro) => {
            tracing::error!(%erro, evento = %id, "falha ao encerrar o evento");
            simples(StatusCode::INTERNAL_SERVER_ERROR, "Erro interno.")
        }
    }
}
