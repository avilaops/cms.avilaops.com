//! O editor de blocos, do lado que não toca em banco: ler o formulário,
//! mexer na lista de blocos e traduzir entre o que a pessoa digita e o
//! contrato do motor.
//!
//! Não existe HTML livre. O parágrafo aceita uma marcação mínima, que vira
//! `Trecho`: `**negrito**`, `_itálico_` e `[texto](endereço)`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use motor_web::tipos::{
    Autor, Bloco, Categoria, Conteudo, ItemTrilha, Midia, Pagina, Pergunta, Post, Seo, TipoPost,
    Trecho,
};

/// Os tipos de bloco, na ordem em que aparecem para escolher.
pub const TIPOS_DE_BLOCO: [(&str, &str); 9] = [
    ("paragrafo", "Parágrafo"),
    ("titulo", "Título"),
    ("lista", "Lista"),
    ("imagem", "Imagem"),
    ("citacao", "Citação"),
    ("perguntas", "Perguntas e respostas"),
    ("tabela", "Tabela"),
    ("video", "Vídeo"),
    ("chamada", "Chamada com botão"),
];

/// Um bloco como está no formulário: tudo texto, do jeito que foi digitado.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlocoDigitado {
    pub tipo: String,
    pub texto: String,
    pub nivel: u8,
    pub ordenada: bool,
    pub fonte: String,
    pub url: String,
    pub rotulo: String,
    /// O identificador da imagem na biblioteca.
    pub midia: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acao {
    /// Salvar o rascunho, inclusive depois de mexer na lista de blocos.
    Salvar,
    EnviarParaRevisao,
    Devolver,
    Publicar,
    Despublicar,
    /// Marcar a publicação para a data do campo `agendar_para`.
    Agendar,
    CancelarAgendamento,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Formulario {
    pub eh_post: bool,
    pub acao: String,
    pub titulo: String,
    pub slug: String,
    pub seo_titulo: String,
    pub seo_descricao: String,
    pub indexar: bool,
    pub capa: String,
    pub tipo_do_post: String,
    pub resumo: String,
    pub autor: String,
    pub categoria: String,
    pub tags: String,
    pub novo_tipo: String,
    /// Data e hora de Brasília, como o campo `datetime-local` manda.
    pub agendar_para: String,
    pub blocos: Vec<BlocoDigitado>,
}

/// O limite de blocos de um documento. Mais do que isso é formulário forjado.
const MAXIMO_DE_BLOCOS: usize = 200;

impl Formulario {
    /// Um documento novo, vazio.
    pub fn novo(eh_post: bool) -> Self {
        Self {
            eh_post,
            indexar: true,
            tipo_do_post: "artigo".to_string(),
            ..Self::default()
        }
    }

    /// Lê os pares do formulário enviado.
    pub fn ler(pares: Vec<(String, String)>) -> Self {
        let campos: HashMap<String, String> = pares.into_iter().collect();
        let campo = |nome: &str| campos.get(nome).cloned().unwrap_or_default();
        let total = campo("n")
            .parse::<usize>()
            .unwrap_or(0)
            .min(MAXIMO_DE_BLOCOS);
        let blocos = (0..total)
            .map(|i| {
                let do_bloco = |nome: &str| campo(&format!("b{i}_{nome}"));
                BlocoDigitado {
                    tipo: do_bloco("tipo"),
                    texto: do_bloco("texto"),
                    nivel: do_bloco("nivel").parse().unwrap_or(2),
                    ordenada: do_bloco("ordenada") == "1",
                    fonte: do_bloco("fonte"),
                    url: do_bloco("url"),
                    rotulo: do_bloco("rotulo"),
                    midia: do_bloco("midia"),
                }
            })
            .collect();
        Self {
            eh_post: campo("especie") == "post",
            acao: campo("acao"),
            titulo: campo("titulo"),
            slug: campo("slug").trim().to_lowercase(),
            seo_titulo: campo("seo_titulo"),
            seo_descricao: campo("seo_descricao"),
            indexar: campo("indexar") == "1",
            capa: campo("capa"),
            tipo_do_post: campo("tipo_do_post"),
            resumo: campo("resumo"),
            autor: campo("autor"),
            categoria: campo("categoria"),
            tags: campo("tags"),
            novo_tipo: campo("novo_tipo"),
            agendar_para: campo("agendar_para"),
            blocos,
        }
    }

    /// Aplica o botão apertado. Mexer na lista de blocos (acrescentar, subir,
    /// descer, remover) muda o formulário e salva o rascunho.
    pub fn aplicar_acao(&mut self) -> Acao {
        let (verbo, indice) = match self.acao.split_once(':') {
            Some((verbo, indice)) => (verbo, indice.parse::<usize>().ok()),
            None => (self.acao.as_str(), None),
        };
        match (verbo, indice) {
            ("adicionar", _) => {
                if TIPOS_DE_BLOCO
                    .iter()
                    .any(|(tipo, _)| *tipo == self.novo_tipo)
                    && self.blocos.len() < MAXIMO_DE_BLOCOS
                {
                    self.blocos.push(BlocoDigitado {
                        tipo: self.novo_tipo.clone(),
                        nivel: 2,
                        ..BlocoDigitado::default()
                    });
                }
                Acao::Salvar
            }
            ("subir", Some(i)) if i > 0 && i < self.blocos.len() => {
                self.blocos.swap(i, i - 1);
                Acao::Salvar
            }
            ("descer", Some(i)) if i + 1 < self.blocos.len() => {
                self.blocos.swap(i, i + 1);
                Acao::Salvar
            }
            ("remover", Some(i)) if i < self.blocos.len() => {
                self.blocos.remove(i);
                Acao::Salvar
            }
            ("revisar", _) => Acao::EnviarParaRevisao,
            ("devolver", _) => Acao::Devolver,
            ("publicar", _) => Acao::Publicar,
            ("despublicar", _) => Acao::Despublicar,
            ("agendar", _) => Acao::Agendar,
            ("desagendar", _) => Acao::CancelarAgendamento,
            _ => Acao::Salvar,
        }
    }

    /// O formulário de um conteúdo já gravado.
    pub fn do_conteudo(conteudo: &Conteudo) -> Self {
        let comum =
            |titulo: &str, slug: &str, seo: &Seo, capa: Option<&Midia>, corpo: &[Bloco]| Self {
                titulo: titulo.to_string(),
                slug: slug.to_string(),
                seo_titulo: seo.titulo.clone(),
                seo_descricao: seo.descricao.clone(),
                indexar: seo.indexar,
                capa: capa.map(|capa| capa.id.clone()).unwrap_or_default(),
                tipo_do_post: "artigo".to_string(),
                blocos: corpo.iter().map(digitar).collect(),
                ..Self::default()
            };
        match conteudo {
            Conteudo::Pagina(pagina) => comum(
                &pagina.titulo,
                &pagina.slug,
                &pagina.seo,
                pagina.capa.as_ref(),
                &pagina.corpo,
            ),
            Conteudo::Post(post) => Self {
                eh_post: true,
                tipo_do_post: match post.tipo {
                    TipoPost::Artigo => "artigo",
                    TipoPost::GuiaTecnico => "guia-tecnico",
                }
                .to_string(),
                resumo: post.resumo.clone(),
                autor: post.autor.slug.clone(),
                categoria: post.categoria.slug.clone(),
                tags: post.tags.join(", "),
                ..comum(
                    &post.titulo,
                    &post.slug,
                    &post.seo,
                    Some(&post.capa),
                    &post.corpo,
                )
            },
            // O CMS não guarda produto: não há o que editar.
            Conteudo::Produto(_) => Self::default(),
        }
    }

    /// Os identificadores das imagens escolhidas: a capa e as dos blocos.
    pub fn midias_escolhidas(&self) -> Vec<&str> {
        std::iter::once(self.capa.as_str())
            .chain(self.blocos.iter().map(|bloco| bloco.midia.as_str()))
            .filter(|id| !id.is_empty())
            .collect()
    }

    /// Monta o conteúdo no contrato do motor. As datas são provisórias: quem
    /// as grava de verdade é a publicação.
    pub fn para_conteudo(&self, resolvido: &Resolvido, agora: DateTime<Utc>) -> Conteudo {
        let titulo = self.titulo.trim().to_string();
        let corpo: Vec<Bloco> = self
            .blocos
            .iter()
            .map(|bloco| montar(bloco, resolvido))
            .collect();
        let seo = Seo {
            titulo: self.seo_titulo.trim().to_string(),
            descricao: self.seo_descricao.trim().to_string(),
            indexar: self.indexar,
            imagem_social: None,
        };
        let inicio = ItemTrilha {
            nome: "Início".to_string(),
            url: "/".to_string(),
        };
        let capa = resolvido.midias.get(&self.capa).cloned();
        if self.eh_post {
            Conteudo::Post(Post {
                tipo: if self.tipo_do_post == "guia-tecnico" {
                    TipoPost::GuiaTecnico
                } else {
                    TipoPost::Artigo
                },
                trilha: vec![
                    inicio,
                    ItemTrilha {
                        nome: "Blog".to_string(),
                        url: "/blog".to_string(),
                    },
                    ItemTrilha {
                        nome: titulo.clone(),
                        url: format!("/blog/{}", self.slug),
                    },
                ],
                slug: self.slug.clone(),
                titulo,
                resumo: self.resumo.trim().to_string(),
                capa: capa.unwrap_or_else(cms_dados::midia_vazia),
                corpo,
                autor: resolvido.autor.clone().unwrap_or_else(|| Autor {
                    slug: String::new(),
                    nome: String::new(),
                    cargo: String::new(),
                    bio: String::new(),
                    foto: cms_dados::midia_vazia(),
                    perfis: Vec::new(),
                    credenciais: Vec::new(),
                }),
                categoria: resolvido.categoria.clone().unwrap_or_else(|| Categoria {
                    slug: String::new(),
                    nome: String::new(),
                }),
                tags: self
                    .tags
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_string)
                    .collect(),
                publicado_em: agora,
                atualizado_em: agora,
                seo,
            })
        } else {
            Conteudo::Pagina(Pagina {
                // A página inicial não tem trilha: ela é o começo.
                trilha: if self.slug.is_empty() {
                    Vec::new()
                } else {
                    vec![
                        inicio,
                        ItemTrilha {
                            nome: titulo.clone(),
                            url: format!("/{}", self.slug),
                        },
                    ]
                },
                slug: self.slug.clone(),
                titulo,
                corpo,
                capa,
                publicado_em: agora,
                atualizado_em: agora,
                seo,
            })
        }
    }
}

/// O que o formulário cita pelo identificador e o banco resolveu.
#[derive(Debug, Clone, Default)]
pub struct Resolvido {
    pub midias: HashMap<String, Midia>,
    pub autor: Option<Autor>,
    pub categoria: Option<Categoria>,
}

fn linhas(texto: &str) -> Vec<String> {
    texto
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .map(str::to_string)
        .collect()
}

fn celulas(linha: &str) -> Vec<String> {
    linha
        .split('|')
        .map(|celula| celula.trim().to_string())
        .collect()
}

/// Lê a data do campo de agendamento, que vem no horário de Brasília (três
/// horas atrás de Greenwich), e a devolve em UTC.
pub fn ler_agendamento(digitado: &str) -> Option<DateTime<Utc>> {
    let local = chrono::NaiveDateTime::parse_from_str(digitado.trim(), "%Y-%m-%dT%H:%M").ok()?;
    Some((local + chrono::Duration::hours(3)).and_utc())
}

/// Do que foi digitado para o bloco do motor.
fn montar(bloco: &BlocoDigitado, resolvido: &Resolvido) -> Bloco {
    let texto = bloco.texto.trim();
    match bloco.tipo.as_str() {
        "titulo" => Bloco::Titulo {
            nivel: bloco.nivel,
            texto: texto.to_string(),
        },
        "lista" => Bloco::Lista {
            ordenada: bloco.ordenada,
            itens: linhas(texto),
        },
        "imagem" => Bloco::Imagem {
            midia: resolvido
                .midias
                .get(&bloco.midia)
                .cloned()
                .unwrap_or_else(cms_dados::midia_vazia),
        },
        "citacao" => Bloco::Citacao {
            texto: texto.to_string(),
            fonte: Some(bloco.fonte.trim().to_string()).filter(|fonte| !fonte.is_empty()),
        },
        "perguntas" => Bloco::Perguntas {
            itens: texto
                .split("\n\n")
                .filter_map(|grupo| {
                    let mut partes = linhas(grupo).into_iter();
                    let pergunta = partes.next()?;
                    Some(Pergunta {
                        pergunta,
                        resposta: partes.collect::<Vec<_>>().join(" "),
                    })
                })
                .collect(),
        },
        "tabela" => {
            let mut todas = linhas(texto).into_iter();
            Bloco::Tabela {
                cabecalho: todas
                    .next()
                    .map(|linha| celulas(&linha))
                    .unwrap_or_default(),
                linhas: todas.map(|linha| celulas(&linha)).collect(),
            }
        }
        "video" => Bloco::Video {
            url: bloco.url.trim().to_string(),
            titulo: texto.to_string(),
        },
        "chamada" => Bloco::Chamada {
            texto: texto.to_string(),
            rotulo: bloco.rotulo.trim().to_string(),
            url: bloco.url.trim().to_string(),
        },
        _ => Bloco::Paragrafo {
            trechos: interpretar_trechos(texto),
        },
    }
}

/// Do bloco do motor para o que aparece no formulário.
fn digitar(bloco: &Bloco) -> BlocoDigitado {
    let base = |tipo: &str, texto: String| BlocoDigitado {
        tipo: tipo.to_string(),
        texto,
        nivel: 2,
        ..BlocoDigitado::default()
    };
    match bloco {
        Bloco::Paragrafo { trechos } => base("paragrafo", escrever_trechos(trechos)),
        Bloco::Titulo { nivel, texto } => BlocoDigitado {
            nivel: *nivel,
            ..base("titulo", texto.clone())
        },
        Bloco::Lista { ordenada, itens } => BlocoDigitado {
            ordenada: *ordenada,
            ..base("lista", itens.join("\n"))
        },
        Bloco::Imagem { midia } => BlocoDigitado {
            midia: midia.id.clone(),
            ..base("imagem", String::new())
        },
        Bloco::Citacao { texto, fonte } => BlocoDigitado {
            fonte: fonte.clone().unwrap_or_default(),
            ..base("citacao", texto.clone())
        },
        Bloco::Perguntas { itens } => base(
            "perguntas",
            itens
                .iter()
                .map(|item| format!("{}\n{}", item.pergunta, item.resposta))
                .collect::<Vec<_>>()
                .join("\n\n"),
        ),
        Bloco::Tabela { cabecalho, linhas } => base(
            "tabela",
            std::iter::once(cabecalho)
                .chain(linhas.iter())
                .map(|linha| linha.join(" | "))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Bloco::Video { url, titulo } => BlocoDigitado {
            url: url.clone(),
            ..base("video", titulo.clone())
        },
        Bloco::Chamada { texto, rotulo, url } => BlocoDigitado {
            rotulo: rotulo.clone(),
            url: url.clone(),
            ..base("chamada", texto.clone())
        },
    }
}

fn trecho(texto: &str, negrito: bool, italico: bool, link: Option<&str>) -> Trecho {
    Trecho {
        texto: texto.to_string(),
        negrito,
        italico,
        link: link.map(str::to_string),
    }
}

/// Lê a marcação mínima do parágrafo. O que não fecha fica como texto.
pub fn interpretar_trechos(texto: &str) -> Vec<Trecho> {
    let mut saida = Vec::new();
    interpretar(texto, false, false, &mut saida);
    saida
}

fn interpretar(texto: &str, negrito: bool, italico: bool, saida: &mut Vec<Trecho>) {
    let mut simples = String::new();
    let mut resto = texto;
    let despejar = |simples: &mut String, saida: &mut Vec<Trecho>| {
        if !simples.is_empty() {
            saida.push(trecho(simples, negrito, italico, None));
            simples.clear();
        }
    };
    while let Some(primeiro) = resto.chars().next() {
        // `**negrito**`
        if let Some(depois) = resto.strip_prefix("**") {
            if let Some(fim) = depois.find("**").filter(|fim| *fim > 0) {
                despejar(&mut simples, saida);
                interpretar(&depois[..fim], true, italico, saida);
                resto = &depois[fim + 2..];
                continue;
            }
        }
        // `_itálico_`, só em volta de palavra: `nome_de_arquivo` fica como está.
        let depois_de_letra = simples.chars().last().is_some_and(char::is_alphanumeric);
        if primeiro == '_' && !depois_de_letra {
            let depois = &resto[1..];
            let fecha = depois.find('_').filter(|fim| {
                *fim > 0
                    && !depois[fim + 1..]
                        .chars()
                        .next()
                        .is_some_and(char::is_alphanumeric)
            });
            if let Some(fim) = fecha {
                despejar(&mut simples, saida);
                interpretar(&depois[..fim], negrito, true, saida);
                resto = &depois[fim + 1..];
                continue;
            }
        }
        // `[texto](endereço)`
        if primeiro == '[' {
            let ligacao = resto[1..].split_once("](").and_then(|(rotulo, depois)| {
                let (endereco, sobra) = depois.split_once(')')?;
                let valido = !rotulo.is_empty() && !rotulo.contains('[') && !endereco.contains(' ');
                valido.then_some((rotulo, endereco, sobra))
            });
            if let Some((rotulo, endereco, sobra)) = ligacao {
                despejar(&mut simples, saida);
                saida.push(trecho(rotulo, negrito, italico, Some(endereco.trim())));
                resto = sobra;
                continue;
            }
        }
        simples.push(primeiro);
        resto = &resto[primeiro.len_utf8()..];
    }
    despejar(&mut simples, saida);
}

/// Escreve os trechos de volta na marcação que a pessoa edita.
pub fn escrever_trechos(trechos: &[Trecho]) -> String {
    trechos
        .iter()
        .map(|trecho| {
            let mut texto = match &trecho.link {
                Some(link) => format!("[{}]({link})", trecho.texto),
                None => trecho.texto.clone(),
            };
            if trecho.italico {
                texto = format!("_{texto}_");
            }
            if trecho.negrito {
                texto = format!("**{texto}**");
            }
            texto
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pares(lista: &[(&str, &str)]) -> Vec<(String, String)> {
        lista
            .iter()
            .map(|(nome, valor)| (nome.to_string(), valor.to_string()))
            .collect()
    }

    #[test]
    fn paragrafo_le_negrito_italico_e_link() {
        let trechos = interpretar_trechos(
            "Use **madeira seca**, _nunca_ verde. Veja [o guia](/blog/guia) e **_as duas_**.",
        );
        assert_eq!(
            trechos,
            vec![
                trecho("Use ", false, false, None),
                trecho("madeira seca", true, false, None),
                trecho(", ", false, false, None),
                trecho("nunca", false, true, None),
                trecho(" verde. Veja ", false, false, None),
                trecho("o guia", false, false, Some("/blog/guia")),
                trecho(" e ", false, false, None),
                trecho("as duas", true, true, None),
                trecho(".", false, false, None),
            ]
        );
    }

    #[test]
    fn o_que_nao_fecha_fica_como_texto() {
        for texto in [
            "2 * 3 ** 4",
            "arquivo_de_exemplo.txt",
            "preço [a combinar",
            "a_b _c",
            "[]() e [x](a b)",
        ] {
            assert_eq!(
                interpretar_trechos(texto),
                vec![trecho(texto, false, false, None)],
                "{texto}"
            );
        }
    }

    #[test]
    fn marcacao_vai_e_volta() {
        for texto in [
            "Texto simples.",
            "Com **negrito** e _itálico_ e [link](https://exemplo.example/a_b).",
            "**_os dois_** no começo",
        ] {
            assert_eq!(escrever_trechos(&interpretar_trechos(texto)), texto);
        }
    }

    #[test]
    fn cada_bloco_vai_e_volta_pelo_formulario() {
        let resolvido = Resolvido::default();
        let digitados = [
            BlocoDigitado {
                tipo: "titulo".into(),
                texto: "O que olhar".into(),
                nivel: 3,
                ..BlocoDigitado::default()
            },
            BlocoDigitado {
                tipo: "lista".into(),
                texto: "Primeiro\nSegundo".into(),
                nivel: 2,
                ordenada: true,
                ..BlocoDigitado::default()
            },
            BlocoDigitado {
                tipo: "citacao".into(),
                texto: "Móvel bom é o que o neto herda.".into(),
                nivel: 2,
                fonte: "Helena".into(),
                ..BlocoDigitado::default()
            },
            BlocoDigitado {
                tipo: "perguntas".into(),
                texto: "Quanto tempo leva?\nUns trinta dias.\n\nVocês entregam?\nSim.".into(),
                nivel: 2,
                ..BlocoDigitado::default()
            },
            BlocoDigitado {
                tipo: "tabela".into(),
                texto: "Serviço | Prazo\nMesa | 30 dias".into(),
                nivel: 2,
                ..BlocoDigitado::default()
            },
            BlocoDigitado {
                tipo: "video".into(),
                texto: "Como lixar".into(),
                nivel: 2,
                url: "https://www.youtube.com/watch?v=abc".into(),
                ..BlocoDigitado::default()
            },
            BlocoDigitado {
                tipo: "chamada".into(),
                texto: "Quer um orçamento?".into(),
                nivel: 2,
                rotulo: "Fale com a gente".into(),
                url: "/contato".into(),
                ..BlocoDigitado::default()
            },
        ];
        for digitado in digitados {
            assert_eq!(digitar(&montar(&digitado, &resolvido)), digitado);
        }

        let Bloco::Perguntas { itens } = montar(
            &BlocoDigitado {
                tipo: "perguntas".into(),
                texto: "Quanto tempo leva?\nUns trinta\ndias.".into(),
                ..BlocoDigitado::default()
            },
            &resolvido,
        ) else {
            panic!("bloco de perguntas");
        };
        assert_eq!(itens[0].resposta, "Uns trinta dias.");
    }

    #[test]
    fn agendamento_e_lido_no_horario_de_brasilia() {
        use chrono::TimeZone;
        assert_eq!(
            ler_agendamento("2026-10-20T09:30"),
            Utc.with_ymd_and_hms(2026, 10, 20, 12, 30, 0).single()
        );
        for invalido in ["", "amanhã", "2026-10-20", "2026-13-40T99:99"] {
            assert_eq!(ler_agendamento(invalido), None, "{invalido}");
        }
    }

    #[test]
    fn botoes_mexem_na_lista_de_blocos() {
        let mut formulario = Formulario::ler(pares(&[
            ("n", "3"),
            ("b0_tipo", "paragrafo"),
            ("b0_texto", "um"),
            ("b1_tipo", "paragrafo"),
            ("b1_texto", "dois"),
            ("b2_tipo", "paragrafo"),
            ("b2_texto", "três"),
            ("novo_tipo", "titulo"),
            ("acao", "subir:2"),
        ]));
        let textos = |f: &Formulario| -> Vec<String> {
            f.blocos.iter().map(|bloco| bloco.texto.clone()).collect()
        };
        assert_eq!(formulario.aplicar_acao(), Acao::Salvar);
        assert_eq!(textos(&formulario), ["um", "três", "dois"]);

        formulario.acao = "descer:0".into();
        formulario.aplicar_acao();
        assert_eq!(textos(&formulario), ["três", "um", "dois"]);

        formulario.acao = "remover:1".into();
        formulario.aplicar_acao();
        assert_eq!(textos(&formulario), ["três", "dois"]);

        formulario.acao = "adicionar".into();
        formulario.aplicar_acao();
        assert_eq!(formulario.blocos.len(), 3);
        assert_eq!(formulario.blocos[2].tipo, "titulo");

        // Índice fora da lista e tipo desconhecido não fazem nada.
        for (acao, tipo) in [
            ("subir:0", "titulo"),
            ("descer:2", "titulo"),
            ("remover:9", "titulo"),
            ("adicionar", "script"),
        ] {
            formulario.acao = acao.into();
            formulario.novo_tipo = tipo.into();
            assert_eq!(formulario.aplicar_acao(), Acao::Salvar);
            assert_eq!(formulario.blocos.len(), 3, "{acao}");
        }
        for (acao, esperada) in [
            ("revisar", Acao::EnviarParaRevisao),
            ("devolver", Acao::Devolver),
            ("publicar", Acao::Publicar),
            ("despublicar", Acao::Despublicar),
            ("agendar", Acao::Agendar),
            ("desagendar", Acao::CancelarAgendamento),
            ("salvar", Acao::Salvar),
            ("", Acao::Salvar),
        ] {
            formulario.acao = acao.into();
            assert_eq!(formulario.aplicar_acao(), esperada, "{acao}");
        }
    }

    #[test]
    fn conteudo_do_exemplo_vai_e_volta_pelo_formulario() {
        let agora = Utc::now();
        for documento in motor_web::demonstracao::documentos() {
            let (capa, corpo, autor, categoria) = match &documento.conteudo {
                Conteudo::Pagina(pagina) => (pagina.capa.clone(), &pagina.corpo, None, None),
                Conteudo::Post(post) => (
                    Some(post.capa.clone()),
                    &post.corpo,
                    Some(post.autor.clone()),
                    Some(post.categoria.clone()),
                ),
                Conteudo::Produto(_) => continue,
            };
            let mut resolvido = Resolvido {
                autor,
                categoria,
                ..Resolvido::default()
            };
            for midia in capa
                .iter()
                .chain(corpo.iter().filter_map(|bloco| match bloco {
                    Bloco::Imagem { midia } => Some(midia),
                    _ => None,
                }))
            {
                resolvido.midias.insert(midia.id.clone(), midia.clone());
            }

            let formulario = Formulario::do_conteudo(&documento.conteudo);
            let refeito = formulario.para_conteudo(&resolvido, agora);
            let (corpo_refeito, titulo) = match &refeito {
                Conteudo::Pagina(pagina) => (&pagina.corpo, &pagina.titulo),
                Conteudo::Post(post) => (&post.corpo, &post.titulo),
                Conteudo::Produto(_) => continue,
            };
            assert_eq!(corpo_refeito, corpo, "{}", documento.caminho);
            assert_eq!(titulo, documento.titulo(), "{}", documento.caminho);
            assert_eq!(refeito_slug(&refeito), documento.slug());
            assert_eq!(seo_de(&refeito), documento.seo());
        }
    }

    fn refeito_slug(conteudo: &Conteudo) -> &str {
        match conteudo {
            Conteudo::Pagina(pagina) => &pagina.slug,
            Conteudo::Post(post) => &post.slug,
            Conteudo::Produto(produto) => &produto.slug,
        }
    }

    fn seo_de(conteudo: &Conteudo) -> &Seo {
        match conteudo {
            Conteudo::Pagina(pagina) => &pagina.seo,
            Conteudo::Post(post) => &post.seo,
            Conteudo::Produto(produto) => &produto.seo,
        }
    }
}
