//! O site público: do host ao que está publicado.

use askama::Template;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use chrono::{DateTime, Utc};
use cms_dados::{Ausencia, SiteGravado};
use cms_dominio::site::aparece_na_busca;
use cms_dominio::{Endereco, Host, Situacao, classificar, ler_host};
use motor_web::descoberta::{
    ItensDoSite, OpcoesRobots, PaginaDeCategoria, gerar_llms, gerar_llms_completo, gerar_robots,
    gerar_sitemaps,
};
use motor_web::seo::montar_cabecalho;
use motor_web::tipos::{Conteudo, Documento, ItemTrilha, Pagina, Seo, Site};

use crate::Estado;
use crate::midia;
use crate::resposta::{ErroWeb, html, redirecionar, simples, texto};
use crate::visao::{Cartao, Moldura, PaginaDeAviso, PaginaDeDocumento, PaginaDoBlog};

const CAMINHO_DO_BLOG: &str = "/blog";

pub async fn atender(
    estado: &Estado,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    uri: &Uri,
) -> Response {
    if metodo != Method::GET && metodo != Method::HEAD {
        return simples(StatusCode::METHOD_NOT_ALLOWED, "Método não permitido.");
    }
    match responder(estado, cabecalhos, uri).await {
        Ok(resposta) => resposta,
        Err(erro) => {
            tracing::error!(%erro, caminho = uri.path(), "falha ao atender o pedido");
            simples(StatusCode::INTERNAL_SERVER_ERROR, "Erro interno.")
        }
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
    match caminho {
        "/robots.txt" => Ok(robots(&pedido)),
        "/llms.txt" | "/llms-full.txt" => descoberta_para_ia(estado, &pedido, caminho).await,
        CAMINHO_DO_BLOG => blog(estado, &pedido).await,
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
            None => documento(estado, &pedido, caminho).await,
        },
    }
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

/// A listagem do blog entra no sitemap como uma página de listagem.
fn listagens(documentos: &[Documento]) -> Vec<PaginaDeCategoria> {
    let mais_recente = documentos
        .iter()
        .filter(|documento| matches!(documento.conteudo, Conteudo::Post(_)))
        .map(Documento::atualizado_em)
        .max();
    mais_recente
        .map(|atualizado_em| PaginaDeCategoria {
            caminho: CAMINHO_DO_BLOG.into(),
            nome: "Blog".into(),
            atualizado_em,
        })
        .into_iter()
        .collect()
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
) -> Documento {
    Documento {
        caminho: caminho.to_string(),
        conteudo: Conteudo::Pagina(Pagina {
            slug: caminho.trim_matches('/').to_string(),
            titulo: titulo.to_string(),
            corpo: Vec::new(),
            capa: None,
            publicado_em: datas.0,
            atualizado_em: datas.1,
            seo: Seo {
                titulo: format!("{titulo} | {}", pedido.site.nome),
                descricao,
                indexar: indexar && pedido.indexavel,
                imagem_social: None,
            },
            trilha: vec![
                ItemTrilha {
                    nome: "Início".into(),
                    url: "/".into(),
                },
                ItemTrilha {
                    nome: titulo.to_string(),
                    url: caminho.to_string(),
                },
            ],
        }),
    }
}

async fn blog(estado: &Estado, pedido: &Pedido) -> Result<Response, ErroWeb> {
    let documentos = cms_dados::documentos_publicados(&estado.pool, pedido.gravado.id).await?;
    let mut posts: Vec<_> = documentos
        .iter()
        .filter_map(|documento| match &documento.conteudo {
            Conteudo::Post(post) => Some((documento.caminho.as_str(), post)),
            _ => None,
        })
        .collect();
    // Mais recente primeiro; empate fica em ordem de caminho.
    posts.sort_by(|a, b| {
        b.1.publicado_em
            .cmp(&a.1.publicado_em)
            .then_with(|| a.0.cmp(b.0))
    });

    let (Some(primeiro), Some(ultimo)) = (
        posts.iter().map(|(_, post)| post.publicado_em).min(),
        posts.iter().map(|(_, post)| post.atualizado_em).max(),
    ) else {
        return nao_encontrado(estado, pedido).await;
    };

    let listagem = pagina_avulsa(
        pedido,
        CAMINHO_DO_BLOG,
        "Blog",
        format!("Artigos e guias de {}.", pedido.site.nome),
        true,
        (primeiro, ultimo),
    );
    let moldura = moldura(estado, pedido, &listagem).await?;
    let cartoes = posts
        .iter()
        .map(|(caminho, post)| Cartao::do_post(caminho, post))
        .collect();
    let pagina = PaginaDoBlog {
        moldura,
        titulo: "Blog".into(),
        cartoes,
    }
    .render()?;
    Ok(html(StatusCode::OK, pagina, pedido.indexavel))
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
