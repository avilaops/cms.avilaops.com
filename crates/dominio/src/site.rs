//! O site como o CMS o guarda.

use std::fmt;

use motor_web::tipos::{Direitos, Midia, Organizacao, Site};
use serde::{Deserialize, Serialize};

/// Peso máximo recomendado de uma página, até o Dono mudar.
const ORCAMENTO_DE_PESO_PADRAO_KB: u32 = 500;

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
    /// A cor da marca, como `#225cf2`. Pinta links, botões e destaques. Sem
    /// ela, vale a cor do tema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cor_de_destaque: Option<String>,
}

/// O contraste mínimo de texto normal contra o fundo, pela WCAG.
const CONTRASTE_MINIMO: f64 = 4.5;

/// Por que uma cor de destaque não serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CorRecusada {
    #[error("Escreva a cor como #225cf2: cerquilha e seis dígitos de 0 a 9 ou de a até f.")]
    Formato,
    #[error(
        "Esta cor é clara demais: o texto dos links e dos botões ficaria difícil de ler. Escolha um tom mais escuro."
    )]
    Clara,
}

/// A luminância relativa de uma cor, de 0 (preto) a 1 (branco).
fn luminancia(cor: [u8; 3]) -> f64 {
    let canal = |valor: u8| {
        let v = f64::from(valor) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * canal(cor[0]) + 0.7152 * canal(cor[1]) + 0.0722 * canal(cor[2])
}

/// Confere a cor de destaque e a devolve em minúsculas. A cor é usada como
/// texto sobre fundo branco (links) e como fundo de texto branco (botões):
/// nos dois casos o contraste é o dela contra o branco.
pub fn conferir_cor(cor: &str) -> Result<String, CorRecusada> {
    let cor = cor.trim().to_ascii_lowercase();
    let digitos = cor.strip_prefix('#').ok_or(CorRecusada::Formato)?;
    if digitos.len() != 6 || !digitos.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CorRecusada::Formato);
    }
    let canal = |i: usize| u8::from_str_radix(&digitos[i..i + 2], 16).unwrap_or(0);
    let contraste = 1.05 / (luminancia([canal(0), canal(2), canal(4)]) + 0.05);
    if contraste < CONTRASTE_MINIMO {
        return Err(CorRecusada::Clara);
    }
    Ok(cor)
}

impl PerfilDoSite {
    /// O perfil de um site recém-criado: só o nome. O resto o Dono preenche,
    /// e o painel aponta o que falta.
    pub fn inicial(nome: &str) -> Self {
        Self {
            nome: nome.to_string(),
            descricao: String::new(),
            idioma: "pt-BR".to_string(),
            logo: Midia {
                id: String::new(),
                alt: String::new(),
                largura: 0,
                altura: 0,
                variantes: Vec::new(),
                legenda: None,
                credito: None,
                direitos: Direitos::default(),
            },
            organizacao: Organizacao::default(),
            diretrizes_ia: Vec::new(),
            orcamento_peso_kb: ORCAMENTO_DE_PESO_PADRAO_KB,
            cor_de_destaque: None,
        }
    }

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
            cor_de_destaque: None,
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
    fn cor_de_destaque_precisa_ser_legivel() {
        assert_eq!(conferir_cor(" #225CF2 "), Ok("#225cf2".to_string()));
        assert_eq!(conferir_cor("#9a3412"), Ok("#9a3412".to_string()));
        for torta in ["225cf2", "#225cf", "#225cfg", "azul", ""] {
            assert_eq!(conferir_cor(torta), Err(CorRecusada::Formato), "{torta}");
        }
        // Amarelo e azul claro não seguram texto branco nem leem sobre branco.
        for clara in ["#f4c544", "#7fb2ff", "#ffffff"] {
            assert_eq!(conferir_cor(clara), Err(CorRecusada::Clara), "{clara}");
        }
    }

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
