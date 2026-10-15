//! Projeta os blocos publicados em um cardápio. Não mantém cópia do catálogo.

use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use cms_dominio::PerfilDoSite;
use cms_dominio::cardapio::ler_categoria;
use motor_web::conteudo::renderizar_corpo;
use motor_web::tipos::{Bloco, Conteudo, Documento};
use serde::Serialize;

use crate::resposta::texto;

const PEDIDO: &str = include_str!("../recursos/pedido.mjs");
const APLICACAO: &str = include_str!("../recursos/cardapio.js");

fn script() -> &'static str {
    static SCRIPT: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| format!("{}\n{APLICACAO}", PEDIDO.replace("export ", "")));
    &SCRIPT
}

pub fn caminho_do_script() -> &'static str {
    static CAMINHO: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        let resumo = cms_dados::hash_de_conteudo(script().as_bytes());
        format!("/recursos/cardapio-{}.js", &resumo[..12])
    });
    &CAMINHO
}

pub fn recurso(caminho: &str) -> Option<Response> {
    (caminho == caminho_do_script()).then(|| {
        let mut resposta = texto(
            StatusCode::OK,
            "text/javascript; charset=utf-8",
            script().to_string(),
        );
        resposta.headers_mut().insert(
            "cache-control",
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
        resposta
    })
}

#[derive(Serialize)]
struct Produto<'a> {
    id: String,
    name: &'a str,
    ingredients: &'a str,
    price: u64,
}

#[derive(Serialize)]
struct Categoria<'a> {
    id: String,
    path: &'a str,
    name: &'a str,
    kind: &'a str,
    items: Vec<Produto<'a>>,
}

pub struct Cardapio {
    pub escopo: String,
    pub dados: String,
    pub ativo: String,
    pub whatsapp: String,
    pub alternativa: String,
    pub complemento: String,
    pub script: &'static str,
}

/// Sem moeda, intervalo ou período: só um preço exato, em centavos.
fn centavos(preco: &str) -> Option<u64> {
    let valor = preco.trim().strip_prefix("R$")?.trim();
    let (reais, centavos) = valor.split_once(',')?;
    if centavos.len() != 2 || !centavos.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let grupos: Vec<&str> = reais.split('.').collect();
    if grupos
        .iter()
        .any(|g| g.is_empty() || !g.bytes().all(|c| c.is_ascii_digit()))
        || (grupos.len() > 1 && (grupos[0].len() > 3 || grupos[1..].iter().any(|g| g.len() != 3)))
    {
        return None;
    }
    let reais: u64 = grupos.concat().parse().ok()?;
    let centavos: u64 = centavos.parse().ok()?;
    let total = reais.checked_mul(100)?.checked_add(centavos)?;
    // 100 linhas, 99 unidades e um adicional: a soma continua exata em JS.
    (total <= 100_000_000).then_some(total)
}

fn whatsapp(telefone: &str) -> Option<String> {
    let digitos: String = telefone.chars().filter(char::is_ascii_digit).collect();
    let numero = match digitos.len() {
        10 | 11 => format!("55{digitos}"),
        12 | 13 if digitos.starts_with("55") => digitos,
        _ => return None,
    };
    Some(numero)
}

pub fn montar(
    perfil: &PerfilDoSite,
    documentos: &[Documento],
    caminho: &str,
    site_id: uuid::Uuid,
) -> Option<Cardapio> {
    let whatsapp = whatsapp(perfil.organizacao.telefone.as_deref()?)?;
    let mut categorias = Vec::new();
    let mut ids = std::collections::HashSet::new();
    let mut alternativa = String::new();
    for linha in &perfil.paginas_do_cardapio {
        let (kind, caminho_categoria) = ler_categoria(linha)?;
        let Some(documento) = documentos.iter().find(|d| d.caminho == caminho_categoria) else {
            continue;
        };
        let Conteudo::Pagina(pagina) = &documento.conteudo else {
            continue;
        };
        let mut items = Vec::new();
        for secao in &pagina.corpo {
            if let Bloco::Planos { itens, .. } = secao {
                for item in itens {
                    if item
                        .periodo
                        .as_deref()
                        .is_some_and(|p| !p.trim().is_empty())
                    {
                        return None;
                    }
                    let resumo = cms_dados::hash_de_conteudo(item.nome.as_bytes());
                    let id = format!("{caminho_categoria}:{}", &resumo[..16]);
                    if !ids.insert(id.clone()) {
                        return None;
                    }
                    // Um preço ambíguo desativa o modo interativo inteiro.
                    // O conteúdo original continua legível e com os links do CMS.
                    items.push(Produto {
                        id,
                        name: &item.nome,
                        ingredients: item.descricao.as_deref().unwrap_or_default(),
                        price: centavos(&item.preco)?,
                    });
                }
            }
        }
        if !items.is_empty() {
            let planos: Vec<Bloco> = pagina
                .corpo
                .iter()
                .filter(|bloco| matches!(bloco, Bloco::Planos { .. }))
                .cloned()
                .collect();
            alternativa.push_str(&renderizar_corpo(&planos));
            categorias.push(Categoria {
                id: format!("categoria-{}", categorias.len()),
                path: caminho_categoria,
                name: pagina.menu.as_deref().unwrap_or(&pagina.titulo),
                kind,
                items,
            });
        }
    }
    let primeira = categorias.first()?;
    let ativo = categorias
        .iter()
        .find(|c| c.path == caminho)
        .unwrap_or(primeira)
        .id
        .clone();
    Some(Cardapio {
        escopo: site_id.to_string(),
        dados: serde_json::to_string(&categorias).ok()?,
        ativo,
        whatsapp,
        alternativa,
        complemento: documentos
            .iter()
            .find(|d| d.caminho == caminho)
            .map(|documento| {
                let blocos: Vec<Bloco> = documento
                    .corpo()
                    .iter()
                    .filter(|bloco| !matches!(bloco, Bloco::Planos { .. }))
                    .cloned()
                    .collect();
                renderizar_corpo(&blocos)
            })
            .unwrap_or_default(),
        script: caminho_do_script(),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn preco_exato_sem_arredondamento_nem_inferencia() {
        assert_eq!(centavos("R$ 35,90"), Some(3590));
        assert_eq!(centavos("R$\u{a0}1.234,56"), Some(123456));
        for ruim in [
            "35,90",
            "R$ 35.90",
            "R$ 12,00/mês",
            "a partir de R$ 30,00",
            "R$ 1.23,00",
            "R$ -3,00",
            "R$ 5,999",
            "R$ 9007199254740991,00",
        ] {
            assert_eq!(centavos(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn telefone_so_vira_whatsapp_com_numero_brasileiro_completo() {
        assert_eq!(
            whatsapp("(11) 99999-0000"),
            Some("5511999990000".to_string())
        );
        assert_eq!(
            whatsapp("+55 (11) 99999-0000"),
            Some("5511999990000".to_string())
        );
        assert_eq!(whatsapp("9999-0000"), None);
    }
}
