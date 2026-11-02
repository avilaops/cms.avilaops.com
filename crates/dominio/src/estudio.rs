//! Regras do estúdio: arquivo, página, nó, sistema de design e a biblioteca
//! de documentação. Sem banco e sem rede.
//!
//! A biblioteca `docs` reimplementa o inventário de componentes de
//! documentação. Os valores de cor vêm do tema neutro do Fumadocs, com a
//! licença e a revisão registradas em `crates/web/recursos/biblioteca-docs`.
//! O código daquele projeto não entra aqui.

use crate::{Ator, Papel};

/// Revisão do catálogo gravada no nó e na ativação da biblioteca.
pub const VERSAO_DO_CATALOGO_DOCS: &str = "2026-11-02";

pub const SLUG_DA_BIBLIOTECA_DOCS: &str = "docs";

pub const LIMITE_DE_ARQUIVOS: i64 = 200;
pub const LIMITE_DE_NOS: i64 = 300;
pub const PEDIDO_MAXIMO: usize = 4_000;
pub const NOME_MAXIMO: usize = 80;

pub const TIPO_FRAME: &str = "frame";
pub const TIPO_RETANGULO: &str = "retangulo";
pub const TIPO_TEXTO: &str = "texto";
pub const TIPO_COMPONENTE: &str = "componente";

pub const VARIANTE_CHAMADA: &str = "chamada";
pub const VARIANTE_CARTAO: &str = "cartao";
pub const VARIANTE_ABAS: &str = "abas";
pub const VARIANTE_NAVEGACAO: &str = "navegacao";
pub const VARIANTE_SUMARIO: &str = "sumario";
pub const VARIANTE_LATERAL: &str = "lateral";
pub const VARIANTE_BUSCA: &str = "busca";

const VARIANTES_DOCS: [&str; 7] = [
    VARIANTE_LATERAL,
    VARIANTE_SUMARIO,
    VARIANTE_NAVEGACAO,
    VARIANTE_CHAMADA,
    VARIANTE_CARTAO,
    VARIANTE_ABAS,
    VARIANTE_BUSCA,
];

/// Quem participa do site pode criar arquivo. Quem não participa nem chega
/// aqui: a tela já foi barrada.
pub fn pode_criar_arquivo(ator: &Ator) -> bool {
    let _ = ator;
    true
}

/// Autor mexe só no arquivo que criou. Editor e Dono mexem em todos.
pub fn pode_editar_arquivo(ator: &Ator, criado_por: &str) -> bool {
    match ator.papel {
        Papel::Dono | Papel::Editor => true,
        Papel::Autor => ator.conta == criado_por,
    }
}

/// Publicar a prévia é pôr um recorte para fora do painel. Mesma régua de
/// publicar conteúdo.
pub fn pode_publicar_previa(ator: &Ator) -> bool {
    matches!(ator.papel, Papel::Dono | Papel::Editor)
}

/// Ligar ou desligar a biblioteca vale para o site inteiro.
pub fn pode_ativar_biblioteca(ator: &Ator) -> bool {
    matches!(ator.papel, Papel::Dono | Papel::Editor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atalho {
    Entrada,
    Painel,
    Aplicativo,
    Biblioteca,
}

impl Atalho {
    /// `Some(None)` é atalho ausente. `None` é valor desconhecido.
    pub fn ler(valor: &str) -> Option<Option<Self>> {
        match valor {
            "" => Some(None),
            "entrada" => Some(Some(Self::Entrada)),
            "painel" => Some(Some(Self::Painel)),
            "aplicativo" => Some(Some(Self::Aplicativo)),
            "biblioteca" => Some(Some(Self::Biblioteca)),
            _ => None,
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Entrada => "Página de entrada",
            Self::Painel => "Painel",
            Self::Aplicativo => "Aplicativo",
            Self::Biblioteca => "Biblioteca de componentes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Medida {
    pub largura: i32,
    pub altura: i32,
}

pub fn medida_do_atalho(atalho: Option<Atalho>) -> Medida {
    match atalho {
        Some(Atalho::Aplicativo) => Medida {
            largura: 390,
            altura: 844,
        },
        Some(Atalho::Biblioteca) => Medida {
            largura: 1200,
            altura: 900,
        },
        Some(Atalho::Entrada | Atalho::Painel) | None => Medida {
            largura: 1280,
            altura: 800,
        },
    }
}

/// O nome do arquivo sai da primeira linha do pedido. Sem pedido, sai do
/// atalho. Nunca fica vazio e nunca passa de [`NOME_MAXIMO`].
pub fn nome_de_arquivo(pedido: &str, atalho: Option<Atalho>) -> String {
    let linha = pedido
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !linha.is_empty() {
        return linha.chars().take(NOME_MAXIMO).collect();
    }
    match atalho {
        Some(atalho) => atalho.rotulo().to_string(),
        None => "Arquivo sem título".to_string(),
    }
}

pub fn pedido_valido(pedido: &str) -> Option<String> {
    let pedido = pedido.trim();
    if pedido.chars().count() > PEDIDO_MAXIMO {
        None
    } else {
        Some(pedido.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonte {
    SemSerifa,
    Serifa,
    Condensada,
    Mono,
}

impl Fonte {
    pub fn slug(self) -> &'static str {
        match self {
            Self::SemSerifa => "sem-serifa",
            Self::Serifa => "serifa",
            Self::Condensada => "condensada",
            Self::Mono => "mono",
        }
    }

    pub fn ler(slug: &str) -> Option<Self> {
        match slug {
            "sem-serifa" => Some(Self::SemSerifa),
            "serifa" => Some(Self::Serifa),
            "condensada" => Some(Self::Condensada),
            "mono" => Some(Self::Mono),
            _ => None,
        }
    }
}

/// Cores em `#rrggbb` e fontes de uma lista fechada. O que sai daqui pode ir
/// num atributo `style` sem virar CSS injetado.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tokens {
    pub fundo: String,
    pub superficie: String,
    pub texto: String,
    pub suave: String,
    pub destaque: String,
    pub borda: String,
    pub fonte_titulo: String,
    pub fonte_corpo: String,
}

impl Tokens {
    pub fn validos(&self) -> bool {
        [
            &self.fundo,
            &self.superficie,
            &self.texto,
            &self.suave,
            &self.destaque,
            &self.borda,
        ]
        .into_iter()
        .all(|cor| cor_valida(cor))
            && Fonte::ler(&self.fonte_titulo).is_some()
            && Fonte::ler(&self.fonte_corpo).is_some()
    }
}

pub fn cor_valida(cor: &str) -> bool {
    let Some(hex) = cor.strip_prefix('#') else {
        return false;
    };
    hex.len() == 6
        && hex
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

struct Cores<'a> {
    fundo: &'a str,
    superficie: &'a str,
    texto: &'a str,
    suave: &'a str,
    destaque: &'a str,
    borda: &'a str,
}

fn tokens(cores: Cores<'_>, titulo: Fonte, corpo: Fonte) -> Tokens {
    Tokens {
        fundo: cores.fundo.to_string(),
        superficie: cores.superficie.to_string(),
        texto: cores.texto.to_string(),
        suave: cores.suave.to_string(),
        destaque: cores.destaque.to_string(),
        borda: cores.borda.to_string(),
        fonte_titulo: titulo.slug().to_string(),
        fonte_corpo: corpo.slug().to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sistema {
    pub slug: &'static str,
    pub nome: &'static str,
    pub tokens: Tokens,
}

pub fn sistemas() -> &'static [Sistema] {
    use std::sync::LazyLock;
    static SISTEMAS: LazyLock<Vec<Sistema>> = LazyLock::new(|| {
        vec![
            Sistema {
                slug: "padrao",
                nome: "Padrão",
                tokens: tokens(
                    Cores {
                        fundo: "#f6f5f2",
                        superficie: "#ffffff",
                        texto: "#1c1917",
                        suave: "#57534e",
                        destaque: "#1a4fa0",
                        borda: "#dedbd3",
                    },
                    Fonte::SemSerifa,
                    Fonte::SemSerifa,
                ),
            },
            Sistema {
                slug: "tinta",
                nome: "Tinta",
                tokens: tokens(
                    Cores {
                        fundo: "#fafafa",
                        superficie: "#ffffff",
                        texto: "#111111",
                        suave: "#404040",
                        destaque: "#e11d2e",
                        borda: "#111111",
                    },
                    Fonte::Condensada,
                    Fonte::SemSerifa,
                ),
            },
            Sistema {
                slug: "papel",
                nome: "Papel",
                tokens: tokens(
                    Cores {
                        fundo: "#fbfaf7",
                        superficie: "#fffdf8",
                        texto: "#1f2937",
                        suave: "#5b6470",
                        destaque: "#7c2d12",
                        borda: "#e5e0d8",
                    },
                    Fonte::Serifa,
                    Fonte::Serifa,
                ),
            },
            Sistema {
                slug: "noite",
                nome: "Noite",
                tokens: tokens(
                    Cores {
                        fundo: "#141210",
                        superficie: "#24211e",
                        texto: "#f5f5f4",
                        suave: "#a8a29e",
                        destaque: "#8fb4f0",
                        borda: "#3a3632",
                    },
                    Fonte::SemSerifa,
                    Fonte::SemSerifa,
                ),
            },
            Sistema {
                slug: "oficina",
                nome: "Oficina",
                tokens: tokens(
                    Cores {
                        fundo: "#fff7ed",
                        superficie: "#ffffff",
                        texto: "#1c1917",
                        suave: "#57534e",
                        destaque: "#c2410c",
                        borda: "#fed7aa",
                    },
                    Fonte::Condensada,
                    Fonte::SemSerifa,
                ),
            },
            Sistema {
                slug: "campo",
                nome: "Campo",
                tokens: tokens(
                    Cores {
                        fundo: "#f3f6ef",
                        superficie: "#ffffff",
                        texto: "#1a2e1a",
                        suave: "#3f6212",
                        destaque: "#3f6212",
                        borda: "#d9e2d1",
                    },
                    Fonte::SemSerifa,
                    Fonte::SemSerifa,
                ),
            },
            Sistema {
                slug: "costa",
                nome: "Costa",
                tokens: tokens(
                    Cores {
                        fundo: "#f3f7fb",
                        superficie: "#ffffff",
                        texto: "#0f172a",
                        suave: "#475569",
                        destaque: "#0369a1",
                        borda: "#dbe4ee",
                    },
                    Fonte::SemSerifa,
                    Fonte::SemSerifa,
                ),
            },
            Sistema {
                slug: "trama",
                nome: "Trama",
                tokens: tokens(
                    Cores {
                        fundo: "#faf5ff",
                        superficie: "#ffffff",
                        texto: "#1e1b4b",
                        suave: "#6b21a8",
                        destaque: "#7c3aed",
                        borda: "#e9d5ff",
                    },
                    Fonte::SemSerifa,
                    Fonte::SemSerifa,
                ),
            },
        ]
    });
    SISTEMAS.as_slice()
}

pub fn sistema(slug: &str) -> Option<&'static Sistema> {
    sistemas().iter().find(|sistema| sistema.slug == slug)
}

/// Sistema gravado que o código não reconhece mais: a tela usa o Padrão e
/// não regrava sozinha.
pub fn sistema_ou_padrao(slug: &str) -> &'static Sistema {
    match sistema(slug) {
        Some(sistema) => sistema,
        // `padrao` é o primeiro item montado em `sistemas`.
        None => &sistemas()[0],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct PropsDeComponente {
    #[serde(default)]
    pub tom: String,
    #[serde(default)]
    pub titulo: String,
    #[serde(default)]
    pub texto: String,
    #[serde(default)]
    pub descricao: String,
    #[serde(default)]
    pub aba_a: String,
    #[serde(default)]
    pub aba_b: String,
    #[serde(default)]
    pub painel_a: String,
    #[serde(default)]
    pub painel_b: String,
    #[serde(default)]
    pub anterior: String,
    #[serde(default)]
    pub proximo: String,
    #[serde(default)]
    pub itens: Vec<String>,
    #[serde(default)]
    pub rotulo: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecaDocs {
    pub variante: &'static str,
    pub nome: &'static str,
    pub medida: Medida,
    pub props: PropsDeComponente,
}

pub fn pecas_docs() -> &'static [PecaDocs] {
    use std::sync::LazyLock;
    static PECAS: LazyLock<Vec<PecaDocs>> = LazyLock::new(|| {
        vec![
            PecaDocs {
                variante: VARIANTE_LATERAL,
                nome: "Barra lateral",
                medida: Medida {
                    largura: 240,
                    altura: 280,
                },
                props: PropsDeComponente {
                    itens: vec!["Início".into(), "Guia".into(), "Referência".into()],
                    ..PropsDeComponente::default()
                },
            },
            PecaDocs {
                variante: VARIANTE_SUMARIO,
                nome: "Sumário",
                medida: Medida {
                    largura: 240,
                    altura: 200,
                },
                props: PropsDeComponente {
                    titulo: "Nesta página".into(),
                    itens: vec!["Visão geral".into(), "Como usar".into(), "Limites".into()],
                    ..PropsDeComponente::default()
                },
            },
            PecaDocs {
                variante: VARIANTE_NAVEGACAO,
                nome: "Navegação anterior e próxima",
                medida: Medida {
                    largura: 480,
                    altura: 72,
                },
                props: PropsDeComponente {
                    anterior: "Introdução".into(),
                    proximo: "Próximo passo".into(),
                    ..PropsDeComponente::default()
                },
            },
            PecaDocs {
                variante: VARIANTE_CHAMADA,
                nome: "Chamada",
                medida: Medida {
                    largura: 440,
                    altura: 120,
                },
                props: PropsDeComponente {
                    tom: "info".into(),
                    titulo: "Nota".into(),
                    texto: "Um aviso curto para quem lê a página.".into(),
                    ..PropsDeComponente::default()
                },
            },
            PecaDocs {
                variante: VARIANTE_CARTAO,
                nome: "Cartão",
                medida: Medida {
                    largura: 320,
                    altura: 140,
                },
                props: PropsDeComponente {
                    titulo: "Comece por aqui".into(),
                    descricao: "Um cartão de documentação.".into(),
                    ..PropsDeComponente::default()
                },
            },
            PecaDocs {
                variante: VARIANTE_ABAS,
                nome: "Abas",
                medida: Medida {
                    largura: 480,
                    altura: 180,
                },
                props: PropsDeComponente {
                    aba_a: "Visão".into(),
                    aba_b: "Detalhe".into(),
                    painel_a: "O que esta tela mostra.".into(),
                    painel_b: "O que fica para a fase seguinte.".into(),
                    ..PropsDeComponente::default()
                },
            },
            PecaDocs {
                variante: VARIANTE_BUSCA,
                nome: "Busca",
                medida: Medida {
                    largura: 360,
                    altura: 88,
                },
                props: PropsDeComponente {
                    rotulo: "Buscar na documentação".into(),
                    texto: "Esta versão não indexa páginas.".into(),
                    ..PropsDeComponente::default()
                },
            },
        ]
    });
    PECAS.as_slice()
}

pub fn peca_docs(variante: &str) -> Option<&'static PecaDocs> {
    pecas_docs().iter().find(|peca| peca.variante == variante)
}

pub fn rotulo_do_tipo(tipo: &str) -> &'static str {
    match tipo {
        TIPO_FRAME => "Frame",
        TIPO_RETANGULO => "Retângulo",
        TIPO_TEXTO => "Texto",
        TIPO_COMPONENTE => "Componente",
        _ => "Nó",
    }
}

pub fn variavel_do_tom(tom: &str) -> Option<&'static str> {
    Some(match tom {
        "info" => "--docs-info",
        "aviso" => "--docs-aviso",
        "erro" => "--docs-erro",
        "sucesso" => "--docs-sucesso",
        "ideia" => "--docs-ideia",
        _ => return None,
    })
}

fn texto_curto(valor: &str, maximo: usize) -> bool {
    valor.chars().count() <= maximo
}

pub fn props_validas(variante: &str, props: &PropsDeComponente) -> bool {
    if !VARIANTES_DOCS.contains(&variante) {
        return false;
    }
    let campos = [
        props.titulo.as_str(),
        props.texto.as_str(),
        props.descricao.as_str(),
        props.aba_a.as_str(),
        props.aba_b.as_str(),
        props.painel_a.as_str(),
        props.painel_b.as_str(),
        props.anterior.as_str(),
        props.proximo.as_str(),
        props.rotulo.as_str(),
    ];
    if campos.iter().any(|campo| !texto_curto(campo, 240)) || props.itens.len() > 12 {
        return false;
    }
    if props.itens.iter().any(|item| !texto_curto(item, 80)) {
        return false;
    }
    if variante == VARIANTE_CHAMADA {
        return variavel_do_tom(&props.tom).is_some();
    }
    true
}

pub fn medida_do_no(tipo: &str, variante: &str, pagina: Medida) -> Option<Medida> {
    let base = match tipo {
        TIPO_FRAME => Medida {
            largura: 320,
            altura: 200,
        },
        TIPO_RETANGULO => Medida {
            largura: 180,
            altura: 120,
        },
        TIPO_TEXTO => Medida {
            largura: 420,
            altura: 96,
        },
        TIPO_COMPONENTE => peca_docs(variante)?.medida,
        _ => return None,
    };
    let largura = base.largura.min(pagina.largura - 16).max(32);
    let altura = base.altura.min(pagina.altura - 16).max(32);
    (largura <= pagina.largura && altura <= pagina.altura).then_some(Medida { largura, altura })
}

pub fn posicao_inicial(indice: i32, no: Medida, pagina: Medida) -> Option<(i32, i32)> {
    if indice < 0 || no.largura > pagina.largura || no.altura > pagina.altura {
        return None;
    }
    let passo = (indice % 8) * 16;
    Some((
        (24 + passo).min(pagina.largura - no.largura),
        (24 + passo).min(pagina.altura - no.altura),
    ))
}

pub fn encaixar(x: i32, y: i32, no: Medida, pagina: Medida) -> Option<(i32, i32)> {
    if x < 0 || y < 0 || no.largura < 8 || no.altura < 8 {
        return None;
    }
    let direita = x.checked_add(no.largura)?;
    let baixo = y.checked_add(no.altura)?;
    (direita <= pagina.largura && baixo <= pagina.altura).then_some((x, y))
}

/// Encosta o nó na página quando o ponto cai na borda. Recusa nó maior que a página.
pub fn prender(x: i32, y: i32, no: Medida, pagina: Medida) -> Option<(i32, i32)> {
    if no.largura > pagina.largura || no.altura > pagina.altura || no.largura < 8 || no.altura < 8 {
        return None;
    }
    Some((
        x.clamp(0, pagina.largura - no.largura),
        y.clamp(0, pagina.altura - no.altura),
    ))
}

/// Converte um ponto da tela para o quadro.
///
/// `origem` é o canto superior esquerdo do viewport. `pan` é o deslocamento
/// em pixels de tela. `zoom` é a escala do quadro (1 = 100%).
pub fn ponto_no_quadro(
    cliente_x: f64,
    cliente_y: f64,
    origem_x: f64,
    origem_y: f64,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
) -> Option<(i32, i32)> {
    if !zoom.is_finite() || !(0.25..=4.0).contains(&zoom) {
        return None;
    }
    let numeros = [cliente_x, cliente_y, origem_x, origem_y, pan_x, pan_y];
    if numeros.iter().any(|numero| !numero.is_finite()) {
        return None;
    }
    let x = (cliente_x - origem_x - pan_x) / zoom;
    let y = (cliente_y - origem_y - pan_y) / zoom;
    Some((x.round() as i32, y.round() as i32))
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DefinicaoDeArquivo {
    pub nome: String,
    pub pedido: String,
    pub sistema: String,
    pub tokens: Tokens,
    pub paginas: Vec<PaginaDaDefinicao>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PaginaDaDefinicao {
    pub nome: String,
    pub largura: i32,
    pub altura: i32,
    pub nos: Vec<NoDaDefinicao>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NoDaDefinicao {
    pub tipo: String,
    pub x: i32,
    pub y: i32,
    pub largura: i32,
    pub altura: i32,
    pub texto: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preenchimento: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biblioteca: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variante: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogo_versao: Option<String>,
    pub props: PropsDeComponente,
}

pub fn definicao_valida(definicao: &DefinicaoDeArquivo) -> bool {
    if !definicao.tokens.validos() || definicao.paginas.is_empty() {
        return false;
    }
    definicao.paginas.iter().all(|pagina| {
        let medida = Medida {
            largura: pagina.largura,
            altura: pagina.altura,
        };
        pagina.nos.iter().all(|no| {
            let preenchimento_ok = no.preenchimento.as_deref().is_none_or(cor_valida);
            let componente_ok = if no.tipo == TIPO_COMPONENTE {
                no.biblioteca.as_deref() == Some(SLUG_DA_BIBLIOTECA_DOCS)
                    && no
                        .variante
                        .as_deref()
                        .is_some_and(|variante| props_validas(variante, &no.props))
            } else {
                no.biblioteca.is_none() && no.variante.is_none()
            };
            preenchimento_ok
                && componente_ok
                && encaixar(
                    no.x,
                    no.y,
                    Medida {
                        largura: no.largura,
                        altura: no.altura,
                    },
                    medida,
                )
                .is_some()
        })
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn autor(conta: &str) -> Ator {
        Ator::do_site(conta, Papel::Autor)
    }

    #[test]
    fn autor_edita_o_proprio_arquivo_e_nao_publica() {
        let autor = autor("conta-a");
        assert!(pode_editar_arquivo(&autor, "conta-a"));
        assert!(!pode_editar_arquivo(&autor, "conta-b"));
        assert!(!pode_publicar_previa(&autor));
        assert!(!pode_ativar_biblioteca(&autor));
        let editor = Ator::do_site("conta-b", Papel::Editor);
        assert!(pode_editar_arquivo(&editor, "conta-a"));
        assert!(pode_publicar_previa(&editor));
        assert!(pode_publicar_previa(&Ator::da_equipe("equipe")));
    }

    #[test]
    fn nome_sai_do_pedido_e_o_atalho_cobre_o_vazio() {
        assert_eq!(
            nome_de_arquivo("  Habit tracker\nsegunda linha", None),
            "Habit tracker"
        );
        assert_eq!(
            nome_de_arquivo("   ", Some(Atalho::Aplicativo)),
            "Aplicativo"
        );
        assert_eq!(nome_de_arquivo("", None), "Arquivo sem título");
        assert!(pedido_valido(&"a".repeat(PEDIDO_MAXIMO)).is_some());
        assert!(pedido_valido(&"a".repeat(PEDIDO_MAXIMO + 1)).is_none());
    }

    #[test]
    fn sistemas_tem_cor_e_fonte_fechadas() {
        assert_eq!(sistemas().len(), 8);
        assert!(sistemas().iter().all(|sistema| sistema.tokens.validos()));
        assert_eq!(sistema("tinta").expect("tinta").tokens.destaque, "#e11d2e");
        assert!(sistema("openai").is_none());
        assert!(cor_valida("#e11d2e"));
        assert!(!cor_valida("#E11D2E"));
        assert!(!cor_valida("red"));
    }

    #[test]
    fn catalogo_docs_tem_as_sete_pecas_e_rejeita_o_resto() {
        let variantes: Vec<_> = pecas_docs().iter().map(|peca| peca.variante).collect();
        assert_eq!(
            variantes,
            vec![
                "lateral",
                "sumario",
                "navegacao",
                "chamada",
                "cartao",
                "abas",
                "busca",
            ]
        );
        let chamada = peca_docs("chamada").expect("chamada");
        assert!(props_validas(chamada.variante, &chamada.props));
        let mut quebrada = chamada.props.clone();
        quebrada.tom = "marca-alheia".into();
        assert!(!props_validas("chamada", &quebrada));
        assert!(peca_docs("react").is_none());
    }

    #[test]
    fn ponto_no_quadro_desfaz_pan_e_zoom() {
        assert_eq!(
            ponto_no_quadro(150.0, 80.0, 10.0, 20.0, 40.0, 10.0, 2.0),
            Some((50, 25))
        );
        assert_eq!(ponto_no_quadro(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.1), None);
        assert_eq!(
            ponto_no_quadro(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            None
        );
    }

    #[test]
    fn no_tem_que_caber_na_pagina() {
        let pagina = Medida {
            largura: 390,
            altura: 844,
        };
        let texto = medida_do_no(TIPO_TEXTO, "", pagina).expect("texto");
        assert!(texto.largura <= 390 - 16);
        assert!(encaixar(0, 0, texto, pagina).is_some());
        assert!(encaixar(-1, 0, texto, pagina).is_none());
        assert!(
            encaixar(380, 0, texto, pagina).is_none(),
            "o texto não pode sair pela direita"
        );
    }

    #[test]
    fn atalho_desconhecido_nao_vira_medida() {
        assert_eq!(Atalho::ler(""), Some(None));
        assert_eq!(Atalho::ler("aplicativo"), Some(Some(Atalho::Aplicativo)));
        assert_eq!(Atalho::ler("landing"), None);
        assert_eq!(medida_do_atalho(Some(Atalho::Aplicativo)).largura, 390);
    }
}
