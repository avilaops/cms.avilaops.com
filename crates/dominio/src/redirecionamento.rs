//! A regra de um redirecionamento cadastrado: de onde, para onde, e como o
//! prefixo é aplicado.
//!
//! `de` é um caminho do site. Terminado em `/*`, vale para tudo o que começa
//! com o trecho anterior, e o resto do caminho é levado para o destino quando
//! ele também termina em `/*`. `para` é um caminho do site ou um endereço
//! `https://` de outro site.

/// O sufixo que faz a regra valer para um prefixo.
const CURINGA: &str = "/*";

/// Por que um redirecionamento não serve. As mensagens são para quem digitou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RegraRecusada {
    #[error(
        "O endereço de origem é um caminho do site, como /guias/antigo ou /guias/*, sem espaço e sem \"?\"."
    )]
    Origem,
    #[error("A página inicial e o site inteiro não podem ser redirecionados.")]
    SiteInteiro,
    #[error(
        "O destino é um caminho do site, como /blog/novo, ou um endereço que começa com https://."
    )]
    Destino,
    #[error("A origem e o destino são o mesmo endereço.")]
    Circular,
}

fn caminho_limpo(texto: &str) -> bool {
    texto.starts_with('/')
        && !texto.starts_with("//")
        && !texto.contains(|c: char| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#'))
}

/// Confere a regra e a devolve como vai para o banco: sem espaço em volta e
/// sem barra no fim (a não ser a do curinga).
pub fn conferir(de: &str, para: &str) -> Result<(String, String), RegraRecusada> {
    let (de, para) = (de.trim(), para.trim());
    let (corpo, curinga) = match de.strip_suffix(CURINGA) {
        Some(corpo) => (corpo, true),
        None => (de.trim_end_matches('/'), false),
    };
    if !caminho_limpo(de) || corpo.contains('*') {
        return Err(RegraRecusada::Origem);
    }
    if corpo.is_empty() {
        return Err(RegraRecusada::SiteInteiro);
    }
    let de = if curinga {
        format!("{corpo}{CURINGA}")
    } else {
        corpo.to_string()
    };

    let externo = para.starts_with("https://") && para.len() > "https://".len();
    let destino_valido = (externo || caminho_limpo(para))
        && !para.contains(|c: char| c.is_whitespace() || c.is_control())
        && !para.trim_end_matches(CURINGA).contains('*');
    if !destino_valido {
        return Err(RegraRecusada::Destino);
    }
    let para = if para.len() > 1 && !para.ends_with(CURINGA) {
        para.trim_end_matches('/').to_string()
    } else {
        para.to_string()
    };
    if de == para {
        return Err(RegraRecusada::Circular);
    }
    Ok((de, para))
}

/// O destino de `caminho` por uma regra de prefixo (`de` termina em `/*`).
/// `None` se a regra não é de prefixo ou não cobre o caminho.
pub fn destino_por_prefixo(de: &str, para: &str, caminho: &str) -> Option<String> {
    let prefixo = de.strip_suffix('*')?;
    let resto = caminho.strip_prefix(prefixo)?;
    Some(match para.strip_suffix('*') {
        Some(base) => format!("{base}{}", resto.trim_end_matches('/')),
        None => para.to_string(),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn regra_e_normalizada_e_recusa_o_que_nao_e_endereco() {
        assert_eq!(
            conferir(" /guias/antigo/ ", "/blog/novo/"),
            Ok(("/guias/antigo".into(), "/blog/novo".into()))
        );
        assert_eq!(
            conferir("/guias/*", "/blog/*"),
            Ok(("/guias/*".into(), "/blog/*".into()))
        );
        assert_eq!(
            conferir("/login", "https://app.exemplo.example/login"),
            Ok(("/login".into(), "https://app.exemplo.example/login".into()))
        );
        for origem in ["guias", "//outro.example/x", "/a b", "/a?b=1", "/a/*/b", ""] {
            assert_eq!(
                conferir(origem, "/x"),
                Err(RegraRecusada::Origem),
                "{origem}"
            );
        }
        for origem in ["/", "/*"] {
            assert_eq!(
                conferir(origem, "/x"),
                Err(RegraRecusada::SiteInteiro),
                "{origem}"
            );
        }
        for destino in [
            "blog",
            "http://inseguro.example/",
            "javascript:alert(1)",
            "https://",
            "/a b",
            "//x.example",
        ] {
            assert_eq!(
                conferir("/a", destino),
                Err(RegraRecusada::Destino),
                "{destino}"
            );
        }
        assert_eq!(conferir("/a/", "/a"), Err(RegraRecusada::Circular));
    }

    #[test]
    fn prefixo_leva_o_resto_do_caminho() {
        assert_eq!(
            destino_por_prefixo("/guias/*", "/blog/*", "/guias/o-que-e-erp"),
            Some("/blog/o-que-e-erp".into())
        );
        // A barra no fim do endereço antigo não vai para o novo.
        assert_eq!(
            destino_por_prefixo("/guias/*", "/blog/*", "/guias/o-que-e-erp/"),
            Some("/blog/o-que-e-erp".into())
        );
        // Destino sem curinga recebe todo mundo no mesmo lugar.
        assert_eq!(
            destino_por_prefixo("/app/*", "https://app.exemplo.example", "/app/painel"),
            Some("https://app.exemplo.example".into())
        );
        assert_eq!(destino_por_prefixo("/guias/*", "/blog/*", "/guia"), None);
        assert_eq!(destino_por_prefixo("/guias", "/blog", "/guias"), None);
    }
}
