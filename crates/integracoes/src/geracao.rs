//! Interface do provedor que, numa fase seguinte, gera telas a partir do
//! pedido do estúdio.
//!
//! Não há cliente de modelo neste repositório. Este módulo só descreve o
//! pedido, lê a configuração e devolve o adaptador desligado. Uma URL
//! preenchida não faz este binário chamar serviço nenhum.

use std::future::Future;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguracaoDeGeracao {
    /// `nenhum` quando a variável não foi definida.
    pub provedor: String,
    pub url: Option<String>,
    pub modelo: Option<String>,
}

impl ConfiguracaoDeGeracao {
    pub fn do_ambiente(ler: impl Fn(&str) -> Option<String>) -> Result<Self, ErroDeConfiguracao> {
        let provedor = ler("ESTUDIO_GERACAO_PROVEDOR")
            .map(|valor| valor.trim().to_string())
            .filter(|valor| !valor.is_empty())
            .unwrap_or_else(|| "nenhum".to_string());
        if !provedor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || provedor.len() > 40
        {
            return Err(ErroDeConfiguracao::Provedor);
        }
        let url = ler("ESTUDIO_GERACAO_URL").map(|valor| valor.trim().to_string());
        let url = match url.as_deref() {
            None | Some("") => None,
            Some(url) => {
                let endereco = reqwest::Url::parse(url).map_err(|_| ErroDeConfiguracao::Url)?;
                if endereco.scheme() != "https" || endereco.host_str().is_none() {
                    return Err(ErroDeConfiguracao::Url);
                }
                Some(endereco.to_string())
            }
        };
        let modelo = ler("ESTUDIO_GERACAO_MODELO")
            .map(|valor| valor.trim().to_string())
            .filter(|valor| !valor.is_empty());
        if let Some(modelo) = &modelo {
            let ok = modelo.len() <= 80
                && modelo
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
            if !ok {
                return Err(ErroDeConfiguracao::Modelo);
            }
        }
        Ok(Self {
            provedor,
            url,
            modelo,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroDeConfiguracao {
    Provedor,
    Url,
    Modelo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoDeGeracao {
    /// Nenhuma URL foi informada. É o estado deste binário.
    Ausente,
    /// Há URL e modelo, mas o adaptador HTTP ainda não existe.
    ConfiguradoSemAdaptador { modelo: String },
}

pub fn estado(config: &ConfiguracaoDeGeracao) -> EstadoDeGeracao {
    match (&config.url, &config.modelo) {
        (Some(_), Some(modelo)) => EstadoDeGeracao::ConfiguradoSemAdaptador {
            modelo: modelo.clone(),
        },
        _ => EstadoDeGeracao::Ausente,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PedidoDeGeracao {
    pub pedido: String,
    pub sistema: Option<String>,
}

/// O que o adaptador devolveria para o estúdio gravar como nós. Nesta fase
/// nenhum adaptador produz isso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropostaDeTela {
    pub nome: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErroDeGeracao {
    #[error("geração de telas não está configurada")]
    NaoConfigurado,
    #[error("a configuração existe, mas não há adaptador de modelo neste binário")]
    SemAdaptador,
}

pub trait ProvedorDeGeracao: Send + Sync {
    fn nome(&self) -> &'static str;

    fn gerar(
        &self,
        pedido: PedidoDeGeracao,
    ) -> impl Future<Output = Result<PropostaDeTela, ErroDeGeracao>> + Send;
}

/// O único adaptador ligado no processo. Não fala com a rede.
#[derive(Debug, Clone, Copy, Default)]
pub struct AdaptadorDesligado;

impl ProvedorDeGeracao for AdaptadorDesligado {
    fn nome(&self) -> &'static str {
        "nenhum"
    }

    async fn gerar(&self, pedido: PedidoDeGeracao) -> Result<PropostaDeTela, ErroDeGeracao> {
        let _ = pedido;
        Err(ErroDeGeracao::NaoConfigurado)
    }
}

/// Mesmo com URL e modelo válidos, esta versão devolve o adaptador desligado.
/// A fase seguinte é que passa a escolher um cliente HTTP.
pub fn adaptador_inicial(config: &ConfiguracaoDeGeracao) -> AdaptadorDesligado {
    let _ = config;
    AdaptadorDesligado
}

#[cfg(test)]
mod testes {
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use super::*;

    fn resolver<F: Future>(futuro: F) -> F::Output {
        let mut futuro = std::pin::pin!(futuro);
        let waker = Waker::noop();
        let mut contexto = Context::from_waker(waker);
        match futuro.as_mut().poll(&mut contexto) {
            Poll::Ready(valor) => valor,
            Poll::Pending => panic!("o adaptador desligado não espera"),
        }
    }

    #[test]
    fn sem_variavel_o_provedor_fica_ausente_e_nao_gera() {
        let config = ConfiguracaoDeGeracao::do_ambiente(|_| None).expect("padrão");
        assert_eq!(config.provedor, "nenhum");
        assert_eq!(estado(&config), EstadoDeGeracao::Ausente);
        let adaptador = adaptador_inicial(&config);
        let erro = resolver(adaptador.gerar(PedidoDeGeracao {
            pedido: "uma página".into(),
            sistema: None,
        }));
        assert_eq!(erro, Err(ErroDeGeracao::NaoConfigurado));
    }

    #[test]
    fn url_e_modelo_nao_ligam_um_cliente() {
        let config = ConfiguracaoDeGeracao::do_ambiente(|nome| match nome {
            "ESTUDIO_GERACAO_PROVEDOR" => Some("http".into()),
            "ESTUDIO_GERACAO_URL" => Some("https://modelo.example/v1".into()),
            "ESTUDIO_GERACAO_MODELO" => Some("modelo-teste".into()),
            _ => None,
        })
        .expect("configuração válida");
        assert_eq!(
            estado(&config),
            EstadoDeGeracao::ConfiguradoSemAdaptador {
                modelo: "modelo-teste".into(),
            }
        );
        assert_eq!(adaptador_inicial(&config).nome(), "nenhum");
        assert!(
            ConfiguracaoDeGeracao::do_ambiente(|nome| {
                (nome == "ESTUDIO_GERACAO_URL").then(|| "http://modelo.example".into())
            })
            .is_err()
        );
    }
}
