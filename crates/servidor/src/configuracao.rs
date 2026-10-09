//! Configuração lida do ambiente. Ver `.env.example`.

use std::path::PathBuf;

use cms_web::Configuracao;

use crate::Erro;

#[derive(Debug, Clone)]
pub struct Ambiente {
    pub banco: String,
    pub porta: u16,
    pub web: Configuracao,
}

fn variavel(nome: &str) -> Option<String> {
    std::env::var(nome)
        .ok()
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
}

fn obrigatoria(nome: &str) -> Result<String, Erro> {
    variavel(nome).ok_or_else(|| Erro::Configuracao(format!("defina {nome}")))
}

impl Ambiente {
    pub fn ler() -> Result<Self, Erro> {
        let porta = match variavel("PORTA") {
            Some(texto) => texto.parse().map_err(|_| {
                Erro::Configuracao(format!("PORTA não é um número de porta: {texto}"))
            })?,
            None => 3090,
        };
        let esquema = variavel("ESQUEMA").unwrap_or_else(|| "https".to_string());
        if esquema != "https" && esquema != "http" {
            return Err(Erro::Configuracao(format!(
                "ESQUEMA precisa ser http ou https, veio {esquema}"
            )));
        }
        Ok(Self {
            banco: obrigatoria("DATABASE_URL")?,
            porta,
            web: Configuracao {
                dominio_base: obrigatoria("DOMINIO_BASE")?.to_ascii_lowercase(),
                esquema,
                diretorio_de_midia: PathBuf::from(
                    variavel("DIRETORIO_DE_MIDIA").unwrap_or_else(|| "./var/midia".to_string()),
                ),
            },
        })
    }
}
