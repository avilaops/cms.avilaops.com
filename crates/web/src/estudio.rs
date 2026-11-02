//! Estúdio do site: arquivos, canvas e prévia publicada.
//!
//! A geração por chat não roda aqui. O pedido fica gravado para a fase
//! seguinte. A biblioteca de documentação insere componentes HTML; não há
//! React nem busca indexada.

use askama::Template;
use axum::http::header::CACHE_CONTROL;
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use cms_dados::{ArquivoListado, ErroDeEstudio, NoAberto, NovoArquivo, NovoNo};
use cms_dominio::conta::pode_administrar;
use cms_dominio::estudio::{
    self, Medida, NoDaDefinicao, PropsDeComponente, TIPO_COMPONENTE, TIPO_FRAME, TIPO_RETANGULO,
    TIPO_TEXTO, VERSAO_DO_CATALOGO_DOCS,
};
use cms_dominio::{Ator, Conta};
use serde::Deserialize;
use uuid::Uuid;

use crate::Estado;
use crate::acesso::ao_site;
use crate::modelos::arquivo_pronto;
use crate::resposta::{ErroWeb, html, html_privado, liberar_script_proprio, redirecionar, simples};

const HTMX: &str = include_str!("../recursos/htmx-2.0.7.min.js");
const SCRIPT: &str = include_str!("../recursos/estudio.js");

fn caminho_com_resumo(prefixo: &str, bytes: &[u8]) -> String {
    let resumo = cms_dados::hash_de_conteudo(bytes);
    format!("/recursos/{prefixo}-{}.js", &resumo[..12])
}

pub fn caminho_htmx() -> &'static str {
    static CAMINHO: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| caminho_com_resumo("htmx", HTMX.as_bytes()));
    &CAMINHO
}

pub fn caminho_do_script() -> &'static str {
    static CAMINHO: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| caminho_com_resumo("estudio", SCRIPT.as_bytes()));
    &CAMINHO
}

pub fn recurso(caminho: &str) -> Option<Response> {
    let corpo = if caminho == caminho_htmx() {
        HTMX
    } else if caminho == caminho_do_script() {
        SCRIPT
    } else {
        return None;
    };
    Some(arquivo_pronto(corpo, "text/javascript; charset=utf-8"))
}

pub fn token_publico(caminho: &str) -> Option<&str> {
    let token = caminho.strip_prefix("/estudio/p/")?;
    let ok = token.len() == 64
        && token
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    ok.then_some(token)
}

struct Destino {
    id: String,
    nome: String,
}

struct SistemaVisivel {
    slug: String,
    nome: String,
    fundo: String,
    destaque: String,
    texto: String,
    selecionado: bool,
}

struct ArquivoVisivel {
    nome: String,
    href: String,
    quando: String,
    etiquetas: String,
}

struct PecaVisivel {
    nome: String,
    variante: String,
}

struct NoVisivel {
    id: String,
    tipo: String,
    rotulo: String,
    x: i32,
    y: i32,
    largura: i32,
    altura: i32,
    texto: String,
    preenchimento: String,
    variante: String,
    tom_var: String,
    titulo: String,
    descricao: String,
    aba_a: String,
    aba_b: String,
    painel_a: String,
    painel_b: String,
    anterior: String,
    proximo: String,
    itens: Vec<String>,
    rotulo_busca: String,
}

#[derive(Template)]
#[template(path = "estudio.html")]
struct PaginaDoEstudio {
    site: String,
    slug: String,
    dono: bool,
    aba: String,
    pedido: String,
    erro: String,
    aviso: String,
    destinos: Vec<Destino>,
    sistemas: Vec<SistemaVisivel>,
    arquivos: Vec<ArquivoVisivel>,
    biblioteca_ativa: bool,
    biblioteca_versao: String,
    pode_ativar: bool,
    pecas: Vec<PecaVisivel>,
    script_htmx: String,
}

#[derive(Template)]
#[template(path = "estudio_arquivo.html")]
struct PaginaDoArquivo {
    site: String,
    slug: String,
    dono: bool,
    nome: String,
    arquivo_id: String,
    sistema_slug: String,
    sistema_nome: String,
    sistemas: Vec<SistemaVisivel>,
    favorito: bool,
    edita: bool,
    pode_publicar: bool,
    link_da_previa: String,
    pedido: String,
    aviso: String,
    biblioteca_ativa: bool,
    biblioteca_versao: String,
    pecas: Vec<PecaVisivel>,
    largura: i32,
    altura: i32,
    fundo: String,
    superficie: String,
    cor_texto: String,
    suave: String,
    destaque: String,
    borda: String,
    fonte_titulo: String,
    fonte_corpo: String,
    nos: Vec<NoVisivel>,
    script_htmx: String,
    script: String,
}

#[derive(Template)]
#[template(path = "estudio_previa.html")]
struct PaginaDePrevia {
    nome: String,
    edita: bool,
    slug: String,
    arquivo_id: String,
    largura: i32,
    altura: i32,
    fundo: String,
    superficie: String,
    cor_texto: String,
    suave: String,
    destaque: String,
    borda: String,
    fonte_titulo: String,
    fonte_corpo: String,
    nos: Vec<NoVisivel>,
    script: String,
}

#[derive(Deserialize, Default)]
struct FormularioDeArquivo {
    #[serde(default)]
    pedido: String,
    #[serde(default)]
    arquivo: String,
    #[serde(default)]
    sistema: String,
    #[serde(default)]
    atalho: String,
}

#[derive(Deserialize, Default)]
struct FormularioDoNo {
    #[serde(default)]
    tipo: String,
    #[serde(default)]
    variante: String,
    #[serde(default)]
    x: String,
    #[serde(default)]
    y: String,
}

#[derive(Deserialize, Default)]
struct FormularioDoSistema {
    #[serde(default)]
    sistema: String,
}

#[derive(Deserialize, Default)]
struct FormularioDaBiblioteca {
    #[serde(default)]
    ativa: String,
}

#[derive(Deserialize, Default)]
struct FormularioDoMovimento {
    #[serde(default)]
    x: String,
    #[serde(default)]
    y: String,
}

fn html_do_estudio(status: StatusCode, corpo: String) -> Response {
    let mut resposta = html_privado(status, corpo);
    liberar_script_proprio(&mut resposta);
    resposta
}

fn html_da_previa(corpo: String) -> Response {
    let mut resposta = html(StatusCode::OK, corpo, false);
    resposta
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    liberar_script_proprio(&mut resposta);
    resposta
}

fn consulta<'a>(uri: &'a Uri, chave: &str) -> Option<&'a str> {
    uri.query()?.split('&').find_map(|par| {
        let (nome, valor) = par.split_once('=')?;
        (nome == chave).then_some(valor)
    })
}

fn textos(uri: &Uri) -> (String, String) {
    let aviso = match consulta(uri, "aviso") {
        Some("criado") => "Arquivo criado. A geração por chat entra na fase seguinte.",
        Some("pedido") => {
            "Pedido guardado neste arquivo. A geração por chat entra na fase seguinte."
        }
        Some("sistema") => "Sistema de design aplicado.",
        Some("previa") => "Prévia publicada. O link abaixo é somente leitura.",
        Some("favorito") => "Favorito atualizado.",
        Some("biblioteca") => "Biblioteca atualizada.",
        _ => "",
    };
    (aviso.to_string(), String::new())
}

fn aba_de(uri: &Uri) -> &'static str {
    match consulta(uri, "aba") {
        Some("favoritos") => "favoritos",
        Some("bibliotecas") => "bibliotecas",
        Some("compartilhados") => "compartilhados",
        _ => "recentes",
    }
}

fn sistemas_visiveis(selecionado: &str) -> Vec<SistemaVisivel> {
    estudio::sistemas()
        .iter()
        .map(|sistema| SistemaVisivel {
            selecionado: sistema.slug == selecionado,
            slug: sistema.slug.to_string(),
            nome: sistema.nome.to_string(),
            fundo: sistema.tokens.fundo.clone(),
            destaque: sistema.tokens.destaque.clone(),
            texto: sistema.tokens.texto.clone(),
        })
        .collect()
}

fn pecas_visiveis() -> Vec<PecaVisivel> {
    estudio::pecas_docs()
        .iter()
        .map(|peca| PecaVisivel {
            nome: peca.nome.to_string(),
            variante: peca.variante.to_string(),
        })
        .collect()
}

struct NoBruto<'a> {
    id: String,
    tipo: &'a str,
    x: i32,
    y: i32,
    largura: i32,
    altura: i32,
    texto: &'a str,
    preenchimento: Option<&'a str>,
    variante: Option<&'a str>,
    props: &'a PropsDeComponente,
}

fn no_visivel(no: NoBruto<'_>) -> NoVisivel {
    let variante = no.variante.unwrap_or("");
    let rotulo = estudio::peca_docs(variante)
        .map(|peca| peca.nome)
        .unwrap_or_else(|| estudio::rotulo_do_tipo(no.tipo));
    NoVisivel {
        id: no.id,
        tipo: no.tipo.to_string(),
        rotulo: rotulo.to_string(),
        x: no.x,
        y: no.y,
        largura: no.largura,
        altura: no.altura,
        texto: if no.texto.is_empty() {
            no.props.texto.clone()
        } else {
            no.texto.to_string()
        },
        preenchimento: no
            .preenchimento
            .filter(|cor| estudio::cor_valida(cor))
            .unwrap_or("")
            .to_string(),
        tom_var: estudio::variavel_do_tom(&no.props.tom)
            .unwrap_or("--docs-info")
            .to_string(),
        titulo: no.props.titulo.clone(),
        descricao: no.props.descricao.clone(),
        aba_a: no.props.aba_a.clone(),
        aba_b: no.props.aba_b.clone(),
        painel_a: no.props.painel_a.clone(),
        painel_b: no.props.painel_b.clone(),
        anterior: no.props.anterior.clone(),
        proximo: no.props.proximo.clone(),
        itens: no.props.itens.clone(),
        rotulo_busca: no.props.rotulo.clone(),
        variante: variante.to_string(),
    }
}

fn no_do_arquivo(no: &NoAberto) -> NoVisivel {
    no_visivel(NoBruto {
        id: no.id.to_string(),
        tipo: &no.tipo,
        x: no.x,
        y: no.y,
        largura: no.largura,
        altura: no.altura,
        texto: &no.texto,
        preenchimento: no.preenchimento.as_deref(),
        variante: no.variante.as_deref(),
        props: &no.props,
    })
}

fn no_da_definicao(indice: usize, no: &NoDaDefinicao) -> NoVisivel {
    no_visivel(NoBruto {
        id: indice.to_string(),
        tipo: &no.tipo,
        x: no.x,
        y: no.y,
        largura: no.largura,
        altura: no.altura,
        texto: &no.texto,
        preenchimento: no.preenchimento.as_deref(),
        variante: no.variante.as_deref(),
        props: &no.props,
    })
}

macro_rules! ou_falha {
    ($resultado:expr) => {
        match $resultado {
            Ok(valor) => valor,
            Err(erro) => return falha(erro),
        }
    };
}

fn falha(erro: ErroDeEstudio) -> Result<Response, ErroWeb> {
    match erro {
        ErroDeEstudio::Dados(erro) => Err(erro.into()),
        ErroDeEstudio::NaoEncontrado | ErroDeEstudio::NoNaoEncontrado => {
            Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."))
        }
        ErroDeEstudio::PosicaoInvalida => {
            Ok(simples(StatusCode::BAD_REQUEST, "O nó não cabe na página."))
        }
        ErroDeEstudio::BibliotecaInativa => Ok(simples(
            StatusCode::FORBIDDEN,
            "A biblioteca de documentação não está ativa neste site.",
        )),
        ErroDeEstudio::LimiteDeArquivos => Ok(simples(
            StatusCode::CONFLICT,
            "Este site chegou ao limite de arquivos do estúdio.",
        )),
        ErroDeEstudio::LimiteDeNos => Ok(simples(
            StatusCode::CONFLICT,
            "Este arquivo chegou ao limite de nós.",
        )),
    }
}

fn sem_permissao() -> Response {
    simples(StatusCode::FORBIDDEN, "Você não pode alterar este arquivo.")
}

fn formulario_invalido() -> Response {
    simples(StatusCode::BAD_REQUEST, "Formulário inválido.")
}

fn volta(slug: &str, arquivo: Uuid, aviso: &str) -> Response {
    redirecionar(
        StatusCode::SEE_OTHER,
        &format!("/painel/sites/{slug}/estudio/{arquivo}?aviso={aviso}"),
    )
}

pub async fn inicio(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    abrir_home(estado, conta, slug, uri, String::new(), "padrao").await
}

async fn abrir_home(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    uri: &Uri,
    erro: String,
    sistema_escolhido: &str,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let arquivos = ou_falha!(cms_dados::listar_arquivos(&estado.pool, site.id).await);
    let biblioteca = ou_falha!(cms_dados::biblioteca_docs(&estado.pool, site.id).await);
    let aba = aba_de(uri);
    let (aviso, _) = textos(uri);
    let visiveis: Vec<_> = match aba {
        "favoritos" => arquivos.iter().filter(|arquivo| arquivo.favorito).collect(),
        "compartilhados" => arquivos
            .iter()
            .filter(|arquivo| arquivo.compartilhado)
            .collect(),
        _ => arquivos.iter().collect(),
    };
    let corpo = PaginaDoEstudio {
        site: site.perfil.nome.clone(),
        slug: site.slug.clone(),
        dono: pode_administrar(&ator),
        aba: aba.to_string(),
        pedido: String::new(),
        erro,
        aviso,
        destinos: arquivos
            .iter()
            .map(|arquivo| Destino {
                id: arquivo.id.to_string(),
                nome: arquivo.nome.clone(),
            })
            .collect(),
        sistemas: sistemas_visiveis(sistema_escolhido),
        arquivos: visiveis
            .iter()
            .map(|arquivo| linha(slug, arquivo))
            .collect(),
        biblioteca_ativa: biblioteca.as_ref().is_some_and(|item| item.ativa),
        biblioteca_versao: biblioteca
            .map(|item| item.versao)
            .unwrap_or_else(|| VERSAO_DO_CATALOGO_DOCS.to_string()),
        pode_ativar: estudio::pode_ativar_biblioteca(&ator),
        pecas: pecas_visiveis(),
        script_htmx: caminho_htmx().to_string(),
    }
    .render()?;
    Ok(html_do_estudio(StatusCode::OK, corpo))
}

fn linha(slug: &str, arquivo: &ArquivoListado) -> ArquivoVisivel {
    let mut partes = vec![
        estudio::sistema_ou_padrao(&arquivo.sistema)
            .nome
            .to_string(),
    ];
    if arquivo.favorito {
        partes.push("favorito".to_string());
    }
    if arquivo.compartilhado {
        partes.push("prévia".to_string());
    }
    ArquivoVisivel {
        href: format!("/painel/sites/{slug}/estudio/{}", arquivo.id),
        nome: arquivo.nome.clone(),
        quando: cms_dominio::datas::por_extenso(arquivo.atualizado_em),
        etiquetas: partes.join(" · "),
    }
}

pub async fn criar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Ok(formulario) = serde_urlencoded::from_bytes::<FormularioDeArquivo>(corpo) else {
        return Ok(formulario_invalido());
    };
    let Some(pedido) = estudio::pedido_valido(&formulario.pedido) else {
        return Ok(simples(StatusCode::BAD_REQUEST, "O pedido é longo demais."));
    };
    let sistema = if formulario.sistema.is_empty() {
        "padrao"
    } else {
        formulario.sistema.as_str()
    };
    if estudio::sistema(sistema).is_none() {
        return Ok(simples(
            StatusCode::BAD_REQUEST,
            "Sistema de design desconhecido.",
        ));
    }
    let Some(atalho) = estudio::Atalho::ler(&formulario.atalho) else {
        return Ok(formulario_invalido());
    };
    if !formulario.arquivo.is_empty() {
        let Ok(arquivo_id) = Uuid::parse_str(formulario.arquivo.trim()) else {
            return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
        };
        let Some(arquivo) =
            ou_falha!(cms_dados::abrir_arquivo(&estado.pool, site.id, arquivo_id).await)
        else {
            return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
        };
        if !estudio::pode_editar_arquivo(&ator, &arquivo.criado_por) {
            return Ok(sem_permissao());
        }
        ou_falha!(
            cms_dados::guardar_pedido_e_sistema(
                &estado.pool,
                site.id,
                arquivo_id,
                &pedido,
                sistema
            )
            .await
        );
        return Ok(volta(slug, arquivo_id, "pedido"));
    }
    if !estudio::pode_criar_arquivo(&ator) {
        return Ok(sem_permissao());
    }
    let medida = estudio::medida_do_atalho(atalho);
    let nome = estudio::nome_de_arquivo(&pedido, atalho);
    let texto = if pedido.is_empty() {
        nome.clone()
    } else {
        pedido.clone()
    };
    let texto: String = texto.chars().take(2000).collect();
    let texto_medida = estudio::medida_do_no(TIPO_TEXTO, "", medida).ok_or_else(|| {
        ErroWeb::Dados(cms_dados::ErroDeDados::DadoInvalido(
            "página do estúdio sem medida".into(),
        ))
    })?;
    let (x, y) = estudio::posicao_inicial(0, texto_medida, medida).ok_or_else(|| {
        ErroWeb::Dados(cms_dados::ErroDeDados::DadoInvalido(
            "texto inicial fora da página".into(),
        ))
    })?;
    match cms_dados::criar_arquivo(
        &estado.pool,
        NovoArquivo {
            site_id: site.id,
            nome,
            pedido,
            criado_por: ator.conta.clone(),
            sistema: sistema.to_string(),
            largura: medida.largura,
            altura: medida.altura,
            texto,
            texto_x: x,
            texto_y: y,
            texto_largura: texto_medida.largura,
            texto_altura: texto_medida.altura,
        },
    )
    .await
    {
        Ok(id) => Ok(volta(slug, id, "criado")),
        Err(erro) => falha(erro),
    }
}

pub async fn abrir(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo_id: Uuid,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Some(arquivo) =
        ou_falha!(cms_dados::abrir_arquivo(&estado.pool, site.id, arquivo_id).await)
    else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    let biblioteca = ou_falha!(cms_dados::biblioteca_docs(&estado.pool, site.id).await);
    Ok(html_do_estudio(
        StatusCode::OK,
        pagina_do_arquivo(&site.perfil.nome, slug, &ator, &arquivo, &biblioteca, uri)?.render()?,
    ))
}

fn pagina_do_arquivo(
    site: &str,
    slug: &str,
    ator: &Ator,
    arquivo: &cms_dados::ArquivoAberto,
    biblioteca: &Option<cms_dados::BibliotecaDoSite>,
    uri: &Uri,
) -> Result<PaginaDoArquivo, ErroWeb> {
    let pagina = arquivo.paginas.first().ok_or_else(|| {
        ErroWeb::Dados(cms_dados::ErroDeDados::DadoInvalido(
            "arquivo do estúdio sem página".into(),
        ))
    })?;
    let sistema = estudio::sistema_ou_padrao(&arquivo.sistema);
    let (aviso, _) = textos(uri);
    let biblioteca_ativa = biblioteca.as_ref().is_some_and(|item| item.ativa);
    let biblioteca_versao = biblioteca
        .as_ref()
        .map(|item| item.versao.clone())
        .unwrap_or_else(|| VERSAO_DO_CATALOGO_DOCS.to_string());
    Ok(PaginaDoArquivo {
        site: site.to_string(),
        slug: slug.to_string(),
        dono: pode_administrar(ator),
        nome: arquivo.nome.clone(),
        arquivo_id: arquivo.id.to_string(),
        sistema_slug: sistema.slug.to_string(),
        sistema_nome: sistema.nome.to_string(),
        sistemas: sistemas_visiveis(sistema.slug),
        favorito: arquivo.favorito,
        edita: estudio::pode_editar_arquivo(ator, &arquivo.criado_por),
        pode_publicar: estudio::pode_publicar_previa(ator),
        link_da_previa: arquivo
            .token_da_previa
            .as_ref()
            .map(|token| format!("/estudio/p/{token}"))
            .unwrap_or_default(),
        pedido: arquivo.pedido.clone(),
        aviso,
        biblioteca_ativa,
        biblioteca_versao,
        pecas: pecas_visiveis(),
        largura: pagina.largura,
        altura: pagina.altura,
        fundo: sistema.tokens.fundo.clone(),
        superficie: sistema.tokens.superficie.clone(),
        cor_texto: sistema.tokens.texto.clone(),
        suave: sistema.tokens.suave.clone(),
        destaque: sistema.tokens.destaque.clone(),
        borda: sistema.tokens.borda.clone(),
        fonte_titulo: sistema.tokens.fonte_titulo.clone(),
        fonte_corpo: sistema.tokens.fonte_corpo.clone(),
        nos: pagina.nos.iter().map(no_do_arquivo).collect(),
        script_htmx: caminho_htmx().to_string(),
        script: caminho_do_script().to_string(),
    })
}

pub async fn aplicar_sistema(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo_id: Uuid,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site_id, _) = match exigir_edicao(estado, conta, slug, arquivo_id).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Ok(formulario) = serde_urlencoded::from_bytes::<FormularioDoSistema>(corpo) else {
        return Ok(formulario_invalido());
    };
    if estudio::sistema(&formulario.sistema).is_none() {
        return Ok(simples(
            StatusCode::BAD_REQUEST,
            "Sistema de design desconhecido.",
        ));
    }
    ou_falha!(
        cms_dados::definir_sistema(&estado.pool, site_id, arquivo_id, &formulario.sistema).await
    );
    Ok(volta(slug, arquivo_id, "sistema"))
}

pub async fn favorito(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo_id: Uuid,
) -> Result<Response, ErroWeb> {
    let (site_id, _) = match exigir_edicao(estado, conta, slug, arquivo_id).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    ou_falha!(cms_dados::alternar_favorito(&estado.pool, site_id, arquivo_id).await);
    Ok(volta(slug, arquivo_id, "favorito"))
}

pub async fn criar_no(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo_id: Uuid,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site_id, arquivo) = match exigir_edicao(estado, conta, slug, arquivo_id).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Ok(formulario) = serde_urlencoded::from_bytes::<FormularioDoNo>(corpo) else {
        return Ok(formulario_invalido());
    };
    let pagina = arquivo.paginas.first().ok_or_else(|| {
        ErroWeb::Dados(cms_dados::ErroDeDados::DadoInvalido(
            "arquivo do estúdio sem página".into(),
        ))
    })?;
    let medida = Medida {
        largura: pagina.largura,
        altura: pagina.altura,
    };
    let variante = formulario.variante.trim();
    let (props, biblioteca, catalogo, texto) = if formulario.tipo == TIPO_COMPONENTE {
        let Some(peca) = estudio::peca_docs(variante) else {
            return Ok(formulario_invalido());
        };
        if !estudio::props_validas(peca.variante, &peca.props) {
            return Ok(formulario_invalido());
        }
        (
            peca.props.clone(),
            Some(estudio::SLUG_DA_BIBLIOTECA_DOCS.to_string()),
            Some(VERSAO_DO_CATALOGO_DOCS.to_string()),
            String::new(),
        )
    } else if matches!(
        formulario.tipo.as_str(),
        TIPO_FRAME | TIPO_RETANGULO | TIPO_TEXTO
    ) {
        let texto = match formulario.tipo.as_str() {
            TIPO_FRAME => "Frame",
            TIPO_TEXTO => "Texto",
            _ => "",
        };
        (PropsDeComponente::default(), None, None, texto.to_string())
    } else {
        return Ok(formulario_invalido());
    };
    let no_medida = estudio::medida_do_no(&formulario.tipo, variante, medida).ok_or_else(|| {
        ErroWeb::Dados(cms_dados::ErroDeDados::DadoInvalido("nó sem medida".into()))
    })?;
    let (x, y) = match (numero(&formulario.x), numero(&formulario.y)) {
        (None, None) => estudio::posicao_inicial(pagina.nos.len() as i32, no_medida, medida)
            .ok_or_else(|| {
                ErroWeb::Dados(cms_dados::ErroDeDados::DadoInvalido(
                    "sem posição para o nó".into(),
                ))
            })?,
        (Some(x), Some(y)) => (x, y),
        _ => return Ok(formulario_invalido()),
    };
    match cms_dados::criar_no(
        &estado.pool,
        NovoNo {
            site_id,
            arquivo_id,
            tipo: formulario.tipo,
            x,
            y,
            largura: no_medida.largura,
            altura: no_medida.altura,
            texto,
            preenchimento: None,
            biblioteca,
            variante: (!variante.is_empty()).then(|| variante.to_string()),
            catalogo_versao: catalogo,
            props,
        },
    )
    .await
    {
        Ok(_) => Ok(volta(slug, arquivo_id, "no")),
        Err(erro) => falha(erro),
    }
}

pub async fn mover_no(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo_id: Uuid,
    no_id: Uuid,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site_id, _) = match exigir_edicao(estado, conta, slug, arquivo_id).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Ok(formulario) = serde_urlencoded::from_bytes::<FormularioDoMovimento>(corpo) else {
        return Ok(formulario_invalido());
    };
    let (Some(x), Some(y)) = (numero(&formulario.x), numero(&formulario.y)) else {
        return Ok(formulario_invalido());
    };
    ou_falha!(cms_dados::mover_no(&estado.pool, site_id, arquivo_id, no_id, x, y).await);
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn publicar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo_id: Uuid,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    if !estudio::pode_publicar_previa(&ator) {
        return Ok(simples(
            StatusCode::FORBIDDEN,
            "Só editor e dono publicam a prévia.",
        ));
    }
    if ou_falha!(cms_dados::abrir_arquivo(&estado.pool, site.id, arquivo_id).await).is_none() {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    }
    ou_falha!(cms_dados::publicar_previa(&estado.pool, site.id, arquivo_id, &ator.conta).await);
    Ok(volta(slug, arquivo_id, "previa"))
}

pub async fn biblioteca(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    qual: &str,
    corpo: &[u8],
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    if qual != estudio::SLUG_DA_BIBLIOTECA_DOCS {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    }
    if !estudio::pode_ativar_biblioteca(&ator) {
        return Ok(simples(
            StatusCode::FORBIDDEN,
            "Só editor e dono ativam a biblioteca.",
        ));
    }
    let Ok(formulario) = serde_urlencoded::from_bytes::<FormularioDaBiblioteca>(corpo) else {
        return Ok(formulario_invalido());
    };
    let ativa = match formulario.ativa.as_str() {
        "1" => true,
        "0" => false,
        _ => return Ok(formulario_invalido()),
    };
    ou_falha!(
        cms_dados::definir_biblioteca_docs(
            &estado.pool,
            site.id,
            ativa,
            VERSAO_DO_CATALOGO_DOCS,
            &ator.conta,
        )
        .await
    );
    Ok(redirecionar(
        StatusCode::SEE_OTHER,
        &format!("/painel/sites/{slug}/estudio?aba=bibliotecas&aviso=biblioteca"),
    ))
}

pub async fn previa_publica(estado: &Estado, token: &str) -> Result<Response, ErroWeb> {
    let Some(definicao) = ou_falha!(cms_dados::previa_por_token(&estado.pool, token).await) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    if !estudio::definicao_valida(&definicao) {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    }
    let Some(pagina) = definicao.paginas.first() else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    let corpo = PaginaDePrevia {
        nome: definicao.nome.clone(),
        edita: false,
        slug: String::new(),
        arquivo_id: String::new(),
        largura: pagina.largura,
        altura: pagina.altura,
        fundo: definicao.tokens.fundo.clone(),
        superficie: definicao.tokens.superficie.clone(),
        cor_texto: definicao.tokens.texto.clone(),
        suave: definicao.tokens.suave.clone(),
        destaque: definicao.tokens.destaque.clone(),
        borda: definicao.tokens.borda.clone(),
        fonte_titulo: definicao.tokens.fonte_titulo.clone(),
        fonte_corpo: definicao.tokens.fonte_corpo.clone(),
        nos: pagina
            .nos
            .iter()
            .enumerate()
            .map(|(indice, no)| no_da_definicao(indice, no))
            .collect(),
        script: caminho_do_script().to_string(),
    }
    .render()?;
    Ok(html_da_previa(corpo))
}

/// Site, ator e arquivo, se a conta pode editar. A resposta pronta interrompe.
async fn exigir_edicao(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    arquivo_id: Uuid,
) -> Result<Result<(Uuid, cms_dados::ArquivoAberto), Response>, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(Err(resposta)),
    };
    let arquivo = match cms_dados::abrir_arquivo(&estado.pool, site.id, arquivo_id).await {
        Ok(Some(arquivo)) => arquivo,
        Ok(None) => return Ok(Err(simples(StatusCode::NOT_FOUND, "Não encontrado."))),
        Err(erro) => return falha(erro).map(Err),
    };
    if !estudio::pode_editar_arquivo(&ator, &arquivo.criado_por) {
        return Ok(Err(sem_permissao()));
    }
    Ok(Ok((site.id, arquivo)))
}

fn numero(valor: &str) -> Option<i32> {
    let valor = valor.trim();
    if valor.is_empty() {
        None
    } else {
        valor.parse().ok()
    }
}

#[cfg(test)]
mod atribuicao {
    #[test]
    fn aviso_mantem_a_licenca_do_fumadocs_e_o_script_faz_pan() {
        let aviso = include_str!("../recursos/biblioteca-docs/AVISO.txt");
        assert!(aviso.contains("MIT License"));
        assert!(aviso.contains("Copyright (c) 2023 Fuma"));
        assert!(aviso.contains("76408e07e37cbe9708c194ad4f0ebd49a974a236"));
        let script = include_str!("../recursos/estudio.js");
        assert!(script.contains("function pontoNoQuadro"));
        assert!(script.contains("translate"));
    }
}
