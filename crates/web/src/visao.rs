//! O que os templates recebem. Tudo aqui já vem pronto para mostrar: o
//! template compõe, não decide.

use askama::Template;
use cms_dados::ItemDeNavegacao;
use cms_dominio::fluxo::CAMINHO_DOS_PRODUTOS;
use cms_dominio::{EntradaDoMenu, datas, ler_entrada_do_menu};
use motor_web::conteudo::renderizar_corpo;
use motor_web::midia::{Papel, renderizar_midia, url_principal};
use motor_web::secoes::{ESTILO_DAS_SECOES, renderizar_abertura};
use motor_web::tipos::{Autor, Bloco, Conteudo, Documento, Item, ItemTrilha, Post, Site};
use motor_web::validacao::normalizar_slug;

pub struct Link {
    pub rotulo: String,
    pub url: String,
    pub atual: bool,
    /// As páginas de um grupo do menu, que abrem embaixo dele. Com filhos, o
    /// link é só o nome do grupo e não tem endereço.
    pub filhos: Vec<Link>,
}

/// O que toda página do site tem em volta do conteúdo.
pub struct Moldura {
    pub idioma: String,
    /// Título, metas, canônica e JSON-LD, já em HTML, vindos do motor.
    pub head: String,
    pub nome_do_site: String,
    pub navegacao: Vec<Link>,
    /// As páginas que o site deixou fora do menu: aparecem no rodapé.
    pub mais_paginas: Vec<Link>,
    pub rodape: Vec<String>,
    /// O estilo das seções, que vem do motor para ser igual em todo site.
    pub estilo_das_secoes: &'static str,
    /// O estilo do tema do site, por cima do estilo de base. Vazio no
    /// clássico.
    pub estilo_do_tema: &'static str,
    /// A cor da marca do site, já conferida. Sem ela, vale a do tema.
    pub cor_de_destaque: Option<String>,
    /// A logo no cabeçalho, quando o site tem uma pronta.
    pub logo: Option<Logo>,
    /// A versão do ícone do site, quando ele tem favicon ou logo. Vai no
    /// endereço do ícone, para o navegador buscar de novo quando a imagem muda.
    pub icone: Option<String>,
    /// A versão do atalho do iPhone, que pode sair de outra imagem.
    pub icone_apple: Option<String>,
    /// A tag do Google do site, já conferida. É o único script que uma página
    /// carrega, e só quando o site informou a tag.
    pub tag_do_google: Option<TagDoGoogle>,
}

/// A tag do Google como a página a carrega.
pub struct TagDoGoogle {
    pub id: String,
    /// Contêiner do Tag Manager (`GTM-`); o contrário é propriedade do
    /// Analytics (`G-`), que tem outro script.
    pub eh_gtm: bool,
}

impl TagDoGoogle {
    pub fn nova(id: String) -> Self {
        Self {
            eh_gtm: id.starts_with("GTM-"),
            id,
        }
    }
}

/// A logo do site como o cabeçalho a mostra.
pub struct Logo {
    pub url: String,
    pub largura: u32,
    pub altura: u32,
}

/// A logo do site para o cabeçalho, se ela já tem variante gerada.
pub fn logo_do_cabecalho(site: &Site) -> Option<Logo> {
    let logo = &site.logo;
    let url = url_principal(logo)?;
    (logo.largura > 0 && logo.altura > 0).then(|| Logo {
        url: url.to_string(),
        largura: logo.largura,
        altura: logo.altura,
    })
}

impl Moldura {
    /// Monta o menu que o site definiu: "Início" na frente e as entradas da
    /// lista, na ordem dela. Uma entrada é uma página ou um grupo, que junta
    /// páginas num submenu. O que não está na lista sai do topo e vai para o
    /// rodapé, na ordem em que chegou: é onde ficam a política de
    /// privacidade e as páginas que não são do dia a dia. Sem lista, tudo
    /// fica no topo.
    pub fn ordenar_menu(&mut self, ordem: &[String]) {
        if ordem.is_empty() {
            return;
        }
        let mut soltos = std::mem::take(&mut self.navegacao);
        let mut tirar = |caminho: &str| {
            let caminho = match caminho.trim_end_matches('/') {
                "" => "/",
                sem_barra => sem_barra,
            };
            let posicao = soltos.iter().position(|link| link.url == caminho)?;
            Some(soltos.remove(posicao))
        };
        let mut menu: Vec<Link> = tirar("/").into_iter().collect();
        for entrada in ordem.iter().filter_map(|linha| ler_entrada_do_menu(linha)) {
            match entrada {
                EntradaDoMenu::Pagina(caminho) => menu.extend(tirar(caminho)),
                EntradaDoMenu::Grupo { rotulo, caminhos } => {
                    let filhos: Vec<Link> = caminhos.into_iter().filter_map(&mut tirar).collect();
                    // Grupo sem página no ar não aparece.
                    if !filhos.is_empty() {
                        menu.push(Link {
                            rotulo: rotulo.to_string(),
                            url: String::new(),
                            atual: filhos.iter().any(|filho| filho.atual),
                            filhos,
                        });
                    }
                }
            }
        }
        self.navegacao = menu;
        self.mais_paginas = soltos;
    }
}

impl Moldura {
    pub fn nova(site: &Site, head: String, itens: &[ItemDeNavegacao], caminho_atual: &str) -> Self {
        let mut navegacao = vec![Link {
            rotulo: "Início".into(),
            url: "/".into(),
            atual: caminho_atual == "/",
            filhos: Vec::new(),
        }];
        navegacao.extend(
            itens
                .iter()
                .filter(|item| !item.eh_post && !item.eh_item && item.caminho != "/")
                .map(|item| Link {
                    rotulo: item.titulo.clone(),
                    url: item.caminho.clone(),
                    atual: item.caminho == caminho_atual,
                    filhos: Vec::new(),
                }),
        );
        // Site com produto no ar ganha o catálogo no menu, antes do blog.
        if itens.iter().any(|item| item.eh_item) {
            navegacao.push(Link {
                rotulo: "Produtos".into(),
                url: CAMINHO_DOS_PRODUTOS.into(),
                atual: caminho_atual.starts_with(CAMINHO_DOS_PRODUTOS),
                filhos: Vec::new(),
            });
        }
        if itens.iter().any(|item| item.eh_post) {
            navegacao.push(Link {
                rotulo: "Blog".into(),
                url: "/blog".into(),
                atual: caminho_atual.starts_with("/blog"),
                filhos: Vec::new(),
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
            mais_paginas: Vec::new(),
            rodape,
            estilo_das_secoes: ESTILO_DAS_SECOES,
            estilo_do_tema: "",
            cor_de_destaque: None,
            logo: None,
            icone: None,
            icone_apple: None,
            tag_do_google: None,
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
    /// O endereço do visualizador de modelo 3D, só na página que tem o bloco.
    pub visualizador_3d: Option<&'static str>,
    pub cardapio: Option<crate::cardapio::Cardapio>,
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
                filhos: Vec::new(),
            }),
            // A capa é a única imagem com prioridade de carregamento.
            capa: documento
                .capa()
                .map(|capa| renderizar_midia(capa, Papel::Principal, "100vw"))
                .unwrap_or_default(),
            corpo: renderizar_corpo(documento.corpo()),
            autor: post.map(|post| CaixaDoAutor::nova(&post.autor)),
            visualizador_3d: documento
                .tem_modelo_3d()
                .then(crate::modelos::caminho_do_visualizador),
            cardapio: None,
        }
    }
}

/// O endereço do catálogo já filtrado por uma característica, como
/// `/produtos?linha=peso-livre`.
pub fn caminho_do_filtro(nome: &str, valor: &str) -> String {
    format!(
        "{CAMINHO_DOS_PRODUTOS}?{}={}",
        normalizar_slug(nome),
        normalizar_slug(valor)
    )
}

/// Uma linha da ficha técnica na página do produto.
pub struct LinhaDaFicha {
    pub nome: String,
    pub valor: String,
    /// Na característica que separa o catálogo, os outros produtos iguais.
    pub filtro: Option<String>,
}

/// A página de um produto de catálogo.
#[derive(Template)]
#[template(path = "item.html")]
pub struct PaginaDeItem {
    pub moldura: Moldura,
    pub trilha: Vec<ItemTrilha>,
    pub nome: String,
    pub codigo: Option<String>,
    pub resumo: String,
    /// O que fica em destaque ao lado do texto: o modelo 3D ou, sem ele, a
    /// foto principal.
    pub destaque: String,
    /// As outras fotos, já em HTML.
    pub fotos: Vec<String>,
    pub ficha: Vec<LinhaDaFicha>,
    pub acao: Option<Link>,
    pub corpo: String,
    pub visualizador_3d: Option<&'static str>,
}

impl PaginaDeItem {
    pub fn nova(moldura: Moldura, documento: &Documento, item: &Item) -> Self {
        let sizes = "(min-width: 60rem) 36rem, 100vw";
        let destaque = match &item.modelo {
            // Com modelo, a primeira foto é a capa dele: aparece até o modelo
            // carregar.
            Some(modelo) => renderizar_corpo(&[Bloco::Modelo3d {
                modelo: modelo.clone(),
                capa: item.imagens.first().cloned(),
                legenda: None,
            }]),
            None => item
                .imagens
                .first()
                .map(|foto| renderizar_midia(foto, Papel::Principal, sizes))
                .unwrap_or_default(),
        };
        Self {
            moldura,
            trilha: documento.trilha().to_vec(),
            nome: item.nome.clone(),
            codigo: item
                .codigo
                .clone()
                .filter(|codigo| !codigo.trim().is_empty()),
            resumo: item.resumo.clone(),
            destaque,
            fotos: item
                .imagens
                .iter()
                .skip(1)
                .map(|foto| {
                    renderizar_midia(foto, Papel::Conteudo, "(min-width: 60rem) 18rem, 50vw")
                })
                .collect(),
            ficha: item
                .ficha
                .iter()
                .map(|linha| LinhaDaFicha {
                    nome: linha.nome.clone(),
                    valor: linha.valor.clone(),
                    filtro: linha
                        .filtro
                        .then(|| caminho_do_filtro(&linha.nome, &linha.valor)),
                })
                .collect(),
            acao: item.acao.as_ref().map(|acao| Link {
                rotulo: acao.rotulo.clone(),
                url: acao.url.clone(),
                atual: false,
                filhos: Vec::new(),
            }),
            corpo: renderizar_corpo(&item.corpo),
            visualizador_3d: documento
                .tem_modelo_3d()
                .then(crate::modelos::caminho_do_visualizador),
        }
    }
}

/// Um produto na listagem do catálogo.
pub struct CartaoDeItem {
    pub url: String,
    pub nome: String,
    pub codigo: Option<String>,
    pub resumo: String,
    /// A foto principal, já em HTML. Vazia quando o produto não tem foto.
    pub foto: String,
    /// Os valores das características que separam o catálogo.
    pub marcas: Vec<String>,
}

impl CartaoDeItem {
    pub fn novo(caminho: &str, item: &Item) -> Self {
        Self {
            url: caminho.to_string(),
            nome: item.nome.clone(),
            codigo: item
                .codigo
                .clone()
                .filter(|codigo| !codigo.trim().is_empty()),
            resumo: item.resumo.clone(),
            foto: item
                .imagens
                .first()
                .map(|foto| {
                    renderizar_midia(
                        foto,
                        Papel::Conteudo,
                        "(min-width: 60rem) 22rem, (min-width: 36rem) 50vw, 100vw",
                    )
                })
                .unwrap_or_default(),
            marcas: item
                .ficha
                .iter()
                .filter(|linha| linha.filtro)
                .map(|linha| linha.valor.clone())
                .collect(),
        }
    }
}

/// Uma opção de um filtro do catálogo.
pub struct OpcaoDeFiltro {
    pub rotulo: String,
    pub url: String,
    pub total: usize,
    pub escolhida: bool,
}

/// Um filtro do catálogo: a característica e os valores que ela tem.
pub struct Filtro {
    pub nome: String,
    /// O catálogo sem este filtro (os outros continuam).
    pub todos: String,
    /// Nenhum valor deste filtro está escolhido.
    pub livre: bool,
    pub opcoes: Vec<OpcaoDeFiltro>,
}

/// A listagem do catálogo.
#[derive(Template)]
#[template(path = "produtos.html")]
pub struct PaginaDoCatalogo {
    pub moldura: Moldura,
    pub titulo: String,
    pub filtros: Vec<Filtro>,
    /// Quantos produtos a escolha achou, por extenso.
    pub resumo: String,
    /// Há filtro escolhido: a página oferece "limpar".
    pub filtrado: bool,
    pub cartoes: Vec<CartaoDeItem>,
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
    /// Os posts desta página da lista.
    pub cartoes: Vec<Cartao>,
    pub pagina: usize,
    pub paginas: usize,
    /// Os endereços das páginas vizinhas, quando existem.
    pub anterior: Option<String>,
    pub proxima: Option<String>,
}

#[derive(Template)]
#[template(path = "aviso.html")]
pub struct PaginaDeAviso {
    pub moldura: Moldura,
    pub titulo: String,
    pub mensagem: String,
}

#[cfg(test)]
mod testes {
    use super::*;

    fn link(url: &str) -> Link {
        Link {
            rotulo: url.to_string(),
            url: url.to_string(),
            atual: false,
            filhos: Vec::new(),
        }
    }

    #[test]
    fn menu_segue_a_ordem_do_site_e_o_resto_vai_para_o_rodape() {
        let mut moldura = Moldura {
            idioma: String::new(),
            head: String::new(),
            nome_do_site: String::new(),
            navegacao: [
                "/",
                "/comparativos",
                "/contato",
                "/segmentos",
                "/servicos",
                "/blog",
            ]
            .map(link)
            .into(),
            mais_paginas: Vec::new(),
            rodape: Vec::new(),
            estilo_das_secoes: "",
            estilo_do_tema: "",
            cor_de_destaque: None,
            logo: None,
            icone: None,
            icone_apple: None,
            tag_do_google: None,
        };
        moldura.ordenar_menu(&["/servicos/".to_string(), "/blog".to_string()]);
        let ordem: Vec<&str> = moldura.navegacao.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(ordem, ["/", "/servicos", "/blog"]);
        let fora: Vec<&str> = moldura
            .mais_paginas
            .iter()
            .map(|l| l.url.as_str())
            .collect();
        assert_eq!(fora, ["/comparativos", "/contato", "/segmentos"]);

        // Um grupo junta páginas num submenu e as tira do rodapé.
        moldura.navegacao.append(&mut moldura.mais_paginas);
        moldura
            .navegacao
            .iter_mut()
            .for_each(|l| l.atual = l.url == "/contato");
        moldura.ordenar_menu(&[
            "/servicos".to_string(),
            "Empresa: /contato /segmentos /nao-existe".to_string(),
            "Vazio: /nao-existe".to_string(),
        ]);
        let ordem: Vec<&str> = moldura
            .navegacao
            .iter()
            .map(|l| l.rotulo.as_str())
            .collect();
        assert_eq!(ordem, ["/", "/servicos", "Empresa"]);
        let grupo = &moldura.navegacao[2];
        assert!(grupo.atual);
        let filhos: Vec<&str> = grupo.filhos.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(filhos, ["/contato", "/segmentos"]);
        let fora: Vec<&str> = moldura
            .mais_paginas
            .iter()
            .map(|l| l.url.as_str())
            .collect();
        assert_eq!(fora, ["/blog", "/comparativos"]);

        // Sem ordem definida, nada sai do topo.
        let grupo = moldura.navegacao.pop().expect("grupo");
        moldura.navegacao.extend(grupo.filhos);
        moldura.navegacao.append(&mut moldura.mais_paginas);
        moldura.ordenar_menu(&[]);
        assert_eq!(moldura.navegacao.len(), 6);
        assert!(moldura.mais_paginas.is_empty());
    }
}
