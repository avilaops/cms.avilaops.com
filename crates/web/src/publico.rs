//! O site público: do host ao que está publicado.

use askama::Template;
use axum::http::header::COOKIE;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use cms_dados::{Ausencia, SiteGravado};
use cms_dominio::eventos::origem_do_site;
use cms_dominio::site::aparece_na_busca;
use cms_dominio::{Endereco, Host, Situacao, classificar, ler_host};
use motor_web::descoberta::{
    ItensDoSite, OpcoesRobots, PaginaDeCategoria, gerar_llms, gerar_llms_completo, gerar_robots,
    gerar_sitemaps,
};
use motor_web::seo::montar_cabecalho;
use motor_web::tipos::{Conteudo, Documento, ItemTrilha, Pagina, Post, Seo, Site};
use motor_web::validacao::normalizar_slug;

use crate::Estado;
use crate::midia;
use crate::resposta::{ErroWeb, html, redirecionar, simples, texto};
use crate::visao::{
    CaixaDoAutor, Cartao, Moldura, PaginaDeAviso, PaginaDeDocumento, PaginaDoBlog,
    caminho_da_categoria, caminho_do_autor,
};

const CAMINHO_DO_BLOG: &str = "/blog";

/// O site que respondeu um pedido. Vai junto da resposta para o cache saber
/// de quem é a página.
#[derive(Debug, Clone, Copy)]
struct SiteDoPedido(uuid::Uuid);

/// A chave do cache para um pedido, se ele pode ser servido do cache: só
/// leitura, sem sessão do painel, e nunca arquivo de imagem.
fn chave_de_cache(metodo: &Method, cabecalhos: &HeaderMap, uri: &Uri) -> Option<String> {
    let com_sessao = cabecalhos
        .get_all(COOKIE)
        .iter()
        .filter_map(|valor| valor.to_str().ok())
        .any(|valor| valor.contains("avila_sso="));
    if metodo != Method::GET || com_sessao || uri.path().starts_with("/midia/") {
        return None;
    }
    let host = host_do_pedido(cabecalhos)?;
    let caminho = uri.path_and_query().map_or("/", |valor| valor.as_str());
    Some(format!("{}{caminho}", host.com_porta))
}

pub async fn atender(
    estado: &Estado,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    uri: &Uri,
) -> Response {
    if metodo != Method::GET && metodo != Method::HEAD {
        return simples(StatusCode::METHOD_NOT_ALLOWED, "Método não permitido.");
    }
    let chave = chave_de_cache(metodo, cabecalhos, uri);
    if let Some(guardada) = chave
        .as_deref()
        .and_then(|chave| estado.cache.buscar(chave))
    {
        return guardada;
    }
    let resposta = match responder(estado, cabecalhos, uri).await {
        Ok(resposta) => resposta,
        Err(erro) => {
            tracing::error!(%erro, caminho = uri.path(), "falha ao atender o pedido");
            return simples(StatusCode::INTERNAL_SERVER_ERROR, "Erro interno.");
        }
    };
    let site = resposta.extensions().get::<SiteDoPedido>().copied();
    match (chave, site) {
        (Some(chave), Some(SiteDoPedido(site_id))) if resposta.status() == StatusCode::OK => {
            let (partes, corpo) = resposta.into_parts();
            match axum::body::to_bytes(corpo, usize::MAX).await {
                Ok(bytes) => estado.cache.guardar(chave, site_id, partes.headers, bytes),
                Err(erro) => {
                    tracing::error!(%erro, "corpo da página não pôde ser lido");
                    simples(StatusCode::INTERNAL_SERVER_ERROR, "Erro interno.")
                }
            }
        }
        _ => resposta,
    }
}

/// O servidor fica atrás do Caddy, que repassa o host original.
pub(crate) fn host_do_pedido(cabecalhos: &HeaderMap) -> Option<Host> {
    ["x-forwarded-host", "host"]
        .into_iter()
        .find_map(|nome| cabecalhos.get(nome))
        .and_then(|valor| valor.to_str().ok())
        .and_then(ler_host)
}

/// O site de um pedido, já com o endereço em que ele respondeu.
struct Pedido {
    gravado: SiteGravado,
    site: Site,
    indexavel: bool,
}

async fn responder(
    estado: &Estado,
    cabecalhos: &HeaderMap,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let configuracao = &estado.configuracao;
    let Some(host) = host_do_pedido(cabecalhos) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Site não encontrado."));
    };
    let caminho_e_consulta = uri.path_and_query().map_or("/", |valor| valor.as_str());

    let endereco = classificar(&host, &configuracao.dominio_base);
    let gravado = match &endereco {
        Endereco::Provisorio { slug } => cms_dados::site_por_slug(&estado.pool, slug).await?,
        Endereco::Proprio { host } => cms_dados::site_por_dominio(&estado.pool, host).await?,
    };
    let Some(gravado) = gravado else {
        // `www.` de um domínio conhecido vai para o endereço sem `www.`.
        if let Some(sem_www) = host.nome.strip_prefix("www.") {
            if cms_dados::site_por_dominio(&estado.pool, sem_www)
                .await?
                .is_some()
            {
                let destino = host.com_porta.trim_start_matches("www.");
                return Ok(redirecionar(
                    StatusCode::PERMANENT_REDIRECT,
                    &format!("{}://{destino}{caminho_e_consulta}", configuracao.esquema),
                ));
            }
        }
        return Ok(simples(StatusCode::NOT_FOUND, "Site não encontrado."));
    };

    if gravado.situacao == Situacao::Suspenso {
        return Ok(simples(
            StatusCode::GONE,
            "Este site não está mais disponível.",
        ));
    }
    // Com domínio próprio, o endereço provisório deixa de responder conteúdo.
    if let (Endereco::Provisorio { .. }, Some(dominio)) = (&endereco, &gravado.dominio_ativo) {
        return Ok(redirecionar(
            StatusCode::MOVED_PERMANENTLY,
            &format!("{}://{dominio}{caminho_e_consulta}", configuracao.esquema),
        ));
    }

    let origem = format!("{}://{}", configuracao.esquema, host.com_porta);
    let indexavel = aparece_na_busca(
        gravado.situacao,
        gravado.dominio_ativo.is_some(),
        gravado.provisorio_definitivo,
    );
    let pedido = Pedido {
        site: gravado.perfil.para_site(&origem),
        gravado,
        indexavel,
    };

    let caminho = uri.path();
    let mut resposta = match caminho {
        "/robots.txt" => Ok(robots(&pedido)),
        "/llms.txt" | "/llms-full.txt" => descoberta_para_ia(estado, &pedido, caminho).await,
        _ if eh_arquivo_do_indexnow(caminho, configuracao.chave_do_indexnow.as_deref()) => {
            Ok(texto(
                StatusCode::OK,
                "text/plain; charset=utf-8",
                configuracao.chave_do_indexnow.clone().unwrap_or_default(),
            ))
        }
        _ if caminho.starts_with("/sitemap") && caminho.ends_with(".xml") => {
            sitemap(estado, &pedido, caminho).await
        }
        _ => match caminho.strip_prefix("/midia/") {
            Some(arquivo) => {
                midia::servir(&configuracao.diretorio_de_midia, pedido.gravado.id, arquivo).await
            }
            None => match Recorte::do_caminho(caminho) {
                Some(recorte) => listagem(estado, &pedido, recorte).await,
                None => documento(estado, &pedido, caminho).await,
            },
        },
    }?;
    resposta
        .extensions_mut()
        .insert(SiteDoPedido(pedido.gravado.id));
    Ok(resposta)
}

/// `/<chave>.txt`, com a chave no conteúdo: a prova que o IndexNow pede.
fn eh_arquivo_do_indexnow(caminho: &str, chave: Option<&str>) -> bool {
    let arquivo = caminho
        .strip_prefix('/')
        .and_then(|resto| resto.strip_suffix(".txt"));
    chave.is_some() && arquivo == chave
}

fn robots(pedido: &Pedido) -> Response {
    let corpo = if pedido.indexavel {
        gerar_robots(&pedido.site, &OpcoesRobots::default())
    } else {
        // Site em montagem ou sem endereço definitivo fica fora da busca.
        "User-agent: *\nDisallow: /\n".to_string()
    };
    texto(StatusCode::OK, "text/plain; charset=utf-8", corpo)
}

/// Os posts no ar, do mais recente para trás; empate fica em ordem de caminho.
fn posts_em_ordem(documentos: &[Documento]) -> Vec<(&str, &Post)> {
    let mut posts: Vec<_> = documentos
        .iter()
        .filter_map(|documento| match &documento.conteudo {
            Conteudo::Post(post) => Some((documento.caminho.as_str(), post)),
            _ => None,
        })
        .collect();
    posts.sort_by(|a, b| {
        b.1.publicado_em
            .cmp(&a.1.publicado_em)
            .then_with(|| a.0.cmp(b.0))
    });
    posts
}

/// O blog, cada categoria e cada autor entram no sitemap como páginas de
/// listagem, com a data do post mexido por último em cada uma.
fn listagens(documentos: &[Documento]) -> Vec<PaginaDeCategoria> {
    let mut paginas: BTreeMap<String, PaginaDeCategoria> = BTreeMap::new();
    for (_, post) in posts_em_ordem(documentos) {
        let de_listagem = [
            (CAMINHO_DO_BLOG.to_string(), "Blog"),
            (
                caminho_da_categoria(&post.categoria.slug),
                &*post.categoria.nome,
            ),
            (caminho_do_autor(&post.autor.slug), &*post.autor.nome),
        ];
        for (caminho, nome) in de_listagem {
            // O nome é o do post mais recente: é o primeiro a chegar.
            let pagina = paginas
                .entry(caminho.clone())
                .or_insert_with(|| PaginaDeCategoria {
                    caminho,
                    nome: nome.to_string(),
                    atualizado_em: post.atualizado_em,
                });
            pagina.atualizado_em = pagina.atualizado_em.max(post.atualizado_em);
        }
    }
    paginas.into_values().collect()
}

async fn sitemap(estado: &Estado, pedido: &Pedido, caminho: &str) -> Result<Response, ErroWeb> {
    if !pedido.indexavel {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    }
    let documentos = cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id).await?;
    let categorias = listagens(&documentos);
    let itens = ItensDoSite {
        documentos: &documentos,
        categorias: &categorias,
        destaques: &[],
    };
    let pedido_arquivo = caminho.trim_start_matches('/');
    match gerar_sitemaps(&pedido.site, &itens)
        .into_iter()
        .find(|arquivo| arquivo.caminho == pedido_arquivo)
    {
        Some(arquivo) => Ok(texto(
            StatusCode::OK,
            "application/xml; charset=utf-8",
            arquivo.conteudo,
        )),
        None => Ok(simples(StatusCode::NOT_FOUND, "Não encontrado.")),
    }
}

async fn descoberta_para_ia(
    estado: &Estado,
    pedido: &Pedido,
    caminho: &str,
) -> Result<Response, ErroWeb> {
    if !pedido.indexavel {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    }
    let documentos = cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id).await?;
    let itens = ItensDoSite {
        documentos: &documentos,
        categorias: &[],
        destaques: &[],
    };
    let corpo = if caminho == "/llms.txt" {
        gerar_llms(&pedido.site, &itens)
    } else {
        gerar_llms_completo(&pedido.site, &itens)
    };
    Ok(texto(StatusCode::OK, "text/markdown; charset=utf-8", corpo))
}

/// Site fora da busca: todo documento sai com `noindex`, diga o que disser o
/// cadastro dele.
fn ajustar_indexacao(documento: &mut Documento, indexavel: bool) {
    if indexavel {
        return;
    }
    match &mut documento.conteudo {
        Conteudo::Pagina(pagina) => pagina.seo.indexar = false,
        Conteudo::Post(post) => post.seo.indexar = false,
        Conteudo::Produto(produto) => produto.seo.indexar = false,
    }
}

async fn moldura(
    estado: &Estado,
    pedido: &Pedido,
    documento: &Documento,
) -> Result<Moldura, ErroWeb> {
    let head = montar_cabecalho(documento, &pedido.site)?.para_html();
    let itens = cms_dados::navegacao(&estado.pool, pedido.gravado.id).await?;
    Ok(Moldura::nova(
        &pedido.site,
        head,
        &itens,
        &documento.caminho,
    ))
}

async fn documento(estado: &Estado, pedido: &Pedido, caminho: &str) -> Result<Response, ErroWeb> {
    // Um endereço só por página: a barra final redireciona para o sem barra.
    if caminho.len() > 1 && caminho.ends_with('/') {
        return Ok(redirecionar(
            StatusCode::PERMANENT_REDIRECT,
            caminho.trim_end_matches('/'),
        ));
    }
    let Some(mut documento) =
        cms_dados::documento_publicado(&estado.pool, pedido.gravado.id, caminho).await?
    else {
        return match cms_dados::ausencia(&estado.pool, pedido.gravado.id, caminho).await? {
            Ausencia::Redirecionado(destino) => {
                Ok(redirecionar(StatusCode::MOVED_PERMANENTLY, &destino))
            }
            Ausencia::Despublicado => saiu_do_ar(estado, pedido).await,
            Ausencia::Inexistente => nao_encontrado(estado, pedido).await,
        };
    };
    ajustar_indexacao(&mut documento, pedido.indexavel);

    let moldura = moldura(estado, pedido, &documento).await?;
    let pagina = PaginaDeDocumento::nova(moldura, &documento).render()?;
    Ok(html(
        StatusCode::OK,
        pagina,
        pedido.indexavel && documento.seo().indexar,
    ))
}

/// Uma página que não está gravada: a listagem do blog e os avisos. Passa
/// pelo motor como qualquer outra, para ter cabeçalho e canônica iguais.
fn pagina_avulsa(
    pedido: &Pedido,
    caminho: &str,
    titulo: &str,
    descricao: String,
    indexar: bool,
    datas: (DateTime<Utc>, DateTime<Utc>),
    acima: Option<ItemTrilha>,
) -> Documento {
    let inicio = ItemTrilha {
        nome: "Início".into(),
        url: "/".into(),
    };
    let aqui = ItemTrilha {
        nome: titulo.to_string(),
        url: caminho.to_string(),
    };
    Documento {
        caminho: caminho.to_string(),
        conteudo: Conteudo::Pagina(Pagina {
            // O endereço é o caminho; o slug só precisa passar pela validação.
            slug: normalizar_slug(caminho),
            titulo: titulo.to_string(),
            menu: None,
            corpo: Vec::new(),
            capa: None,
            abertura: None,
            publicado_em: datas.0,
            atualizado_em: datas.1,
            seo: Seo {
                titulo: format!("{titulo} | {}", pedido.site.nome),
                descricao,
                indexar: indexar && pedido.indexavel,
                imagem_social: None,
            },
            trilha: [Some(inicio), acima, Some(aqui)]
                .into_iter()
                .flatten()
                .collect(),
        }),
    }
}

/// Uma listagem de posts: o blog inteiro, uma categoria ou quem escreveu.
enum Recorte<'a> {
    Blog,
    Categoria(&'a str),
    Autor(&'a str),
}

impl<'a> Recorte<'a> {
    fn do_caminho(caminho: &'a str) -> Option<Self> {
        if caminho == CAMINHO_DO_BLOG {
            return Some(Self::Blog);
        }
        let (slug, recorte): (_, fn(&'a str) -> Self) =
            match caminho.strip_prefix("/blog/categoria/") {
                Some(slug) => (slug, Self::Categoria),
                None => (caminho.strip_prefix("/autor/")?, Self::Autor),
            };
        // Com barra no fim, quem responde é o redirecionamento dos documentos.
        (!slug.is_empty() && !slug.contains('/')).then(|| recorte(slug))
    }

    fn inclui(&self, post: &Post) -> bool {
        match self {
            Self::Blog => true,
            Self::Categoria(slug) => post.categoria.slug == *slug,
            Self::Autor(slug) => post.autor.slug == *slug,
        }
    }
}

/// O nome da categoria e os dados de quem escreveu são os do post mais
/// recente: o que foi ao ar por último é o que vale.
async fn listagem(
    estado: &Estado,
    pedido: &Pedido,
    recorte: Recorte<'_>,
) -> Result<Response, ErroWeb> {
    let documentos = cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id).await?;
    let posts: Vec<_> = posts_em_ordem(&documentos)
        .into_iter()
        .filter(|(_, post)| recorte.inclui(post))
        .collect();
    let (Some((_, recente)), Some(primeiro), Some(ultimo)) = (
        posts.first(),
        posts.iter().map(|(_, post)| post.publicado_em).min(),
        posts.iter().map(|(_, post)| post.atualizado_em).max(),
    ) else {
        return nao_encontrado(estado, pedido).await;
    };

    let site = &pedido.site.nome;
    let blog = ItemTrilha {
        nome: "Blog".into(),
        url: CAMINHO_DO_BLOG.into(),
    };
    let (caminho, titulo, descricao, acima, autor) = match recorte {
        Recorte::Blog => (
            CAMINHO_DO_BLOG.to_string(),
            "Blog",
            format!("Artigos e guias de {site}."),
            None,
            None,
        ),
        Recorte::Categoria(slug) => (
            caminho_da_categoria(slug),
            &*recente.categoria.nome,
            format!(
                "Artigos e guias de {site} sobre {}.",
                recente.categoria.nome
            ),
            Some(blog),
            None,
        ),
        Recorte::Autor(slug) => (
            caminho_do_autor(slug),
            &*recente.autor.nome,
            format!("Quem é {} e o que escreveu em {site}.", recente.autor.nome),
            Some(blog),
            Some(CaixaDoAutor::nova(&recente.autor)),
        ),
    };
    let documento = pagina_avulsa(
        pedido,
        &caminho,
        titulo,
        descricao,
        true,
        (primeiro, ultimo),
        acima,
    );
    let moldura = moldura(estado, pedido, &documento).await?;
    let pagina = PaginaDoBlog {
        moldura,
        // O blog é a raiz da própria trilha: só as páginas abaixo dele mostram.
        trilha: if caminho == CAMINHO_DO_BLOG {
            Vec::new()
        } else {
            documento.trilha().to_vec()
        },
        titulo: titulo.to_string(),
        autor,
        cartoes: posts
            .iter()
            .map(|(caminho, post)| Cartao::do_post(caminho, post))
            .collect(),
    }
    .render()?;
    Ok(html(StatusCode::OK, pagina, pedido.indexavel))
}

/// A página como o visitante a veria, montada a partir de um rascunho. Só lê:
/// não grava e sai sempre fora da busca.
pub(crate) async fn previa(
    estado: &Estado,
    gravado: SiteGravado,
    mut documento: Documento,
) -> Result<String, ErroWeb> {
    let configuracao = &estado.configuracao;
    let origem = origem_do_site(
        &configuracao.esquema,
        &configuracao.dominio_base,
        &gravado.slug,
        gravado.dominio_ativo.as_deref(),
    );
    let pedido = Pedido {
        site: gravado.perfil.para_site(&origem),
        gravado,
        indexavel: false,
    };
    ajustar_indexacao(&mut documento, false);
    let moldura = moldura(estado, &pedido, &documento).await?;
    Ok(PaginaDeDocumento::nova(moldura, &documento).render()?)
}

/// O que o visitante lê quando o endereço não tem página.
struct Aviso {
    status: StatusCode,
    caminho: &'static str,
    titulo: &'static str,
    descricao: &'static str,
    mensagem: &'static str,
}

async fn avisar(estado: &Estado, pedido: &Pedido, aviso: Aviso) -> Result<Response, ErroWeb> {
    let agora = Utc::now();
    let documento = pagina_avulsa(
        pedido,
        aviso.caminho,
        aviso.titulo,
        aviso.descricao.into(),
        false,
        (agora, agora),
        None,
    );
    let moldura = moldura(estado, pedido, &documento).await?;
    let pagina = PaginaDeAviso {
        moldura,
        titulo: aviso.titulo.into(),
        mensagem: aviso.mensagem.into(),
    }
    .render()?;
    Ok(html(aviso.status, pagina, false))
}

async fn nao_encontrado(estado: &Estado, pedido: &Pedido) -> Result<Response, ErroWeb> {
    let aviso = Aviso {
        status: StatusCode::NOT_FOUND,
        caminho: "/pagina-nao-encontrada",
        titulo: "Página não encontrada",
        descricao: "O endereço pedido não existe neste site.",
        mensagem: "O endereço que você abriu não existe ou saiu do ar.",
    };
    avisar(estado, pedido, aviso).await
}

/// 410 diz ao buscador que a página saiu de vez, e ele a tira do índice mais
/// rápido do que com 404.
async fn saiu_do_ar(estado: &Estado, pedido: &Pedido) -> Result<Response, ErroWeb> {
    let aviso = Aviso {
        status: StatusCode::GONE,
        caminho: "/pagina-removida",
        titulo: "Página removida",
        descricao: "Esta página foi tirada do ar.",
        mensagem: "O conteúdo que estava neste endereço foi tirado do ar.",
    };
    avisar(estado, pedido, aviso).await
}
