//! Assets of the private React editor. The existing Rust handlers are its BFF.
use axum::response::Response;

const SCRIPT: &str = include_str!("../../../dashboard/dist/editor.js");
const STYLE: &str = include_str!("../../../dashboard/dist/editor.css");

pub fn script() -> &'static str {
    static PATH: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "/recursos/editor-{}.js",
            &cms_dados::hash_de_conteudo(SCRIPT.as_bytes())[..12]
        )
    });
    &PATH
}

pub fn style() -> &'static str {
    static PATH: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "/recursos/editor-{}.css",
            &cms_dados::hash_de_conteudo(STYLE.as_bytes())[..12]
        )
    });
    &PATH
}

pub fn recurso(path: &str) -> Option<Response> {
    if path == script() {
        Some(crate::modelos::arquivo_pronto(
            SCRIPT,
            "text/javascript; charset=utf-8",
        ))
    } else if path == style() {
        Some(crate::modelos::arquivo_pronto(
            STYLE,
            "text/css; charset=utf-8",
        ))
    } else {
        None
    }
}

/// Reescreve apenas referências tipadas de mídia de uma cópia do rascunho.
/// Texto editorial e URLs externas permanecem intactos.
pub fn preparar_imagens(conteudo: &mut motor_web::tipos::Conteudo, slug: &str) {
    use motor_web::tipos::{Conteudo, Midia};
    let adaptar = |midia: &mut Midia| {
        for variante in &mut midia.variantes {
            if let Some(arquivo) = variante.url.strip_prefix("/midia/") {
                variante.url = format!("/painel/sites/{slug}/midia/arquivo/{arquivo}");
            }
        }
    };
    let corpo = match conteudo {
        Conteudo::Pagina(pagina) => {
            pagina.capa.iter_mut().for_each(adaptar);
            &mut pagina.corpo
        }
        Conteudo::Post(post) => {
            adaptar(&mut post.capa);
            adaptar(&mut post.autor.foto);
            &mut post.corpo
        }
        Conteudo::Item(item) => {
            item.imagens.iter_mut().for_each(adaptar);
            &mut item.corpo
        }
        Conteudo::Produto(_) => return,
    };
    for bloco in corpo {
        bloco.midias_mut().into_iter().for_each(adaptar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motor_web::tipos::{Conteudo, Formato, Variante};

    #[test]
    fn previa_adapta_todas_as_larguras_sem_alterar_o_original() {
        let mut original = cms_dominio::modelos::conteudo_do_modelo("inicio", chrono::Utc::now())
            .expect("modelo inicial");
        let Conteudo::Pagina(pagina) = &mut original else {
            panic!("modelo de página");
        };
        let midia = pagina
            .corpo
            .iter_mut()
            .flat_map(|b| b.midias_mut())
            .next()
            .expect("imagem");
        midia.variantes = [480, 768, 1200]
            .into_iter()
            .map(|largura| Variante {
                formato: Formato::Avif,
                largura,
                url: format!("/midia/exemplo-{largura}.avif"),
            })
            .collect();
        midia.variantes.push(Variante {
            formato: Formato::Webp,
            largura: 480,
            url: "https://example.com/imagem.webp".to_string(),
        });
        let snapshot = original.clone();
        let mut previa = original.clone();
        preparar_imagens(&mut previa, "padaria");
        let json = serde_json::to_string(&previa).expect("conteúdo");
        for largura in [480, 768, 1200] {
            assert!(json.contains(&format!(
                "/painel/sites/padaria/midia/arquivo/exemplo-{largura}.avif"
            )));
        }
        assert!(json.contains("https://example.com/imagem.webp"));
        assert_eq!(original, snapshot);
        let adaptada = previa.clone();
        preparar_imagens(&mut previa, "padaria");
        assert_eq!(previa, adaptada);
    }
}
