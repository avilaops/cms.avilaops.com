//! Os fatos que viram automação no n8n e o corpo com que são entregues.
//!
//! O que vai em `dados` é escolhido campo a campo. Renomear um campo aqui não
//! quebra o build: quebra o workflow, em produção. Por isso cada tipo tem um
//! teste de referência logo abaixo.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConteudoPublicado {
    pub documento_id: Uuid,
    pub especie: String,
    pub caminho: String,
    pub titulo: String,
    /// O endereço que o documento tinha, quando a publicação o trocou.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caminho_anterior: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConteudoDespublicado {
    pub documento_id: Uuid,
    pub especie: String,
    pub caminho: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConteudoEnviadoParaRevisao {
    pub documento_id: Uuid,
    pub especie: String,
    pub titulo: String,
    /// A conta que pediu a revisão.
    pub pedido_por: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteCriado {
    /// O e-mail da conta que criou. A criação é aberta: alguém da equipe
    /// precisa olhar.
    pub criado_por: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConviteCriado {
    pub email: String,
    /// "Autor", "Editor" ou "Dono", como a pessoa lê.
    pub papel: String,
    /// O link pronto, com token de uso único. As execuções do n8n guardam o
    /// corpo recebido: por isso a validade é curta.
    pub link: String,
    pub expira_em: DateTime<Utc>,
}

/// Um fato. Serializa só os dados; o tipo vai em campo próprio do corpo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Evento {
    ConteudoPublicado(ConteudoPublicado),
    ConteudoDespublicado(ConteudoDespublicado),
    ConteudoEnviadoParaRevisao(ConteudoEnviadoParaRevisao),
    SiteCriado(SiteCriado),
    ConviteCriado(ConviteCriado),
}

impl Evento {
    /// Estável: é por ele que o workflow escolhe o que fazer.
    pub fn tipo(&self) -> &'static str {
        match self {
            Evento::ConteudoPublicado(_) => "conteudo.publicado",
            Evento::ConteudoDespublicado(_) => "conteudo.despublicado",
            Evento::ConteudoEnviadoParaRevisao(_) => "conteudo.enviado_para_revisao",
            Evento::SiteCriado(_) => "site.criado",
            Evento::ConviteCriado(_) => "convite.criado",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteDoEvento {
    pub id: Uuid,
    pub slug: String,
    pub nome: String,
    /// `https://exemplo.com.br`, sem barra final: o endereço canônico do site.
    pub origem: String,
    /// O site aparece na busca. Fora dela, não se avisa buscador.
    pub na_busca: bool,
    /// A chave do IndexNow, servida em `<origem>/<chave>.txt`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexnow_chave: Option<String>,
}

/// O corpo do `POST` ao n8n.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpoDoEvento {
    pub id: Uuid,
    pub tipo: String,
    pub chave: String,
    pub ocorrido_em: DateTime<Utc>,
    pub site: SiteDoEvento,
    pub dados: Value,
}

/// O endereço canônico do site: o domínio próprio em uso ou, sem ele, o
/// provisório.
pub fn origem_do_site(
    esquema: &str,
    dominio_base: &str,
    slug: &str,
    dominio_ativo: Option<&str>,
) -> String {
    match dominio_ativo {
        Some(dominio) => format!("{esquema}://{dominio}"),
        None => format!("{esquema}://{slug}.{dominio_base}"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    const DOCUMENTO: &str = "0b9f6a3e-3f1c-4d0a-9d55-6f0c2a8f1e11";

    fn documento_id() -> Uuid {
        Uuid::parse_str(DOCUMENTO).expect("uuid válido")
    }

    fn dados(evento: &Evento) -> Value {
        serde_json::to_value(evento).expect("evento serializa")
    }

    #[test]
    fn conteudo_publicado_bate_com_a_referencia() {
        let mut evento = ConteudoPublicado {
            documento_id: documento_id(),
            especie: "post".into(),
            caminho: "/blog/como-escolher".into(),
            titulo: "Como escolher".into(),
            caminho_anterior: None,
        };
        assert_eq!(
            Evento::ConteudoPublicado(evento.clone()).tipo(),
            "conteudo.publicado"
        );
        assert_eq!(
            dados(&Evento::ConteudoPublicado(evento.clone())),
            json!({
                "documentoId": DOCUMENTO,
                "especie": "post",
                "caminho": "/blog/como-escolher",
                "titulo": "Como escolher"
            })
        );

        evento.caminho_anterior = Some("/blog/escolher".into());
        assert_eq!(
            dados(&Evento::ConteudoPublicado(evento))["caminhoAnterior"],
            json!("/blog/escolher")
        );
    }

    #[test]
    fn conteudo_despublicado_bate_com_a_referencia() {
        let evento = Evento::ConteudoDespublicado(ConteudoDespublicado {
            documento_id: documento_id(),
            especie: "pagina".into(),
            caminho: "/contato".into(),
        });
        assert_eq!(evento.tipo(), "conteudo.despublicado");
        assert_eq!(
            dados(&evento),
            json!({ "documentoId": DOCUMENTO, "especie": "pagina", "caminho": "/contato" })
        );
    }

    #[test]
    fn conteudo_enviado_para_revisao_bate_com_a_referencia() {
        let evento = Evento::ConteudoEnviadoParaRevisao(ConteudoEnviadoParaRevisao {
            documento_id: documento_id(),
            especie: "post".into(),
            titulo: "Como escolher".into(),
            pedido_por: "conta-da-autora".into(),
        });
        assert_eq!(evento.tipo(), "conteudo.enviado_para_revisao");
        assert_eq!(
            dados(&evento),
            json!({
                "documentoId": DOCUMENTO,
                "especie": "post",
                "titulo": "Como escolher",
                "pedidoPor": "conta-da-autora"
            })
        );
    }

    #[test]
    fn site_criado_e_convite_criado_batem_com_a_referencia() {
        let site = Evento::SiteCriado(SiteCriado {
            criado_por: "ana@exemplo.example".into(),
        });
        assert_eq!(site.tipo(), "site.criado");
        assert_eq!(dados(&site), json!({ "criadoPor": "ana@exemplo.example" }));

        let convite = Evento::ConviteCriado(ConviteCriado {
            email: "bia@exemplo.example".into(),
            papel: "Editor".into(),
            link: "https://cms.example/convite/abc".into(),
            expira_em: Utc
                .with_ymd_and_hms(2026, 10, 11, 12, 0, 0)
                .single()
                .expect("data válida"),
        });
        assert_eq!(convite.tipo(), "convite.criado");
        assert_eq!(
            dados(&convite),
            json!({
                "email": "bia@exemplo.example",
                "papel": "Editor",
                "link": "https://cms.example/convite/abc",
                "expiraEm": "2026-10-11T12:00:00Z"
            })
        );
    }

    #[test]
    fn corpo_bate_com_a_referencia() {
        let corpo = CorpoDoEvento {
            id: documento_id(),
            tipo: "conteudo.despublicado".into(),
            chave: "historico:42".into(),
            ocorrido_em: Utc
                .with_ymd_and_hms(2026, 10, 9, 12, 0, 0)
                .single()
                .expect("data válida"),
            site: SiteDoEvento {
                id: documento_id(),
                slug: "oficina".into(),
                nome: "Oficina Exemplo".into(),
                origem: "https://oficina.sites.example".into(),
                na_busca: true,
                indexnow_chave: Some("0123456789abcdef".into()),
            },
            dados: json!({ "caminho": "/contato" }),
        };
        assert_eq!(
            serde_json::to_value(&corpo).expect("corpo serializa"),
            json!({
                "id": DOCUMENTO,
                "tipo": "conteudo.despublicado",
                "chave": "historico:42",
                "ocorridoEm": "2026-10-09T12:00:00Z",
                "site": {
                    "id": DOCUMENTO,
                    "slug": "oficina",
                    "nome": "Oficina Exemplo",
                    "origem": "https://oficina.sites.example",
                    "naBusca": true,
                    "indexnowChave": "0123456789abcdef"
                },
                "dados": { "caminho": "/contato" }
            })
        );
    }

    #[test]
    fn a_origem_e_o_dominio_proprio_ou_o_provisorio() {
        assert_eq!(
            origem_do_site("https", "sites.example", "oficina", None),
            "https://oficina.sites.example"
        );
        assert_eq!(
            origem_do_site("https", "sites.example", "oficina", Some("oficina.example")),
            "https://oficina.example"
        );
    }
}
