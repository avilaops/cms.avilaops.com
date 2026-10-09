//! Do cabeçalho `Host` ao endereço de um site.

/// O host de um pedido, já conferido e em minúsculas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    /// Sem porta: é por ele que o site é procurado.
    pub nome: String,
    /// Como veio, com a porta se havia. Usado para montar endereços.
    pub com_porta: String,
}

/// Lê o valor de `Host` ou de `X-Forwarded-Host`. Devolve `None` para o que
/// não é um nome de host: o pedido é recusado sem tocar em dado de site.
pub fn ler_host(bruto: &str) -> Option<Host> {
    let bruto = bruto.trim().to_ascii_lowercase();
    let (nome, porta) = match bruto.rsplit_once(':') {
        Some((nome, porta)) if !porta.is_empty() && porta.chars().all(|c| c.is_ascii_digit()) => {
            (nome, Some(porta))
        }
        Some(_) => return None,
        None => (bruto.as_str(), None),
    };
    let nome = nome.trim_end_matches('.');

    let rotulo_valido = |rotulo: &str| {
        !rotulo.is_empty()
            && rotulo.len() <= 63
            && !rotulo.starts_with('-')
            && !rotulo.ends_with('-')
            && rotulo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    if nome.is_empty() || nome.len() > 253 || !nome.split('.').all(rotulo_valido) {
        return None;
    }

    let com_porta = match porta {
        Some(porta) => format!("{nome}:{porta}"),
        None => nome.to_string(),
    };
    Some(Host {
        nome: nome.to_string(),
        com_porta,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endereco {
    /// `<slug>.<domínio-base>`: o endereço com que todo site nasce.
    Provisorio { slug: String },
    /// Qualquer outro host: só vale se for domínio ativo de algum site.
    Proprio { host: String },
}

pub fn classificar(host: &Host, dominio_base: &str) -> Endereco {
    let slug = host
        .nome
        .strip_suffix(dominio_base)
        .and_then(|resto| resto.strip_suffix('.'));
    match slug {
        // Um rótulo só: `a.b.<base>` não é endereço provisório de ninguém.
        Some(slug) if !slug.is_empty() && !slug.contains('.') => Endereco::Provisorio {
            slug: slug.to_string(),
        },
        _ => Endereco::Proprio {
            host: host.nome.clone(),
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn host(texto: &str) -> Host {
        ler_host(texto).unwrap_or_else(|| panic!("host inválido: {texto}"))
    }

    #[test]
    fn le_host_com_e_sem_porta() {
        assert_eq!(
            host("Exemplo.COM.br"),
            Host {
                nome: "exemplo.com.br".into(),
                com_porta: "exemplo.com.br".into()
            }
        );
        assert_eq!(
            host("demo.localhost:3090"),
            Host {
                nome: "demo.localhost".into(),
                com_porta: "demo.localhost:3090".into()
            }
        );
        assert_eq!(host("exemplo.com.").nome, "exemplo.com");
    }

    #[test]
    fn recusa_o_que_nao_e_nome_de_host() {
        for ruim in [
            "",
            " ",
            "a b.com",
            "exemplo.com/caminho",
            "exemplo..com",
            "-a.com",
            "a.com:porta",
            "[::1]:80",
            "a_b.com",
            "usuario@a.com",
        ] {
            assert_eq!(ler_host(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn subdominio_do_dominio_base_e_provisorio() {
        assert_eq!(
            classificar(&host("oficina.sites.avilaops.com"), "sites.avilaops.com"),
            Endereco::Provisorio {
                slug: "oficina".into()
            }
        );
    }

    #[test]
    fn o_resto_e_dominio_proprio() {
        let base = "sites.avilaops.com";
        for outro in [
            "oficina.com.br",
            "sites.avilaops.com",
            "a.b.sites.avilaops.com",
            "xsites.avilaops.com",
            "oficina.sites.avilaops.com.golpe.com",
        ] {
            assert_eq!(
                classificar(&host(outro), base),
                Endereco::Proprio { host: outro.into() },
                "{outro}"
            );
        }
    }
}
