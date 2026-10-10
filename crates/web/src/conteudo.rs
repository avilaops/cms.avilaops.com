//! As telas de conteúdo de um site: a lista de páginas e posts, o editor de
//! blocos, a prévia e o cadastro de autores e categorias.
//!
//! Todo `POST` grava e redireciona para a tela, que lê de novo do banco: o
//! que aparece é sempre o que ficou gravado.

use askama::Template;
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use chrono::Utc;
use cms_dados::fluxo::{self, ErroDeFluxo};
use cms_dados::{DadosDoAutor, DocumentoAberto, ErroDeCatalogo, SiteGravado};
use cms_dominio::conta::pode_administrar;
use cms_dominio::eventos::origem_do_site;
use cms_dominio::fluxo::pode;
use cms_dominio::{Acao as Permissao, Ator, Conta, Situacao};
use motor_web::tipos::Documento;
use motor_web::validacao::Gravidade;
use serde::Deserialize;
use uuid::Uuid;

use crate::acesso::ao_site;
use crate::editor::{Acao, BlocoDigitado, Formulario, Resolvido, TIPOS_DE_BLOCO};
use crate::resposta::{ErroWeb, html_privado, redirecionar, simples};
use crate::{Estado, publico};

struct Opcao {
    valor: String,
    rotulo: String,
    escolhida: bool,
}

fn opcao(valor: &str, rotulo: &str, escolhido: &str) -> Opcao {
    Opcao {
        valor: valor.to_string(),
        rotulo: rotulo.to_string(),
        escolhida: valor == escolhido,
    }
}

fn nao_encontrado() -> Response {
    simples(StatusCode::NOT_FOUND, "Não encontrado.")
}

/// O recado que a tela mostra depois de um `POST`, pelo código em `?r=`.
fn recado(uri: &Uri) -> (Option<String>, Option<String>) {
    let codigo = uri
        .query()
        .and_then(|consulta| consulta.split('&').find_map(|par| par.strip_prefix("r=")));
    let aviso = |texto: &str| (None, Some(texto.to_string()));
    let erro = |texto: &str| (Some(texto.to_string()), None);
    match codigo {
        Some("salvo") => aviso("Rascunho salvo."),
        Some("revisao") => aviso("Enviado para revisão."),
        Some("devolvido") => aviso("Devolvido a quem escreveu."),
        Some("publicado") => aviso("Publicado. Já está no ar."),
        Some("despublicado") => aviso("Tirado do ar."),
        Some("agendado") => aviso("Publicação agendada."),
        Some("desagendado") => aviso("Agendamento cancelado."),
        Some("data") => erro("Informe uma data e hora no futuro para agendar."),
        Some("autor") => aviso("Autor salvo."),
        Some("categoria") => aviso("Categoria salva."),
        Some("recusado") => {
            erro("Ainda não dá: corrija o que está em vermelho abaixo e tente de novo.")
        }
        Some("sem-permissao") => erro("Sua conta não pode fazer isto neste conteúdo."),
        Some("sem-rascunho") => erro("Não há rascunho novo para esta ação."),
        Some("sem-nome") => erro("Informe o nome."),
        Some("foto") => erro("A foto escolhida não é da biblioteca deste site."),
        _ => (None, None),
    }
}

struct LinhaDeDocumento {
    id: Uuid,
    titulo: String,
    especie: &'static str,
    estado: String,
}

fn estado_de(situacao: &str, tem_rascunho: bool, em_revisao: bool) -> String {
    let base = match situacao {
        "publicado" => "No ar",
        "despublicado" => "Fora do ar",
        _ => "Rascunho",
    };
    match (situacao == "rascunho", tem_rascunho, em_revisao) {
        (true, _, true) => "Em revisão".to_string(),
        (true, _, false) => base.to_string(),
        (false, true, true) => format!("{base} · mudança em revisão"),
        (false, true, false) => format!("{base} · com rascunho novo"),
        (false, false, _) => base.to_string(),
    }
}

#[derive(Template)]
#[template(path = "site.html")]
struct PaginaDoSite {
    site: String,
    slug: String,
    url: String,
    endereco: String,
    situacao: &'static str,
    dono: bool,
    documentos: Vec<LinhaDeDocumento>,
}

/// A página do site no painel: atalhos e a lista de páginas e posts.
pub async fn inicio(estado: &Estado, conta: &Conta, slug: &str) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let configuracao = &estado.configuracao;
    let documentos = cms_dados::documentos_do_site(&estado.pool, site.id)
        .await?
        .into_iter()
        .map(|documento| LinhaDeDocumento {
            id: documento.id,
            titulo: if documento.titulo.trim().is_empty() {
                "(sem título)".to_string()
            } else {
                documento.titulo
            },
            especie: if documento.especie == "post" {
                "Post"
            } else {
                "Página"
            },
            estado: estado_de(
                &documento.situacao,
                documento.tem_rascunho,
                documento.em_revisao,
            ),
        })
        .collect();
    let corpo = PaginaDoSite {
        url: origem_do_site(
            &configuracao.esquema,
            &configuracao.dominio_base,
            &site.slug,
            site.dominio_ativo.as_deref(),
        ),
        endereco: site
            .dominio_ativo
            .clone()
            .unwrap_or_else(|| format!("{}.{}", site.slug, configuracao.dominio_base)),
        situacao: match site.situacao {
            Situacao::EmMontagem => "Em montagem",
            Situacao::Ativo => "No ar",
            Situacao::Suspenso => "Suspenso",
        },
        dono: pode_administrar(&ator),
        site: site.perfil.nome,
        slug: site.slug,
        documentos,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}

struct BlocoNaTela {
    i: usize,
    tipo: String,
    nome: &'static str,
    dica: &'static str,
    texto: String,
    niveis: Vec<Opcao>,
    usa_midia: bool,
    midias: Vec<Opcao>,
    usa_ordenada: bool,
    ordenada: bool,
    usa_fonte: bool,
    fonte: String,
    usa_rotulo: bool,
    rotulo: String,
    usa_url: bool,
    url: String,
}

fn bloco_na_tela(i: usize, bloco: &BlocoDigitado, biblioteca: &[(String, String)]) -> BlocoNaTela {
    let tipo = bloco.tipo.as_str();
    let nome = TIPOS_DE_BLOCO
        .iter()
        .find(|(valor, _)| *valor == tipo)
        .map_or("Parágrafo", |(_, nome)| *nome);
    let dica = match tipo {
        "titulo" => "Texto do título",
        "lista" => "Um item por linha",
        "citacao" => "Texto da citação",
        "perguntas" => {
            "A pergunta em uma linha, a resposta na de baixo; uma linha em branco entre uma e outra"
        }
        "tabela" => {
            "Uma linha por linha da tabela, colunas separadas por |; a primeira é o cabeçalho"
        }
        "video" => "Título do vídeo",
        "chamada" => "Texto da chamada",
        _ => "Texto. **negrito**, _itálico_ e [texto](endereço)",
    };
    BlocoNaTela {
        i,
        tipo: bloco.tipo.clone(),
        nome,
        dica,
        texto: bloco.texto.clone(),
        niveis: if tipo == "titulo" {
            [("2", "Seção"), ("3", "Subseção"), ("4", "Item da subseção")]
                .into_iter()
                .map(|(valor, rotulo)| opcao(valor, rotulo, &bloco.nivel.to_string()))
                .collect()
        } else {
            Vec::new()
        },
        usa_midia: tipo == "imagem",
        midias: if tipo == "imagem" {
            biblioteca
                .iter()
                .map(|(id, rotulo)| opcao(id, rotulo, &bloco.midia))
                .collect()
        } else {
            Vec::new()
        },
        usa_ordenada: tipo == "lista",
        ordenada: bloco.ordenada,
        usa_fonte: tipo == "citacao",
        fonte: bloco.fonte.clone(),
        usa_rotulo: tipo == "chamada",
        rotulo: bloco.rotulo.clone(),
        usa_url: tipo == "video" || tipo == "chamada",
        url: bloco.url.clone(),
    }
}

struct ProblemaNaTela {
    mensagem: String,
    bloqueia: bool,
}

#[derive(Template)]
#[template(path = "editor.html")]
struct PaginaDoEditor {
    site: String,
    slug: String,
    cabecalho: &'static str,
    estado: String,
    erro: Option<String>,
    aviso: Option<String>,
    problemas: Vec<ProblemaNaTela>,
    destino: String,
    previa: Option<String>,
    f: Formulario,
    blocos: Vec<BlocoNaTela>,
    tipos_de_bloco: Vec<Opcao>,
    tipos_de_post: Vec<Opcao>,
    autores: Vec<Opcao>,
    categorias: Vec<Opcao>,
    capas: Vec<Opcao>,
    pode_revisar: bool,
    pode_devolver: bool,
    pode_publicar: bool,
    pode_despublicar: bool,
    pode_agendar: bool,
    agendado: bool,
}

/// Monta a tela do editor para um documento novo (`aberto` vazio) ou gravado.
async fn editor(
    estado: &Estado,
    site: &SiteGravado,
    ator: &Ator,
    aberto: Option<&DocumentoAberto>,
    formulario: Formulario,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let biblioteca: Vec<(String, String)> = cms_dados::midias_do_site(&estado.pool, site.id)
        .await?
        .into_iter()
        .filter(|midia| midia.situacao != "falhou")
        .map(|midia| {
            let rotulo = if midia.situacao == "pronta" {
                midia.alt
            } else {
                format!("{} (preparando)", midia.alt)
            };
            (midia.id.to_string(), rotulo)
        })
        .collect();
    let (autores, categorias) = if formulario.eh_post {
        (
            cms_dados::autores_do_site(&estado.pool, site.id)
                .await?
                .iter()
                .map(|autor| opcao(&autor.slug, &autor.nome, &formulario.autor))
                .collect(),
            cms_dados::categorias_do_site(&estado.pool, site.id)
                .await?
                .iter()
                .map(|categoria| opcao(&categoria.slug, &categoria.nome, &formulario.categoria))
                .collect(),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let problemas = match aberto {
        Some(aberto) => fluxo::problemas_do_rascunho(&estado.pool, site.id, aberto.id)
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    }
    .into_iter()
    .map(|problema| ProblemaNaTela {
        mensagem: problema.mensagem,
        bloqueia: problema.gravidade == Gravidade::Bloqueia,
    })
    .collect();

    let criado_por = aberto.and_then(|aberto| aberto.criado_por.as_deref());
    let permite = |acao: Permissao| pode(ator, acao, criado_por);
    let tem_rascunho = aberto.is_some_and(|aberto| aberto.tem_rascunho);
    let em_revisao = aberto.is_some_and(|aberto| aberto.em_revisao);
    let situacao = aberto.map_or("rascunho", |aberto| aberto.situacao.as_str());
    let (erro, aviso) = recado(uri);
    let base = format!("/painel/sites/{}/doc", site.slug);

    let corpo = PaginaDoEditor {
        site: site.perfil.nome.clone(),
        slug: site.slug.clone(),
        cabecalho: match (formulario.eh_post, aberto.is_some()) {
            (true, true) => "Post",
            (true, false) => "Novo post",
            (false, true) => "Página",
            (false, false) => "Nova página",
        },
        estado: match aberto {
            Some(aberto) => {
                let base = estado_de(&aberto.situacao, aberto.tem_rascunho, aberto.em_revisao);
                match aberto.agendado_para {
                    Some(quando) => format!(
                        "{base} · publicação agendada para {}",
                        cms_dominio::datas::por_extenso(quando)
                    ),
                    None => base,
                }
            }
            None => "Ainda não salvo".to_string(),
        },
        erro,
        aviso,
        problemas,
        destino: match aberto {
            Some(aberto) => format!("{base}/{}", aberto.id),
            None => base.clone(),
        },
        previa: aberto.map(|aberto| format!("{base}/{}/previa", aberto.id)),
        blocos: formulario
            .blocos
            .iter()
            .enumerate()
            .map(|(i, bloco)| bloco_na_tela(i, bloco, &biblioteca))
            .collect(),
        tipos_de_bloco: TIPOS_DE_BLOCO
            .iter()
            .map(|(valor, nome)| opcao(valor, nome, "paragrafo"))
            .collect(),
        tipos_de_post: [("artigo", "Artigo"), ("guia-tecnico", "Guia técnico")]
            .into_iter()
            .map(|(valor, nome)| opcao(valor, nome, &formulario.tipo_do_post))
            .collect(),
        autores,
        categorias,
        capas: biblioteca
            .iter()
            .map(|(id, rotulo)| opcao(id, rotulo, &formulario.capa))
            .collect(),
        pode_revisar: tem_rascunho && !em_revisao && permite(Permissao::EnviarParaRevisao),
        pode_devolver: em_revisao && permite(Permissao::Devolver),
        pode_publicar: (tem_rascunho || situacao == "despublicado" || aberto.is_none())
            && permite(Permissao::Publicar),
        pode_despublicar: situacao == "publicado" && permite(Permissao::Despublicar),
        pode_agendar: permite(Permissao::Agendar),
        agendado: aberto.is_some_and(|aberto| aberto.agendado_para.is_some()),
        f: formulario,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}

/// O editor vazio, para uma página ou um post novo.
pub async fn novo(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    especie: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let eh_post = match especie {
        "post" => true,
        "pagina" => false,
        _ => return Ok(nao_encontrado()),
    };
    match ao_site(estado, conta, slug).await? {
        Ok((site, ator)) => {
            editor(estado, &site, &ator, None, Formulario::novo(eh_post), uri).await
        }
        Err(resposta) => Ok(resposta),
    }
}

/// O site, o ator e o documento pedido, ou a resposta que barra.
async fn ao_documento(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    documento_id: &str,
) -> Result<Result<(SiteGravado, Ator, DocumentoAberto), Response>, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(Err(resposta)),
    };
    let Ok(documento_id) = Uuid::parse_str(documento_id) else {
        return Ok(Err(nao_encontrado()));
    };
    match cms_dados::abrir_documento(&estado.pool, site.id, documento_id).await? {
        Some(aberto) => Ok(Ok((site, ator, aberto))),
        None => Ok(Err(nao_encontrado())),
    }
}

pub async fn abrir(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    documento_id: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    match ao_documento(estado, conta, slug, documento_id).await? {
        Ok((site, ator, aberto)) => {
            let formulario = Formulario::do_conteudo(&aberto.conteudo);
            editor(estado, &site, &ator, Some(&aberto), formulario, uri).await
        }
        Err(resposta) => Ok(resposta),
    }
}

/// A página como o visitante a veria, a partir do rascunho. As imagens saem
/// pelo painel, porque esta tela não é servida no host do site.
pub async fn previa(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    documento_id: &str,
) -> Result<Response, ErroWeb> {
    let (site, _, aberto) = match ao_documento(estado, conta, slug, documento_id).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Some(caminho) = cms_dominio::fluxo::caminho_de(&aberto.conteudo) else {
        return Ok(nao_encontrado());
    };
    let imagens = format!("\"/painel/sites/{}/midia/arquivo/", site.slug);
    let documento = Documento {
        caminho,
        conteudo: aberto.conteudo,
    };
    match publico::previa(estado, site, documento).await {
        Ok(pagina) => Ok(html_privado(
            StatusCode::OK,
            pagina.replace("\"/midia/", &imagens),
        )),
        // Rascunho que o motor não consegue montar ainda não tem prévia.
        Err(ErroWeb::Documento(_)) => Ok(simples(
            StatusCode::UNPROCESSABLE_ENTITY,
            "A prévia aparece depois que o título e a descrição de busca estiverem preenchidos.",
        )),
        Err(erro) => Err(erro),
    }
}

/// Busca no banco o que o formulário cita pelo identificador.
async fn resolver(
    estado: &Estado,
    site_id: Uuid,
    formulario: &Formulario,
) -> Result<Resolvido, ErroWeb> {
    let mut resolvido = Resolvido::default();
    for id in formulario.midias_escolhidas() {
        let Ok(midia_id) = Uuid::parse_str(id) else {
            continue;
        };
        if let Some(midia) = cms_dados::midia_para_conteudo(&estado.pool, site_id, midia_id).await?
        {
            resolvido.midias.insert(id.to_string(), midia);
        }
    }
    if formulario.eh_post {
        resolvido.autor =
            cms_dados::autor_para_conteudo(&estado.pool, site_id, &formulario.autor).await?;
        resolvido.categoria = cms_dados::categorias_do_site(&estado.pool, site_id)
            .await?
            .into_iter()
            .find(|categoria| categoria.slug == formulario.categoria);
    }
    Ok(resolvido)
}

/// Salva o rascunho e, se o botão pediu, segue com a ação. Devolve o
/// documento e o código do recado.
async fn executar(
    estado: &Estado,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Option<Uuid>,
    corpo: &[u8],
) -> Result<Result<(Uuid, &'static str), ErroDeFluxo>, ErroWeb> {
    let pares = serde_urlencoded::from_bytes::<Vec<(String, String)>>(corpo).unwrap_or_default();
    let mut formulario = Formulario::ler(pares);
    let acao = formulario.aplicar_acao();
    let pool = &estado.pool;

    // Devolver, tirar do ar e cancelar o agendamento agem sobre o que está
    // gravado, sem salvar nada.
    if let (Some(id), Acao::Devolver | Acao::Despublicar | Acao::CancelarAgendamento) =
        (documento_id, acao)
    {
        let feito = match acao {
            Acao::Devolver => fluxo::devolver(pool, site_id, ator, id)
                .await
                .map(|()| "devolvido"),
            Acao::Despublicar => fluxo::despublicar(pool, site_id, ator, id)
                .await
                .map(|()| "despublicado"),
            _ => fluxo::cancelar_agendamento(pool, site_id, ator, id)
                .await
                .map(|()| "desagendado"),
        };
        return Ok(feito.map(|codigo| (id, codigo)));
    }

    let conteudo =
        formulario.para_conteudo(&resolver(estado, site_id, &formulario).await?, Utc::now());
    let id = match fluxo::salvar_rascunho(pool, site_id, ator, documento_id, &conteudo).await {
        Ok(salvo) => salvo.documento_id,
        Err(erro) => return Ok(Err(erro)),
    };
    let seguinte = match acao {
        Acao::EnviarParaRevisao => fluxo::enviar_para_revisao(pool, site_id, ator, id)
            .await
            .map(|()| "revisao"),
        Acao::Publicar => fluxo::publicar(pool, site_id, ator, id, Utc::now())
            .await
            .map(|()| "publicado"),
        Acao::Agendar => {
            match crate::editor::ler_agendamento(&formulario.agendar_para)
                .filter(|quando| *quando > Utc::now())
            {
                Some(quando) => fluxo::agendar(pool, site_id, ator, id, quando)
                    .await
                    .map(|()| "agendado"),
                None => Ok("data"),
            }
        }
        _ => Ok("salvo"),
    };
    Ok(match seguinte {
        Ok(codigo) => Ok((id, codigo)),
        // O rascunho ficou salvo: a recusa volta para o editor dele.
        Err(ErroDeFluxo::Recusado(_)) => Ok((id, "recusado")),
        Err(ErroDeFluxo::SemPermissao) => Ok((id, "sem-permissao")),
        Err(erro) => Err(erro),
    })
}

/// `POST` do editor, de documento novo (`documento_id` vazio) ou gravado.
pub async fn gravar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    documento_id: Option<&str>,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let documento_id = match documento_id.map(Uuid::parse_str) {
        Some(Ok(id)) => Some(id),
        Some(Err(_)) => return Ok(nao_encontrado()),
        None => None,
    };
    let base = format!("/painel/sites/{}", site.slug);
    let para = |id: Uuid, codigo: &str| {
        redirecionar(
            StatusCode::SEE_OTHER,
            &format!("{base}/doc/{id}?r={codigo}"),
        )
    };
    match executar(estado, site.id, &ator, documento_id, corpo).await? {
        Ok((id, codigo)) => {
            // O que mudou no ar não espera o cache vencer.
            if matches!(codigo, "publicado" | "despublicado") {
                estado.cache.invalidar_site(site.id);
            }
            Ok(para(id, codigo))
        }
        Err(ErroDeFluxo::Dados(erro)) => Err(erro.into()),
        Err(ErroDeFluxo::NaoEncontrado) => Ok(nao_encontrado()),
        Err(erro) => {
            let codigo = match erro {
                ErroDeFluxo::SemPermissao => "sem-permissao",
                ErroDeFluxo::Recusado(_) => "recusado",
                _ => "sem-rascunho",
            };
            Ok(match documento_id {
                Some(id) => para(id, codigo),
                None => redirecionar(StatusCode::SEE_OTHER, &format!("{base}?r={codigo}")),
            })
        }
    }
}

struct AutorNaTela {
    nome: String,
    cargo: String,
    sem_foto: bool,
    sem_bio: bool,
}

#[derive(Template)]
#[template(path = "catalogo.html")]
struct PaginaDoCatalogo {
    site: String,
    slug: String,
    erro: Option<String>,
    aviso: Option<String>,
    autores: Vec<AutorNaTela>,
    fotos: Vec<Opcao>,
    categorias: Vec<motor_web::tipos::Categoria>,
}

pub async fn catalogo(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let (site, _) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let (erro, aviso) = recado(uri);
    let corpo = PaginaDoCatalogo {
        erro,
        aviso,
        autores: cms_dados::autores_do_site(&estado.pool, site.id)
            .await?
            .into_iter()
            .map(|autor| AutorNaTela {
                sem_foto: autor.foto_id.is_none(),
                sem_bio: autor.bio.trim().is_empty(),
                nome: autor.nome,
                cargo: autor.cargo,
            })
            .collect(),
        fotos: cms_dados::midias_do_site(&estado.pool, site.id)
            .await?
            .iter()
            .filter(|midia| midia.situacao != "falhou")
            .map(|midia| opcao(&midia.id.to_string(), &midia.alt, ""))
            .collect(),
        categorias: cms_dados::categorias_do_site(&estado.pool, site.id).await?,
        site: site.perfil.nome,
        slug: site.slug,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FormularioDeAutor {
    nome: String,
    cargo: String,
    bio: String,
    foto: String,
    perfis: String,
    credenciais: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FormularioDeCategoria {
    nome: String,
}

/// `POST` de autor (`eh_autor`) ou de categoria.
pub async fn salvar_no_catalogo(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    eh_autor: bool,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let salvo = if eh_autor {
        let formulario =
            serde_urlencoded::from_bytes::<FormularioDeAutor>(corpo).unwrap_or_default();
        let dados = DadosDoAutor {
            nome: formulario.nome,
            cargo: formulario.cargo,
            bio: formulario.bio,
            foto_id: Uuid::parse_str(&formulario.foto).ok(),
            perfis: formulario.perfis,
            credenciais: formulario.credenciais,
        };
        cms_dados::salvar_autor(&estado.pool, site.id, &ator, &dados)
            .await
            .map(|_| "autor")
    } else {
        let formulario =
            serde_urlencoded::from_bytes::<FormularioDeCategoria>(corpo).unwrap_or_default();
        cms_dados::salvar_categoria(&estado.pool, site.id, &ator, &formulario.nome)
            .await
            .map(|_| "categoria")
    };
    let codigo = match salvo {
        Ok(codigo) => codigo,
        Err(ErroDeCatalogo::Dados(erro)) => return Err(erro.into()),
        Err(ErroDeCatalogo::SemPermissao) => "sem-permissao",
        Err(ErroDeCatalogo::SemNome) => "sem-nome",
        Err(ErroDeCatalogo::FotoDesconhecida) => "foto",
    };
    Ok(redirecionar(
        StatusCode::SEE_OTHER,
        &format!("/painel/sites/{}/catalogo?r={codigo}", site.slug),
    ))
}
