//! O cache de página: a resposta pronta, em memória, por host e caminho.
//!
//! Vale um minuto e é derrubado por site quando algo é publicado, tirado do
//! ar ou muda de endereço. Tem teto de entradas e de tamanho: passou, a página
//! simplesmente não é guardada.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

const VALIDADE: Duration = Duration::from_secs(60);
const MAXIMO_DE_ENTRADAS: usize = 2000;
const MAXIMO_POR_PAGINA: usize = 512 * 1024;

struct Entrada {
    site_id: Uuid,
    expira: Instant,
    cabecalhos: HeaderMap,
    corpo: Bytes,
}

#[derive(Default)]
pub struct Cache {
    entradas: Mutex<HashMap<String, Entrada>>,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cache")
    }
}

fn com_marca(mut resposta: Response, marca: &'static str) -> Response {
    resposta
        .headers_mut()
        .insert("x-cache", HeaderValue::from_static(marca));
    resposta
}

impl Cache {
    pub fn novo() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// A página guardada para esta chave, se ainda vale.
    pub fn buscar(&self, chave: &str) -> Option<Response> {
        let entradas = self.entradas.lock().unwrap_or_else(PoisonError::into_inner);
        let entrada = entradas.get(chave).filter(|e| e.expira > Instant::now())?;
        let mut resposta = (StatusCode::OK, entrada.corpo.clone()).into_response();
        *resposta.headers_mut() = entrada.cabecalhos.clone();
        Some(com_marca(resposta, "hit"))
    }

    /// Guarda uma resposta 200 já montada e a devolve para seguir ao
    /// visitante.
    pub fn guardar(
        &self,
        chave: String,
        site_id: Uuid,
        cabecalhos: HeaderMap,
        corpo: Bytes,
    ) -> Response {
        if corpo.len() <= MAXIMO_POR_PAGINA {
            let mut entradas = self.entradas.lock().unwrap_or_else(PoisonError::into_inner);
            if entradas.len() >= MAXIMO_DE_ENTRADAS {
                let agora = Instant::now();
                entradas.retain(|_, entrada| entrada.expira > agora);
            }
            if entradas.len() < MAXIMO_DE_ENTRADAS {
                entradas.insert(
                    chave,
                    Entrada {
                        site_id,
                        expira: Instant::now() + VALIDADE,
                        cabecalhos: cabecalhos.clone(),
                        corpo: corpo.clone(),
                    },
                );
            }
        }
        let mut resposta = (StatusCode::OK, corpo).into_response();
        *resposta.headers_mut() = cabecalhos;
        com_marca(resposta, "miss")
    }

    /// Derruba tudo o que é de um site.
    pub fn invalidar_site(&self, site_id: Uuid) {
        self.entradas
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, entrada| entrada.site_id != site_id);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pagina(cache: &Cache, chave: &str, site_id: Uuid) {
        cache.guardar(
            chave.to_string(),
            site_id,
            HeaderMap::new(),
            Bytes::from_static(b"<h1>oi</h1>"),
        );
    }

    #[test]
    fn guarda_devolve_e_derruba_por_site() {
        let cache = Cache::default();
        let (padaria, oficina) = (Uuid::new_v4(), Uuid::new_v4());
        assert!(cache.buscar("padaria.example/").is_none());
        pagina(&cache, "padaria.example/", padaria);
        pagina(&cache, "padaria.example/sobre", padaria);
        pagina(&cache, "oficina.example/", oficina);

        let achada = cache.buscar("padaria.example/").expect("página guardada");
        assert_eq!(achada.headers().get("x-cache").expect("marca"), "hit");

        cache.invalidar_site(padaria);
        assert!(cache.buscar("padaria.example/").is_none());
        assert!(cache.buscar("padaria.example/sobre").is_none());
        assert!(cache.buscar("oficina.example/").is_some());
    }

    #[test]
    fn pagina_grande_demais_nao_e_guardada() {
        let cache = Cache::default();
        cache.guardar(
            "grande".to_string(),
            Uuid::new_v4(),
            HeaderMap::new(),
            Bytes::from(vec![b'x'; MAXIMO_POR_PAGINA + 1]),
        );
        assert!(cache.buscar("grande").is_none());
    }
}
