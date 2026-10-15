//! Negociação explícita de Markdown; o navegador continua recebendo HTML.
use axum::http::{HeaderMap, HeaderValue};
use axum::response::Response;

pub fn prefere_markdown(cabecalhos: &HeaderMap) -> bool {
    let mut markdown: Option<f32> = None;
    let mut html: Option<f32> = None;
    let mut geral: f32 = 0.0;
    for valor in cabecalhos
        .get_all("accept")
        .iter()
        .filter_map(|v| v.to_str().ok())
    {
        for faixa in valor.split(',') {
            let mut partes = faixa.split(';');
            let tipo = partes
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let qualidade = partes
                .filter_map(|p| p.trim().split_once('='))
                .find(|(k, _)| k.trim().eq_ignore_ascii_case("q"))
                .map_or(1.0, |(_, v)| {
                    v.trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|q| (0.0..=1.0).contains(q))
                        .unwrap_or(0.0)
                });
            match tipo.as_str() {
                "text/markdown" => markdown = Some(markdown.unwrap_or(0.0).max(qualidade)),
                "text/html" => html = Some(html.unwrap_or(0.0).max(qualidade)),
                "text/*" | "*/*" => geral = geral.max(qualidade),
                _ => {}
            }
        }
    }
    markdown.is_some_and(|q| q > 0.0 && q >= html.unwrap_or(geral))
}

/// Só aponta para documentos públicos que o CMS efetivamente oferece.
pub fn marcar(resposta: &mut Response, indexavel: bool) {
    if resposta.status() != axum::http::StatusCode::OK {
        return;
    }
    let tipo = resposta
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !tipo.starts_with("text/html") && !tipo.starts_with("text/markdown") {
        return;
    }
    resposta
        .headers_mut()
        .append("vary", HeaderValue::from_static("Accept"));
    if indexavel {
        resposta.headers_mut().insert("link", HeaderValue::from_static("</llms.txt>; rel=\"describedby\"; type=\"text/markdown\", </llms-full.txt>; rel=\"describedby\"; type=\"text/markdown\""));
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn markdown_exige_preferencia_explicita_e_qualidade_positiva() {
        for (accept, esperado) in [
            ("text/markdown", true),
            ("text/html, text/markdown;q=0.5", false),
            ("text/markdown;q=0, */*;q=1", false),
            ("text/markdown;q=0.8, text/html;q=0.3", true),
            ("text/markdown;q=invalid", false),
            ("text/markdown;q=NaN", false),
            ("*/*", false),
            ("text/html,application/xhtml+xml,*/*;q=0.8", false),
        ] {
            let mut h = HeaderMap::new();
            h.insert("accept", HeaderValue::from_str(accept).expect("cabeçalho"));
            assert_eq!(prefere_markdown(&h), esperado, "{accept}");
        }
        assert!(!prefere_markdown(&HeaderMap::new()));
    }
}
