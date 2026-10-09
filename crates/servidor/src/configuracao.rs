//! Configuração lida do ambiente. Ver `.env.example`.

use std::path::PathBuf;

use cms_dominio::LimitesDeCriacao;
use cms_web::{Configuracao, Segredo};

use crate::Erro;

#[derive(Debug, Clone)]
pub struct Ambiente {
    pub banco: String,
    pub porta: u16,
    pub web: Configuracao,
    /// Para onde os eventos vão. Sem isso, ficam na fila.
    pub n8n: Option<SaidaParaN8n>,
    /// O login do painel. Sem isso, a aplicação só serve sites.
    pub auth: Option<LoginPeloAuth>,
}

#[derive(Debug, Clone)]
pub struct LoginPeloAuth {
    pub url: String,
    /// O identificador do CMS no cadastro de aplicações do Auth.
    pub app: String,
}

fn inteiro(nome: &str, padrao: i64) -> Result<i64, Erro> {
    match variavel(nome) {
        Some(texto) => texto
            .parse()
            .ok()
            .filter(|valor| *valor >= 0)
            .ok_or_else(|| Erro::Configuracao(format!("{nome} precisa ser um número inteiro"))),
        None => Ok(padrao),
    }
}

#[derive(Debug, Clone)]
pub struct SaidaParaN8n {
    pub url: String,
    /// O valor inteiro do cabeçalho `authorization`.
    pub autorizacao: Segredo,
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
        let n8n = match (
            variavel("N8N_EVENTOS_URL"),
            variavel("N8N_EVENTOS_AUTORIZACAO"),
        ) {
            (Some(url), Some(autorizacao)) => Some(SaidaParaN8n {
                url,
                autorizacao: Segredo::novo(autorizacao),
            }),
            (None, None) => None,
            _ => {
                return Err(Erro::Configuracao(
                    "N8N_EVENTOS_URL e N8N_EVENTOS_AUTORIZACAO andam juntas: defina as duas ou nenhuma"
                        .into(),
                ));
            }
        };
        let chave_do_indexnow = variavel("INDEXNOW_CHAVE");
        // O protocolo aceita de 8 a 128 caracteres entre letras, números e hífen.
        if let Some(chave) = &chave_do_indexnow {
            let valida = (8..=128).contains(&chave.len())
                && chave.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            if !valida {
                return Err(Erro::Configuracao(
                    "INDEXNOW_CHAVE precisa ter de 8 a 128 letras, números ou hífens".into(),
                ));
            }
        }
        let host_do_painel = variavel("HOST_DO_PAINEL").map(|host| host.to_ascii_lowercase());
        let auth = match (&host_do_painel, variavel("SSO_APP_ID")) {
            (Some(_), Some(app)) => Some(LoginPeloAuth {
                url: variavel("SSO_URL").unwrap_or_else(|| "https://auth.avilaops.com".to_string()),
                app,
            }),
            (None, None) => None,
            _ => {
                return Err(Erro::Configuracao(
                    "HOST_DO_PAINEL e SSO_APP_ID andam juntas: defina as duas ou nenhuma".into(),
                ));
            }
        };
        let padrao = LimitesDeCriacao::default();
        let limites_de_criacao = LimitesDeCriacao {
            sites_por_conta: inteiro("LIMITE_DE_SITES_POR_CONTA", padrao.sites_por_conta)?,
            criacoes_por_dia: inteiro("LIMITE_DE_CRIACOES_POR_DIA", padrao.criacoes_por_dia)?,
        };
        Ok(Self {
            n8n,
            auth,
            banco: obrigatoria("DATABASE_URL")?,
            porta,
            web: Configuracao {
                dominio_base: obrigatoria("DOMINIO_BASE")?.to_ascii_lowercase(),
                esquema,
                diretorio_de_midia: PathBuf::from(
                    variavel("DIRETORIO_DE_MIDIA").unwrap_or_else(|| "./var/midia".to_string()),
                ),
                token_do_n8n: variavel("N8N_TOKEN_DE_VOLTA").map(Segredo::novo),
                chave_do_indexnow,
                host_do_painel,
                limites_de_criacao,
            },
        })
    }
}
