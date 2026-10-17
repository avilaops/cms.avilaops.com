use cms_dominio::app_registry::AppRole;

use crate::ErroDeDados;

pub(crate) fn papel_lido(texto: &str) -> Result<AppRole, ErroDeDados> {
    match texto {
        "owner" => Ok(AppRole::Owner),
        "editor" => Ok(AppRole::Editor),
        "viewer" => Ok(AppRole::Viewer),
        outro => Err(ErroDeDados::DadoInvalido(format!(
            "papel de aplicativo desconhecido: {outro}"
        ))),
    }
}
