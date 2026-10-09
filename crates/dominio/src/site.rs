//! O site como o CMS o guarda.

use std::fmt;

use motor_web::tipos::{Midia, Organizacao, Site};
use serde::{Deserialize, Serialize};

/// O que o dono define sobre o site. É o `Site` do motor sem a origem, que
/// depende do host em que o pedido chegou.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerfilDoSite {
    pub nome: String,
    pub descricao: String,
    pub idioma: String,
    pub logo: Midia,
    pub organizacao: Organizacao,
    #[serde(default)]
    pub diretrizes_ia: Vec<String>,
    pub orcamento_peso_kb: u32,
}

impl PerfilDoSite {
    pub fn para_site(&self, origem: &str) -> Site {
        Site {
            origem: origem.to_string(),
            nome: self.nome.clone(),
            descricao: self.descricao.clone(),
            idioma: self.idioma.clone(),
            logo: self.logo.clone(),
            organizacao: self.organizacao.clone(),
            diretrizes_ia: self.diretrizes_ia.clone(),
            orcamento_peso_kb: self.orcamento_peso_kb,
        }
    }
}

impl From<Site> for PerfilDoSite {
    fn from(site: Site) -> Self {
        Self {
            nome: site.nome,
            descricao: site.descricao,
            idioma: site.idioma,
            logo: site.logo,
            organizacao: site.organizacao,
            diretrizes_ia: site.diretrizes_ia,
            orcamento_peso_kb: site.orcamento_peso_kb,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situacao {
    /// Recém-criado: responde, mas fora da busca.
    EmMontagem,
    Ativo,
    /// Tirado do ar pela equipe. Suspender é decisão de gente.
    Suspenso,
}

impl Situacao {
    pub fn como_texto(self) -> &'static str {
        match self {
            Situacao::EmMontagem => "em-montagem",
            Situacao::Ativo => "ativo",
            Situacao::Suspenso => "suspenso",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SituacaoDesconhecida(pub String);

impl fmt::Display for SituacaoDesconhecida {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "situação de site desconhecida: {}", self.0)
    }
}

impl std::error::Error for SituacaoDesconhecida {}

impl TryFrom<&str> for Situacao {
    type Error = SituacaoDesconhecida;

    fn try_from(texto: &str) -> Result<Self, Self::Error> {
        match texto {
            "em-montagem" => Ok(Situacao::EmMontagem),
            "ativo" => Ok(Situacao::Ativo),
            "suspenso" => Ok(Situacao::Suspenso),
            outro => Err(SituacaoDesconhecida(outro.to_string())),
        }
    }
}

/// Um site só entra na busca quando está ativo e tem endereço definitivo:
/// domínio próprio, ou o provisório assumido como definitivo pelo dono. Sem
/// isso haveria dois endereços indexados para o mesmo conteúdo.
pub fn aparece_na_busca(
    situacao: Situacao,
    tem_dominio_proprio: bool,
    provisorio_definitivo: bool,
) -> bool {
    situacao == Situacao::Ativo && (tem_dominio_proprio || provisorio_definitivo)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn situacao_vai_e_volta_como_texto() {
        for situacao in [Situacao::EmMontagem, Situacao::Ativo, Situacao::Suspenso] {
            assert_eq!(Situacao::try_from(situacao.como_texto()), Ok(situacao));
        }
        assert!(Situacao::try_from("apagado").is_err());
    }

    #[test]
    fn so_ativo_com_endereco_definitivo_aparece_na_busca() {
        assert!(aparece_na_busca(Situacao::Ativo, true, false));
        assert!(aparece_na_busca(Situacao::Ativo, false, true));
        assert!(!aparece_na_busca(Situacao::Ativo, false, false));
        assert!(!aparece_na_busca(Situacao::EmMontagem, true, true));
        assert!(!aparece_na_busca(Situacao::Suspenso, true, true));
    }

    #[test]
    fn perfil_recusa_campo_desconhecido() {
        let perfil = PerfilDoSite::from(motor_web::demonstracao::site());
        assert_eq!(
            perfil.para_site("https://x.example").origem,
            "https://x.example"
        );
    }
}
