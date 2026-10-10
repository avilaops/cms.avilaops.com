//! O editor de blocos, do lado que não toca em banco: ler o formulário,
//! mexer na lista de blocos e traduzir entre o que a pessoa digita e o
//! contrato do motor.
//!
//! Não existe HTML livre. O parágrafo aceita uma marcação mínima, que vira
//! `Trecho`: `**negrito**`, `_itálico_` e `[texto](endereço)`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use motor_web::tipos::{
    Abertura, Acao as Botao, Autor, Bloco, Cartao, Categoria, Conteudo, Depoimento, ItemTrilha,
    Midia, Numero, Pagina, Passo, Pergunta, Plano, Post, Seo, TipoPost, Trecho,
};

/// Os tipos de bloco, na ordem em que aparecem para escolher.
pub const TIPOS_DE_BLOCO: [(&str, &str); 19] = [
    ("paragrafo", "Parágrafo"),
    ("titulo", "Título"),
    ("lista", "Lista"),
    ("imagem", "Imagem"),
    ("citacao", "Citação"),
    ("perguntas", "Perguntas e respostas"),
    ("tabela", "Tabela"),
    ("video", "Vídeo"),
    ("chamada", "Chamada com botão"),
    ("cartoes", "Seção: cartões"),
    ("destaque", "Seção: imagem e texto"),
    ("depoimentos", "Seção: depoimentos"),
    ("numeros", "Seção: números"),
    ("passos", "Seção: passo a passo"),
    ("planos", "Seção: planos e preços"),
    ("galeria", "Seção: galeria"),
    ("logos", "Seção: logos"),
    ("faixa", "Seção: faixa de chamada"),
    ("contato", "Seção: contato"),
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
    /// O título de uma seção.
    pub titulo: String,
    /// O texto de apresentação de uma seção, abaixo do título.
    pub resumo: String,
    /// Na seção de imagem e texto, a imagem vai para a direita.
    pub invertido: bool,
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
    /// A abertura da página: a linha acima do título, o texto abaixo dele e
    /// os botões, um por linha, como `Rótulo | endereço`.
    pub abertura_sobretitulo: String,
    pub abertura_texto: String,
    pub abertura_acoes: String,
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
        // O navegador manda quebra de linha como `\r\n`; aqui é sempre `\n`.
        let campos: HashMap<String, String> = pares
            .into_iter()
            .map(|(nome, valor)| (nome, valor.replace("\r\n", "\n")))
            .collect();
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
                    titulo: do_bloco("titulo"),
                    resumo: do_bloco("resumo"),
                    invertido: do_bloco("invertido") == "1",
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
            abertura_sobretitulo: campo("abertura_sobretitulo"),
            abertura_texto: campo("abertura_texto"),
            abertura_acoes: campo("abertura_acoes"),
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
            Conteudo::Pagina(pagina) => {
                let abertura = pagina.abertura.clone().unwrap_or_default();
                Self {
                    abertura_sobretitulo: abertura.sobretitulo.unwrap_or_default(),
                    abertura_texto: abertura.texto,
                    abertura_acoes: escrever_botoes(&abertura.acoes),
                    ..comum(
                        &pagina.titulo,
                        &pagina.slug,
                        &pagina.seo,
                        pagina.capa.as_ref(),
                        &pagina.corpo,
                    )
                }
            }
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
            .chain(self.blocos.iter().flat_map(imagens_citadas))
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
                abertura: self.abertura(),
                publicado_em: agora,
                atualizado_em: agora,
                seo,
            })
        }
    }
}

impl Formulario {
    /// A abertura digitada. Tudo em branco é página sem abertura.
    fn abertura(&self) -> Option<Abertura> {
        let abertura = Abertura {
            sobretitulo: opcional(&self.abertura_sobretitulo),
            texto: self.abertura_texto.trim().to_string(),
            acoes: ler_botoes(&self.abertura_acoes),
        };
        (abertura != Abertura::default()).then_some(abertura)
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

fn opcional(texto: &str) -> Option<String> {
    Some(texto.trim().to_string()).filter(|texto| !texto.is_empty())
}

/// A célula `i` de uma linha, ou vazio.
fn celula(celulas: &[String], i: usize) -> &str {
    celulas.get(i).map_or("", String::as_str)
}

/// Junta as células de uma linha, sem as vazias do fim.
fn escrever_linha(celulas: &[&str]) -> String {
    let usadas = celulas
        .iter()
        .rposition(|celula| !celula.trim().is_empty())
        .map_or(0, |ultima| ultima + 1);
    celulas[..usadas].join(" | ")
}

/// Um botão só existe com rótulo ou endereço: o que faltar, a validação cobra.
fn botao(rotulo: &str, url: &str) -> Option<Botao> {
    (!rotulo.is_empty() || !url.is_empty()).then(|| Botao {
        rotulo: rotulo.to_string(),
        url: url.to_string(),
    })
}

/// Um botão por linha, como `Rótulo | endereço`.
fn ler_botoes(texto: &str) -> Vec<Botao> {
    linhas(texto)
        .iter()
        .filter_map(|linha| {
            let celulas = celulas(linha);
            botao(celula(&celulas, 0), celula(&celulas, 1))
        })
        .collect()
}

fn escrever_botoes(botoes: &[Botao]) -> String {
    botoes
        .iter()
        .map(|botao| escrever_linha(&[&botao.rotulo, &botao.url]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Em que coluna das linhas de uma seção fica o identificador da imagem.
fn coluna_da_imagem(tipo: &str) -> Option<usize> {
    match tipo {
        "cartoes" => Some(4),
        "depoimentos" => Some(3),
        "galeria" | "logos" => Some(0),
        _ => None,
    }
}

/// Os identificadores de imagem que um bloco cita: o do campo de imagem e os
/// escritos nas linhas de uma seção.
fn imagens_citadas(bloco: &BlocoDigitado) -> Vec<&str> {
    let nas_linhas = coluna_da_imagem(&bloco.tipo)
        .into_iter()
        .flat_map(|coluna| {
            bloco
                .texto
                .lines()
                .filter_map(move |linha| linha.split('|').nth(coluna))
                .map(str::trim)
        });
    std::iter::once(bloco.midia.as_str())
        .chain(nas_linhas)
        .collect()
}

/// A imagem citada. Identificador que a biblioteca não tem vira imagem vazia,
/// e a validação pede para escolher outra.
fn imagem(resolvido: &Resolvido, id: &str) -> Option<Midia> {
    (!id.is_empty()).then(|| {
        resolvido
            .midias
            .get(id)
            .cloned()
            .unwrap_or_else(cms_dados::midia_vazia)
    })
}

/// As linhas de contato, como `telefone: (11) 4000-0000`.
const CANAIS: [&str; 5] = ["telefone", "whatsapp", "email", "endereco", "horario"];

/// Do que foi digitado para a seção do motor. `None` para o que não é seção.
fn montar_secao(bloco: &BlocoDigitado, resolvido: &Resolvido) -> Option<Bloco> {
    let titulo = opcional(&bloco.titulo);
    let por_linha = || {
        linhas(&bloco.texto)
            .into_iter()
            .map(|linha| celulas(&linha))
    };
    Some(match bloco.tipo.as_str() {
        "cartoes" => Bloco::Cartoes {
            titulo,
            texto: opcional(&bloco.resumo),
            itens: por_linha()
                .map(|c| Cartao {
                    titulo: celula(&c, 0).to_string(),
                    texto: celula(&c, 1).to_string(),
                    acao: botao(celula(&c, 2), celula(&c, 3)),
                    midia: imagem(resolvido, celula(&c, 4)),
                })
                .collect(),
        },
        "destaque" => Bloco::Destaque {
            titulo: bloco.titulo.trim().to_string(),
            texto: bloco.texto.trim().to_string(),
            midia: imagem(resolvido, &bloco.midia).unwrap_or_else(cms_dados::midia_vazia),
            invertido: bloco.invertido,
            acao: botao(bloco.rotulo.trim(), bloco.url.trim()),
        },
        "depoimentos" => Bloco::Depoimentos {
            titulo,
            itens: por_linha()
                .map(|c| Depoimento {
                    texto: celula(&c, 0).to_string(),
                    autor: celula(&c, 1).to_string(),
                    cargo: opcional(celula(&c, 2)),
                    foto: imagem(resolvido, celula(&c, 3)),
                })
                .collect(),
        },
        "numeros" => Bloco::Numeros {
            titulo,
            itens: por_linha()
                .map(|c| Numero {
                    valor: celula(&c, 0).to_string(),
                    rotulo: celula(&c, 1).to_string(),
                })
                .collect(),
        },
        "passos" => Bloco::Passos {
            titulo,
            itens: por_linha()
                .map(|c| Passo {
                    titulo: celula(&c, 0).to_string(),
                    texto: celula(&c, 1).to_string(),
                })
                .collect(),
        },
        "planos" => Bloco::Planos {
            titulo,
            itens: bloco
                .texto
                .split("\n\n")
                .filter_map(|grupo| {
                    let mut do_plano = linhas(grupo).into_iter();
                    let cabeca = celulas(&do_plano.next()?);
                    let acao = do_plano
                        .next()
                        .map(|linha| celulas(&linha))
                        .unwrap_or_default();
                    let nome = celula(&cabeca, 0);
                    Some(Plano {
                        nome: nome.trim_start_matches('*').trim().to_string(),
                        preco: celula(&cabeca, 1).to_string(),
                        periodo: opcional(celula(&cabeca, 2)),
                        descricao: opcional(celula(&cabeca, 3)),
                        itens: do_plano.collect(),
                        acao: botao(celula(&acao, 0), celula(&acao, 1)).unwrap_or_default(),
                        destaque: nome.starts_with('*'),
                    })
                })
                .collect(),
        },
        "galeria" | "logos" => {
            let itens = linhas(&bloco.texto)
                .iter()
                .filter_map(|id| imagem(resolvido, id))
                .collect();
            if bloco.tipo == "galeria" {
                Bloco::Galeria { titulo, itens }
            } else {
                Bloco::Logos { titulo, itens }
            }
        }
        "faixa" => Bloco::Faixa {
            titulo: bloco.titulo.trim().to_string(),
            texto: opcional(&bloco.resumo),
            acoes: ler_botoes(&bloco.texto),
        },
        "contato" => {
            let canal = |nome: &str| {
                linhas(&bloco.texto).iter().find_map(|linha| {
                    let (chave, valor) = linha.split_once(':')?;
                    (chave.trim().to_lowercase() == nome)
                        .then(|| opcional(valor))
                        .flatten()
                })
            };
            Bloco::Contato {
                titulo,
                texto: opcional(&bloco.resumo),
                telefone: canal(CANAIS[0]),
                whatsapp: canal(CANAIS[1]),
                email: canal(CANAIS[2]),
                endereco: canal(CANAIS[3]),
                horario: canal(CANAIS[4]),
            }
        }
        _ => return None,
    })
}

/// O identificador de uma imagem que pode faltar, ou vazio.
fn id(midia: &Option<Midia>) -> &str {
    midia.as_ref().map_or("", |midia| midia.id.as_str())
}

/// Da seção do motor para o que aparece no formulário.
fn digitar_secao(bloco: &Bloco) -> Option<BlocoDigitado> {
    let texto = |texto: &Option<String>| texto.clone().unwrap_or_default();
    let base = |tipo: &str, titulo: &Option<String>, linhas: Vec<String>| BlocoDigitado {
        tipo: tipo.to_string(),
        titulo: titulo.clone().unwrap_or_default(),
        texto: linhas.join("\n"),
        nivel: 2,
        ..BlocoDigitado::default()
    };
    Some(match bloco {
        Bloco::Cartoes {
            titulo,
            texto: resumo,
            itens,
        } => BlocoDigitado {
            resumo: texto(resumo),
            ..base(
                "cartoes",
                titulo,
                itens
                    .iter()
                    .map(|item| {
                        let acao = item.acao.clone().unwrap_or_default();
                        escrever_linha(&[
                            &item.titulo,
                            &item.texto,
                            &acao.rotulo,
                            &acao.url,
                            id(&item.midia),
                        ])
                    })
                    .collect(),
            )
        },
        Bloco::Destaque {
            titulo,
            texto,
            midia,
            invertido,
            acao,
        } => {
            let acao = acao.clone().unwrap_or_default();
            BlocoDigitado {
                tipo: "destaque".to_string(),
                titulo: titulo.clone(),
                texto: texto.clone(),
                midia: midia.id.clone(),
                invertido: *invertido,
                rotulo: acao.rotulo,
                url: acao.url,
                nivel: 2,
                ..BlocoDigitado::default()
            }
        }
        Bloco::Depoimentos { titulo, itens } => base(
            "depoimentos",
            titulo,
            itens
                .iter()
                .map(|item| {
                    escrever_linha(&[
                        &item.texto,
                        &item.autor,
                        item.cargo.as_deref().unwrap_or(""),
                        id(&item.foto),
                    ])
                })
                .collect(),
        ),
        Bloco::Numeros { titulo, itens } => base(
            "numeros",
            titulo,
            itens
                .iter()
                .map(|item| escrever_linha(&[&item.valor, &item.rotulo]))
                .collect(),
        ),
        Bloco::Passos { titulo, itens } => base(
            "passos",
            titulo,
            itens
                .iter()
                .map(|item| escrever_linha(&[&item.titulo, &item.texto]))
                .collect(),
        ),
        Bloco::Planos { titulo, itens } => BlocoDigitado {
            texto: itens
                .iter()
                .map(|plano| {
                    let nome = if plano.destaque {
                        format!("* {}", plano.nome)
                    } else {
                        plano.nome.clone()
                    };
                    let mut do_plano = vec![
                        escrever_linha(&[
                            &nome,
                            &plano.preco,
                            plano.periodo.as_deref().unwrap_or(""),
                            plano.descricao.as_deref().unwrap_or(""),
                        ]),
                        escrever_linha(&[&plano.acao.rotulo, &plano.acao.url]),
                    ];
                    do_plano.extend(plano.itens.iter().cloned());
                    do_plano.join("\n")
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            ..base("planos", titulo, Vec::new())
        },
        Bloco::Galeria { titulo, itens } => base(
            "galeria",
            titulo,
            itens.iter().map(|midia| midia.id.clone()).collect(),
        ),
        Bloco::Logos { titulo, itens } => base(
            "logos",
            titulo,
            itens.iter().map(|midia| midia.id.clone()).collect(),
        ),
        Bloco::Faixa {
            titulo,
            texto: resumo,
            acoes,
        } => BlocoDigitado {
            titulo: titulo.clone(),
            resumo: texto(resumo),
            texto: escrever_botoes(acoes),
            ..base("faixa", &None, Vec::new())
        },
        Bloco::Contato {
            titulo,
            texto: resumo,
            telefone,
            whatsapp,
            email,
            endereco,
            horario,
        } => BlocoDigitado {
            resumo: texto(resumo),
            ..base(
                "contato",
                titulo,
                CANAIS
                    .iter()
                    .zip([telefone, whatsapp, email, endereco, horario])
                    .filter_map(|(nome, valor)| Some(format!("{nome}: {}", valor.as_ref()?)))
                    .collect(),
            )
        },
        _ => return None,
    })
}

/// Do que foi digitado para o bloco do motor.
fn montar(bloco: &BlocoDigitado, resolvido: &Resolvido) -> Bloco {
    if let Some(secao) = montar_secao(bloco, resolvido) {
        return secao;
    }
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
    if let Some(secao) = digitar_secao(bloco) {
        return secao;
    }
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
        }, // As seções saíram acima, por `digitar_secao`.
        _ => base("paragrafo", String::new()),
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

        let secao = |tipo: &str, titulo: &str, texto: &str| BlocoDigitado {
            tipo: tipo.into(),
            titulo: titulo.into(),
            texto: texto.into(),
            nivel: 2,
            ..BlocoDigitado::default()
        };
        let secoes = [
            BlocoDigitado {
                resumo: "Do desenho à instalação.".into(),
                ..secao(
                    "cartoes",
                    "O que fazemos",
                    "Móveis sob medida | Feitos para o espaço | Saiba mais | /servicos\nRestauro | Peças de família recuperadas",
                )
            },
            BlocoDigitado {
                invertido: true,
                rotulo: "Conheça".into(),
                url: "/sobre".into(),
                ..secao(
                    "destaque",
                    "Madeira com procedência",
                    "Toda tábua tem origem.\n\nVocê recebe o registro.",
                )
            },
            secao(
                "depoimentos",
                "Clientes",
                "Ficou ótimo | Marina Teles | Ribeirão Preto\nRecomendo | Caio",
            ),
            secao(
                "numeros",
                "Em números",
                "15 anos | de oficina\n1.200 | peças entregues",
            ),
            secao(
                "passos",
                "Como funciona",
                "Visita | Medimos o espaço\nProjeto | Você aprova o desenho",
            ),
            secao(
                "planos",
                "Formas de contratar",
                "Peça única | R$ 1.800 | por projeto | Um móvel\nPedir orçamento | /contato\nVisita técnica\nDesenho\n\n* Ambiente completo | Sob consulta\nFalar com a oficina | /contato",
            ),
            BlocoDigitado {
                titulo: "Vamos conversar?".into(),
                resumo: "A visita é sem compromisso.".into(),
                ..secao(
                    "faixa",
                    "",
                    "Agendar visita | /contato\nVer serviços | /servicos",
                )
            },
            BlocoDigitado {
                resumo: "Respondemos em um dia útil.".into(),
                ..secao(
                    "contato",
                    "Fale com a oficina",
                    "telefone: (16) 5550-0100\nemail: contato@oficina.example\nhorario: Segunda a sexta, das 8h às 18h",
                )
            },
        ];
        for digitado in secoes {
            assert_eq!(
                digitar(&montar(&digitado, &resolvido)),
                digitado,
                "{}",
                digitado.tipo
            );
        }
        let Bloco::Planos { itens, .. } = montar(
            &secao(
                "planos",
                "",
                "* Completo | R$ 900 | por mês\nQuero este | /contato\nSuporte",
            ),
            &resolvido,
        ) else {
            panic!("seção de planos");
        };
        assert!(itens[0].destaque);
        assert_eq!(itens[0].nome, "Completo");
        assert_eq!(itens[0].itens, vec!["Suporte".to_string()]);

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
    fn quebra_de_linha_do_navegador_separa_grupos_como_a_digitada() {
        let formulario = Formulario::ler(pares(&[
            ("n", "1"),
            ("b0_tipo", "perguntas"),
            (
                "b0_texto",
                "Entregam?\r\nSim.\r\n\r\nParcelam?\r\nEm três vezes.",
            ),
            ("abertura_texto", "Pão fresco."),
            (
                "abertura_acoes",
                "Encomendar | /contato\r\nVer pães | /paes",
            ),
        ]));
        let Conteudo::Pagina(pagina) = formulario.para_conteudo(&Resolvido::default(), Utc::now())
        else {
            panic!("página");
        };
        let Bloco::Perguntas { itens } = &pagina.corpo[0] else {
            panic!("perguntas");
        };
        assert_eq!(itens.len(), 2);
        let abertura = pagina.abertura.expect("abertura");
        assert_eq!(abertura.acoes.len(), 2);
        assert_eq!(abertura.acoes[1].url, "/paes");
        // Página sem nada na abertura não ganha uma vazia.
        let sem = Formulario::ler(pares(&[("n", "0")]));
        let Conteudo::Pagina(pagina) = sem.para_conteudo(&Resolvido::default(), Utc::now()) else {
            panic!("página");
        };
        assert_eq!(pagina.abertura, None);
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
