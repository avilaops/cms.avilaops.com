//! Busca e páginas das listas longas do painel. Sem JavaScript: a busca é um
//! formulário GET e cada página da lista tem endereço próprio.

use axum::http::Uri;
use motor_web::validacao::normalizar_slug;

/// Quantos itens cabem em uma página de lista do painel.
pub const POR_PAGINA: usize = 24;

/// O que a pessoa pediu na lista: o que procura e em que página está.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Consulta {
    busca: String,
    pagina: usize,
}

impl Consulta {
    /// Lê `busca` e `pagina` do endereço. Página que não é número vale 1.
    pub fn da(uri: &Uri) -> Self {
        let pares = serde_urlencoded::from_str::<Vec<(String, String)>>(uri.query().unwrap_or(""))
            .unwrap_or_default();
        let valor = |nome: &str| {
            pares
                .iter()
                .find(|(chave, _)| chave == nome)
                .map(|(_, valor)| valor.trim())
        };
        Self {
            busca: valor("busca").unwrap_or_default().to_string(),
            pagina: valor("pagina")
                .and_then(|pagina| pagina.parse().ok())
                .unwrap_or(1),
        }
    }

    /// Todas as palavras procuradas aparecem no texto, sem ligar para acento
    /// nem maiúscula. Busca vazia casa com tudo.
    pub fn casa(&self, texto: &str) -> bool {
        let alvo = normalizar_slug(texto);
        self.busca
            .split_whitespace()
            .map(normalizar_slug)
            .filter(|palavra| !palavra.is_empty())
            .all(|palavra| alvo.contains(&palavra))
    }

    /// A página pedida da lista já filtrada. Página além do fim vira a última.
    pub fn recortar<T>(&self, itens: Vec<T>, caminho: &str) -> Lista<T> {
        let total = itens.len();
        let paginas = total.div_ceil(POR_PAGINA).max(1);
        let pagina = self.pagina.clamp(1, paginas);
        let endereco = |numero: usize| {
            let mut pares: Vec<(&str, String)> = Vec::new();
            if !self.busca.is_empty() {
                pares.push(("busca", self.busca.clone()));
            }
            if numero > 1 {
                pares.push(("pagina", numero.to_string()));
            }
            match serde_urlencoded::to_string(&pares) {
                Ok(consulta) if !consulta.is_empty() => format!("{caminho}?{consulta}"),
                _ => caminho.to_string(),
            }
        };
        Lista {
            itens: itens
                .into_iter()
                .skip((pagina - 1) * POR_PAGINA)
                .take(POR_PAGINA)
                .collect(),
            caminho: caminho.to_string(),
            busca: self.busca.clone(),
            total,
            pagina,
            paginas,
            anterior: (pagina > 1).then(|| endereco(pagina - 1)),
            proxima: (pagina < paginas).then(|| endereco(pagina + 1)),
        }
    }
}

/// Uma página de uma lista do painel, com o que a tela precisa para a busca
/// e para a navegação.
#[derive(Debug)]
pub struct Lista<T> {
    pub itens: Vec<T>,
    /// O endereço da lista, para onde a busca é enviada.
    pub caminho: String,
    pub busca: String,
    /// Quantos itens a busca achou, em todas as páginas.
    pub total: usize,
    pub pagina: usize,
    pub paginas: usize,
    pub anterior: Option<String>,
    pub proxima: Option<String>,
}

impl<T> Lista<T> {
    /// A frase acima da lista: quantos itens há e, com busca, o que foi
    /// procurado. Recebe o nome do item sem nenhum, com um e com vários.
    pub fn resumo(&self, nenhum: &str, um: &str, varios: &str) -> String {
        let quantos = match self.total {
            0 => nenhum.to_string(),
            1 => format!("1 {um}"),
            n => format!("{n} {varios}"),
        };
        if self.busca.is_empty() {
            quantos
        } else {
            format!("{quantos} com \"{}\"", self.busca)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn consulta(endereco: &str) -> Consulta {
        Consulta::da(&endereco.parse::<Uri>().expect("endereço"))
    }

    #[test]
    fn busca_ignora_acento_maiuscula_e_ordem_das_palavras() {
        let procura = consulta("/lista?busca=PIX+comprovante");
        assert!(procura.casa("Golpe do comprovante falso de Pix"));
        assert!(!procura.casa("Prazo de entrega"));
        assert!(consulta("/lista?busca=preco").casa("Preço de referência"));
        assert!(consulta("/lista").casa("qualquer coisa"));
        assert!(consulta("/lista?busca=%20%20").casa("qualquer coisa"));
    }

    #[test]
    fn recorte_tem_pagina_total_e_vizinhas_com_a_busca_no_endereco() {
        let itens: Vec<usize> = (0..50).collect();
        let primeira = consulta("/lista").recortar(itens.clone(), "/lista");
        assert_eq!(
            (primeira.itens.len(), primeira.total, primeira.paginas),
            (24, 50, 3)
        );
        assert_eq!(primeira.anterior, None);
        assert_eq!(primeira.proxima.as_deref(), Some("/lista?pagina=2"));

        let terceira = consulta("/lista?busca=a+b&pagina=3").recortar(itens.clone(), "/lista");
        assert_eq!(terceira.itens, vec![48, 49]);
        assert_eq!(
            terceira.anterior.as_deref(),
            Some("/lista?busca=a+b&pagina=2")
        );
        assert_eq!(terceira.proxima, None);

        // Página que não existe ou não é número não quebra a tela.
        assert_eq!(
            consulta("/lista?pagina=99")
                .recortar(itens.clone(), "/lista")
                .pagina,
            3
        );
        assert_eq!(
            consulta("/lista?pagina=x").recortar(itens, "/lista").pagina,
            1
        );
        let vazia = consulta("/lista?busca=nada").recortar(Vec::<usize>::new(), "/lista");
        assert_eq!((vazia.pagina, vazia.paginas, vazia.total), (1, 1, 0));
        assert_eq!(
            vazia.resumo("Nenhuma imagem", "imagem", "imagens"),
            "Nenhuma imagem com \"nada\""
        );
    }
}
