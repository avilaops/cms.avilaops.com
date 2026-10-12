//! O site como o CMS o guarda.

use std::fmt;

use motor_web::tipos::{Direitos, Midia, Organizacao, Site};
use serde::{Deserialize, Serialize};

/// Peso máximo recomendado de uma página, até o Dono mudar.
const ORCAMENTO_DE_PESO_PADRAO_KB: u32 = 500;

/// A página de cada site que diz em que termos as imagens dele podem ser
/// usadas e como pedir autorização. É para onde aponta a licença de toda
/// imagem de autoria própria.
pub const CAMINHO_DA_LICENCA_DE_IMAGENS: &str = "/licenca-de-imagens";

/// A aparência do site: fontes, cores de base e formas. A lista é fechada e
/// os temas moram no CMS, com as fontes servidas por ele; o site escolhe um
/// e, se quiser, troca a cor da marca.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tema {
    /// O de sempre: fonte do próprio aparelho, sem arquivo de fonte.
    #[default]
    Classico,
    /// Títulos condensados em caixa alta e cantos retos.
    Industrial,
    /// Títulos com serifa sobre fundo de papel.
    Editorial,
    /// Sem serifa geométrica, títulos pesados e cantos redondos.
    Moderno,
}

impl Tema {
    pub const TODOS: [Tema; 4] = [
        Tema::Classico,
        Tema::Industrial,
        Tema::Editorial,
        Tema::Moderno,
    ];

    /// O nome que vai gravado e que o conector usa.
    pub fn slug(self) -> &'static str {
        match self {
            Tema::Classico => "classico",
            Tema::Industrial => "industrial",
            Tema::Editorial => "editorial",
            Tema::Moderno => "moderno",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Tema::Classico => "Clássico",
            Tema::Industrial => "Industrial",
            Tema::Editorial => "Editorial",
            Tema::Moderno => "Moderno",
        }
    }

    /// Para quem escolhe: como o tema é, em uma frase.
    pub fn descricao(self) -> &'static str {
        match self {
            Tema::Classico => "fonte do aparelho, leve e neutro",
            Tema::Industrial => "títulos condensados em caixa alta, cantos retos",
            Tema::Editorial => "títulos com serifa, fundo de papel",
            Tema::Moderno => "títulos pesados, cantos redondos",
        }
    }

    /// O tema pelo nome gravado. Vazio é o clássico.
    pub fn do_slug(slug: &str) -> Option<Self> {
        let slug = slug.trim();
        if slug.is_empty() {
            return Some(Tema::default());
        }
        Self::TODOS.into_iter().find(|tema| tema.slug() == slug)
    }

    fn eh_o_padrao(&self) -> bool {
        *self == Tema::default()
    }
}

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
    /// O tema do site. O clássico não vai gravado.
    #[serde(default, skip_serializing_if = "Tema::eh_o_padrao")]
    pub tema: Tema,
    /// As páginas do menu do topo, na ordem, pelos caminhos (`/servicos`,
    /// `/blog`). O que não está na lista vai para o rodapé. Vazia, todas as
    /// páginas ficam no topo, em ordem de endereço.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu: Vec<String>,
    /// A tag do Google do site, como `GTM-ABC1234` ou `G-ABC1234567`. Com
    /// ela, as páginas carregam o script do Google para medir visitas; sem
    /// ela, o site não carrega script nenhum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_do_google: Option<String>,
    /// Os termos de uso das imagens do site, um parágrafo por linha. É o
    /// texto da página de licença. Sem ele, vale o texto padrão: todos os
    /// direitos reservados.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licenca_de_imagens: Option<String>,
}

/// A tag do Google não tem o formato de um contêiner nem de uma propriedade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "A tag do Google começa com GTM- (Tag Manager) ou G- (Analytics), seguida de letras e números, como GTM-ABC1234."
)]
pub struct TagRecusada;

/// Confere a tag do Google e a devolve em maiúsculas. Ela vai dentro de um
/// script da página: só passa o formato exato, letras, números e o hífen.
pub fn conferir_tag_do_google(tag: &str) -> Result<String, TagRecusada> {
    let tag = tag.trim().to_ascii_uppercase();
    let codigo = tag
        .strip_prefix("GTM-")
        .or_else(|| tag.strip_prefix("G-"))
        .ok_or(TagRecusada)?;
    let valido = (4..=16).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if valido { Ok(tag) } else { Err(TagRecusada) }
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

/// O texto de um campo opcional, se tiver algo escrito.
fn preenchido(valor: &Option<String>) -> Option<&str> {
    valor
        .as_deref()
        .map(str::trim)
        .filter(|texto| !texto.is_empty())
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
            tema: Tema::default(),
            menu: Vec::new(),
            tag_do_google: None,
            licenca_de_imagens: None,
        }
    }

    /// Quem responde pelo site: a razão social, ou o nome do site sem ela.
    pub fn titular(&self) -> &str {
        preenchido(&self.organizacao.razao_social).unwrap_or(&self.nome)
    }

    /// O texto da página de licença de imagens, um parágrafo por item: os
    /// termos que o dono escreveu, ou o texto padrão, e depois como pedir
    /// autorização, com o contato do cadastro.
    pub fn termos_das_imagens(&self) -> Vec<String> {
        let titular = self.titular();
        let proprios: Vec<String> = self
            .licenca_de_imagens
            .iter()
            .flat_map(|texto| texto.lines())
            .map(str::trim)
            .filter(|linha| !linha.is_empty())
            .map(str::to_string)
            .collect();
        let mut termos = if proprios.is_empty() {
            vec![
                format!(
                    "As imagens deste site com crédito de {titular} pertencem a {titular}. Todos os direitos reservados."
                ),
                "Copiar, republicar ou usar essas imagens, inteiras ou em parte, depende de autorização por escrito."
                    .to_string(),
                "Imagem com crédito de outra pessoa ou empresa segue a licença de quem a fez."
                    .to_string(),
            ]
        } else {
            proprios
        };
        let organizacao = &self.organizacao;
        termos.push(
            match (
                preenchido(&organizacao.email),
                preenchido(&organizacao.telefone),
            ) {
                (Some(email), Some(telefone)) => format!(
                    "Para pedir autorização de uso, escreva para {email} ou ligue para {telefone}."
                ),
                (Some(email), None) => {
                    format!("Para pedir autorização de uso, escreva para {email}.")
                }
                (None, Some(telefone)) => {
                    format!("Para pedir autorização de uso, ligue para {telefone}.")
                }
                (None, None) => format!(
                    "Para pedir autorização de uso, fale com {titular} pelos canais deste site."
                ),
            },
        );
        termos
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
            tema: Tema::default(),
            menu: Vec::new(),
            tag_do_google: None,
            licenca_de_imagens: None,
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
    fn tag_do_google_so_passa_no_formato_exato() {
        assert_eq!(
            conferir_tag_do_google(" gtm-mtg7chvv "),
            Ok("GTM-MTG7CHVV".to_string())
        );
        assert_eq!(
            conferir_tag_do_google("G-T7DZ51MVZK"),
            Ok("G-T7DZ51MVZK".to_string())
        );
        for torta in [
            "",
            "GTM-",
            "UA-12345-1",
            "GTM-ABC'><script>",
            "G-AB",
            "GTM-ABC 123",
            "AW-123456789",
        ] {
            assert_eq!(conferir_tag_do_google(torta), Err(TagRecusada), "{torta}");
        }
    }

    #[test]
    fn licenca_de_imagens_tem_texto_padrao_e_o_contato_do_cadastro() {
        let mut perfil = PerfilDoSite::inicial("Padaria da Ana");
        let padrao = perfil.termos_das_imagens();
        assert!(padrao[0].contains("pertencem a Padaria da Ana. Todos os direitos reservados."));
        assert_eq!(
            padrao.last().map(String::as_str),
            Some("Para pedir autorização de uso, fale com Padaria da Ana pelos canais deste site.")
        );

        // Com razão social, é ela que assina; os termos do dono trocam o padrão.
        perfil.organizacao.razao_social = Some("Padaria da Ana Ltda.".to_string());
        perfil.organizacao.email = Some("contato@padaria.example".to_string());
        perfil.licenca_de_imagens =
            Some("Uso livre com crédito.\n\n  Sem uso comercial.  ".to_string());
        assert_eq!(perfil.titular(), "Padaria da Ana Ltda.");
        assert_eq!(
            perfil.termos_das_imagens(),
            [
                "Uso livre com crédito.",
                "Sem uso comercial.",
                "Para pedir autorização de uso, escreva para contato@padaria.example.",
            ]
        );
    }

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
