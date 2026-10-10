//! O que os templates recebem. Tudo aqui já vem pronto para mostrar: o
//! template compõe, não decide.

use askama::Template;
use cms_dados::ItemDeNavegacao;
use cms_dominio::datas;
use motor_web::conteudo::renderizar_corpo;
use motor_web::midia::{Papel, renderizar_midia};
use motor_web::secoes::{ESTILO_DAS_SECOES, renderizar_abertura};
use motor_web::tipos::{Autor, Conteudo, Documento, ItemTrilha, Post, Site};

pub struct Link {
    pub rotulo: String,
    pub url: String,
    pub atual: bool,
}

/// O que toda página do site tem em volta do conteúdo.
pub struct Moldura {
    pub idioma: String,
    /// Título, metas, canônica e JSON-LD, já em HTML, vindos do motor.
    pub head: String,
    pub nome_do_site: String,
    pub navegacao: Vec<Link>,
    pub rodape: Vec<String>,
    /// O estilo das seções, que vem do motor para ser igual em todo site.
    pub estilo_das_secoes: &'static str,
}

impl Moldura {
    pub fn nova(site: &Site, head: String, itens: &[ItemDeNavegacao], caminho_atual: &str) -> Self {
        let mut navegacao = vec![Link {
            rotulo: "Início".into(),
            url: "/".into(),
            atual: caminho_atual == "/",
        }];
        navegacao.extend(
            itens
                .iter()
                .filter(|item| !item.eh_post && item.caminho != "/")
                .map(|item| Link {
                    rotulo: item.titulo.clone(),
                    url: item.caminho.clone(),
                    atual: item.caminho == caminho_atual,
                }),
        );
        if itens.iter().any(|item| item.eh_post) {
            navegacao.push(Link {
                rotulo: "Blog".into(),
                url: "/blog".into(),
                atual: caminho_atual.starts_with("/blog"),
            });
        }

        // Quem está por trás do site: sinal de confiança que sai do cadastro.
        let organizacao = &site.organizacao;
        let mut rodape = vec![
            organizacao
                .razao_social
                .clone()
                .unwrap_or_else(|| site.nome.clone()),
        ];
        if let Some(endereco) = &organizacao.endereco {
            rodape.push(format!(
                "{}, {} - {}, CEP {}",
                endereco.logradouro, endereco.cidade, endereco.uf, endereco.cep
            ));
        }
        rodape.extend(
            [organizacao.telefone.clone(), organizacao.email.clone()]
                .into_iter()
                .flatten(),
        );

        Self {
            idioma: site.idioma.clone(),
            head,
            nome_do_site: site.nome.clone(),
            navegacao,
            rodape,
            estilo_das_secoes: ESTILO_DAS_SECOES,
        }
    }
}

pub struct Datas {
    pub publicado: String,
    pub publicado_iso: String,
    pub atualizado: String,
    pub atualizado_iso: String,
    /// Só aparece quando o conteúdo mudou depois da publicação.
    pub mostrar_atualizado: bool,
}

impl Datas {
    fn do_post(post: &Post) -> Self {
        Self {
            publicado: datas::por_extenso(post.publicado_em),
            publicado_iso: datas::iso(post.publicado_em),
            atualizado: datas::por_extenso(post.atualizado_em),
            atualizado_iso: datas::iso(post.atualizado_em),
            mostrar_atualizado: datas::iso(post.atualizado_em) != datas::iso(post.publicado_em),
        }
    }
}

pub fn caminho_da_categoria(slug: &str) -> String {
    format!("/blog/categoria/{slug}")
}

pub fn caminho_do_autor(slug: &str) -> String {
    format!("/autor/{slug}")
}

pub struct CaixaDoAutor {
    /// A página com os posts de quem escreveu.
    pub url: String,
    pub nome: String,
    pub cargo: String,
    pub bio: String,
    pub foto: String,
    pub credenciais: Vec<String>,
    pub perfis: Vec<String>,
}

impl CaixaDoAutor {
    pub fn nova(autor: &Autor) -> Self {
        Self {
            url: caminho_do_autor(&autor.slug),
            nome: autor.nome.clone(),
            cargo: autor.cargo.clone(),
            bio: autor.bio.clone(),
            foto: renderizar_midia(&autor.foto, Papel::Conteudo, "96px"),
            credenciais: autor.credenciais.clone(),
            // O motor só deixa passar link `http(s)`; aqui ficam só os externos.
            perfis: autor
                .perfis
                .iter()
                .filter(|p| p.starts_with("https://"))
                .cloned()
                .collect(),
        }
    }
}

#[derive(Template)]
#[template(path = "documento.html")]
pub struct PaginaDeDocumento {
    pub moldura: Moldura,
    pub trilha: Vec<ItemTrilha>,
    pub titulo: String,
    /// O topo de uma página de apresentação, já com o `<h1>`. Vazio nas outras.
    pub abertura: String,
    /// Página de apresentação: o conteúdo ocupa a largura inteira.
    pub larga: bool,
    pub datas: Option<Datas>,
    /// A categoria do post, com o endereço da listagem dela.
    pub categoria: Option<Link>,
    pub capa: String,
    pub corpo: String,
    pub autor: Option<CaixaDoAutor>,
}

impl PaginaDeDocumento {
    pub fn nova(moldura: Moldura, documento: &Documento) -> Self {
        let post = match &documento.conteudo {
            Conteudo::Post(post) => Some(post),
            _ => None,
        };
        Self {
            moldura,
            trilha: documento.trilha().to_vec(),
            titulo: documento.titulo().to_string(),
            abertura: match &documento.conteudo {
                Conteudo::Pagina(pagina) => renderizar_abertura(pagina).unwrap_or_default(),
                _ => String::new(),
            },
            larga: documento.eh_larga(),
            datas: post.map(Datas::do_post),
            categoria: post.map(|post| Link {
                rotulo: post.categoria.nome.clone(),
                url: caminho_da_categoria(&post.categoria.slug),
                atual: false,
            }),
            // A capa é a única imagem com prioridade de carregamento.
            capa: documento
                .capa()
                .map(|capa| renderizar_midia(capa, Papel::Principal, "100vw"))
                .unwrap_or_default(),
            corpo: renderizar_corpo(documento.corpo()),
            autor: post.map(|post| CaixaDoAutor::nova(&post.autor)),
        }
    }
}

pub struct Cartao {
    pub url: String,
    pub titulo: String,
    pub resumo: String,
    pub data: String,
    pub data_iso: String,
    pub autor: String,
}

impl Cartao {
    pub fn do_post(caminho: &str, post: &Post) -> Self {
        Self {
            url: caminho.to_string(),
            titulo: post.titulo.clone(),
            resumo: post.resumo.clone(),
            data: datas::por_extenso(post.publicado_em),
            data_iso: datas::iso(post.publicado_em),
            autor: post.autor.nome.clone(),
        }
    }
}

#[derive(Template)]
#[template(path = "blog.html")]
pub struct PaginaDoBlog {
    pub moldura: Moldura,
    /// Vazia no blog; nas listagens abaixo dele, o caminho até aqui.
    pub trilha: Vec<ItemTrilha>,
    pub titulo: String,
    /// Só na página de quem escreveu.
    pub autor: Option<CaixaDoAutor>,
    pub cartoes: Vec<Cartao>,
}

#[derive(Template)]
#[template(path = "aviso.html")]
pub struct PaginaDeAviso {
    pub moldura: Moldura,
    pub titulo: String,
    pub mensagem: String,
}
