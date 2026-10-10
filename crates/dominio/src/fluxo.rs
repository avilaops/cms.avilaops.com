//! O fluxo de publicação: quem pode fazer o quê com um documento, o endereço
//! que ele ganha e as datas com que vai ao ar.

use chrono::{DateTime, Utc};
use motor_web::tipos::{Conteudo, Documento};
use motor_web::validacao::{Gravidade, Problema, deve_atualizar_data};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    Autor,
    Editor,
    Dono,
}

/// Quem está agindo em um site. O login monta; as regras só leem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ator {
    /// O `sub` da conta no Auth.
    pub conta: String,
    pub papel: Papel,
    /// Equipe da Ávila Ops: age com poder de Dono, e o histórico registra.
    pub equipe: bool,
}

impl Ator {
    pub fn do_site(conta: impl Into<String>, papel: Papel) -> Self {
        Self {
            conta: conta.into(),
            papel,
            equipe: false,
        }
    }

    pub fn da_equipe(conta: impl Into<String>) -> Self {
        Self {
            conta: conta.into(),
            papel: Papel::Dono,
            equipe: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acao {
    CriarRascunho,
    EditarRascunho,
    EnviarParaRevisao,
    Devolver,
    Publicar,
    Despublicar,
    /// Marcar a publicação para depois. Quem pode publicar pode agendar.
    Agendar,
}

impl Acao {
    /// Como a ação fica gravada no histórico.
    pub fn como_texto(self) -> &'static str {
        match self {
            Acao::CriarRascunho => "rascunho.criado",
            Acao::EditarRascunho => "rascunho.editado",
            Acao::EnviarParaRevisao => "revisao.pedida",
            Acao::Devolver => "revisao.devolvida",
            Acao::Publicar => "conteudo.publicado",
            Acao::Despublicar => "conteudo.despublicado",
            Acao::Agendar => "publicacao.agendada",
        }
    }
}

/// A permissão por ação. `criado_por` é a conta de quem criou o documento.
///
/// Autor mexe só no que é dele e não põe nem tira nada do ar. Editor e Dono
/// fazem tudo o que é de conteúdo.
pub fn pode(ator: &Ator, acao: Acao, criado_por: Option<&str>) -> bool {
    match ator.papel {
        Papel::Editor | Papel::Dono => true,
        Papel::Autor => match acao {
            Acao::CriarRascunho => true,
            Acao::EditarRascunho | Acao::EnviarParaRevisao => {
                criado_por == Some(ator.conta.as_str())
            }
            Acao::Devolver | Acao::Publicar | Acao::Despublicar | Acao::Agendar => false,
        },
    }
}

/// Caminhos que o servidor usa para outra coisa: uma página com um destes
/// slugs nunca seria alcançada.
const SLUGS_RESERVADOS: [&str; 4] = ["api", "autor", "blog", "midia"];

/// Onde o documento é servido. O CMS não serve produto.
pub fn caminho_de(conteudo: &Conteudo) -> Option<String> {
    match conteudo {
        Conteudo::Pagina(pagina) if pagina.slug.is_empty() => Some("/".to_string()),
        Conteudo::Pagina(pagina) => Some(format!("/{}", pagina.slug)),
        Conteudo::Post(post) => Some(format!("/blog/{}", post.slug)),
        Conteudo::Produto(_) => None,
    }
}

/// A trava do endereço reservado, no formato das travas do motor.
pub fn problema_de_slug_reservado(conteudo: &Conteudo) -> Option<Problema> {
    match conteudo {
        Conteudo::Pagina(pagina) if SLUGS_RESERVADOS.contains(&pagina.slug.as_str()) => {
            Some(Problema {
                codigo: "slug.reservado",
                campo: "slug".to_string(),
                gravidade: Gravidade::Bloqueia,
                mensagem: format!(
                    "O endereço \"{}\" é usado pelo próprio site. Escolha outro.",
                    pagina.slug
                ),
            })
        }
        _ => None,
    }
}

/// Outro documento do site já ocupa o endereço.
pub fn problema_de_slug_em_uso() -> Problema {
    Problema {
        codigo: "slug.em-uso",
        campo: "slug".to_string(),
        gravidade: Gravidade::Bloqueia,
        mensagem: "Outra página do site já usa este endereço. Escolha outro.".to_string(),
    }
}

/// As datas com que um documento vai ao ar. São do servidor, nunca de quem
/// edita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Datas {
    pub publicado_em: DateTime<Utc>,
    pub atualizado_em: DateTime<Utc>,
}

/// "Publicado em" é a primeira publicação e não muda mais. "Atualizado em" só
/// anda quando o que o visitante lê mudou em relação à última versão no ar.
pub fn datas_da_publicacao(
    primeira_publicacao: Option<DateTime<Utc>>,
    ultima_no_ar: Option<&Documento>,
    novo: &Documento,
    agora: DateTime<Utc>,
) -> Datas {
    let atualizado_em = match ultima_no_ar {
        Some(antes) if !deve_atualizar_data(antes, novo) => antes.atualizado_em(),
        _ => agora,
    };
    Datas {
        publicado_em: primeira_publicacao.unwrap_or(agora),
        atualizado_em,
    }
}

/// Grava as datas no conteúdo, que é de onde o motor as lê.
pub fn carimbar(conteudo: &mut Conteudo, datas: Datas) {
    match conteudo {
        Conteudo::Pagina(pagina) => {
            pagina.publicado_em = datas.publicado_em;
            pagina.atualizado_em = datas.atualizado_em;
        }
        Conteudo::Post(post) => {
            post.publicado_em = datas.publicado_em;
            post.atualizado_em = datas.atualizado_em;
        }
        Conteudo::Produto(produto) => produto.atualizado_em = datas.atualizado_em,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use chrono::TimeZone;
    use motor_web::demonstracao;

    const TODAS: [Acao; 7] = [
        Acao::CriarRascunho,
        Acao::EditarRascunho,
        Acao::EnviarParaRevisao,
        Acao::Devolver,
        Acao::Publicar,
        Acao::Despublicar,
        Acao::Agendar,
    ];

    fn dia(dia: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, dia, 12, 0, 0)
            .single()
            .expect("data válida")
    }

    fn documento(caminho: &str) -> Documento {
        demonstracao::documentos()
            .into_iter()
            .find(|d| d.caminho == caminho)
            .unwrap_or_else(|| panic!("o exemplo não tem {caminho}"))
    }

    #[test]
    fn editor_dono_e_equipe_fazem_tudo_de_conteudo() {
        for ator in [
            Ator::do_site("editor", Papel::Editor),
            Ator::do_site("dono", Papel::Dono),
            Ator::da_equipe("equipe"),
        ] {
            for acao in TODAS {
                assert!(pode(&ator, acao, Some("outra-conta")), "{ator:?} {acao:?}");
            }
        }
    }

    #[test]
    fn autor_so_mexe_no_que_e_dele_e_nao_publica() {
        let autor = Ator::do_site("autor", Papel::Autor);
        assert!(pode(&autor, Acao::CriarRascunho, None));
        for acao in [Acao::EditarRascunho, Acao::EnviarParaRevisao] {
            assert!(pode(&autor, acao, Some("autor")), "{acao:?}");
            assert!(!pode(&autor, acao, Some("outra-conta")), "{acao:?}");
            assert!(!pode(&autor, acao, None), "{acao:?}");
        }
        for acao in [
            Acao::Devolver,
            Acao::Publicar,
            Acao::Despublicar,
            Acao::Agendar,
        ] {
            assert!(!pode(&autor, acao, Some("autor")), "{acao:?}");
        }
    }

    #[test]
    fn cada_especie_tem_o_seu_caminho() {
        assert_eq!(caminho_de(&documento("/").conteudo).as_deref(), Some("/"));
        assert_eq!(
            caminho_de(&documento("/sobre").conteudo).as_deref(),
            Some("/sobre")
        );
        let post = "/blog/como-escolher-a-madeira-da-mesa";
        assert_eq!(caminho_de(&documento(post).conteudo).as_deref(), Some(post));
        let produto = demonstracao::documentos()
            .into_iter()
            .find(|d| matches!(d.conteudo, Conteudo::Produto(_)))
            .expect("o exemplo tem produto");
        assert_eq!(caminho_de(&produto.conteudo), None);
    }

    #[test]
    fn pagina_nao_toma_endereco_do_proprio_site() {
        let mut sobre = documento("/sobre");
        assert_eq!(problema_de_slug_reservado(&sobre.conteudo), None);
        if let Conteudo::Pagina(pagina) = &mut sobre.conteudo {
            pagina.slug = "blog".into();
        }
        let problema = problema_de_slug_reservado(&sobre.conteudo).expect("slug reservado");
        assert_eq!(problema.codigo, "slug.reservado");
        assert_eq!(problema.gravidade, Gravidade::Bloqueia);
    }

    #[test]
    fn primeira_publicacao_usa_a_hora_do_servidor() {
        let novo = documento("/sobre");
        let datas = datas_da_publicacao(None, None, &novo, dia(5));
        assert_eq!(
            datas,
            Datas {
                publicado_em: dia(5),
                atualizado_em: dia(5)
            }
        );
    }

    #[test]
    fn atualizado_em_so_anda_quando_o_que_se_le_mudou() {
        let mut no_ar = documento("/sobre");
        carimbar(
            &mut no_ar.conteudo,
            Datas {
                publicado_em: dia(1),
                atualizado_em: dia(2),
            },
        );

        // Só a descrição de busca mudou: o visitante lê a mesma página.
        let mut so_busca = no_ar.clone();
        if let Conteudo::Pagina(pagina) = &mut so_busca.conteudo {
            pagina.seo.descricao = "Outra descrição para o buscador.".into();
        }
        let datas = datas_da_publicacao(Some(dia(1)), Some(&no_ar), &so_busca, dia(9));
        assert_eq!(
            datas,
            Datas {
                publicado_em: dia(1),
                atualizado_em: dia(2)
            }
        );

        let mut outro_titulo = no_ar.clone();
        if let Conteudo::Pagina(pagina) = &mut outro_titulo.conteudo {
            pagina.titulo = "Quem faz a oficina".into();
        }
        let datas = datas_da_publicacao(Some(dia(1)), Some(&no_ar), &outro_titulo, dia(9));
        assert_eq!(
            datas,
            Datas {
                publicado_em: dia(1),
                atualizado_em: dia(9)
            }
        );
    }

    #[test]
    fn carimbar_grava_as_duas_datas_no_conteudo() {
        let mut post = documento("/blog/como-escolher-a-madeira-da-mesa");
        carimbar(
            &mut post.conteudo,
            Datas {
                publicado_em: dia(3),
                atualizado_em: dia(4),
            },
        );
        let Conteudo::Post(dados) = &post.conteudo else {
            panic!("o exemplo é um post");
        };
        assert_eq!(dados.publicado_em, dia(3));
        assert_eq!(post.atualizado_em(), dia(4));
    }
}
