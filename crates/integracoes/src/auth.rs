//! O login único da Ávila Ops (`auth.avilaops.com`).
//!
//! O cookie `avila_sso` vale em todo `*.avilaops.com`, então chega aqui também
//! de quem entrou em outro sistema e nunca foi liberado para o CMS. Por isso o
//! token não é conferido localmente: vai ao `/api/session?app=…` do Auth, que
//! diz quem é a pessoa e se ela pode entrar. De quebra, o CMS não guarda o
//! segredo que assina a sessão de todos os outros sistemas.

use std::time::Duration;

use cms_dominio::Conta;
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, COOKIE};
use reqwest::redirect::Policy;
use serde::Deserialize;

pub const COOKIE_DE_SESSAO: &str = "avila_sso";

const PRAZO: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RespostaDoAuth {
    /// Sem cookie, cookie vencido ou que o Auth não reconhece.
    SemSessao,
    /// A conta existe, mas não foi liberada para o CMS.
    SemAcesso,
    Entrou(Conta),
    /// O Auth não respondeu, ou respondeu algo que não é um "sim" claro.
    Indisponivel,
}

#[derive(Deserialize)]
struct Corpo {
    #[serde(default)]
    autenticado: bool,
    #[serde(default)]
    permitido: bool,
    sessao: Option<Sessao>,
}

#[derive(Deserialize)]
struct Sessao {
    #[serde(default)]
    sub: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    nome: String,
    #[serde(default)]
    papel: String,
}

/// Fecha em tudo o que não for um "sim" explícito.
fn ler(status: StatusCode, corpo: Option<Corpo>) -> RespostaDoAuth {
    if status == StatusCode::UNAUTHORIZED {
        return RespostaDoAuth::SemSessao;
    }
    let (StatusCode::OK, Some(corpo)) = (status, corpo) else {
        return RespostaDoAuth::Indisponivel;
    };
    if !corpo.autenticado {
        return RespostaDoAuth::SemSessao;
    }
    let Some(sessao) = corpo.sessao else {
        return RespostaDoAuth::Indisponivel;
    };
    if sessao.sub.is_empty() || !sessao.email.contains('@') {
        return RespostaDoAuth::Indisponivel;
    }
    if !corpo.permitido {
        return RespostaDoAuth::SemAcesso;
    }
    RespostaDoAuth::Entrou(Conta::nova(
        &sessao.sub,
        &sessao.email,
        &sessao.nome,
        &sessao.papel,
    ))
}

/// Três blocos base64url separados por ponto: o formato de um JWT. O token
/// vira o valor de um cabeçalho `Cookie` montado aqui; qualquer outro
/// caractere seria um cabeçalho forjado por quem controla o próprio cookie.
fn tem_formato_de_token(token: &str) -> bool {
    let blocos: Vec<&str> = token.split('.').collect();
    blocos.len() == 3
        && blocos.iter().all(|bloco| {
            !bloco.is_empty()
                && bloco
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
}

/// Codifica um valor de parâmetro de URL.
fn codificar(valor: &str) -> String {
    let mut saida = String::with_capacity(valor.len());
    for byte in valor.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            saida.push(char::from(byte));
        } else {
            saida.push_str(&format!("%{byte:02X}"));
        }
    }
    saida
}

#[derive(Debug, Clone)]
pub struct ClienteAuth {
    http: reqwest::Client,
    url: String,
    app: String,
}

impl ClienteAuth {
    /// `url` é a origem do Auth, sem barra final; `app` é o identificador do
    /// CMS no cadastro de aplicações de lá.
    pub fn novo(url: &str, app: &str) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(PRAZO)
            .redirect(Policy::none())
            .build()?;
        Ok(Self {
            http,
            url: url.trim_end_matches('/').to_string(),
            app: app.to_string(),
        })
    }

    /// A tela de login do Auth, voltando para `retorno`.
    pub fn url_de_login(&self, retorno: &str) -> String {
        format!(
            "{}/login?app={}&returnTo={}",
            self.url,
            codificar(&self.app),
            codificar(retorno)
        )
    }

    /// Pergunta ao Auth quem é o dono deste cookie e se pode entrar no CMS.
    pub async fn consultar(&self, token: Option<&str>) -> RespostaDoAuth {
        let Some(token) = token.filter(|token| tem_formato_de_token(token)) else {
            return RespostaDoAuth::SemSessao;
        };
        let resposta = self
            .http
            .get(format!(
                "{}/api/session?app={}",
                self.url,
                codificar(&self.app)
            ))
            .header(COOKIE, format!("{COOKIE_DE_SESSAO}={token}"))
            .header(ACCEPT, "application/json")
            .send()
            .await;
        match resposta {
            Ok(resposta) => {
                let status = resposta.status();
                ler(status, resposta.json::<Corpo>().await.ok())
            }
            Err(_) => RespostaDoAuth::Indisponivel,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn corpo(json: &str) -> Option<Corpo> {
        serde_json::from_str(json).ok()
    }

    const SESSAO: &str =
        r#""sessao":{"sub":"42","email":"Ana@Exemplo.example","nome":"Ana","papel":"CLIENTE"}"#;

    #[test]
    fn so_um_sim_explicito_entra() {
        let entrou = ler(
            StatusCode::OK,
            corpo(&format!(
                r#"{{"autenticado":true,"permitido":true,{SESSAO}}}"#
            )),
        );
        assert_eq!(
            entrou,
            RespostaDoAuth::Entrou(Conta::nova("42", "ana@exemplo.example", "Ana", "CLIENTE"))
        );

        // Auth antigo, que não conhece `?app=`, responde sem `permitido`.
        assert_eq!(
            ler(
                StatusCode::OK,
                corpo(&format!(r#"{{"autenticado":true,{SESSAO}}}"#))
            ),
            RespostaDoAuth::SemAcesso
        );
        assert_eq!(
            ler(StatusCode::UNAUTHORIZED, corpo(r#"{"autenticado":false}"#)),
            RespostaDoAuth::SemSessao
        );
        assert_eq!(
            ler(StatusCode::OK, corpo(r#"{"autenticado":false}"#)),
            RespostaDoAuth::SemSessao
        );
        for (status, json) in [
            (StatusCode::OK, "não é json"),
            (StatusCode::OK, r#"{"autenticado":true,"permitido":true}"#),
            (
                StatusCode::OK,
                r#"{"autenticado":true,"permitido":true,"sessao":{"sub":"","email":"a@b.example"}}"#,
            ),
            (StatusCode::BAD_GATEWAY, r#"{"autenticado":true}"#),
        ] {
            assert_eq!(
                ler(status, corpo(json)),
                RespostaDoAuth::Indisponivel,
                "{json}"
            );
        }
    }

    #[test]
    fn token_que_nao_parece_jwt_nem_sai_daqui() {
        assert!(tem_formato_de_token("abc.DEF_1-2.xyz"));
        for ruim in [
            "",
            "abc",
            "a.b",
            "a.b.c.d",
            "a..c",
            "a.b.c; outro=1",
            "a.b.c\r\nx: y",
        ] {
            assert!(!tem_formato_de_token(ruim), "{ruim:?}");
        }
    }

    #[test]
    fn o_endereco_de_login_leva_o_retorno_codificado() {
        let cliente = ClienteAuth::novo("https://auth.example/", "cms").expect("cliente");
        assert_eq!(
            cliente.url_de_login("https://cms.example/painel?volta=1"),
            "https://auth.example/login?app=cms&returnTo=https%3A%2F%2Fcms.example%2Fpainel%3Fvolta%3D1"
        );
    }
}
