//! O site público: do host ao que está publicado.

use askama::Template;
use axum::http::header::{COOKIE, ORIGIN};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use cms_dados::{Ausencia, SiteGravado};
use cms_dominio::eventos::CampoRecebido;
use cms_dominio::eventos::origem_do_site;
use cms_dominio::fluxo::CAMINHO_DOS_PRODUTOS;
use cms_dominio::redirecionamento::{SITE_INTEIRO, destino_por_prefixo};
use cms_dominio::site::{
    CAMINHO_DA_LICENCA_DE_IMAGENS, Tema, aparece_na_busca, conferir_cor, conferir_tag_do_google,
};
use cms_dominio::{Endereco, Host, Situacao, classificar, ler_host};
use motor_web::descoberta::{
    ItensDoSite, OpcoesRobots, PaginaDeCategoria, gerar_llms, gerar_llms_completo, gerar_robots,
    gerar_sitemaps,
};
use motor_web::seo::montar_cabecalho;
use motor_web::tipos::{
    Bloco, Conteudo, Documento, Item, ItemTrilha, Pagina, Post, Seo, Site, TipoDeCampo, Trecho,
};
use motor_web::validacao::normalizar_slug;

use crate::Estado;
use crate::icone::{self, Icone};
use crate::midia;
use crate::modelos;
use crate::resposta::{
    ErroWeb, html, liberar_script_proprio, liberar_tag_do_google, redirecionar, simples, texto,
};
use crate::tema;
use crate::visao::logo_do_cabecalho;
use crate::visao::{
    CaixaDoAutor, Cartao, CartaoDeItem, Filtro, Moldura, OpcaoDeFiltro, PaginaDeAviso,
    PaginaDeDocumento, PaginaDeItem, PaginaDoBlog, PaginaDoCatalogo, TagDoGoogle,
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
    Some(format!(
        "{}{caminho}|{}",
        host.com_porta,
        crate::descoberta::prefere_markdown(cabecalhos)
    ))
}

pub async fn atender(
    estado: &Estado,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    uri: &Uri,
    corpo: &[u8],
) -> Response {
    // O único envio que um site aceita é o de um formulário da própria página.
    if metodo == Method::POST {
        return match responder(estado, cabecalhos, uri, Some(corpo)).await {
            Ok(resposta) => resposta,
            Err(erro) => {
                tracing::error!(%erro, caminho = uri.path(), "falha ao receber o formulário");
                simples(StatusCode::INTERNAL_SERVER_ERROR, "Erro interno.")
            }
        };
    }
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
    let resposta = match responder(estado, cabecalhos, uri, None).await {
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
    envio: Option<&[u8]>,
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
    // Site aposentado: tudo vai para o endereço novo, antes de qualquer página.
    // Com /* no destino, caminho e consulta vão junto.
    if let Some(para) = gravado.mudou_para.as_deref() {
        if let Some(destino) = destino_por_prefixo(SITE_INTEIRO, para, uri.path()) {
            let destino = match uri.query() {
                Some(consulta) if para.ends_with('*') => format!("{destino}?{consulta}"),
                _ => destino,
            };
            return Ok(redirecionar(StatusCode::MOVED_PERMANENTLY, &destino));
        }
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
    if let Some(corpo) = envio {
        return receber_formulario(estado, &pedido, cabecalhos, caminho, corpo).await;
    }
    let tem_tag = pedido.gravado.perfil.tag_do_google.is_some();
    if let Some(mut resposta) = agradecer(estado, &pedido, uri).await? {
        // A página de agradecimento é onde a tag conta o formulário enviado.
        if tem_tag {
            liberar_tag_do_google(&mut resposta);
        }
        return Ok(resposta);
    }
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
        _ if caminho.starts_with("/recursos/") => Ok(modelos::visualizador(caminho)
            .or_else(|| tema::fonte(caminho))
            .or_else(|| crate::cardapio::recurso(caminho))
            .unwrap_or_else(|| simples(StatusCode::NOT_FOUND, "Não encontrado."))),
        _ => match (caminho.strip_prefix("/midia/"), Icone::do_caminho(caminho)) {
            (Some(arquivo), _) => {
                let diretorio = &configuracao.diretorio_de_midia;
                match arquivo.strip_prefix("modelos/") {
                    Some(modelo) => modelos::servir(diretorio, pedido.gravado.id, modelo).await,
                    None => midia::servir(diretorio, pedido.gravado.id, arquivo).await,
                }
            }
            (None, Some(icone_pedido)) => {
                let perfil = &pedido.gravado.perfil;
                let origem = if icone_pedido.eh_do_iphone() {
                    perfil.origem_do_icone_apple()
                } else {
                    perfil.origem_do_favicon()
                };
                icone::servir(
                    &configuracao.diretorio_de_midia,
                    pedido.gravado.id,
                    origem,
                    icone_pedido,
                )
                .await
            }
            (None, None) => match Recorte::do_caminho(caminho) {
                Some((recorte, pagina)) => listagem(estado, &pedido, recorte, pagina).await,
                None if caminho == CAMINHO_DOS_PRODUTOS => {
                    catalogo(estado, &pedido, uri.query().unwrap_or_default()).await
                }
                None => {
                    documento(
                        estado,
                        &pedido,
                        caminho,
                        crate::descoberta::prefere_markdown(cabecalhos),
                    )
                    .await
                }
            },
        },
    }?;
    if pedido.gravado.perfil.tema == Tema::Cardapio {
        crate::descoberta::marcar(&mut resposta, pedido.indexavel);
    }
    if tem_tag {
        liberar_tag_do_google(&mut resposta);
    }
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
    // O catálogo entra uma vez, com a data do produto mexido por último.
    let produtos = documentos.iter().filter_map(|d| match &d.conteudo {
        Conteudo::Item(item) => Some(item.atualizado_em),
        _ => None,
    });
    if let Some(atualizado_em) = produtos.max() {
        paginas.insert(
            CAMINHO_DOS_PRODUTOS.to_string(),
            PaginaDeCategoria {
                caminho: CAMINHO_DOS_PRODUTOS.to_string(),
                nome: "Produtos".to_string(),
                atualizado_em,
            },
        );
    }
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
        Conteudo::Item(item) => item.seo.indexar = false,
        Conteudo::Produto(produto) => produto.seo.indexar = false,
    }
}

/// A versão que vai no endereço do ícone: o começo do identificador da
/// imagem de que ele sai. Sem imagem válida, não há ícone.
fn versao_do_icone(origem: Option<&str>) -> Option<String> {
    origem
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .map(|id| id.simple().to_string()[..8].to_string())
}

async fn moldura(
    estado: &Estado,
    pedido: &Pedido,
    documento: &Documento,
) -> Result<Moldura, ErroWeb> {
    let head = montar_cabecalho(documento, &pedido.site)?.para_html();
    let itens = cms_dados::navegacao(&estado.pool, pedido.gravado.id).await?;
    let mut moldura = Moldura::nova(&pedido.site, head, &itens, &documento.caminho);
    // Só entra na página a cor que passa na conferência: o que está gravado
    // pode ser de antes de a regra existir.
    moldura.ordenar_menu(&pedido.gravado.perfil.menu);
    moldura.estilo_do_tema = tema::estilo(pedido.gravado.perfil.tema);
    moldura.logo = logo_do_cabecalho(&pedido.site);
    // Só entra na página a tag que passa na conferência, como a cor.
    moldura.tag_do_google = pedido
        .gravado
        .perfil
        .tag_do_google
        .as_deref()
        .and_then(|tag| conferir_tag_do_google(tag).ok())
        .map(TagDoGoogle::nova);
    // Com favicon ou logo, o site tem ícone. A versão no endereço troca junto
    // com a imagem de que ele sai.
    let perfil = &pedido.gravado.perfil;
    moldura.icone = versao_do_icone(perfil.origem_do_favicon());
    moldura.icone_apple = versao_do_icone(perfil.origem_do_icone_apple());
    // A logo pode ter sido escolhida enquanto ainda era convertida: o que
    // ficou gravado não tem variante, e a biblioteca já tem.
    if moldura.logo.is_none() {
        if let Ok(id) = uuid::Uuid::parse_str(&pedido.site.logo.id) {
            let pronta =
                cms_dados::midia_para_conteudo(&estado.pool, pedido.gravado.id, id).await?;
            moldura.logo = pronta.and_then(|logo| {
                logo_do_cabecalho(&Site {
                    logo,
                    ..pedido.site.clone()
                })
            });
        }
    }
    moldura.cor_de_destaque = pedido
        .gravado
        .perfil
        .cor_de_destaque
        .as_deref()
        .and_then(|cor| conferir_cor(cor).ok());
    // A página pode ter a cor dela, como a de um produto na página que o
    // apresenta. Passa pela mesma conferência que a do site.
    if let Conteudo::Pagina(pagina) = &documento.conteudo {
        if let Some(cor) = pagina
            .destaque
            .as_deref()
            .and_then(|cor| conferir_cor(cor).ok())
        {
            moldura.cor_de_destaque = Some(cor);
        }
    }
    Ok(moldura)
}

async fn documento(
    estado: &Estado,
    pedido: &Pedido,
    caminho: &str,
    markdown: bool,
) -> Result<Response, ErroWeb> {
    // Um endereço só por página: a barra final redireciona para o sem barra.
    if caminho.len() > 1 && caminho.ends_with('/') {
        let sem_barra = caminho.trim_end_matches('/');
        // Endereço antigo com barra que tem regra de redirecionamento vai
        // direto ao destino, em um salto só.
        let sem_pagina = cms_dados::documento_publicado(&estado.pool, pedido.gravado.id, sem_barra)
            .await?
            .is_none();
        if sem_pagina {
            if let Ausencia::Redirecionado(destino) =
                cms_dados::ausencia(&estado.pool, pedido.gravado.id, caminho).await?
            {
                return Ok(redirecionar(StatusCode::MOVED_PERMANENTLY, &destino));
            }
        }
        return Ok(redirecionar(StatusCode::PERMANENT_REDIRECT, sem_barra));
    }
    let Some(mut documento) =
        cms_dados::documento_publicado(&estado.pool, pedido.gravado.id, caminho).await?
    else {
        // A página de licença é do CMS, a não ser que o site publique a dele
        // no mesmo endereço.
        if caminho == CAMINHO_DA_LICENCA_DE_IMAGENS {
            return licenca_de_imagens(estado, pedido).await;
        }
        return match cms_dados::ausencia(&estado.pool, pedido.gravado.id, caminho).await? {
            Ausencia::Redirecionado(destino) => {
                Ok(redirecionar(StatusCode::MOVED_PERMANENTLY, &destino))
            }
            Ausencia::Despublicado => saiu_do_ar(estado, pedido).await,
            Ausencia::Inexistente => nao_encontrado(estado, pedido).await,
        };
    };
    ajustar_indexacao(&mut documento, pedido.indexavel);

    if markdown && pedido.indexavel && pedido.gravado.perfil.tema == Tema::Cardapio {
        let documentos = if caminho == "/" {
            cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id)
                .await?
                .into_iter()
                .filter(|d| {
                    d.caminho == "/"
                        || pedido
                            .gravado
                            .perfil
                            .paginas_do_cardapio
                            .iter()
                            .any(|linha| {
                                cms_dominio::cardapio::ler_categoria(linha)
                                    .is_some_and(|(_, p)| p == d.caminho)
                            })
                })
                .collect()
        } else {
            vec![documento]
        };
        let itens = ItensDoSite {
            documentos: &documentos,
            categorias: &[],
            destaques: &[],
        };
        let mut resposta = texto(
            StatusCode::OK,
            "text/markdown; charset=utf-8",
            gerar_llms_completo(&pedido.site, &itens),
        );
        resposta.headers_mut().insert(
            "cache-control",
            axum::http::HeaderValue::from_static(
                "public, max-age=0, s-maxage=60, stale-while-revalidate=600",
            ),
        );
        return Ok(resposta);
    }

    let moldura = moldura(estado, pedido, &documento).await?;
    let mut tem_cardapio = false;
    let pagina = match &documento.conteudo {
        Conteudo::Item(item) => PaginaDeItem::nova(moldura, &documento, item).render()?,
        _ => {
            let mut pagina = PaginaDeDocumento::nova(moldura, &documento);
            if usa_cardapio(pedido, &documento) {
                let documentos =
                    cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id).await?;
                pagina.cardapio = crate::cardapio::montar(
                    &pedido.gravado.perfil,
                    &documentos,
                    caminho,
                    pedido.gravado.id,
                );
                tem_cardapio = pagina.cardapio.is_some();
            }
            pagina.render()?
        }
    };
    let mut resposta = html(
        StatusCode::OK,
        pagina,
        pedido.indexavel && documento.seo().indexar,
    );
    if documento.tem_modelo_3d() || tem_cardapio {
        liberar_script_proprio(&mut resposta);
    }
    Ok(resposta)
}

fn usa_cardapio(pedido: &Pedido, documento: &Documento) -> bool {
    pedido.gravado.perfil.tema == Tema::Cardapio
        && matches!(documento.conteudo, Conteudo::Pagina(_))
        && (documento.caminho == "/"
            || pedido
                .gravado
                .perfil
                .paginas_do_cardapio
                .iter()
                .any(|linha| {
                    cms_dominio::cardapio::ler_categoria(linha)
                        .is_some_and(|(_, caminho)| caminho == documento.caminho)
                }))
}

/// A página que diz em que termos as imagens do site podem ser usadas e como
/// pedir autorização. É o endereço que a imagem de autoria própria leva como
/// licença, e de onde a busca de imagens tira o selo de imagem licenciável.
async fn licenca_de_imagens(estado: &Estado, pedido: &Pedido) -> Result<Response, ErroWeb> {
    let agora = Utc::now();
    let mut documento = pagina_avulsa(
        pedido,
        CAMINHO_DA_LICENCA_DE_IMAGENS,
        "Licença de imagens",
        format!(
            "Em que termos as imagens de {} podem ser usadas e como pedir autorização.",
            pedido.site.nome
        ),
        true,
        (agora, agora),
        None,
    );
    if let Conteudo::Pagina(pagina) = &mut documento.conteudo {
        pagina.corpo = pedido
            .gravado
            .perfil
            .termos_das_imagens()
            .into_iter()
            .map(|texto| Bloco::Paragrafo {
                trechos: vec![Trecho {
                    texto,
                    ..Trecho::default()
                }],
            })
            .collect();
    }
    let moldura = moldura(estado, pedido, &documento).await?;
    let pagina = PaginaDeDocumento::nova(moldura, &documento).render()?;
    Ok(html(StatusCode::OK, pagina, pedido.indexavel))
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
            destaque: None,
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
                canonica: None,
            },
            trilha: [Some(inicio), acima, Some(aqui)]
                .into_iter()
                .flatten()
                .collect(),
        }),
    }
}

/// O que a pessoa escolheu nos filtros do catálogo: pares de característica
/// e valor, já no formato de endereço. Só o que vem escrito como o catálogo
/// escreve; o resto da consulta é ignorado.
fn filtros_escolhidos(consulta: &str) -> Vec<(String, String)> {
    serde_urlencoded::from_str::<Vec<(String, String)>>(consulta)
        .unwrap_or_default()
        .into_iter()
        .filter(|(nome, valor)| {
            !valor.is_empty() && normalizar_slug(nome) == *nome && normalizar_slug(valor) == *valor
        })
        .collect()
}

/// O catálogo com os filtros dados, na ordem em que o catálogo os mostra.
fn caminho_do_catalogo(escolhidos: &[(String, String)]) -> String {
    match serde_urlencoded::to_string(escolhidos) {
        Ok(consulta) if !consulta.is_empty() => format!("{CAMINHO_DOS_PRODUTOS}?{consulta}"),
        _ => CAMINHO_DOS_PRODUTOS.to_string(),
    }
}

/// Os filtros que o catálogo oferece: cada característica marcada como
/// filtro em algum produto, com os valores que ela tem e quantos produtos há
/// em cada um, contados dentro do que os outros filtros já escolheram.
fn filtros_do_catalogo(itens: &[(&str, &Item)], escolhidos: &[(String, String)]) -> Vec<Filtro> {
    // O nome e os valores aparecem como foram escritos pela primeira vez.
    let mut nomes: Vec<(String, String)> = Vec::new();
    for linha in itens
        .iter()
        .flat_map(|(_, item)| &item.ficha)
        .filter(|l| l.filtro)
    {
        let chave = normalizar_slug(&linha.nome);
        if !chave.is_empty() && !nomes.iter().any(|(existente, _)| *existente == chave) {
            nomes.push((chave, linha.nome.trim().to_string()));
        }
    }
    nomes
        .into_iter()
        .map(|(chave, nome)| {
            let outros: Vec<(String, String)> = escolhidos
                .iter()
                .filter(|(escolhido, _)| *escolhido != chave)
                .cloned()
                .collect();
            let atual = escolhidos
                .iter()
                .find(|(escolhido, _)| *escolhido == chave)
                .map(|(_, valor)| valor.as_str());
            let mut valores: BTreeMap<String, (String, usize)> = BTreeMap::new();
            for (_, item) in itens.iter().filter(|(_, item)| passa(item, &outros)) {
                for linha in item
                    .ficha
                    .iter()
                    .filter(|l| l.filtro && normalizar_slug(&l.nome) == chave)
                {
                    let valor = normalizar_slug(&linha.valor);
                    if !valor.is_empty() {
                        valores
                            .entry(valor)
                            .or_insert_with(|| (linha.valor.trim().to_string(), 0))
                            .1 += 1;
                    }
                }
            }
            Filtro {
                nome,
                todos: caminho_do_catalogo(&outros),
                livre: atual.is_none(),
                opcoes: valores
                    .into_iter()
                    .map(|(valor, (rotulo, total))| {
                        let mut com_este = outros.clone();
                        com_este.push((chave.clone(), valor.clone()));
                        OpcaoDeFiltro {
                            rotulo,
                            url: caminho_do_catalogo(&com_este),
                            total,
                            escolhida: atual == Some(valor.as_str()),
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}

/// O produto tem todas as características escolhidas.
fn passa(item: &Item, escolhidos: &[(String, String)]) -> bool {
    escolhidos.iter().all(|(nome, valor)| {
        item.ficha.iter().any(|linha| {
            linha.filtro
                && normalizar_slug(&linha.nome) == *nome
                && normalizar_slug(&linha.valor) == *valor
        })
    })
}

/// `/produtos`: o catálogo do site, com os filtros como links. Site sem
/// produto no ar não tem catálogo, e o endereço segue como o de uma página.
async fn catalogo(estado: &Estado, pedido: &Pedido, consulta: &str) -> Result<Response, ErroWeb> {
    let documentos = cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id).await?;
    let mut itens: Vec<(&str, &Item)> = documentos
        .iter()
        .filter_map(|documento| match &documento.conteudo {
            Conteudo::Item(item) => Some((documento.caminho.as_str(), item)),
            _ => None,
        })
        .collect();
    let (Some(primeiro), Some(ultimo)) = (
        itens.iter().map(|(_, item)| item.publicado_em).min(),
        itens.iter().map(|(_, item)| item.atualizado_em).max(),
    ) else {
        return documento(estado, pedido, CAMINHO_DOS_PRODUTOS, false).await;
    };
    // Pelo código, quando há; depois pelo nome.
    itens.sort_by(|a, b| {
        (a.1.codigo.as_deref().unwrap_or("~"), &a.1.nome)
            .cmp(&(b.1.codigo.as_deref().unwrap_or("~"), &b.1.nome))
    });

    let escolhidos = filtros_escolhidos(consulta);
    let filtros = filtros_do_catalogo(&itens, &escolhidos);
    let achados: Vec<_> = itens
        .iter()
        .filter(|(_, item)| passa(item, &escolhidos))
        .collect();
    // O catálogo filtrado é a mesma página para a busca: a canônica é a
    // listagem inteira.
    let documento = pagina_avulsa(
        pedido,
        CAMINHO_DOS_PRODUTOS,
        "Produtos",
        format!(
            "Catálogo de {}: veja os produtos, com ficha técnica e fotos de cada um.",
            pedido.site.nome
        ),
        true,
        (primeiro, ultimo),
        None,
    );
    let moldura = moldura(estado, pedido, &documento).await?;
    let pagina = PaginaDoCatalogo {
        moldura,
        titulo: "Produtos".to_string(),
        filtros,
        resumo: match achados.len() {
            0 => "Nenhum produto com estes filtros".to_string(),
            1 => "1 produto".to_string(),
            n => format!("{n} produtos"),
        },
        filtrado: !escolhidos.is_empty(),
        cartoes: achados
            .iter()
            .map(|(caminho, item)| CartaoDeItem::novo(caminho, item))
            .collect(),
    }
    .render()?;
    Ok(html(StatusCode::OK, pagina, pedido.indexavel))
}

/// Uma listagem de posts: o blog inteiro, uma categoria ou quem escreveu.
enum Recorte<'a> {
    Blog,
    Categoria(&'a str),
    Autor(&'a str),
}

/// Quantos posts cabem em uma página de listagem.
const POSTS_POR_PAGINA: usize = 24;

impl<'a> Recorte<'a> {
    /// A listagem e a página dela. A primeira página é o endereço sem
    /// `/pagina/N`; `/pagina/1` não existe, para não haver dois endereços
    /// para a mesma lista.
    fn do_caminho(caminho: &'a str) -> Option<(Self, usize)> {
        match caminho.rsplit_once("/pagina/") {
            Some((base, numero)) => {
                let pagina = numero.parse::<usize>().ok().filter(|pagina| *pagina >= 2)?;
                // Só o número, sem zero à esquerda nem sinal.
                (numero == pagina.to_string())
                    .then(|| Self::da_primeira_pagina(base))
                    .flatten()
                    .map(|recorte| (recorte, pagina))
            }
            None => Self::da_primeira_pagina(caminho).map(|recorte| (recorte, 1)),
        }
    }

    fn da_primeira_pagina(caminho: &'a str) -> Option<Self> {
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
    pagina: usize,
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
    let paginas = posts.len().div_ceil(POSTS_POR_PAGINA);
    if pagina > paginas {
        return nao_encontrado(estado, pedido).await;
    }

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
    // Cada página da lista tem o próprio endereço, título e descrição.
    let da_pagina = |numero: usize| match numero {
        1 => caminho.clone(),
        _ => format!("{caminho}/pagina/{numero}"),
    };
    let (titulo_da_pagina, descricao) = match pagina {
        1 => (titulo.to_string(), descricao),
        _ => (
            format!("{titulo}, página {pagina}"),
            format!("{descricao} Página {pagina} de {paginas}."),
        ),
    };
    let documento = pagina_avulsa(
        pedido,
        &da_pagina(pagina),
        &titulo_da_pagina,
        descricao,
        true,
        (primeiro, ultimo),
        acima,
    );
    let moldura = moldura(estado, pedido, &documento).await?;
    let visao = PaginaDoBlog {
        moldura,
        // O blog é a raiz da própria trilha: só as páginas abaixo dele mostram.
        trilha: if caminho == CAMINHO_DO_BLOG {
            Vec::new()
        } else {
            documento.trilha().to_vec()
        },
        titulo: titulo.to_string(),
        // Quem escreveu aparece só na primeira página.
        autor: autor.filter(|_| pagina == 1),
        cartoes: posts
            .iter()
            .skip((pagina - 1) * POSTS_POR_PAGINA)
            .take(POSTS_POR_PAGINA)
            .map(|(caminho, post)| Cartao::do_post(caminho, post))
            .collect(),
        pagina,
        paginas,
        anterior: (pagina > 1).then(|| da_pagina(pagina - 1)),
        proxima: (pagina < paginas).then(|| da_pagina(pagina + 1)),
    }
    .render()?;
    let pagina = visao;
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
    let mut moldura = moldura(estado, &pedido, &documento).await?;
    // Prévia não é visita: não carrega a tag de medição.
    moldura.tag_do_google = None;
    let mut pagina = PaginaDeDocumento::nova(moldura, &documento);
    if usa_cardapio(&pedido, &documento) {
        let mut documentos =
            cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id).await?;
        documentos.retain(|d| d.caminho != documento.caminho);
        documentos.push(documento.clone());
        pagina.cardapio = crate::cardapio::montar(
            &pedido.gravado.perfil,
            &documentos,
            &documento.caminho,
            pedido.gravado.id,
        );
    }
    Ok(pagina.render()?)
}

/// O que o visitante lê quando o endereço não tem página.
struct Aviso {
    status: StatusCode,
    caminho: &'static str,
    titulo: &'static str,
    descricao: &'static str,
    mensagem: String,
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
        mensagem: aviso.mensagem,
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
        mensagem: "O endereço que você abriu não existe ou saiu do ar.".into(),
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
        mensagem: "O conteúdo que estava neste endereço foi tirado do ar.".into(),
    };
    avisar(estado, pedido, aviso).await
}

/// O maior envio de formulário aceito: oito campos, um deles de texto longo.
const MAXIMO_DO_ENVIO: usize = 32 * 1024;

fn recado(status: StatusCode, titulo: &'static str, mensagem: String) -> Aviso {
    Aviso {
        status,
        caminho: "/mensagem",
        titulo,
        descricao: "Envio de mensagem pelo site.",
        mensagem,
    }
}

/// Recebe o envio de um formulário da página: confere contra a definição do
/// bloco, guarda a mensagem e leva o visitante ao agradecimento.
async fn receber_formulario(
    estado: &Estado,
    pedido: &Pedido,
    cabecalhos: &HeaderMap,
    caminho: &str,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    // Página de outro site não envia para cá.
    let de_fora = cabecalhos
        .get(ORIGIN)
        .and_then(|valor| valor.to_str().ok())
        .is_some_and(|origem| origem != pedido.site.origem.trim_end_matches('/'));
    if de_fora || corpo.len() > MAXIMO_DO_ENVIO {
        return Ok(simples(StatusCode::FORBIDDEN, "Envio recusado."));
    }
    let pares: Vec<(String, String)> = serde_urlencoded::from_bytes(corpo).unwrap_or_default();
    let valor = |nome: &str| {
        pares
            .iter()
            .find(|(chave, _)| chave == nome)
            .map(|(_, valor)| valor.replace("\r\n", "\n").trim().to_string())
            .unwrap_or_default()
    };
    let documento =
        cms_dados::documento_publicado(&estado.pool, pedido.gravado.id, caminho).await?;
    let formulario = valor("formulario")
        .parse::<usize>()
        .ok()
        .and_then(|indice| match documento.as_ref()?.corpo().get(indice)? {
            Bloco::Formulario { campos, .. } => Some((indice, campos)),
            _ => None,
        });
    // Página sem aquele formulário não aceita envio nenhum.
    let Some((indice, campos)) = formulario else {
        return Ok(simples(
            StatusCode::METHOD_NOT_ALLOWED,
            "Método não permitido.",
        ));
    };
    let destino = format!("{caminho}?enviado={indice}");
    // Robô que preencheu a isca recebe o mesmo agradecimento, e nada é guardado.
    if !valor("site").is_empty() {
        return Ok(redirecionar(StatusCode::SEE_OTHER, &destino));
    }

    let mut recebidos = Vec::new();
    let mut faltas: Vec<&str> = Vec::new();
    for (posicao, campo) in campos.iter().enumerate() {
        let digitado: String = valor(&format!("c{posicao}"))
            .chars()
            .take(campo.tipo.limite())
            .collect();
        let email_torto = campo.tipo == TipoDeCampo::Email
            && !digitado.is_empty()
            && !digitado
                .split_once('@')
                .is_some_and(|(nome, dominio)| !nome.is_empty() && dominio.contains('.'));
        // Em campo de escolha só vale uma das alternativas do formulário.
        let fora_das_opcoes = campo.tipo == TipoDeCampo::Escolha
            && !digitado.is_empty()
            && !campo.opcoes.iter().any(|opcao| opcao.trim() == digitado);
        if (digitado.is_empty() && campo.obrigatorio) || email_torto || fora_das_opcoes {
            faltas.push(&campo.rotulo);
        } else if !digitado.is_empty() {
            recebidos.push(CampoRecebido {
                rotulo: campo.rotulo.clone(),
                valor: digitado,
            });
        }
    }
    if !faltas.is_empty() || recebidos.is_empty() {
        let mensagem = if faltas.is_empty() {
            "A mensagem chegou vazia. Volte e preencha o formulário.".to_string()
        } else {
            format!("Volte e confira: {}.", faltas.join(", "))
        };
        let aviso = recado(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Faltou preencher",
            mensagem,
        );
        return avisar(estado, pedido, aviso).await;
    }
    let avisar_para = pedido.gravado.perfil.organizacao.email.as_deref();
    let guardada = cms_dados::guardar_mensagem(
        &estado.pool,
        pedido.gravado.id,
        caminho,
        &recebidos,
        avisar_para,
    )
    .await?;
    match guardada {
        cms_dados::Recebimento::Guardada(_) => Ok(redirecionar(StatusCode::SEE_OTHER, &destino)),
        cms_dados::Recebimento::Freada => {
            let aviso = recado(
                StatusCode::TOO_MANY_REQUESTS,
                "Muitas mensagens agora",
                "O site recebeu muitas mensagens em pouco tempo. Tente de novo em alguns minutos."
                    .to_string(),
            );
            avisar(estado, pedido, aviso).await
        }
    }
}

/// A página de agradecimento, para onde o envio leva (`?enviado=<bloco>`).
/// `None` quando o pedido não é esse.
async fn agradecer(
    estado: &Estado,
    pedido: &Pedido,
    uri: &Uri,
) -> Result<Option<Response>, ErroWeb> {
    let indice = uri.query().and_then(|consulta| {
        consulta
            .split('&')
            .find_map(|par| par.strip_prefix("enviado="))?
            .parse::<usize>()
            .ok()
    });
    let Some(indice) = indice else {
        return Ok(None);
    };
    let documento =
        cms_dados::documento_publicado(&estado.pool, pedido.gravado.id, uri.path()).await?;
    let agradecimento =
        documento
            .as_ref()
            .and_then(|documento| match documento.corpo().get(indice)? {
                Bloco::Formulario { agradecimento, .. } => Some(agradecimento.clone()),
                _ => None,
            });
    match agradecimento {
        Some(mensagem) => {
            let aviso = recado(StatusCode::OK, "Mensagem enviada", mensagem);
            Ok(Some(avisar(estado, pedido, aviso).await?))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn listagem_e_pagina_saem_do_caminho() {
        assert!(matches!(
            Recorte::do_caminho("/blog"),
            Some((Recorte::Blog, 1))
        ));
        assert!(matches!(
            Recorte::do_caminho("/blog/pagina/3"),
            Some((Recorte::Blog, 3))
        ));
        assert!(matches!(
            Recorte::do_caminho("/blog/categoria/vendas/pagina/12"),
            Some((Recorte::Categoria("vendas"), 12))
        ));
        assert!(matches!(
            Recorte::do_caminho("/autor/helena-prado"),
            Some((Recorte::Autor("helena-prado"), 1))
        ));
        // A primeira página não tem endereço próprio, e o número vem limpo.
        for recusado in [
            "/blog/pagina/1",
            "/blog/pagina/0",
            "/blog/pagina/03",
            "/blog/pagina/+3",
            "/blog/pagina/",
            "/servicos/pagina/2",
            "/blog/categoria/pagina/2",
        ] {
            assert!(Recorte::do_caminho(recusado).is_none(), "{recusado}");
        }
    }
}
