//! Configuração explícita das categorias; não deduz regras pelo nome da página.

/// O papel e o caminho de uma categoria do cardápio.
pub fn ler_categoria(linha: &str) -> Option<(&str, &str)> {
    let mut partes = linha.split_whitespace();
    let papel = partes.next()?;
    let caminho = partes.next()?;
    if partes.next().is_some()
        || !matches!(papel, "pizza" | "borda" | "produto")
        || !caminho.starts_with('/')
        || caminho.len() > 160
        || caminho[1..].split('/').any(|segmento| {
            segmento.is_empty()
                || !segmento
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
    {
        return None;
    }
    Some((papel, caminho))
}

/// Normaliza a configuração e recusa categorias repetidas ou inválidas.
pub fn conferir_categorias(texto: &str) -> Result<Vec<String>, &'static str> {
    let mut categorias = Vec::new();
    let mut caminhos = std::collections::HashSet::new();
    for linha in texto.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (papel, caminho) = ler_categoria(linha).ok_or(
            "Use uma categoria por linha: pizza /pizzas, borda /bordas ou produto /bebidas.",
        )?;
        if !caminhos.insert(caminho) || categorias.len() >= 12 {
            return Err("Use até 12 categorias, sem repetir o caminho da página.");
        }
        categorias.push(format!("{papel} {caminho}"));
    }
    Ok(categorias)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn categorias_so_aceitam_paginas_locais_e_papeis_conhecidos() {
        assert_eq!(ler_categoria("pizza /pizzas"), Some(("pizza", "/pizzas")));
        for ruim in [
            "pizza /",
            "pizza //outro",
            "produto https://outro.example",
            "pizza /a?x=1",
            "pizza /../a",
            "outro /a",
            "pizza /a extra",
        ] {
            assert_eq!(ler_categoria(ruim), None, "{ruim}");
        }
        assert!(conferir_categorias("pizza /a\nproduto /a").is_err());
    }
}
