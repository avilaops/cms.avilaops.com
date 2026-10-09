//! Quem pode abrir as telas de um site no painel.

use axum::http::StatusCode;
use axum::response::Response;
use cms_dados::SiteGravado;
use cms_dominio::{Ator, Conta};

use crate::Estado;
use crate::resposta::{ErroWeb, simples};

/// O site e o ator, ou a resposta que barra.
pub type Acesso = Result<(SiteGravado, Ator), Response>;

/// Resolve o site pelo slug e o papel da conta nele. Quem não participa
/// recebe o mesmo 404 de um site que não existe: o painel não confirma
/// endereços.
pub async fn ao_site(estado: &Estado, conta: &Conta, slug: &str) -> Result<Acesso, ErroWeb> {
    let nao_encontrado = || simples(StatusCode::NOT_FOUND, "Não encontrado.");
    let Some(site) = cms_dados::site_por_slug(&estado.pool, slug).await? else {
        return Ok(Err(nao_encontrado()));
    };
    let papel = cms_dados::papel_no_site(&estado.pool, site.id, &conta.sub).await?;
    match conta.ator(papel) {
        Some(ator) => Ok(Ok((site, ator))),
        None => Ok(Err(nao_encontrado())),
    }
}
