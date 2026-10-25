//! Os modelos 3D de um site: envio, lista, remoção e entrega dos arquivos.
//!
//! O arquivo é conferido na chegada e gravado como veio. Não há conversão:
//! o visualizador do site abre o GLB, e o iPhone abre o USDZ.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use askama::Template;
use axum::body::Bytes;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use cms_dados::{ErroDeModelo, ModeloDoSite, NovoModelo, SiteGravado};
use cms_dominio::conta::pode_administrar;
use cms_dominio::{Ator, Conta, Papel};
use motor_web::validacao::MODELO_MAXIMO_BYTES;
use uuid::Uuid;

use crate::Estado;
use crate::acesso::ao_site;
use crate::lista::{Consulta, Lista};
use crate::resposta::{ErroWeb, html_privado, simples};

/// O visualizador do site: o único script que uma página carrega além da tag
/// do Google, e só a página que tem um bloco de modelo 3D.
const VISUALIZADOR: &str = include_str!("../recursos/modelo-3d.js");

/// O arquivo nunca muda depois do envio, e o nome leva o identificador.
const CACHE_DE_ARQUIVO: &str = "public, max-age=31536000, immutable";

/// O endereço do visualizador. Leva o resumo do conteúdo: mudou o script,
/// mudou o endereço, e o cache longo não segura versão antiga.
pub fn caminho_do_visualizador() -> &'static str {
    static CAMINHO: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        let resumo = cms_dados::hash_de_conteudo(VISUALIZADOR.as_bytes());
        format!("/recursos/modelo-3d-{}.js", &resumo[..12])
    });
    &CAMINHO
}

pub(crate) fn arquivo_pronto(bytes: impl IntoResponse, tipo: &'static str) -> Response {
    let mut resposta = bytes.into_response();
    let cabecalhos = resposta.headers_mut();
    cabecalhos.insert(CONTENT_TYPE, HeaderValue::from_static(tipo));
    cabecalhos.insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_DE_ARQUIVO));
    cabecalhos.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    resposta
}

/// `GET /recursos/modelo-3d-<resumo>.js`
pub fn visualizador(caminho: &str) -> Option<Response> {
    (caminho == caminho_do_visualizador())
        .then(|| arquivo_pronto(VISUALIZADOR, "text/javascript; charset=utf-8"))
}

/// O identificador e o tipo de um arquivo de modelo, pelo nome pedido. Só
/// `<uuid>.glb` e `<uuid>.usdz`, escritos como o CMS os grava.
fn arquivo_pedido(arquivo: &str) -> Option<(Uuid, &'static str, &'static str)> {
    let (nome, extensao) = arquivo.rsplit_once('.')?;
    let id = Uuid::parse_str(nome).ok()?;
    if id.to_string() != nome {
        return None;
    }
    match extensao {
        "glb" => Some((id, "glb", "model/gltf-binary")),
        "usdz" => Some((id, "usdz", "model/vnd.usdz+zip")),
        _ => None,
    }
}

fn caminho_do_arquivo(diretorio: &Path, site_id: Uuid, id: Uuid, extensao: &str) -> PathBuf {
    diretorio
        .join(site_id.to_string())
        .join("modelos")
        .join(format!("{id}.{extensao}"))
}

/// `GET /midia/modelos/<arquivo>`: a pasta é a do site do pedido, então um
/// site não lê modelo de outro.
pub async fn servir(diretorio: &Path, site_id: Uuid, arquivo: &str) -> Result<Response, ErroWeb> {
    let Some((id, extensao, tipo)) = arquivo_pedido(arquivo) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    match tokio::fs::read(caminho_do_arquivo(diretorio, site_id, id, extensao)).await {
        Ok(bytes) => Ok(arquivo_pronto(bytes, tipo)),
        Err(erro) if erro.kind() == ErrorKind::NotFound => {
            Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."))
        }
        Err(erro) => Err(erro.into()),
    }
}

const GRANDE_DEMAIS: &str =
    "O arquivo passa de 30 MB. Exporte o modelo com menos detalhe ou com texturas menores.";

/// Confere se o arquivo é um GLB que o visualizador do site abre: glTF 2.0
/// binário, com tudo dentro do próprio arquivo e sem extensão obrigatória
/// (compressão Draco ou meshopt, textura KTX2). `None` quando está certo.
pub fn problema_do_glb(bytes: &[u8]) -> Option<&'static str> {
    const NAO_E_GLB: &str =
        "O arquivo não é um modelo GLB. Exporte o modelo como glTF binário (.glb).";
    if bytes.len() > MODELO_MAXIMO_BYTES {
        return Some(GRANDE_DEMAIS);
    }
    let inteiro = |de: usize| {
        bytes
            .get(de..de + 4)
            .and_then(|quatro| quatro.try_into().ok())
            .map(u32::from_le_bytes)
    };
    let cabecalho_certo = bytes.starts_with(b"glTF")
        && inteiro(4) == Some(2)
        && inteiro(8).is_some_and(|total| total as usize == bytes.len())
        // O primeiro pedaço é sempre o JSON.
        && inteiro(16) == Some(0x4E4F_534A);
    if !cabecalho_certo {
        return Some(NAO_E_GLB);
    }
    let tamanho = inteiro(12)? as usize;
    let Some(json) = bytes
        .get(20..20 + tamanho)
        .and_then(|json| serde_json::from_slice::<serde_json::Value>(json).ok())
    else {
        return Some(NAO_E_GLB);
    };
    let lista = |nome: &str| json.get(nome).and_then(|v| v.as_array()).cloned();
    if lista("extensionsRequired").is_some_and(|exigidas| !exigidas.is_empty()) {
        return Some(
            "Este modelo usa compressão ou textura que o visualizador do site não abre (Draco, meshopt ou KTX2). Exporte o GLB sem compressão.",
        );
    }
    let de_fora = |nome: &str| {
        lista(nome).unwrap_or_default().iter().any(|item| {
            item.get("uri")
                .and_then(|uri| uri.as_str())
                .is_some_and(|uri| !uri.starts_with("data:"))
        })
    };
    if de_fora("buffers") || de_fora("images") {
        return Some(
            "Este modelo depende de arquivos separados. Exporte o GLB com a geometria e as texturas dentro do próprio arquivo.",
        );
    }
    None
}

/// Confere se o arquivo é um USDZ: um pacote zip, que é o que o iPhone abre.
pub fn problema_do_usdz(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > MODELO_MAXIMO_BYTES {
        return Some(GRANDE_DEMAIS);
    }
    (!bytes.starts_with(b"PK\x03\x04"))
        .then_some("O segundo arquivo não é um USDZ. Exporte o modelo para iPhone como .usdz.")
}

/// Um modelo que chegou, pelo painel ou pelo conector.
#[derive(Default)]
pub struct Envio {
    pub nome: String,
    pub descricao: String,
    pub glb: Bytes,
    pub usdz: Option<Bytes>,
}

/// Confere, registra e grava os arquivos de um modelo. A recusa volta como a
/// frase que a pessoa (ou o assistente) lê.
pub async fn registrar(
    estado: &Estado,
    site: &SiteGravado,
    ator: &Ator,
    envio: Envio,
) -> Result<Result<Uuid, String>, ErroWeb> {
    let usdz = envio.usdz.filter(|usdz| !usdz.is_empty());
    let problema =
        problema_do_glb(&envio.glb).or_else(|| usdz.as_deref().and_then(problema_do_usdz));
    if let Some(problema) = problema {
        return Ok(Err(problema.to_string()));
    }
    let novo = NovoModelo {
        nome: envio.nome,
        descricao: envio.descricao,
        hash: cms_dados::hash_de_conteudo(&envio.glb),
        glb_bytes: envio.glb.len() as u64,
        usdz_bytes: usdz.as_ref().map(|usdz| usdz.len() as u64),
    };
    let limite = estado.configuracao.limite_de_midia_por_site;
    let id = match cms_dados::registrar_modelo(&estado.pool, site.id, ator, novo, limite).await {
        Ok(id) => id,
        Err(ErroDeModelo::Dados(erro)) => return Err(erro.into()),
        Err(erro) => return Ok(Err(erro.to_string())),
    };

    let diretorio = &estado.configuracao.diretorio_de_midia;
    let glb = caminho_do_arquivo(diretorio, site.id, id, "glb");
    let gravado = async {
        if let Some(pasta) = glb.parent() {
            tokio::fs::create_dir_all(pasta).await?;
        }
        tokio::fs::write(&glb, &envio.glb).await?;
        if let Some(usdz) = &usdz {
            tokio::fs::write(caminho_do_arquivo(diretorio, site.id, id, "usdz"), usdz).await?;
        }
        Ok::<(), std::io::Error>(())
    }
    .await;
    if let Err(erro) = gravado {
        // Sem o arquivo, a linha só deixaria um modelo que nunca abre.
        if let Err(falha) = remover(estado, site, ator, id).await {
            tracing::error!(%falha, modelo = %id, "modelo 3D sem arquivo ficou registrado");
        }
        return Err(erro.into());
    }
    Ok(Ok(id))
}

/// Apaga o modelo da biblioteca e os arquivos dele no disco. A recusa (em
/// uso, sem permissão, inexistente) volta para quem pediu.
pub async fn remover(
    estado: &Estado,
    site: &SiteGravado,
    ator: &Ator,
    id: Uuid,
) -> Result<Result<(), ErroDeModelo>, ErroWeb> {
    let tinha_usdz = match cms_dados::apagar_modelo(&estado.pool, site.id, ator, id).await {
        Ok(tinha_usdz) => tinha_usdz,
        Err(ErroDeModelo::Dados(erro)) => return Err(erro.into()),
        Err(erro) => return Ok(Err(erro)),
    };
    let diretorio = &estado.configuracao.diretorio_de_midia;
    let extensoes = ["glb"].into_iter().chain(tinha_usdz.then_some("usdz"));
    for caminho in extensoes.map(|extensao| caminho_do_arquivo(diretorio, site.id, id, extensao)) {
        // Arquivo que já não existe não impede a remoção.
        if let Err(erro) = tokio::fs::remove_file(&caminho).await {
            tracing::warn!(%erro, caminho = %caminho.display(), "arquivo de modelo 3D não removido");
        }
    }
    Ok(Ok(()))
}

/// O tamanho de um arquivo como a pessoa lê.
fn tamanho(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / (1024.0 * 1024.0)).replace('.', ",")
    } else {
        format!("{:.0} KB", (bytes / 1024.0).max(1.0))
    }
}

struct Linha {
    id: Uuid,
    nome: String,
    descricao: String,
    arquivos: String,
    pode_apagar: bool,
}

impl Linha {
    fn nova(modelo: ModeloDoSite, ator: &Ator) -> Self {
        let arquivos = match modelo.usdz_bytes {
            Some(usdz) => format!("GLB {} · USDZ {}", tamanho(modelo.glb_bytes), tamanho(usdz)),
            None => format!("GLB {} · sem USDZ (iPhone)", tamanho(modelo.glb_bytes)),
        };
        Self {
            pode_apagar: ator.papel != Papel::Autor || modelo.enviado_por == ator.conta,
            id: modelo.id,
            nome: modelo.nome,
            descricao: modelo.descricao,
            arquivos,
        }
    }
}

#[derive(Template)]
#[template(path = "modelos.html")]
struct PaginaDeModelos {
    site: String,
    slug: String,
    erro: Option<String>,
    aviso: Option<String>,
    lista: Lista<Linha>,
    /// Quantos modelos a busca achou, por extenso.
    resumo: String,
    /// A descrição digitada, de volta no formulário quando algo é recusado.
    descricao: String,
    /// Quem vê o menu inteiro do site.
    dono: bool,
}

#[derive(Default)]
struct Recado {
    erro: Option<String>,
    aviso: Option<String>,
    descricao: String,
}

async fn pagina(
    estado: &Estado,
    site: &SiteGravado,
    ator: &Ator,
    status: StatusCode,
    recado: Recado,
    consulta: &Consulta,
) -> Result<Response, ErroWeb> {
    let modelos = cms_dados::modelos_do_site(&estado.pool, site.id)
        .await?
        .into_iter()
        // A busca olha a descrição, o nome do arquivo e o identificador.
        .filter(|modelo| {
            consulta.casa(&format!(
                "{} {} {}",
                modelo.descricao, modelo.nome, modelo.id
            ))
        })
        .map(|modelo| Linha::nova(modelo, ator))
        .collect();
    let lista = consulta.recortar(modelos, &format!("/painel/sites/{}/modelos", site.slug));
    let corpo = PaginaDeModelos {
        site: site.perfil.nome.clone(),
        slug: site.slug.clone(),
        erro: recado.erro,
        aviso: recado.aviso,
        resumo: lista.resumo("Nenhum modelo", "modelo", "modelos"),
        lista,
        descricao: recado.descricao,
        dono: pode_administrar(ator),
    }
    .render()?;
    Ok(html_privado(status, corpo))
}

pub async fn abrir(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    uri: &Uri,
) -> Result<Response, ErroWeb> {
    match ao_site(estado, conta, slug).await? {
        Ok((site, ator)) => {
            let consulta = Consulta::da(uri);
            let recado = Recado::default();
            pagina(estado, &site, &ator, StatusCode::OK, recado, &consulta).await
        }
        Err(resposta) => Ok(resposta),
    }
}

/// Lê o formulário de envio. `None` para o que não é um formulário válido.
async fn ler_envio(cabecalhos: &HeaderMap, corpo: Bytes) -> Option<Envio> {
    let tipo = cabecalhos.get(CONTENT_TYPE)?.to_str().ok()?;
    let limite = multer::parse_boundary(tipo).ok()?;
    let fluxo = futures_util::stream::once(async move { Ok::<Bytes, std::io::Error>(corpo) });
    let mut partes = multer::Multipart::new(fluxo, limite);
    let mut envio = Envio::default();
    while let Some(campo) = partes.next_field().await.ok()? {
        let nome_do_campo = campo.name().map(str::to_string);
        let nome_do_arquivo = campo.file_name().map(str::to_string);
        let dados = campo.bytes().await.ok()?;
        match nome_do_campo.as_deref() {
            Some("glb") => {
                envio.nome = nome_do_arquivo.unwrap_or_default();
                envio.glb = dados;
            }
            Some("usdz") => envio.usdz = Some(dados),
            Some("descricao") => {
                envio.descricao = String::from_utf8_lossy(&dados).trim().to_string();
            }
            _ => {}
        }
    }
    Some(envio)
}

pub async fn enviar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    cabecalhos: &HeaderMap,
    corpo: &Bytes,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Some(envio) = ler_envio(cabecalhos, corpo.clone()).await else {
        return Ok(simples(StatusCode::BAD_REQUEST, "Formulário inválido."));
    };
    let descricao = envio.descricao.clone();
    let (status, recado) = match registrar(estado, &site, &ator, envio).await? {
        Ok(_) => (
            StatusCode::OK,
            Recado {
                aviso: Some(
                    "Modelo 3D enviado. Ele já pode entrar em uma página ou post.".to_string(),
                ),
                ..Recado::default()
            },
        ),
        Err(mensagem) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Recado {
                erro: Some(mensagem),
                descricao,
                ..Recado::default()
            },
        ),
    };
    // Depois de um envio ou remoção a lista volta para o começo.
    pagina(estado, &site, &ator, status, recado, &Consulta::default()).await
}

pub async fn apagar(
    estado: &Estado,
    conta: &Conta,
    slug: &str,
    id: &str,
) -> Result<Response, ErroWeb> {
    let (site, ator) = match ao_site(estado, conta, slug).await? {
        Ok(acesso) => acesso,
        Err(resposta) => return Ok(resposta),
    };
    let Ok(id) = Uuid::parse_str(id) else {
        return Ok(simples(StatusCode::NOT_FOUND, "Não encontrado."));
    };
    let (status, recado) = match remover(estado, &site, &ator, id).await? {
        Ok(()) => (
            StatusCode::OK,
            Recado {
                aviso: Some("Modelo 3D apagado.".to_string()),
                ..Recado::default()
            },
        ),
        Err(ErroDeModelo::Dados(erro)) => return Err(erro.into()),
        Err(erro) => {
            let status = match erro {
                ErroDeModelo::Inexistente => StatusCode::NOT_FOUND,
                ErroDeModelo::SemPermissao => StatusCode::FORBIDDEN,
                _ => StatusCode::CONFLICT,
            };
            (
                status,
                Recado {
                    erro: Some(erro.to_string()),
                    ..Recado::default()
                },
            )
        }
    };
    // Depois de um envio ou remoção a lista volta para o começo.
    pagina(estado, &site, &ator, status, recado, &Consulta::default()).await
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um GLB mínimo com o JSON dado.
    pub(crate) fn glb(json: &str) -> Vec<u8> {
        let mut json = json.as_bytes().to_vec();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len();
        let mut bytes = b"glTF".to_vec();
        bytes.extend(2u32.to_le_bytes());
        bytes.extend((total as u32).to_le_bytes());
        bytes.extend((json.len() as u32).to_le_bytes());
        bytes.extend(0x4E4F_534Au32.to_le_bytes());
        bytes.extend(json);
        bytes
    }

    #[test]
    fn aceita_glb_que_o_visualizador_abre() {
        assert_eq!(
            problema_do_glb(&glb(r#"{"asset":{"version":"2.0"}}"#)),
            None
        );
        let com_textura_embutida = glb(
            r#"{"asset":{"version":"2.0"},"images":[{"bufferView":0},{"uri":"data:image/png;base64,AA=="}]}"#,
        );
        assert_eq!(problema_do_glb(&com_textura_embutida), None);
    }

    #[test]
    fn recusa_o_que_nao_e_glb_ou_nao_abre() {
        for ruim in [
            b"".to_vec(),
            b"glTF".to_vec(),
            b"<html>glTF</html>".to_vec(),
            // Tamanho declarado diferente do arquivo.
            [glb(r#"{"asset":{}}"#), b"sobra".to_vec()].concat(),
        ] {
            assert!(problema_do_glb(&ruim).is_some_and(|p| p.contains("não é um modelo GLB")));
        }
        let comprimido = glb(r#"{"extensionsRequired":["KHR_draco_mesh_compression"]}"#);
        assert!(problema_do_glb(&comprimido).is_some_and(|p| p.contains("sem compressão")));
        let com_arquivo_de_fora = glb(r#"{"buffers":[{"uri":"geometria.bin"}]}"#);
        assert!(
            problema_do_glb(&com_arquivo_de_fora).is_some_and(|p| p.contains("arquivos separados"))
        );
    }

    #[test]
    fn usdz_e_um_zip() {
        assert_eq!(problema_do_usdz(b"PK\x03\x04resto"), None);
        assert!(problema_do_usdz(b"glTF").is_some());
    }

    #[test]
    fn so_serve_arquivo_com_o_nome_que_o_cms_grava() {
        let id = "0a1b2c3d-0a1b-4c3d-8a1b-0a1b2c3d4e5f";
        assert!(arquivo_pedido(&format!("{id}.glb")).is_some());
        assert_eq!(
            arquivo_pedido(&format!("{id}.usdz")).map(|(_, _, tipo)| tipo),
            Some("model/vnd.usdz+zip")
        );
        for ruim in [
            "",
            ".glb",
            "../segredo.glb",
            "modelo.glb",
            &format!("{id}.gltf"),
            &format!("{}.glb", id.to_uppercase()),
            &format!("pasta/{id}.glb"),
        ] {
            assert_eq!(arquivo_pedido(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn endereco_do_visualizador_leva_o_resumo_do_script() {
        let caminho = caminho_do_visualizador();
        assert!(caminho.starts_with("/recursos/modelo-3d-") && caminho.ends_with(".js"));
        assert!(visualizador(caminho).is_some());
        assert!(visualizador("/recursos/modelo-3d-antigo.js").is_none());
    }
}
