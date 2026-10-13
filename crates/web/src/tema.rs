//! Os temas do site: o estilo de cada um e as fontes que eles usam.
//!
//! A lista é fechada (`cms_dominio::site::Tema`) e tudo mora no CMS: o estilo
//! vai embutido na página, por cima do estilo de base, e as fontes saem do
//! próprio site, em `/recursos/fontes/`. Nenhuma página busca fonte em
//! servidor de terceiros.

use axum::http::HeaderValue;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use cms_dominio::site::Tema;

/// Onde o site serve as fontes dos temas.
const CAMINHO_DAS_FONTES: &str = "/recursos/fontes/";

/// O arquivo de fonte nunca muda: versão nova entra com outro nome.
const CACHE_DE_FONTE: &str = "public, max-age=31536000, immutable";

macro_rules! fontes {
    ($($arquivo:literal),* $(,)?) => {
        [$(($arquivo, include_bytes!(concat!("../recursos/fontes/", $arquivo)).as_slice())),*]
    };
}

/// As fontes dos temas, todas sob a SIL Open Font License (o texto e os
/// créditos estão em `recursos/fontes/OFL.txt`). Só o recorte latino, que
/// cobre o português.
const FONTES: [(&str, &[u8]); 7] = fontes![
    "archivo.woff2",
    "barlow-400.woff2",
    "barlow-600.woff2",
    "barlow-700.woff2",
    "oswald.woff2",
    "plex-serif-500.woff2",
    "plex-serif-700.woff2",
];

/// `GET /recursos/fontes/<arquivo>`
pub fn fonte(caminho: &str) -> Option<Response> {
    let pedido = caminho.strip_prefix(CAMINHO_DAS_FONTES)?;
    let (_, bytes) = FONTES.iter().find(|(arquivo, _)| *arquivo == pedido)?;
    let mut resposta = bytes.into_response();
    let cabecalhos = resposta.headers_mut();
    cabecalhos.insert(CONTENT_TYPE, HeaderValue::from_static("font/woff2"));
    cabecalhos.insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_DE_FONTE));
    cabecalhos.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Some(resposta)
}

const INDUSTRIAL: &str = r#"
@font-face{font-family:"Oswald";font-weight:200 700;font-display:swap;src:url(/recursos/fontes/oswald.woff2) format("woff2")}
@font-face{font-family:"Barlow";font-weight:400;font-display:swap;src:url(/recursos/fontes/barlow-400.woff2) format("woff2")}
@font-face{font-family:"Barlow";font-weight:600;font-display:swap;src:url(/recursos/fontes/barlow-600.woff2) format("woff2")}
@font-face{font-family:"Barlow";font-weight:700;font-display:swap;src:url(/recursos/fontes/barlow-700.woff2) format("woff2")}
:root{--texto:#111827;--suave:#4b5563;--linha:#d1d5db;--fundo:#fff;--destaque:#b45309}
body{font-family:"Barlow",system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
h1,h2,h3,h4,.marca{font-family:"Oswald","Barlow",system-ui,sans-serif;font-weight:600;text-transform:uppercase;letter-spacing:.02em}
header.topo{border-bottom:3px solid var(--texto)}
nav.principal a,nav.principal summary{font-size:.875rem;font-weight:600;text-transform:uppercase;letter-spacing:.06em}
img,iframe,modelo-3d,.chamada,.chamada a,.botao{border-radius:.125rem}
.botao,.chamada a{text-transform:uppercase;letter-spacing:.05em}
.filtros a{border-radius:.125rem}
"#;

const EDITORIAL: &str = r#"
@font-face{font-family:"IBM Plex Serif";font-weight:500;font-display:swap;src:url(/recursos/fontes/plex-serif-500.woff2) format("woff2")}
@font-face{font-family:"IBM Plex Serif";font-weight:700;font-display:swap;src:url(/recursos/fontes/plex-serif-700.woff2) format("woff2")}
:root{--texto:#1f2937;--suave:#5b6470;--linha:#e5e0d8;--fundo:#fbfaf7;--destaque:#7c2d12}
body{font-family:"Iowan Old Style","Palatino Linotype",Palatino,Georgia,serif;font-size:1.125rem}
h1,h2,h3,h4,.marca{font-family:"IBM Plex Serif",Georgia,serif;font-weight:700;letter-spacing:0}
h3,h4,blockquote{font-family:"IBM Plex Serif",Georgia,serif;font-weight:500}
nav.principal a,nav.principal summary,figcaption,.datas,.trilha,.filtros,.ficha,.produto-codigo{font-family:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
img,iframe,modelo-3d,.chamada{border-radius:.25rem}
"#;

const MODERNO: &str = r#"
@font-face{font-family:"Archivo";font-weight:100 900;font-display:swap;src:url(/recursos/fontes/archivo.woff2) format("woff2")}
:root{--texto:#0f172a;--suave:#475569;--linha:#e2e8f0;--fundo:#fff;--destaque:#1d4ed8}
body{font-family:"Archivo",system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
h1,h2,h3,.marca{font-weight:800;letter-spacing:-.025em}
img,iframe,modelo-3d,.chamada{border-radius:1rem}
.botao,.chamada a{border-radius:999px}
header.topo{border-bottom:0;box-shadow:0 1px 0 var(--linha)}
"#;

/// O estilo que o tema põe por cima do estilo de base. O clássico é o
/// próprio estilo de base.
pub fn estilo(tema: Tema) -> &'static str {
    match tema {
        Tema::Classico => "",
        Tema::Industrial => INDUSTRIAL,
        Tema::Editorial => EDITORIAL,
        Tema::Moderno => MODERNO,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_tema_so_usa_fonte_que_o_cms_serve() {
        for tema in Tema::TODOS {
            let estilo = estilo(tema);
            // Nada de fonte nem de folha de estilo de fora.
            assert!(!estilo.contains("http"), "{}", tema.slug());
            for trecho in estilo.split("url(").skip(1) {
                let endereco = trecho.split(')').next().unwrap_or_default();
                assert!(fonte(endereco).is_some(), "{}: {endereco}", tema.slug());
            }
        }
        // E não sobra fonte sem tema que a use.
        let todos: String = Tema::TODOS.into_iter().map(estilo).collect();
        for (arquivo, bytes) in FONTES {
            assert!(todos.contains(arquivo), "{arquivo}");
            assert!(bytes.starts_with(b"wOF2"), "{arquivo}");
        }
    }

    #[test]
    fn fonte_que_nao_existe_nao_sai() {
        for ruim in [
            "/recursos/fontes/",
            "/recursos/fontes/../modelo-3d.js",
            "/recursos/fontes/OFL.txt",
            "/recursos/fontes/barlow-400.woff",
        ] {
            assert!(fonte(ruim).is_none(), "{ruim}");
        }
    }
}
