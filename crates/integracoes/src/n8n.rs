//! Entrega de eventos ao n8n por webhook.

use std::time::Duration;

use cms_dominio::eventos::CorpoDoEvento;
use reqwest::header::AUTHORIZATION;
use reqwest::redirect::Policy;

/// O n8n responde na hora e trabalha depois: dez segundos sem resposta é
/// falha, e o evento volta para a fila.
const PRAZO: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum ErroDeEntrega {
    #[error("o n8n não respondeu: {0}")]
    Rede(#[from] reqwest::Error),
    #[error("o n8n recusou o evento com status {0}")]
    Recusado(u16),
}

// Sem `Debug`: a autorização não pode parar em registro.
#[derive(Clone)]
pub struct ClienteN8n {
    http: reqwest::Client,
    url: String,
    autorizacao: String,
}

impl ClienteN8n {
    /// `url` é o webhook de eventos; `autorizacao` vai inteira no cabeçalho
    /// `authorization`.
    pub fn novo(
        url: impl Into<String>,
        autorizacao: impl Into<String>,
    ) -> Result<Self, ErroDeEntrega> {
        let http = reqwest::Client::builder()
            .timeout(PRAZO)
            // O endereço é configuração da casa; um redirecionamento levaria
            // o cabeçalho de autorização para outro lugar.
            .redirect(Policy::none())
            .build()?;
        Ok(Self {
            http,
            url: url.into(),
            autorizacao: autorizacao.into(),
        })
    }

    /// Só resposta 2xx conta como entregue.
    pub async fn entregar(&self, corpo: &CorpoDoEvento) -> Result<(), ErroDeEntrega> {
        let resposta = self
            .http
            .post(&self.url)
            .header(AUTHORIZATION, &self.autorizacao)
            .json(corpo)
            .send()
            .await?;
        if resposta.status().is_success() {
            Ok(())
        } else {
            Err(ErroDeEntrega::Recusado(resposta.status().as_u16()))
        }
    }
}
