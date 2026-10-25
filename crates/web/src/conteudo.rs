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
use cms_dominio::modelos::{MODELOS, Modelo, conteudo_do_modelo};
use cms_dominio::{Acao as Permissao, Ator, Conta, Situacao};
use motor_web::tipos::Documento;
use motor_web::validacao::Gravidade;
use serde::Deserialize;
use uuid::Uuid;

use crate::acesso::ao_site;
use crate::editor::{Acao, BlocoDigitado, Formulario, Resolvido, TIPOS_DE_BLOCO};
use crate::lista::{Consulta, Lista};
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
        Some("categoria-apagada") => aviso("Categoria apagada."),
        Some("sem-categoria") => erro("Categoria não encontrada neste site."),
        Some("categoria-em-uso") => erro(
            "Esta categoria ainda tem posts. Troque a categoria deles antes de apagar, ou só mude o nome dela.",
        ),
        _ => (None, None),
    }
}

struct LinhaDeDocumento {
    id: Uuid,
    titulo: String,
    especie: &'static str,
    estado: String,
    /// O endereço da página no site, quando ela já tem um.
    caminho: Option<String>,
    /// Onde ver a página: no site, se está no ar; na prévia do painel, se não.
    ver: String,
    no_ar: bool,
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
    lista: Lista<LinhaDeDocumento>,
    /// Quantos conteúdos a busca achou, por extenso.
    resumo: String,
    modelos: &'static [Modelo],
}

/// A página do site no painel: atalhos e a lista de páginas e posts.
pub async fn inicio(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let configuracao = &estado.configuracao;
    let consulta = Consulta::da(uri);
    let origem = origem_do_site(
        &configuracao.esquema,
        &configuracao.dominio_base,
        &site.slug,
        site.dominio_ativo.as_deref(),
    );
    let documentos: Vec<LinhaDeDocumento> = cms_dados::documentos_do_site(&estado.pool, site.id)
        .await?
        .into_iter()
        .map(|documento| {
            let no_ar = documento.situacao == "publicado";
            (documento, no_ar)
        })
        .map(|(documento, no_ar)| LinhaDeDocumento {
            ver: match (&documento.caminho, no_ar) {
                (Some(caminho), true) => format!("{origem}{caminho}"),
                _ => format!("/painel/sites/{}/doc/{}/previa", site.slug, documento.id),
            },
            no_ar,
            caminho: documento.caminho,
            id: documento.id,
            titulo: if documento.titulo.trim().is_empty() {
                "(sem título)".to_string()
            } else {
                documento.titulo
            },
            especie: match documento.especie.as_str() {
                "post" => "Post",
                "item" => "Produto",
                _ => "Página",
            },
            estado: estado_de(
                &documento.situacao,
                documento.tem_rascunho,
                documento.em_revisao,
            ),
        })
        // A busca olha o título, o tipo, a situação e o endereço: "post rascunho" funciona.
        .filter(|linha| {
            consulta.casa(&format!(
                "{} {} {} {}",
                linha.titulo,
                linha.especie,
                linha.estado,
                linha.caminho.as_deref().unwrap_or_default()
            ))
        })
        .collect();
    let lista = consulta.recortar(documentos, &format!("/painel/sites/{}", site.slug));
    let corpo = PaginaDoSite {
        url: origem,
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
        resumo: lista.resumo("Nenhum conteúdo", "conteúdo", "conteúdos"),
        lista,
        modelos: &MODELOS,
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
    /// O que a imagem escolhida é neste bloco.
    rotulo_da_midia: &'static str,
    /// O bloco escolhe um modelo 3D da biblioteca.
    usa_modelo: bool,
    modelos: Vec<Opcao>,
    usa_texto: bool,
    /// A seção cita imagens pelo código, nas linhas: a tela lista os códigos.
    lista_codigos: bool,
    midias: Vec<Opcao>,
    usa_ordenada: bool,
    ordenada: bool,
    usa_fonte: bool,
    fonte: String,
    usa_rotulo: bool,
    rotulo: String,
    usa_url: bool,
    url: String,
    usa_titulo: bool,
    titulo: String,
    usa_resumo: bool,
    resumo: String,
    usa_invertido: bool,
    invertido: bool,
    usa_agradecimento: bool,
    agradecimento: String,
}

fn bloco_na_tela(
    i: usize,
    bloco: &BlocoDigitado,
    biblioteca: &[(String, String)],
    modelos: &[(String, String)],
) -> BlocoNaTela {
    let tipo = bloco.tipo.as_str();
    let nome = TIPOS_DE_BLOCO
        .iter()
        .find(|(valor, _)| *valor == tipo)
        .map_or("Parágrafo", |(_, nome)| *nome);
    let eh_secao = matches!(
        tipo,
        "cartoes"
            | "destaque"
            | "depoimentos"
            | "numeros"
            | "passos"
            | "planos"
            | "galeria"
            | "logos"
            | "faixa"
            | "contato"
            | "formulario"
    );
    let lista_codigos = matches!(tipo, "cartoes" | "depoimentos" | "galeria" | "logos");
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
        "modelo-3d" => "Legenda, embaixo do modelo (opcional)",
        "chamada" => "Texto da chamada",
        "cartoes" => {
            "Um cartão por linha: Título | Texto | Texto do link | Endereço | Código da imagem. Os três últimos são opcionais"
        }
        "destaque" => "Texto. Uma linha em branco separa os parágrafos",
        "depoimentos" => {
            "Um depoimento por linha: Texto | Nome | Cargo ou cidade | Código da foto. Os dois últimos são opcionais"
        }
        "numeros" => "Um número por linha: Valor | O que ele conta",
        "passos" => "Um passo por linha: Título | Texto",
        "planos" => {
            "Cada plano em um grupo, com linha em branco entre eles. Linha 1: Nome | Preço | Período | Descrição (comece o nome com * no plano recomendado). Linha 2: Texto do botão | Endereço. Depois, um item incluído por linha"
        }
        "galeria" | "logos" => "Um código de imagem por linha. Os códigos estão logo abaixo",
        "faixa" => "Um botão por linha: Texto do botão | Endereço. No máximo dois",
        "formulario" => {
            "Um campo por linha: Rótulo | tipo | obrigatório. O tipo é texto, email, telefone, mensagem ou escolha (com as alternativas: escolha: A; B; C); deixe o terceiro em branco se o campo for opcional. Até oito campos"
        }
        "contato" => {
            "Um canal por linha, com o nome na frente: telefone: …, whatsapp: …, email: …, endereco: …, horario: …"
        }
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
        usa_midia: matches!(tipo, "imagem" | "destaque" | "modelo-3d"),
        rotulo_da_midia: if tipo == "modelo-3d" {
            "Imagem de capa, mostrada até o modelo carregar (opcional)"
        } else {
            "Imagem da biblioteca"
        },
        usa_modelo: tipo == "modelo-3d",
        modelos: if tipo == "modelo-3d" {
            modelos
                .iter()
                .map(|(id, rotulo)| opcao(id, rotulo, &bloco.modelo))
                .collect()
        } else {
            Vec::new()
        },
        usa_texto: tipo != "imagem",
        lista_codigos,
        midias: if matches!(tipo, "imagem" | "destaque" | "modelo-3d") || lista_codigos {
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
        usa_rotulo: matches!(tipo, "chamada" | "destaque" | "formulario"),
        rotulo: bloco.rotulo.clone(),
        usa_url: tipo == "video" || tipo == "chamada" || tipo == "destaque",
        url: bloco.url.clone(),
        // Toda seção tem título; as de texto corrido, não.
        usa_titulo: eh_secao,
        titulo: bloco.titulo.clone(),
        usa_resumo: matches!(tipo, "cartoes" | "faixa" | "contato" | "formulario"),
        usa_agradecimento: tipo == "formulario",
        agradecimento: bloco.agradecimento.clone(),
        resumo: bloco.resumo.clone(),
        usa_invertido: tipo == "destaque",
        invertido: bloco.invertido,
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
    /// Os modelos 3D da biblioteca, para o produto de catálogo.
    modelos_do_item: Vec<Opcao>,
    /// Os códigos das imagens, para as outras fotos do produto.
    codigos_das_imagens: Vec<Opcao>,
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
    let modelos: Vec<(String, String)> = cms_dados::modelos_do_site(&estado.pool, site.id)
        .await?
        .into_iter()
        .map(|modelo| (modelo.id.to_string(), modelo.descricao))
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
        cabecalho: match (formulario.eh_post, formulario.eh_item, aberto.is_some()) {
            (true, _, true) => "Post",
            (true, _, false) => "Novo post",
            (_, true, true) => "Produto",
            (_, true, false) => "Novo produto",
            (_, _, true) => "Página",
            (_, _, false) => "Nova página",
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
            .map(|(i, bloco)| bloco_na_tela(i, bloco, &biblioteca, &modelos))
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
        modelos_do_item: modelos
            .iter()
            .map(|(id, rotulo)| opcao(id, rotulo, &formulario.modelo))
            .collect(),
        codigos_das_imagens: if formulario.eh_item {
            biblioteca
                .iter()
                .map(|(id, rotulo)| opcao(id, rotulo, ""))
                .collect()
        } else {
            Vec::new()
        },
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

/// O editor de um documento novo: vazio, para página ou post, ou já montado
/// a partir de um modelo.
pub async fn novo(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    especie: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let formulario = match especie {
        "post" => Formulario::novo(true),
        "pagina" => Formulario::novo(false),
        "produto" => Formulario::novo_item(),
        modelo => match conteudo_do_modelo(modelo, Utc::now()) {
            Some(conteudo) => Formulario::do_conteudo(&conteudo),
            None => return Ok(nao_encontrado()),
        },
    };
    match ao_site(estado, conta, slug).await? {
        Ok((site, ator)) => editor(estado, &site, &ator, None, formulario, uri).await,
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
        Ok(pagina) => {
            let tem_cardapio = pagina.contains("data-cardapio-dados");
            let mut resposta = html_privado(StatusCode::OK, pagina.replace("\"/midia/", &imagens));
            if tem_cardapio {
                crate::resposta::liberar_script_proprio(&mut resposta);
            }
            Ok(resposta)
        }
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
    for id in formulario.modelos_escolhidos() {
        let Ok(modelo_id) = Uuid::parse_str(id) else {
            continue;
        };
        if let Some(modelo) =
            cms_dados::modelo_para_conteudo(&estado.pool, site_id, modelo_id).await?
        {
            resolvido.modelos.insert(id.to_string(), modelo);
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
    slug: String,
    nome: String,
    cargo: String,
    sem_foto: bool,
    sem_bio: bool,
}

/// O formulário de autor já preenchido, quando a pessoa tocou em um autor
/// da lista para alterá-lo.
#[derive(Default)]
struct AutorNoFormulario {
    nome: String,
    cargo: String,
    bio: String,
    foto: String,
    credenciais: String,
    perfis: String,
}

#[derive(Template)]
#[template(path = "catalogo.html")]
struct PaginaDoCatalogo {
    site: String,
    slug: String,
    erro: Option<String>,
    aviso: Option<String>,
    autores: Vec<AutorNaTela>,
    /// O autor aberto para alterar; vazio para cadastrar um novo.
    f: AutorNoFormulario,
    alterando: bool,
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
    let autores = cms_dados::autores_do_site(&estado.pool, site.id).await?;
    // `?autor=<slug>`: o formulário abre com os dados desse autor.
    let pedido = serde_urlencoded::from_str::<Vec<(String, String)>>(uri.query().unwrap_or(""))
        .unwrap_or_default()
        .into_iter()
        .find(|(chave, _)| chave == "autor")
        .map(|(_, slug)| slug);
    let aberto = pedido.and_then(|slug| autores.iter().find(|autor| autor.slug == slug));
    let f = aberto.map_or_else(AutorNoFormulario::default, |autor| AutorNoFormulario {
        nome: autor.nome.clone(),
        cargo: autor.cargo.clone(),
        bio: autor.bio.clone(),
        foto: autor.foto_id.map(|id| id.to_string()).unwrap_or_default(),
        credenciais: autor.credenciais.join("\n"),
        perfis: autor.perfis.join("\n"),
    });
    let corpo = PaginaDoCatalogo {
        erro,
        aviso,
        alterando: aberto.is_some(),
        fotos: cms_dados::midias_do_site(&estado.pool, site.id)
            .await?
            .iter()
            .filter(|midia| midia.situacao != "falhou")
            .map(|midia| opcao(&midia.id.to_string(), &midia.alt, &f.foto))
            .collect(),
        f,
        autores: autores
            .into_iter()
            .map(|autor| AutorNaTela {
                sem_foto: autor.foto_id.is_none(),
                sem_bio: autor.bio.trim().is_empty(),
                slug: autor.slug,
                nome: autor.nome,
                cargo: autor.cargo,
            })
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
    voltar_ao_catalogo(&site.slug, salvo)
}

/// Volta para a tela de autores e categorias com o recado do que aconteceu.
fn voltar_ao_catalogo(
    slug: &str,
    feito: Result<&'static str, ErroDeCatalogo>,
) -> Result<Response, ErroWeb> {
    let codigo = match feito {
        Ok(codigo) => codigo,
        Err(ErroDeCatalogo::Dados(erro)) => return Err(erro.into()),
        Err(ErroDeCatalogo::SemPermissao) => "sem-permissao",
        Err(ErroDeCatalogo::SemNome) => "sem-nome",
        Err(ErroDeCatalogo::FotoDesconhecida) => "foto",
        Err(ErroDeCatalogo::CategoriaInexistente) => "sem-categoria",
        Err(ErroDeCatalogo::CategoriaEmUso) => "categoria-em-uso",
    };
    Ok(redirecionar(
        StatusCode::SEE_OTHER,
        &format!("/painel/sites/{slug}/catalogo?r={codigo}"),
    ))
}

/// Renomeia ou apaga uma categoria, pelos botões da lista.
pub async fn mexer_na_categoria(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    categoria: &str,
    apagar: bool,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let feito = if apagar {
        cms_dados::apagar_categoria(&estado.pool, site.id, &ator, categoria)
            .await
            .map(|()| "categoria-apagada")
    } else {
        let formulario =
            serde_urlencoded::from_bytes::<FormularioDeCategoria>(corpo).unwrap_or_default();
        cms_dados::renomear_categoria(&estado.pool, site.id, &ator, categoria, &formulario.nome)
            .await
            .map(|()| "categoria")
    };
    if feito.is_ok() {
        // O nome da categoria aparece nos posts e nas listas do site.
        estado.cache.invalidar_site(site.id);
    }
    voltar_ao_catalogo(&site.slug, feito)
}
