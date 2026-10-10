//! O conector para assistentes de IA: servidor de autorização (registro
//! dinâmico, código com PKCE S256) e o ponto MCP com as ferramentas.
//!
//! Nenhuma regra mora aqui. Cada ferramenta confere o escopo da conexão,
//! resolve o papel da conta no site e chama a mesma função que o painel usa.

use askama::Template;
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, WWW_AUTHENTICATE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use chrono::Utc;
use cms_dados::fluxo::{self, ErroDeFluxo};
use cms_dados::{
    Autorizacao, Conexao, DadosDoAutor, DescricaoDaMidia, ESCOPO_DE_PUBLICAR, ESCOPOS,
    ErroDeCatalogo, ErroDeConector, ErroDeMidia, Tokens,
};
use cms_dominio::modelos::{MODELOS, conteudo_do_modelo};
use cms_dominio::{Ator, Conta};
use motor_web::tipos::{Conteudo, Direitos};
use motor_web::validacao::Problema;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::identidade::{self, DadosDaIdentidade};
use crate::resposta::{ErroWeb, html_privado, redirecionar, simples};
use crate::{Estado, biblioteca};

const VERSAO_DO_PROTOCOLO: &str = "2025-06-18";

fn json_com(status: StatusCode, corpo: Value) -> Response {
    let mut resposta = (status, corpo.to_string()).into_response();
    let cabecalhos = resposta.headers_mut();
    cabecalhos.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    cabecalhos.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resposta
}

/// Erro no formato do OAuth: um código curto e uma frase.
fn erro_oauth(status: StatusCode, codigo: &str, descricao: &str) -> Response {
    json_com(
        status,
        json!({ "error": codigo, "error_description": descricao }),
    )
}

/// O que o cliente precisa saber para se registrar e pedir autorização.
pub fn metadados(origem: &str) -> Response {
    json_com(
        StatusCode::OK,
        json!({
            "issuer": origem,
            "authorization_endpoint": format!("{origem}/oauth/authorize"),
            "token_endpoint": format!("{origem}/oauth/token"),
            "registration_endpoint": format!("{origem}/oauth/register"),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "scopes_supported": ESCOPOS.iter().map(|(escopo, _)| *escopo).collect::<Vec<_>>(),
        }),
    )
}

pub fn recurso_protegido(origem: &str) -> Response {
    json_com(
        StatusCode::OK,
        json!({ "resource": format!("{origem}/mcp"), "authorization_servers": [origem] }),
    )
}

#[derive(Deserialize)]
struct PedidoDeRegistro {
    #[serde(default)]
    client_name: String,
    #[serde(default)]
    redirect_uris: Vec<String>,
}

/// O retorno só pode ser `https`, ou `http` na própria máquina de quem usa.
fn retorno_aceito(endereco: &str) -> bool {
    let local = ["http://localhost", "http://127.0.0.1"].iter().any(|base| {
        endereco
            .strip_prefix(base)
            .is_some_and(|resto| resto.is_empty() || resto.starts_with([':', '/']))
    });
    let seguro = endereco
        .strip_prefix("https://")
        .is_some_and(|resto| !resto.is_empty() && !resto.starts_with('/'));
    (local || seguro)
        && endereco.len() <= 500
        && !endereco.contains(['#', ' ', '\r', '\n', '"', '<', '>'])
}

/// `POST /oauth/register`: registro dinâmico, aberto. Não há lista de
/// assistentes aceitos: quem autoriza é a pessoa, na tela.
pub async fn registrar(estado: &Estado, corpo: &[u8]) -> Result<Response, ErroWeb> {
    let Ok(pedido) = serde_json::from_slice::<PedidoDeRegistro>(corpo) else {
        return Ok(erro_oauth(
            StatusCode::BAD_REQUEST,
            "invalid_client_metadata",
            "O corpo precisa ser JSON com redirect_uris.",
        ));
    };
    let validos = !pedido.redirect_uris.is_empty()
        && pedido.redirect_uris.len() <= 5
        && pedido.redirect_uris.iter().all(|uri| retorno_aceito(uri));
    if !validos {
        return Ok(erro_oauth(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "Informe de 1 a 5 endereços de retorno, em https ou em http://localhost.",
        ));
    }
    let nome: String = pedido.client_name.trim().chars().take(80).collect();
    let nome = if nome.is_empty() {
        "Assistente sem nome".to_string()
    } else {
        nome
    };
    let id = cms_dados::registrar_cliente(&estado.pool, &nome, &pedido.redirect_uris).await?;
    Ok(json_com(
        StatusCode::CREATED,
        json!({
            "client_id": id,
            "client_name": nome,
            "redirect_uris": pedido.redirect_uris,
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        }),
    ))
}

/// Os parâmetros do pedido de autorização, vindos da consulta ou do
/// formulário de consentimento.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct PedidoDeAutorizacao {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    code_challenge_method: String,
    state: String,
    scope: String,
}

struct EscopoNaTela {
    valor: &'static str,
    rotulo: &'static str,
    marcado: bool,
}

#[derive(Template)]
#[template(path = "autorizar.html")]
struct PaginaDeAutorizacao {
    cliente: String,
    quem: String,
    pedido: PedidoDeAutorizacao,
    escopos: Vec<EscopoNaTela>,
}

/// Confere o pedido contra o cliente registrado. Enquanto o cliente e o
/// retorno não conferem, o erro fica na tela: redirecionar seria mandar a
/// pessoa para um endereço que ninguém registrou.
async fn conferir(
    estado: &Estado,
    pedido: &PedidoDeAutorizacao,
) -> Result<Result<(Uuid, String), Response>, ErroWeb> {
    let recusar = |mensagem: &'static str| Ok(Err(simples(StatusCode::BAD_REQUEST, mensagem)));
    let Ok(cliente_id) = Uuid::parse_str(&pedido.client_id) else {
        return recusar("Pedido de autorização inválido: cliente desconhecido.");
    };
    let Some(cliente) = cms_dados::cliente_mcp(&estado.pool, cliente_id).await? else {
        return recusar("Pedido de autorização inválido: cliente desconhecido.");
    };
    if !cliente.redirect_uris.contains(&pedido.redirect_uri) {
        return recusar("Pedido de autorização inválido: endereço de retorno não registrado.");
    }
    let desafio_valido = (43..=128).contains(&pedido.code_challenge.len())
        && pedido
            .code_challenge
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if pedido.response_type != "code" || pedido.code_challenge_method != "S256" || !desafio_valido {
        return recusar("Pedido de autorização inválido: é preciso código com PKCE S256.");
    }
    Ok(Ok((cliente_id, cliente.nome)))
}

/// `GET /oauth/authorize`: a tela em que a pessoa escolhe o que o assistente
/// pode fazer.
pub async fn tela_de_autorizacao(
    estado: &Estado,
    conta: &Conta,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let pedido = serde_urlencoded::from_str::<PedidoDeAutorizacao>(uri.query().unwrap_or(""))
        .unwrap_or_default();
    let cliente = match conferir(estado, &pedido).await? {
        Ok((_, nome)) => nome,
        Err(resposta) => return Ok(resposta),
    };
    let pedidos: Vec<&str> = pedido.scope.split_whitespace().collect();
    let corpo = PaginaDeAutorizacao {
        cliente,
        quem: conta.email.clone(),
        escopos: ESCOPOS
            .iter()
            .map(|(valor, rotulo)| EscopoNaTela {
                valor,
                rotulo,
                // Publicar nunca vem marcado, peça o cliente o que pedir.
                marcado: *valor != ESCOPO_DE_PUBLICAR
                    && (pedidos.is_empty() || pedidos.contains(valor)),
            })
            .collect(),
        pedido,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}

/// Acrescenta parâmetros a um endereço de retorno já registrado.
fn com_parametros(endereco: &str, parametros: &[(&str, &str)]) -> String {
    let consulta = serde_urlencoded::to_string(parametros).unwrap_or_default();
    let separador = if endereco.contains('?') { '&' } else { '?' };
    format!("{endereco}{separador}{consulta}")
}

/// `POST /oauth/authorize`: a pessoa decidiu. Volta ao cliente com o código,
/// ou com a recusa.
pub async fn autorizar(estado: &Estado, conta: &Conta, corpo: &[u8]) -> Result<Response, ErroWeb> {
    let pares = serde_urlencoded::from_bytes::<Vec<(String, String)>>(corpo).unwrap_or_default();
    let campo = |nome: &str| {
        pares
            .iter()
            .find(|(chave, _)| chave == nome)
            .map(|(_, valor)| valor.clone())
            .unwrap_or_default()
    };
    let pedido = PedidoDeAutorizacao {
        response_type: campo("response_type"),
        client_id: campo("client_id"),
        redirect_uri: campo("redirect_uri"),
        code_challenge: campo("code_challenge"),
        code_challenge_method: campo("code_challenge_method"),
        state: campo("state"),
        scope: String::new(),
    };
    let cliente_id = match conferir(estado, &pedido).await? {
        Ok((id, _)) => id,
        Err(resposta) => return Ok(resposta),
    };
    if campo("decisao") != "autorizar" {
        let destino = com_parametros(
            &pedido.redirect_uri,
            &[("error", "access_denied"), ("state", &pedido.state)],
        );
        return Ok(redirecionar(StatusCode::SEE_OTHER, &destino));
    }
    // Só o que está na lista de escopos, e só o que a pessoa marcou.
    let escopos: Vec<String> = ESCOPOS
        .iter()
        .map(|(escopo, _)| *escopo)
        .filter(|escopo| {
            pares
                .iter()
                .any(|(chave, valor)| chave == "escopo" && valor == escopo)
        })
        .map(str::to_string)
        .collect();
    let autorizacao = Autorizacao {
        cliente_id,
        conta,
        escopos: &escopos,
        redirect_uri: &pedido.redirect_uri,
        desafio: &pedido.code_challenge,
    };
    let codigo = cms_dados::criar_codigo(&estado.pool, &autorizacao).await?;
    let destino = com_parametros(
        &pedido.redirect_uri,
        &[("code", codigo.as_str()), ("state", &pedido.state)],
    );
    Ok(redirecionar(StatusCode::SEE_OTHER, &destino))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PedidoDeToken {
    grant_type: String,
    code: String,
    code_verifier: String,
    client_id: String,
    redirect_uri: String,
    refresh_token: String,
}

fn tokens_em_json(tokens: Tokens) -> Response {
    json_com(
        StatusCode::OK,
        json!({
            "access_token": tokens.acesso,
            "token_type": "Bearer",
            "expires_in": tokens.expira_em_segundos,
            "refresh_token": tokens.renovacao,
            "scope": tokens.escopos.join(" "),
        }),
    )
}

/// `POST /oauth/token`: troca o código, ou renova.
pub async fn token(estado: &Estado, corpo: &[u8]) -> Result<Response, ErroWeb> {
    let pedido = serde_urlencoded::from_bytes::<PedidoDeToken>(corpo).unwrap_or_default();
    let invalida = || {
        erro_oauth(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "Código, verificador ou token que não confere.",
        )
    };
    let Ok(cliente_id) = Uuid::parse_str(&pedido.client_id) else {
        return Ok(invalida());
    };
    let pool = &estado.pool;
    let resultado = match pedido.grant_type.as_str() {
        "authorization_code" => {
            cms_dados::trocar_codigo(
                pool,
                &pedido.code,
                cliente_id,
                &pedido.redirect_uri,
                &pedido.code_verifier,
            )
            .await
        }
        "refresh_token" => cms_dados::renovar_tokens(pool, &pedido.refresh_token, cliente_id).await,
        _ => {
            return Ok(erro_oauth(
                StatusCode::BAD_REQUEST,
                "unsupported_grant_type",
                "Use authorization_code ou refresh_token.",
            ));
        }
    };
    match resultado {
        Ok(tokens) => Ok(tokens_em_json(tokens)),
        Err(ErroDeConector::ConcessaoInvalida) => Ok(invalida()),
        Err(ErroDeConector::Dados(erro)) => Err(erro.into()),
    }
}

struct ConexaoNaTela {
    id: Uuid,
    cliente: String,
    pode: String,
    desde: String,
    chamadas: i64,
}

#[derive(Template)]
#[template(path = "conector.html")]
struct PaginaDoConector {
    endereco: String,
    conexoes: Vec<ConexaoNaTela>,
}

/// `GET /painel/conector`: o endereço para colar no assistente e as conexões
/// em vigor da conta.
pub async fn tela(estado: &Estado, conta: &Conta, origem: &str) -> Result<Response, ErroWeb> {
    let conexoes = cms_dados::conexoes_da_conta(&estado.pool, &conta.sub)
        .await?
        .into_iter()
        .map(|conexao| ConexaoNaTela {
            id: conexao.id,
            cliente: conexao.cliente,
            pode: ESCOPOS
                .iter()
                .filter(|(escopo, _)| conexao.escopos.iter().any(|e| e == escopo))
                .map(|(_, rotulo)| rotulo.to_lowercase())
                .collect::<Vec<_>>()
                .join("; "),
            desde: cms_dominio::datas::por_extenso(conexao.criado_em),
            chamadas: conexao.chamadas,
        })
        .collect();
    let corpo = PaginaDoConector {
        endereco: format!("{origem}/mcp"),
        conexoes,
    }
    .render()?;
    Ok(html_privado(StatusCode::OK, corpo))
}

/// `POST /painel/conector/{id}/revogar`
pub async fn revogar(
    estado: &Estado,
    conta: &Conta,
    conexao_id: &str,
) -> Result<Response, ErroWeb> {
    if let Ok(id) = Uuid::parse_str(conexao_id) {
        cms_dados::revogar_conexao(&estado.pool, &conta.sub, id).await?;
    }
    Ok(redirecionar(StatusCode::SEE_OTHER, "/painel/conector"))
}

/// Uma ferramenta: nome, escopo que exige, o que faz e os argumentos.
struct Ferramenta {
    nome: &'static str,
    escopo: &'static str,
    descricao: &'static str,
    /// Os argumentos obrigatórios, todos texto, e a descrição de cada um.
    argumentos: &'static [(&'static str, &'static str)],
    /// A ferramenta recebe também `conteudo`, um objeto.
    com_conteudo: bool,
    /// Os argumentos que podem faltar, também texto.
    opcionais: &'static [(&'static str, &'static str)],
}

const SITE: (&str, &str) = ("site", "O endereço do site, como em listar_sites (slug).");
const DOCUMENTO: (&str, &str) = ("documento", "O identificador do documento.");
const DESCRICAO_DO_CONTEUDO: &str = "O documento no contrato do CMS: { especie: \"pagina\" | \"post\", dados: { ... } }. Use ver_documento em um documento que já existe para ver o formato. Imagem (capa e bloco de imagem) pode ir só com o id da biblioteca, em texto (listar_midia). No post, autor e categoria podem ir só com o slug do cadastro (listar_autores, listar_categorias). O corpo aceita blocos de texto (paragrafo, titulo, lista, imagem, citacao, tabela, perguntas, video, chamada) e seções de página inteira (cartoes, destaque, depoimentos, numeros, passos, planos, galeria, logos, faixa, contato); a página aceita também \"abertura\" (o topo com texto e botões). Para ver o formato de cada um, use listar_modelos e ver_modelo. As datas são do servidor.";

const LEGENDA: (&str, &str) = ("legenda", "A legenda, que aparece embaixo da imagem.");
const CREDITO: (&str, &str) = ("credito", "Quem leva o crédito, como aparece em \"Foto:\".");
const AUTORIA: (&str, &str) = ("autoria", "Quem fez a imagem.");
const AVISO: (&str, &str) = (
    "aviso",
    "O aviso de direitos autorais, como © 2026 Nome da empresa.",
);
const LICENCA: (&str, &str) = (
    "licenca",
    "O endereço da licença de uso: https:// ou um caminho do site.",
);
const AQUISICAO: (&str, &str) = (
    "aquisicao",
    "O endereço onde se pede a licença: https:// ou um caminho do site.",
);

/// O texto que acompanha uma imagem: legenda, crédito e direitos. É o que a
/// busca de imagens lê para o crédito e o selo de imagem licenciável.
const TEXTOS_DA_IMAGEM: &[(&str, &str)] = &[LEGENDA, CREDITO, AUTORIA, AVISO, LICENCA, AQUISICAO];

/// O que o dono define sobre o site. Os mesmos nomes saem em ver_site.
const IDENTIDADE: &[(&str, &str)] = &[
    ("nome", "O nome do site."),
    ("descricao", "O que é o negócio, em uma ou duas frases."),
    (
        "logo",
        "O id da imagem na biblioteca (listar_midia). Vazio tira a logo.",
    ),
    (
        "razaoSocial",
        "A razão social de quem está por trás do site.",
    ),
    ("telefone", "O telefone de contato."),
    ("email", "O e-mail de contato."),
    (
        "logradouro",
        "Rua e número. O endereço só vale com logradouro, cidade, uf e cep.",
    ),
    ("cidade", "A cidade."),
    ("uf", "A sigla do estado."),
    ("cep", "O CEP."),
    (
        "perfis",
        "Os perfis do negócio em outras redes, um endereço https por linha.",
    ),
    (
        "diretrizes",
        "Orientações para assistentes de IA sobre o site, uma por linha.",
    ),
];

const FERRAMENTAS: [Ferramenta; 21] = [
    Ferramenta {
        nome: "listar_sites",
        escopo: "sites:ler",
        descricao: "Lista os sites de que a conta participa, com o papel em cada um.",
        argumentos: &[],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "ver_site",
        escopo: "sites:ler",
        descricao: "Mostra a situação de um site, o papel da conta nele e a identidade: nome, descrição, logo e organização.",
        argumentos: &[SITE],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "criar_site",
        escopo: "sites:criar",
        descricao: "Cria um site em nome da conta, que vira dona dele. O site nasce em montagem, fora da busca.",
        argumentos: &[
            (
                "slug",
                "O endereço: de 3 a 40 letras minúsculas, números ou hífens.",
            ),
            ("nome", "O nome do site."),
        ],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "listar_documentos",
        escopo: "conteudo:ler",
        descricao: "Lista as páginas e os posts de um site, com a situação de cada um.",
        argumentos: &[SITE],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "ver_documento",
        escopo: "conteudo:ler",
        descricao: "Devolve o conteúdo de um documento: o rascunho, ou o que está no ar quando não há rascunho.",
        argumentos: &[SITE, DOCUMENTO],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "listar_modelos",
        escopo: "conteudo:ler",
        descricao: "Lista os modelos de página e de post: início, sobre, serviços, preços, contato, campanha e outros.",
        argumentos: &[],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "ver_modelo",
        escopo: "conteudo:ler",
        descricao: "Devolve o conteúdo de um modelo, no contrato do CMS, com todas as seções daquele tipo de página. Troque os textos de orientação pelos do negócio e mande em criar_rascunho.",
        argumentos: &[("modelo", "O id do modelo, como em listar_modelos.")],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "criar_rascunho",
        escopo: "conteudo:escrever",
        descricao: "Cria uma página ou um post como rascunho. Nunca é barrado: devolve o identificador e os problemas que impediriam publicar.",
        argumentos: &[SITE],
        com_conteudo: true,
        opcionais: &[],
    },
    Ferramenta {
        nome: "editar_rascunho",
        escopo: "conteudo:escrever",
        descricao: "Regrava o rascunho de um documento. Devolve os problemas que impediriam publicar.",
        argumentos: &[SITE, DOCUMENTO],
        com_conteudo: true,
        opcionais: &[],
    },
    Ferramenta {
        nome: "validar_documento",
        escopo: "conteudo:ler",
        descricao: "Devolve os problemas do rascunho, com código, campo e mensagem. Use para corrigir antes de pedir revisão.",
        argumentos: &[SITE, DOCUMENTO],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "enviar_para_revisao",
        escopo: "conteudo:escrever",
        descricao: "Pede a revisão do rascunho a um editor. É recusado se houver problema que bloqueia.",
        argumentos: &[SITE, DOCUMENTO],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "publicar",
        escopo: "conteudo:publicar",
        descricao: "Põe o rascunho no ar. Exige papel de editor ou dono no site.",
        argumentos: &[SITE, DOCUMENTO],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "despublicar",
        escopo: "conteudo:publicar",
        descricao: "Tira o documento do ar. O endereço passa a responder 410.",
        argumentos: &[SITE, DOCUMENTO],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "listar_midia",
        escopo: "midia:escrever",
        descricao: "Lista as imagens da biblioteca do site, com o id para usar no conteúdo, a legenda, o crédito e os direitos de cada uma.",
        argumentos: &[SITE],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "enviar_midia",
        escopo: "midia:escrever",
        descricao: "Envia uma imagem JPG, PNG ou WebP para a biblioteca. Ela fica pronta para uso em instantes.",
        argumentos: &[
            SITE,
            ("nome", "O nome do arquivo, como foto-da-fachada.jpg."),
            (
                "alt",
                "A descrição da imagem para quem não a enxerga. Obrigatória.",
            ),
            ("base64", "O arquivo, em base64."),
        ],
        com_conteudo: false,
        opcionais: TEXTOS_DA_IMAGEM,
    },
    Ferramenta {
        nome: "editar_midia",
        escopo: "midia:escrever",
        descricao: "Corrige a descrição, a legenda, o crédito e os direitos de uma imagem. Só o que for enviado muda. Onde a imagem já está no ar, a mudança entra na próxima publicação.",
        argumentos: &[SITE, ("id", "O id da imagem, como em listar_midia.")],
        com_conteudo: false,
        opcionais: &[
            (
                "alt",
                "A descrição da imagem para quem não a enxerga. Não pode ficar vazia.",
            ),
            LEGENDA,
            CREDITO,
            AUTORIA,
            AVISO,
            LICENCA,
            AQUISICAO,
        ],
    },
    Ferramenta {
        nome: "listar_autores",
        escopo: "conteudo:ler",
        descricao: "Lista os autores cadastrados no site.",
        argumentos: &[SITE],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "listar_categorias",
        escopo: "conteudo:ler",
        descricao: "Lista as categorias do blog do site.",
        argumentos: &[SITE],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "salvar_autor",
        escopo: "conteudo:escrever",
        descricao: "Cadastra um autor, ou regrava o que já existe com o mesmo nome. Post só vai ao ar com autor que tem foto e bio. Exige papel de editor ou dono.",
        argumentos: &[SITE, ("nome", "O nome de quem assina.")],
        com_conteudo: false,
        opcionais: &[
            ("cargo", "O cargo ou a ocupação."),
            ("bio", "Quem é a pessoa, em poucas frases."),
            ("foto", "O id da foto na biblioteca (listar_midia)."),
            (
                "perfis",
                "Os perfis da pessoa, um endereço https por linha.",
            ),
            (
                "credenciais",
                "Formação, registro ou experiência, uma por linha.",
            ),
        ],
    },
    Ferramenta {
        nome: "salvar_categoria",
        escopo: "conteudo:escrever",
        descricao: "Cadastra uma categoria do blog, ou regrava o nome da que tem o mesmo endereço. Exige papel de editor ou dono.",
        argumentos: &[SITE, ("nome", "O nome da categoria.")],
        com_conteudo: false,
        opcionais: &[],
    },
    Ferramenta {
        nome: "editar_identidade",
        escopo: "sites:editar",
        descricao: "Muda a identidade do site. Só o que for enviado muda; o resto fica como está. Exige papel de dono.",
        argumentos: &[SITE],
        com_conteudo: false,
        opcionais: IDENTIDADE,
    },
];

fn descrever(ferramenta: &Ferramenta) -> Value {
    let mut propriedades = serde_json::Map::new();
    let mut obrigatorios: Vec<&str> = Vec::new();
    for (nome, descricao) in ferramenta.argumentos {
        propriedades.insert(
            (*nome).to_string(),
            json!({ "type": "string", "description": descricao }),
        );
        obrigatorios.push(nome);
    }
    for (nome, descricao) in ferramenta.opcionais {
        propriedades.insert(
            (*nome).to_string(),
            json!({ "type": "string", "description": descricao }),
        );
    }
    if ferramenta.com_conteudo {
        propriedades.insert(
            "conteudo".to_string(),
            json!({ "type": "object", "description": DESCRICAO_DO_CONTEUDO }),
        );
        obrigatorios.push("conteudo");
    }
    json!({
        "name": ferramenta.nome,
        "description": ferramenta.descricao,
        "inputSchema": { "type": "object", "properties": propriedades, "required": obrigatorios },
    })
}

/// O que uma ferramenta devolve: o resultado, e os identificadores que vão
/// para o registro de chamadas.
struct Feito {
    resultado: Value,
    site_id: Option<Uuid>,
    documento_id: Option<Uuid>,
}

/// Uma recusa que o assistente lê e pode corrigir.
enum Recusa {
    Mensagem(String),
    Interna(ErroWeb),
}

impl From<ErroWeb> for Recusa {
    fn from(erro: ErroWeb) -> Self {
        Recusa::Interna(erro)
    }
}

impl From<cms_dados::ErroDeDados> for Recusa {
    fn from(erro: cms_dados::ErroDeDados) -> Self {
        Recusa::Interna(erro.into())
    }
}

fn recusa(mensagem: impl Into<String>) -> Recusa {
    Recusa::Mensagem(mensagem.into())
}

fn problemas_em_json(problemas: &[Problema]) -> Value {
    json!(problemas)
}

fn do_fluxo(erro: ErroDeFluxo) -> Recusa {
    match erro {
        ErroDeFluxo::Dados(erro) => Recusa::Interna(erro.into()),
        ErroDeFluxo::Recusado(problemas) => recusa(format!(
            "Recusado: o documento tem problemas que bloqueiam. {}",
            problemas_em_json(&problemas)
        )),
        outro => recusa(outro.to_string()),
    }
}

/// O contexto de uma chamada: a conexão, os argumentos e o site já resolvido
/// com o papel da conta nele.
struct Chamada<'a> {
    estado: &'a Estado,
    conexao: &'a Conexao,
    argumentos: &'a Value,
}

impl Chamada<'_> {
    fn texto(&self, nome: &str) -> Result<&str, Recusa> {
        self.argumentos
            .get(nome)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|texto| !texto.is_empty())
            .ok_or_else(|| recusa(format!("Falta o argumento \"{nome}\".")))
    }

    /// Um argumento que pode faltar. Vazio é diferente de ausente: vazio apaga.
    fn opcional(&self, nome: &str) -> Option<&str> {
        self.argumentos.get(nome).and_then(Value::as_str)
    }

    /// O site pedido e o ator da conta nele. Site de que a conta não
    /// participa é tratado como site que não existe.
    async fn site(&self) -> Result<(cms_dados::SiteGravado, Ator), Recusa> {
        let slug = self.texto("site")?;
        let nao_achado = || recusa(format!("Site \"{slug}\" não encontrado nesta conta."));
        let site = cms_dados::site_por_slug(&self.estado.pool, slug)
            .await?
            .ok_or_else(nao_achado)?;
        let papel =
            cms_dados::papel_no_site(&self.estado.pool, site.id, &self.conexao.conta.sub).await?;
        let ator = self.conexao.conta.ator(papel).ok_or_else(nao_achado)?;
        Ok((site, ator))
    }

    fn documento(&self) -> Result<Uuid, Recusa> {
        Uuid::parse_str(self.texto("documento")?)
            .map_err(|_| recusa("O argumento \"documento\" não é um identificador válido."))
    }

    /// Uma imagem citada só pelo id vira a imagem da biblioteca.
    async fn imagem(&self, site_id: Uuid, valor: &mut Value) -> Result<(), Recusa> {
        let Some(id) = valor.as_str() else {
            return Ok(());
        };
        let nao_achada = || recusa(format!("Imagem \"{id}\" não está na biblioteca do site."));
        let midia_id = Uuid::parse_str(id.trim()).map_err(|_| nao_achada())?;
        let midia = cms_dados::midia_para_conteudo(&self.estado.pool, site_id, midia_id)
            .await?
            .ok_or_else(nao_achada)?;
        *valor = json!(midia);
        Ok(())
    }

    /// O que veio só como referência (slug de autor ou de categoria, id de
    /// imagem) é trocado pelo cadastro do site.
    async fn resolver_referencias(
        &self,
        site_id: Uuid,
        dados: &mut serde_json::Map<String, Value>,
    ) -> Result<(), Recusa> {
        let pool = &self.estado.pool;
        if let Some(slug) = dados.get("autor").and_then(Value::as_str) {
            let autor = cms_dados::autor_para_conteudo(pool, site_id, slug.trim())
                .await?
                .ok_or_else(|| {
                    recusa(format!(
                        "Autor \"{slug}\" não está cadastrado. Use salvar_autor."
                    ))
                })?;
            dados.insert("autor".into(), json!(autor));
        }
        if let Some(slug) = dados.get("categoria").and_then(Value::as_str) {
            let categoria = cms_dados::categorias_do_site(pool, site_id)
                .await?
                .into_iter()
                .find(|categoria| categoria.slug == slug.trim())
                .ok_or_else(|| {
                    recusa(format!(
                        "Categoria \"{slug}\" não está cadastrada. Use salvar_categoria."
                    ))
                })?;
            dados.insert("categoria".into(), json!(categoria));
        }
        if let Some(capa) = dados.get_mut("capa") {
            self.imagem(site_id, capa).await?;
        }
        let blocos = dados.get_mut("corpo").and_then(Value::as_array_mut);
        for bloco in blocos.into_iter().flatten() {
            if let Some(midia) = bloco.get_mut("midia") {
                self.imagem(site_id, midia).await?;
            }
            // Nas seções, a imagem está em cada item: o item inteiro (galeria
            // e logos), a imagem do cartão ou a foto do depoimento.
            let itens = bloco.get_mut("itens").and_then(Value::as_array_mut);
            for item in itens.into_iter().flatten() {
                self.imagem(site_id, item).await?;
                for campo in ["midia", "foto"] {
                    if let Some(midia) = item.get_mut(campo) {
                        self.imagem(site_id, midia).await?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Lê `conteudo` no contrato do motor. As datas são do servidor: se não
    /// vierem, entram provisórias, e a publicação grava as de verdade.
    async fn conteudo(&self, site_id: Uuid) -> Result<Conteudo, Recusa> {
        let mut valor = self
            .argumentos
            .get("conteudo")
            .cloned()
            .ok_or_else(|| recusa("Falta o argumento \"conteudo\"."))?;
        let agora = json!(Utc::now());
        if let Some(dados) = valor.get_mut("dados").and_then(Value::as_object_mut) {
            for campo in ["publicadoEm", "atualizadoEm"] {
                dados.entry(campo).or_insert_with(|| agora.clone());
            }
            self.resolver_referencias(site_id, dados).await?;
        }
        let conteudo: Conteudo = serde_json::from_value(valor)
            .map_err(|erro| recusa(format!("O conteúdo não segue o contrato: {erro}")))?;
        if matches!(conteudo, Conteudo::Produto(_)) {
            return Err(recusa(
                "O CMS não guarda produto: use \"pagina\" ou \"post\".",
            ));
        }
        Ok(conteudo)
    }
}

/// A identidade com os nomes que `editar_identidade` recebe.
fn identidade_em_json(dados: &mut DadosDaIdentidade) -> Value {
    IDENTIDADE
        .iter()
        .filter_map(|(nome, _)| Some(((*nome).to_string(), json!(dados.campo(nome)?))))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

fn do_catalogo(erro: ErroDeCatalogo) -> Recusa {
    match erro {
        ErroDeCatalogo::Dados(erro) => Recusa::Interna(erro.into()),
        outro => recusa(outro.to_string()),
    }
}

async fn executar(ferramenta: &str, chamada: &Chamada<'_>) -> Result<Feito, Recusa> {
    let pool = &chamada.estado.pool;
    let conta = &chamada.conexao.conta;
    let da_conta = |resultado: Value| Feito {
        resultado,
        site_id: None,
        documento_id: None,
    };
    if ferramenta == "listar_sites" {
        let sites: Vec<Value> = cms_dados::sites_da_conta(pool, &conta.sub)
            .await?
            .into_iter()
            .map(|site| {
                json!({
                    "site": site.slug,
                    "nome": site.nome,
                    "situacao": site.situacao.como_texto(),
                    "papel": site.papel.como_texto(),
                })
            })
            .collect();
        return Ok(da_conta(json!({ "sites": sites })));
    }
    if ferramenta == "listar_modelos" {
        let modelos: Vec<Value> = MODELOS
            .iter()
            .map(|modelo| {
                json!({
                    "modelo": modelo.id,
                    "nome": modelo.nome,
                    "descricao": modelo.descricao,
                    "especie": if modelo.eh_post { "post" } else { "pagina" },
                })
            })
            .collect();
        return Ok(da_conta(json!({ "modelos": modelos })));
    }
    if ferramenta == "ver_modelo" {
        let id = chamada.texto("modelo")?;
        let conteudo = conteudo_do_modelo(id, Utc::now())
            .ok_or_else(|| recusa(format!("Modelo \"{id}\" não existe. Use listar_modelos.")))?;
        return Ok(da_conta(json!({ "conteudo": conteudo })));
    }
    if ferramenta == "criar_site" {
        let slug = chamada.texto("slug")?.to_lowercase();
        let limites = chamada.estado.configuracao.limites_de_criacao;
        return match cms_dados::criar_site_para(pool, conta, &slug, chamada.texto("nome")?, limites)
            .await
        {
            Ok(id) => Ok(Feito {
                resultado: json!({ "site": slug, "situacao": "em-montagem" }),
                site_id: Some(id),
                documento_id: None,
            }),
            Err(cms_dados::ErroDeConta::Dados(erro)) => Err(Recusa::Interna(erro.into())),
            Err(erro) => Err(recusa(erro.to_string())),
        };
    }

    let (site, ator) = chamada.site().await?;
    let do_site = |resultado: Value, documento_id: Option<Uuid>| Feito {
        resultado,
        site_id: Some(site.id),
        documento_id,
    };
    match ferramenta {
        "ver_site" => Ok(do_site(
            json!({
                "site": site.slug,
                "situacao": site.situacao.como_texto(),
                "papel": ator.papel.como_texto(),
                "dominioProprio": site.dominio_ativo,
                "identidade": identidade_em_json(&mut DadosDaIdentidade::do_perfil(&site.perfil)),
            }),
            None,
        )),
        "editar_identidade" => {
            let mut dados = DadosDaIdentidade::do_perfil(&site.perfil);
            for (nome, _) in IDENTIDADE {
                if let (Some(valor), Some(campo)) = (chamada.opcional(nome), dados.campo(nome)) {
                    *campo = valor.to_string();
                }
            }
            match identidade::aplicar(chamada.estado, &site, &ator, &dados).await? {
                Ok(()) => Ok(do_site(
                    json!({ "identidade": identidade_em_json(&mut dados) }),
                    None,
                )),
                Err(motivo) => Err(recusa(motivo)),
            }
        }
        "salvar_autor" => {
            let foto_id =
                match chamada.opcional("foto").map(str::trim) {
                    None | Some("") => None,
                    Some(id) => Some(Uuid::parse_str(id).map_err(|_| {
                        recusa("O argumento \"foto\" não é um identificador válido.")
                    })?),
                };
            let texto = |nome: &str| chamada.opcional(nome).unwrap_or_default().to_string();
            let dados = DadosDoAutor {
                nome: chamada.texto("nome")?.to_string(),
                cargo: texto("cargo"),
                bio: texto("bio"),
                foto_id,
                perfis: texto("perfis"),
                credenciais: texto("credenciais"),
            };
            let slug = cms_dados::salvar_autor(pool, site.id, &ator, &dados)
                .await
                .map_err(do_catalogo)?;
            Ok(do_site(json!({ "slug": slug }), None))
        }
        "salvar_categoria" => {
            let slug = cms_dados::salvar_categoria(pool, site.id, &ator, chamada.texto("nome")?)
                .await
                .map_err(do_catalogo)?;
            Ok(do_site(json!({ "slug": slug }), None))
        }
        "listar_documentos" => {
            let documentos: Vec<Value> = cms_dados::documentos_do_site(pool, site.id)
                .await?
                .into_iter()
                .map(|documento| {
                    json!({
                        "documento": documento.id,
                        "especie": documento.especie,
                        "titulo": documento.titulo,
                        "situacao": documento.situacao,
                        "temRascunho": documento.tem_rascunho,
                        "emRevisao": documento.em_revisao,
                        "caminho": documento.caminho,
                    })
                })
                .collect();
            Ok(do_site(json!({ "documentos": documentos }), None))
        }
        "listar_midia" => {
            let midias: Vec<Value> = cms_dados::midias_do_site(pool, site.id)
                .await?
                .into_iter()
                .map(|midia| {
                    json!({
                        "id": midia.id,
                        "alt": midia.alt,
                        "largura": midia.largura,
                        "altura": midia.altura,
                        "situacao": midia.situacao,
                        "legenda": midia.legenda,
                        "credito": midia.credito,
                        "autoria": midia.autoria,
                        "aviso": midia.aviso,
                        "licenca": midia.licenca,
                        "aquisicao": midia.aquisicao,
                    })
                })
                .collect();
            Ok(do_site(json!({ "midias": midias }), None))
        }
        "enviar_midia" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(chamada.texto("base64")?)
                .map_err(|_| recusa("O argumento \"base64\" não é base64 válido."))?;
            let texto = |nome: &str| chamada.opcional(nome).map(str::to_string);
            let envio = biblioteca::Envio {
                nome: chamada.texto("nome")?.to_string(),
                bytes: bytes.into(),
                alt: chamada.texto("alt")?.to_string(),
                legenda: texto("legenda").unwrap_or_default(),
                credito: texto("credito").unwrap_or_default(),
                direitos: Direitos {
                    autoria: texto("autoria"),
                    aviso: texto("aviso"),
                    licenca: texto("licenca"),
                    aquisicao: texto("aquisicao"),
                },
            };
            match biblioteca::registrar(chamada.estado, &site, &ator, envio).await? {
                Ok(id) => Ok(do_site(json!({ "id": id, "situacao": "pendente" }), None)),
                Err(mensagem) => Err(recusa(mensagem)),
            }
        }
        "editar_midia" => {
            let nao_achada = || recusa("Imagem não encontrada na biblioteca do site.");
            let id = Uuid::parse_str(chamada.texto("id")?).map_err(|_| nao_achada())?;
            let atual = cms_dados::midias_do_site(pool, site.id)
                .await?
                .into_iter()
                .find(|midia| midia.id == id)
                .ok_or_else(nao_achada)?;
            // O que não veio fica como está; vazio apaga.
            let novo = |nome: &str, atual: Option<String>| {
                chamada.opcional(nome).map(str::to_string).or(atual)
            };
            let descricao = DescricaoDaMidia {
                alt: novo("alt", Some(atual.alt)).unwrap_or_default(),
                legenda: novo("legenda", atual.legenda),
                credito: novo("credito", atual.credito),
                direitos: Direitos {
                    autoria: novo("autoria", atual.autoria),
                    aviso: novo("aviso", atual.aviso),
                    licenca: novo("licenca", atual.licenca),
                    aquisicao: novo("aquisicao", atual.aquisicao),
                },
            };
            match cms_dados::atualizar_midia(pool, site.id, &ator, id, &descricao).await {
                Ok(()) => Ok(do_site(json!({ "id": id }), None)),
                Err(ErroDeMidia::Dados(erro)) => Err(Recusa::Interna(erro.into())),
                Err(erro) => Err(recusa(erro.to_string())),
            }
        }
        "listar_autores" => {
            let autores: Vec<Value> = cms_dados::autores_do_site(pool, site.id)
                .await?
                .into_iter()
                .map(|autor| {
                    json!({
                        "slug": autor.slug,
                        "nome": autor.nome,
                        "cargo": autor.cargo,
                        "bio": autor.bio,
                        "foto": autor.foto_id,
                        "perfis": autor.perfis,
                        "credenciais": autor.credenciais,
                    })
                })
                .collect();
            Ok(do_site(json!({ "autores": autores }), None))
        }
        "listar_categorias" => Ok(do_site(
            json!({ "categorias": cms_dados::categorias_do_site(pool, site.id).await? }),
            None,
        )),
        "criar_rascunho" | "editar_rascunho" => {
            let documento_id = if ferramenta == "editar_rascunho" {
                Some(chamada.documento()?)
            } else {
                None
            };
            let mut conteudo = chamada.conteudo(site.id).await?;
            // O que o assistente escreveu sobre a imagem não vale: dimensões e
            // variantes são as da biblioteca.
            cms_dados::hidratar_conteudo(pool, site.id, &mut conteudo).await?;
            let salvo = fluxo::salvar_rascunho(pool, site.id, &ator, documento_id, &conteudo)
                .await
                .map_err(do_fluxo)?;
            Ok(do_site(
                json!({ "documento": salvo.documento_id, "problemas": problemas_em_json(&salvo.problemas) }),
                Some(salvo.documento_id),
            ))
        }
        _ => {
            let id = chamada.documento()?;
            let resultado = match ferramenta {
                "ver_documento" => {
                    let aberto = cms_dados::abrir_documento(pool, site.id, id)
                        .await?
                        .ok_or_else(|| recusa("Documento não encontrado neste site."))?;
                    json!({
                        "documento": aberto.id,
                        "situacao": aberto.situacao,
                        "temRascunho": aberto.tem_rascunho,
                        "emRevisao": aberto.em_revisao,
                        "caminho": aberto.caminho,
                        "conteudo": aberto.conteudo,
                    })
                }
                "validar_documento" => {
                    let problemas = fluxo::problemas_do_rascunho(pool, site.id, id)
                        .await
                        .map_err(do_fluxo)?;
                    json!({ "problemas": problemas_em_json(&problemas) })
                }
                "enviar_para_revisao" => {
                    fluxo::enviar_para_revisao(pool, site.id, &ator, id)
                        .await
                        .map_err(do_fluxo)?;
                    json!({ "situacao": "em revisão" })
                }
                "publicar" => {
                    fluxo::publicar(pool, site.id, &ator, id, Utc::now())
                        .await
                        .map_err(do_fluxo)?;
                    chamada.estado.cache.invalidar_site(site.id);
                    json!({ "situacao": "publicado" })
                }
                "despublicar" => {
                    fluxo::despublicar(pool, site.id, &ator, id)
                        .await
                        .map_err(do_fluxo)?;
                    chamada.estado.cache.invalidar_site(site.id);
                    json!({ "situacao": "despublicado" })
                }
                _ => return Err(recusa("Ferramenta desconhecida.")),
            };
            Ok(do_site(resultado, Some(id)))
        }
    }
}

/// O resultado de uma ferramenta, no formato do MCP.
fn conteudo_de_ferramenta(texto: String, eh_erro: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": texto }], "isError": eh_erro })
}

async fn chamar(estado: &Estado, conexao: &Conexao, parametros: &Value) -> Result<Value, ErroWeb> {
    let nome = parametros.get("name").and_then(Value::as_str).unwrap_or("");
    let Some(ferramenta) = FERRAMENTAS.iter().find(|f| f.nome == nome) else {
        return Ok(conteudo_de_ferramenta(
            format!("Ferramenta desconhecida: {nome}"),
            true,
        ));
    };
    if !conexao.tem(ferramenta.escopo) {
        cms_dados::registrar_chamada(&estado.pool, conexao.id, ferramenta.nome, None, None, false)
            .await?;
        return Ok(conteudo_de_ferramenta(
            format!(
                "Esta conexão não tem a permissão \"{}\". A pessoa precisa autorizar de novo, marcando essa permissão.",
                ferramenta.escopo
            ),
            true,
        ));
    }
    let vazio = json!({});
    let chamada = Chamada {
        estado,
        conexao,
        argumentos: parametros.get("arguments").unwrap_or(&vazio),
    };
    let (texto, eh_erro, site_id, documento_id) = match executar(ferramenta.nome, &chamada).await {
        Ok(feito) => (
            feito.resultado.to_string(),
            false,
            feito.site_id,
            feito.documento_id,
        ),
        Err(Recusa::Mensagem(mensagem)) => (mensagem, true, None, None),
        Err(Recusa::Interna(erro)) => return Err(erro),
    };
    cms_dados::registrar_chamada(
        &estado.pool,
        conexao.id,
        ferramenta.nome,
        site_id,
        documento_id,
        !eh_erro,
    )
    .await?;
    Ok(conteudo_de_ferramenta(texto, eh_erro))
}

fn token_do_pedido(cabecalhos: &HeaderMap) -> Option<&str> {
    cabecalhos
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

/// `POST /mcp`: JSON-RPC do MCP, uma mensagem por pedido.
pub async fn mcp(
    estado: &Estado,
    origem: &str,
    metodo: &Method,
    cabecalhos: &HeaderMap,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let conexao = match token_do_pedido(cabecalhos) {
        Some(token) => cms_dados::conexao_por_token(&estado.pool, token).await?,
        None => None,
    };
    let Some(conexao) = conexao else {
        // É por este cabeçalho que o assistente descobre onde pedir acesso.
        let mut resposta = erro_oauth(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Autorize o conector para usar esta conta.",
        );
        let desafio =
            format!("Bearer resource_metadata=\"{origem}/.well-known/oauth-protected-resource\"");
        if let Ok(valor) = HeaderValue::from_str(&desafio) {
            resposta.headers_mut().insert(WWW_AUTHENTICATE, valor);
        }
        return Ok(resposta);
    };
    if metodo != Method::POST {
        return Ok(simples(
            StatusCode::METHOD_NOT_ALLOWED,
            "Use POST com uma mensagem JSON-RPC.",
        ));
    }
    let Ok(mensagem) = serde_json::from_slice::<Value>(corpo) else {
        return Ok(json_com(
            StatusCode::BAD_REQUEST,
            json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "JSON inválido." } }),
        ));
    };
    let Some(id) = mensagem.get("id").filter(|id| !id.is_null()).cloned() else {
        // Notificação: não tem resposta.
        return Ok(StatusCode::ACCEPTED.into_response());
    };
    let nulo = Value::Null;
    let parametros = mensagem.get("params").unwrap_or(&nulo);
    let resultado = match mensagem.get("method").and_then(Value::as_str).unwrap_or("") {
        "initialize" => Ok(json!({
            "protocolVersion": VERSAO_DO_PROTOCOLO,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "cms-avila-ops", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "CMS da Ávila Ops. Escreva rascunhos, confira com validar_documento e envie para revisão; publicar depende de a pessoa ter liberado.",
        })),
        "ping" => Ok(json!({})),
        "tools/list" => {
            Ok(json!({ "tools": FERRAMENTAS.iter().map(descrever).collect::<Vec<_>>() }))
        }
        "tools/call" => Ok(chamar(estado, &conexao, parametros).await?),
        _ => Err(json!({ "code": -32601, "message": "Método desconhecido." })),
    };
    Ok(json_com(
        StatusCode::OK,
        match resultado {
            Ok(resultado) => json!({ "jsonrpc": "2.0", "id": id, "result": resultado }),
            Err(erro) => json!({ "jsonrpc": "2.0", "id": id, "error": erro }),
        },
    ))
}
