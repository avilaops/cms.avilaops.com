//! Os modelos de página e de post: um rascunho já montado com as seções de
//! cada tipo de página, para quem escreve trocar o texto e publicar.
//!
//! O texto de cada modelo é orientação, não conteúdo: diz o que escrever ali.
//! A descrição de busca vem vazia de propósito, para o modelo não ir ao ar do
//! jeito que nasceu.

use chrono::{DateTime, Utc};
use motor_web::tipos::{
    Abertura, Acao, Autor, Bloco, Cartao, Categoria, Conteudo, Depoimento, Direitos, ItemTrilha,
    Midia, Numero, Pagina, Passo, Pergunta, Plano, Post, Seo, TipoPost, Trecho,
};

/// Um modelo na lista de escolha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modelo {
    pub id: &'static str,
    pub nome: &'static str,
    pub descricao: &'static str,
    pub eh_post: bool,
}

const fn pagina(id: &'static str, nome: &'static str, descricao: &'static str) -> Modelo {
    Modelo {
        id,
        nome,
        descricao,
        eh_post: false,
    }
}

const fn post(id: &'static str, nome: &'static str, descricao: &'static str) -> Modelo {
    Modelo {
        id,
        nome,
        descricao,
        eh_post: true,
    }
}

pub const MODELOS: [Modelo; 12] = [
    pagina(
        "inicio",
        "Página inicial",
        "Abertura, serviços, um destaque com imagem, números, depoimentos e chamada final.",
    ),
    pagina(
        "sobre",
        "Sobre",
        "A história do negócio, o jeito de trabalhar em passos e os números.",
    ),
    pagina(
        "servicos",
        "Serviços",
        "A lista de serviços em cartões, como funciona e chamada para contato.",
    ),
    pagina(
        "servico",
        "Página de um serviço",
        "Um serviço em detalhe: para quem é, o que inclui, perguntas e chamada.",
    ),
    pagina(
        "precos",
        "Planos e preços",
        "Planos lado a lado, perguntas frequentes e chamada.",
    ),
    pagina(
        "contato",
        "Contato",
        "Telefone, WhatsApp, e-mail, endereço e horário.",
    ),
    pagina(
        "perguntas",
        "Perguntas frequentes",
        "Perguntas e respostas, com chamada para quem ainda tem dúvida.",
    ),
    pagina(
        "campanha",
        "Página de campanha",
        "Uma oferta só: abertura, benefícios, prova, passos e um único botão.",
    ),
    pagina(
        "privacidade",
        "Política de privacidade",
        "Texto corrido com os títulos que uma política de privacidade precisa ter.",
    ),
    pagina(
        "termos",
        "Termos de uso",
        "Texto corrido com os títulos de uns termos de uso.",
    ),
    post(
        "artigo",
        "Artigo do blog",
        "Introdução, seções com título e chamada no fim.",
    ),
    post(
        "guia",
        "Guia técnico",
        "Passo a passo numerado, tabela de referência e perguntas.",
    ),
];

fn sem_imagem() -> Midia {
    Midia {
        id: String::new(),
        alt: String::new(),
        largura: 0,
        altura: 0,
        variantes: Vec::new(),
        legenda: None,
        credito: None,
        direitos: Direitos::default(),
    }
}

fn acao(rotulo: &str, url: &str) -> Acao {
    Acao {
        rotulo: rotulo.into(),
        url: url.into(),
    }
}

fn paragrafo(texto: &str) -> Bloco {
    Bloco::Paragrafo {
        trechos: vec![Trecho {
            texto: texto.into(),
            ..Trecho::default()
        }],
    }
}

fn titulo(texto: &str) -> Bloco {
    Bloco::Titulo {
        nivel: 2,
        texto: texto.into(),
    }
}

fn cartoes(titulo: &str, texto: &str, itens: &[(&str, &str)]) -> Bloco {
    Bloco::Cartoes {
        titulo: Some(titulo.into()),
        texto: Some(texto.into()),
        itens: itens
            .iter()
            .map(|(titulo, texto)| Cartao {
                titulo: (*titulo).into(),
                texto: (*texto).into(),
                midia: None,
                acao: None,
            })
            .collect(),
    }
}

fn destaque(titulo: &str, texto: &str, invertido: bool) -> Bloco {
    Bloco::Destaque {
        titulo: titulo.into(),
        texto: texto.into(),
        midia: sem_imagem(),
        invertido,
        acao: None,
    }
}

fn numeros(itens: &[(&str, &str)]) -> Bloco {
    Bloco::Numeros {
        titulo: Some("Em números".into()),
        itens: itens
            .iter()
            .map(|(valor, rotulo)| Numero {
                valor: (*valor).into(),
                rotulo: (*rotulo).into(),
            })
            .collect(),
    }
}

fn passos(titulo: &str, itens: &[(&str, &str)]) -> Bloco {
    Bloco::Passos {
        titulo: Some(titulo.into()),
        itens: itens
            .iter()
            .map(|(titulo, texto)| Passo {
                titulo: (*titulo).into(),
                texto: (*texto).into(),
            })
            .collect(),
    }
}

fn depoimentos() -> Bloco {
    Bloco::Depoimentos {
        titulo: Some("O que dizem os clientes".into()),
        itens: vec![Depoimento {
            texto: "Copie aqui a frase de um cliente, com a autorização dele.".into(),
            autor: "Nome do cliente".into(),
            cargo: Some("Cidade ou empresa".into()),
            foto: None,
        }],
    }
}

fn perguntas(itens: &[(&str, &str)]) -> Bloco {
    Bloco::Perguntas {
        itens: itens
            .iter()
            .map(|(pergunta, resposta)| Pergunta {
                pergunta: (*pergunta).into(),
                resposta: (*resposta).into(),
            })
            .collect(),
    }
}

fn faixa(titulo: &str, texto: &str, rotulo: &str) -> Bloco {
    Bloco::Faixa {
        titulo: titulo.into(),
        texto: Some(texto.into()),
        acoes: vec![acao(rotulo, "/contato")],
    }
}

fn contato() -> Bloco {
    Bloco::Contato {
        titulo: Some("Fale com a gente".into()),
        texto: Some("Diga em quanto tempo vocês costumam responder.".into()),
        telefone: Some("(00) 0000-0000".into()),
        whatsapp: Some("(00) 90000-0000".into()),
        email: Some("contato@seudominio.com.br".into()),
        endereco: Some("Rua, número, bairro, cidade - UF".into()),
        horario: Some("Segunda a sexta, das 8h às 18h".into()),
    }
}

fn abertura(sobretitulo: &str, texto: &str, botoes: &[(&str, &str)]) -> Option<Abertura> {
    Some(Abertura {
        sobretitulo: Some(sobretitulo.into()),
        texto: texto.into(),
        acoes: botoes
            .iter()
            .map(|(rotulo, url)| acao(rotulo, url))
            .collect(),
    })
}

const PERGUNTAS_COMUNS: [(&str, &str); 3] = [
    (
        "Escreva a pergunta que mais chega pelo WhatsApp",
        "Responda em duas ou três frases, do jeito que você responde ao cliente.",
    ),
    (
        "Quanto tempo leva?",
        "Diga o prazo comum e do que ele depende.",
    ),
    (
        "Como é o pagamento?",
        "Liste as formas de pagamento e as condições.",
    ),
];

/// O corpo e a abertura de cada modelo de página.
fn de_pagina(id: &str) -> Option<(&'static str, &'static str, Option<Abertura>, Vec<Bloco>)> {
    let servicos = [
        ("Nome do serviço", "O que ele resolve, em uma frase."),
        ("Outro serviço", "O que ele resolve, em uma frase."),
        ("Mais um serviço", "O que ele resolve, em uma frase."),
    ];
    let como_funciona = [
        (
            "Primeiro contato",
            "Como o cliente começa e o que você precisa saber dele.",
        ),
        ("Proposta", "O que ele recebe e em quanto tempo."),
        ("Entrega", "Como e quando o trabalho é entregue."),
    ];
    Some(match id {
        "inicio" => (
            "",
            "Diga o que o negócio faz e para quem",
            abertura(
                "O ramo ou a cidade",
                "Em uma ou duas frases: o problema que você resolve e o que o cliente ganha.",
                &[
                    ("Pedir orçamento", "/contato"),
                    ("Ver serviços", "/servicos"),
                ],
            ),
            vec![
                cartoes(
                    "O que fazemos",
                    "Uma frase que apresenta os serviços.",
                    &servicos,
                ),
                destaque(
                    "O que faz o seu trabalho diferente",
                    "Conte um diferencial concreto: um método, um material, um prazo.\n\nUm segundo parágrafo, se precisar.",
                    false,
                ),
                numeros(&[("00 anos", "de mercado"), ("000", "clientes atendidos")]),
                depoimentos(),
                faixa(
                    "Vamos conversar?",
                    "Diga o que acontece depois do contato.",
                    "Pedir orçamento",
                ),
            ],
        ),
        "sobre" => (
            "sobre",
            "Sobre a empresa",
            abertura(
                "Quem somos",
                "Quem está por trás do negócio e desde quando.",
                &[("Fale com a gente", "/contato")],
            ),
            vec![
                destaque(
                    "Como tudo começou",
                    "Conte a história em poucas linhas: quem fundou, por quê e o que mudou desde então.",
                    false,
                ),
                passos("Como trabalhamos", &como_funciona),
                numeros(&[("00 anos", "de mercado"), ("000", "projetos entregues")]),
                faixa(
                    "Quer conhecer o nosso trabalho?",
                    "Convide para uma conversa ou uma visita.",
                    "Entrar em contato",
                ),
            ],
        ),
        "servicos" => (
            "servicos",
            "Serviços",
            abertura(
                "O que fazemos",
                "Uma frase que resume o conjunto dos serviços.",
                &[("Pedir orçamento", "/contato")],
            ),
            vec![
                cartoes(
                    "Nossos serviços",
                    "Cada cartão pode levar à página do serviço.",
                    &servicos,
                ),
                passos("Como funciona", &como_funciona),
                faixa(
                    "Não achou o que procura?",
                    "Diga que você avalia casos fora da lista, se for verdade.",
                    "Fale com a gente",
                ),
            ],
        ),
        "servico" => (
            "nome-do-servico",
            "Nome do serviço",
            abertura(
                "Serviços",
                "O que este serviço resolve e para quem ele é.",
                &[("Pedir orçamento", "/contato")],
            ),
            vec![
                cartoes(
                    "O que está incluído",
                    "Liste o que o cliente recebe.",
                    &[
                        ("Item incluído", "Explique em uma frase."),
                        ("Outro item", "Explique em uma frase."),
                        ("Mais um item", "Explique em uma frase."),
                    ],
                ),
                destaque(
                    "Para quem é",
                    "Descreva a situação de quem mais se beneficia deste serviço.",
                    true,
                ),
                passos("Como funciona", &como_funciona),
                titulo("Perguntas sobre este serviço"),
                perguntas(&PERGUNTAS_COMUNS),
                faixa(
                    "Pronto para começar?",
                    "Diga qual é o próximo passo.",
                    "Pedir orçamento",
                ),
            ],
        ),
        "precos" => (
            "precos",
            "Planos e preços",
            abertura(
                "Preços",
                "Diga como o preço funciona: por mês, por projeto, sob consulta.",
                &[],
            ),
            vec![
                Bloco::Planos {
                    titulo: Some("Escolha o plano".into()),
                    itens: [("Básico", false), ("Completo", true), ("Sob medida", false)]
                        .into_iter()
                        .map(|(nome, destaque)| Plano {
                            nome: nome.into(),
                            preco: "R$ 000".into(),
                            periodo: Some("por mês".into()),
                            descricao: Some("Para quem este plano serve.".into()),
                            itens: vec!["O que inclui".into(), "Outro item incluído".into()],
                            acao: acao("Quero este", "/contato"),
                            destaque,
                        })
                        .collect(),
                },
                titulo("Perguntas sobre os planos"),
                perguntas(&PERGUNTAS_COMUNS),
                faixa(
                    "Ficou em dúvida entre os planos?",
                    "Ofereça ajuda para escolher.",
                    "Fale com a gente",
                ),
            ],
        ),
        "contato" => (
            "contato",
            "Contato",
            abertura(
                "Fale com a gente",
                "Diga por onde é mais rápido falar com vocês.",
                &[],
            ),
            vec![contato()],
        ),
        "perguntas" => (
            "perguntas-frequentes",
            "Perguntas frequentes",
            None,
            vec![
                paragrafo("Uma frase dizendo a quem estas respostas ajudam."),
                perguntas(&PERGUNTAS_COMUNS),
                faixa(
                    "Não achou a sua resposta?",
                    "Diga por onde a pessoa pode perguntar.",
                    "Fale com a gente",
                ),
            ],
        ),
        "campanha" => (
            "nome-da-campanha",
            "A promessa da oferta, em uma frase",
            abertura(
                "Nome da campanha",
                "Para quem é a oferta, o que a pessoa ganha e até quando vale.",
                &[("Quero aproveitar", "/contato")],
            ),
            vec![
                cartoes(
                    "O que você ganha",
                    "Três benefícios, do mais importante para o menos.",
                    &[
                        ("Primeiro benefício", "O resultado para o cliente."),
                        ("Segundo benefício", "O resultado para o cliente."),
                        ("Terceiro benefício", "O resultado para o cliente."),
                    ],
                ),
                depoimentos(),
                passos("Como participar", &como_funciona),
                faixa(
                    "A oferta vale até a data tal",
                    "Repita a promessa e diga o que acontece depois do clique.",
                    "Quero aproveitar",
                ),
            ],
        ),
        "privacidade" => (
            "politica-de-privacidade",
            "Política de privacidade",
            None,
            [
                (
                    "Quem somos",
                    "Nome da empresa, CNPJ e como falar com o encarregado dos dados.",
                ),
                (
                    "Quais dados coletamos",
                    "Liste os dados e de onde eles vêm: formulário, WhatsApp, navegação.",
                ),
                (
                    "Para que usamos os dados",
                    "Cada finalidade e a base legal dela, conforme a LGPD.",
                ),
                (
                    "Com quem compartilhamos",
                    "Fornecedores e parceiros que recebem os dados, e por quê.",
                ),
                (
                    "Por quanto tempo guardamos",
                    "O prazo de guarda de cada tipo de dado.",
                ),
                (
                    "Seus direitos",
                    "Como pedir acesso, correção ou exclusão dos dados, e o prazo de resposta.",
                ),
            ]
            .into_iter()
            .flat_map(|(secao, orientacao)| [titulo(secao), paragrafo(orientacao)])
            .collect(),
        ),
        "termos" => (
            "termos-de-uso",
            "Termos de uso",
            None,
            [
                (
                    "Sobre estes termos",
                    "A quem se aplicam e a partir de quando valem.",
                ),
                ("O serviço", "O que é oferecido e o que fica de fora."),
                (
                    "Obrigações de quem usa",
                    "O que a pessoa se compromete a fazer e a não fazer.",
                ),
                (
                    "Pagamento e cancelamento",
                    "Preço, forma de cobrança, cancelamento e reembolso.",
                ),
                (
                    "Responsabilidades",
                    "Os limites de responsabilidade de cada parte.",
                ),
                (
                    "Foro e contato",
                    "A lei aplicável, o foro e por onde tirar dúvidas.",
                ),
            ]
            .into_iter()
            .flat_map(|(secao, orientacao)| [titulo(secao), paragrafo(orientacao)])
            .collect(),
        ),
        _ => return None,
    })
}

/// O tipo, o endereço, o título e o corpo de cada modelo de post.
fn de_post(id: &str) -> Option<(TipoPost, &'static str, &'static str, Vec<Bloco>)> {
    Some(match id {
        "artigo" => (
            TipoPost::Artigo,
            "titulo-do-artigo",
            "O título do artigo, com a dúvida de quem lê",
            vec![
                paragrafo("Abra respondendo à pergunta do título em duas ou três frases."),
                titulo("O primeiro ponto"),
                paragrafo("Desenvolva com um exemplo do seu dia a dia."),
                titulo("O segundo ponto"),
                paragrafo("Desenvolva com um número, um caso ou uma comparação."),
                titulo("O que fazer agora"),
                paragrafo("Feche com o próximo passo de quem leu."),
                Bloco::Chamada {
                    texto: "Convide para o próximo passo.".into(),
                    rotulo: "Fale com a gente".into(),
                    url: "/contato".into(),
                },
            ],
        ),
        "guia" => (
            TipoPost::GuiaTecnico,
            "como-fazer-tal-coisa",
            "Como fazer tal coisa, passo a passo",
            vec![
                paragrafo(
                    "Diga o que a pessoa vai conseguir fazer ao fim do guia e o que precisa ter em mãos.",
                ),
                titulo("Passo a passo"),
                Bloco::Lista {
                    ordenada: true,
                    itens: vec![
                        "O primeiro passo, com o detalhe que evita o erro mais comum.".into(),
                        "O segundo passo.".into(),
                        "O terceiro passo.".into(),
                    ],
                },
                titulo("Referência rápida"),
                Bloco::Tabela {
                    cabecalho: vec!["Situação".into(), "O que usar".into()],
                    linhas: vec![vec!["Um caso comum".into(), "A recomendação".into()]],
                },
                titulo("Perguntas comuns"),
                perguntas(&PERGUNTAS_COMUNS),
            ],
        ),
        _ => return None,
    })
}

fn seo_vazio() -> Seo {
    Seo {
        titulo: String::new(),
        descricao: String::new(),
        indexar: true,
        imagem_social: None,
    }
}

fn item(nome: &str, url: String) -> ItemTrilha {
    ItemTrilha {
        nome: nome.into(),
        url,
    }
}

/// O rascunho de um modelo. As datas são provisórias: quem grava as de
/// verdade é a publicação.
pub fn conteudo_do_modelo(id: &str, agora: DateTime<Utc>) -> Option<Conteudo> {
    let inicio = || item("Início", "/".into());
    if let Some((slug, nome, abertura, corpo)) = de_pagina(id) {
        return Some(Conteudo::Pagina(Pagina {
            slug: slug.into(),
            titulo: nome.into(),
            corpo,
            capa: None,
            abertura,
            publicado_em: agora,
            atualizado_em: agora,
            seo: seo_vazio(),
            trilha: if slug.is_empty() {
                Vec::new()
            } else {
                vec![inicio(), item(nome, format!("/{slug}"))]
            },
        }));
    }
    let (tipo, slug, nome, corpo) = de_post(id)?;
    Some(Conteudo::Post(Post {
        tipo,
        slug: slug.into(),
        titulo: nome.into(),
        resumo: "O resumo que aparece na listagem do blog, em uma ou duas frases.".into(),
        capa: sem_imagem(),
        corpo,
        autor: Autor {
            slug: String::new(),
            nome: String::new(),
            cargo: String::new(),
            bio: String::new(),
            foto: sem_imagem(),
            perfis: Vec::new(),
            credenciais: Vec::new(),
        },
        categoria: Categoria {
            slug: String::new(),
            nome: String::new(),
        },
        tags: Vec::new(),
        publicado_em: agora,
        atualizado_em: agora,
        seo: seo_vazio(),
        trilha: vec![
            inicio(),
            item("Blog", "/blog".into()),
            item(nome, format!("/blog/{slug}")),
        ],
    }))
}

#[cfg(test)]
mod testes {
    use super::*;
    use motor_web::tipos::Documento;
    use motor_web::validacao::{Contexto, validar};

    #[test]
    fn todo_modelo_da_lista_existe_e_e_do_tipo_anunciado() {
        for modelo in MODELOS {
            let conteudo = conteudo_do_modelo(modelo.id, Utc::now())
                .unwrap_or_else(|| panic!("sem o modelo {}", modelo.id));
            assert_eq!(
                matches!(conteudo, Conteudo::Post(_)),
                modelo.eh_post,
                "{}",
                modelo.id
            );
        }
        assert_eq!(conteudo_do_modelo("inexistente", Utc::now()), None);
    }

    /// O que impede um modelo de ir ao ar é só o que a pessoa ainda não
    /// preencheu: busca, imagens e, no post, autor e categoria. A estrutura
    /// (níveis de título, seções, links) já nasce certa.
    #[test]
    fn modelo_nasce_com_a_estrutura_certa_e_so_falta_preencher() {
        let por_preencher = [
            "seo.",
            "secao.imagem.falta",
            "midia.alt.vazio",
            "post.",
            "slug.vazio",
        ];
        for modelo in MODELOS {
            let conteudo = conteudo_do_modelo(modelo.id, Utc::now()).expect("modelo");
            let documento = Documento {
                caminho: "/modelo".into(),
                conteudo,
            };
            let estruturais: Vec<_> = validar(&documento, &Contexto::vazio())
                .into_iter()
                .filter(|problema| {
                    !por_preencher
                        .iter()
                        .any(|prefixo| problema.codigo.starts_with(prefixo))
                })
                .map(|problema| (problema.codigo, problema.campo))
                .collect();
            assert_eq!(estruturais, vec![], "{}", modelo.id);
        }
    }

    #[test]
    fn modelo_nao_vai_ao_ar_sem_a_descricao_de_busca() {
        for modelo in MODELOS {
            let documento = Documento {
                caminho: "/modelo".into(),
                conteudo: conteudo_do_modelo(modelo.id, Utc::now()).expect("modelo"),
            };
            assert!(
                validar(&documento, &Contexto::vazio())
                    .iter()
                    .any(|problema| problema.codigo == "seo.descricao.vazia"),
                "{}",
                modelo.id
            );
        }
    }
}
