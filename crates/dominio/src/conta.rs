//! A conta de quem entra pelo Auth, o papel dela em um site e as regras de
//! criação de site.

use std::fmt;

use crate::fluxo::{Ator, Papel};

/// O papel `ADMIN` no token do Auth: a equipe da Ávila Ops.
const PAPEL_DE_EQUIPE: &str = "ADMIN";

/// Quem o Auth disse que é. Ter conta não dá acesso a site nenhum: isso é da
/// participação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conta {
    /// O `sub` do Auth: é ele que identifica a conta, não o e-mail.
    pub sub: String,
    /// Em minúsculas.
    pub email: String,
    pub nome: String,
    pub equipe: bool,
}

impl Conta {
    pub fn nova(sub: &str, email: &str, nome: &str, papel_no_auth: &str) -> Self {
        Self {
            sub: sub.to_string(),
            email: email.trim().to_lowercase(),
            nome: nome.trim().to_string(),
            equipe: papel_no_auth == PAPEL_DE_EQUIPE,
        }
    }

    /// O ator da conta em um site. A equipe entra em qualquer site com poder
    /// de Dono; os demais, só com participação.
    pub fn ator(&self, participacao: Option<Papel>) -> Option<Ator> {
        if self.equipe {
            Some(Ator::da_equipe(self.sub.as_str()))
        } else {
            participacao.map(|papel| Ator::do_site(self.sub.as_str(), papel))
        }
    }
}

impl Papel {
    pub fn como_texto(self) -> &'static str {
        match self {
            Papel::Autor => "autor",
            Papel::Editor => "editor",
            Papel::Dono => "dono",
        }
    }

    /// Como o papel aparece para quem usa o painel.
    pub fn rotulo(self) -> &'static str {
        match self {
            Papel::Autor => "Autor",
            Papel::Editor => "Editor",
            Papel::Dono => "Dono",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PapelDesconhecido(pub String);

impl fmt::Display for PapelDesconhecido {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "papel desconhecido: {}", self.0)
    }
}

impl std::error::Error for PapelDesconhecido {}

impl TryFrom<&str> for Papel {
    type Error = PapelDesconhecido;

    fn try_from(texto: &str) -> Result<Self, Self::Error> {
        match texto {
            "autor" => Ok(Papel::Autor),
            "editor" => Ok(Papel::Editor),
            "dono" => Ok(Papel::Dono),
            outro => Err(PapelDesconhecido(outro.to_string())),
        }
    }
}

/// Equipe, convites, identidade, domínio e apagar o site são do Dono.
pub fn pode_administrar(ator: &Ator) -> bool {
    ator.papel == Papel::Dono
}

/// Rótulos que não viram endereço de site: são do servidor ou de produtos da
/// casa, e um site com esse nome passaria por coisa oficial.
const SLUGS_RESERVADOS: [&str; 16] = [
    "admin", "api", "app", "auth", "cms", "crm", "docs", "erp", "lojas", "mail", "n8n", "painel",
    "sites", "suporte", "status", "www",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlugInvalido {
    Formato,
    Reservado,
}

impl SlugInvalido {
    /// Para leigo: diz o que fazer.
    pub fn mensagem(self) -> &'static str {
        match self {
            SlugInvalido::Formato => {
                "O endereço precisa ter de 3 a 40 letras minúsculas, números ou hífens, sem começar nem terminar com hífen."
            }
            SlugInvalido::Reservado => "Este endereço é reservado. Escolha outro.",
        }
    }
}

/// O rótulo do endereço provisório, `<slug>.<domínio-base>`.
pub fn validar_slug_de_site(slug: &str) -> Result<(), SlugInvalido> {
    let formato_valido = (3..=40).contains(&slug.len())
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-');
    if !formato_valido {
        return Err(SlugInvalido::Formato);
    }
    if SLUGS_RESERVADOS.contains(&slug) {
        return Err(SlugInvalido::Reservado);
    }
    Ok(())
}

/// Os freios da criação aberta de sites. São configuração.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitesDeCriacao {
    /// Sites de que uma conta pode ser Dono.
    pub sites_por_conta: i64,
    /// Sites que uma conta pode criar em 24 horas.
    pub criacoes_por_dia: i64,
}

impl Default for LimitesDeCriacao {
    fn default() -> Self {
        Self {
            sites_por_conta: 3,
            criacoes_por_dia: 2,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_o_papel_admin_do_auth_e_equipe() {
        assert!(Conta::nova("1", "A@Exemplo.com ", " Ana ", "ADMIN").equipe);
        let cliente = Conta::nova("2", "A@Exemplo.com ", " Ana ", "CLIENTE");
        assert!(!cliente.equipe);
        assert_eq!(cliente.email, "a@exemplo.com");
        assert_eq!(cliente.nome, "Ana");
        assert!(!Conta::nova("3", "b@exemplo.com", "", "admin").equipe);
    }

    #[test]
    fn equipe_entra_como_dono_e_os_demais_pela_participacao() {
        let equipe = Conta::nova("1", "a@exemplo.com", "Ana", "ADMIN");
        assert_eq!(equipe.ator(None), Some(Ator::da_equipe("1")));
        assert_eq!(equipe.ator(Some(Papel::Autor)), Some(Ator::da_equipe("1")));

        let cliente = Conta::nova("2", "b@exemplo.com", "Bia", "CLIENTE");
        assert_eq!(cliente.ator(None), None);
        assert_eq!(
            cliente.ator(Some(Papel::Editor)),
            Some(Ator::do_site("2", Papel::Editor))
        );
    }

    #[test]
    fn papel_vai_e_volta_como_texto() {
        for papel in [Papel::Autor, Papel::Editor, Papel::Dono] {
            assert_eq!(Papel::try_from(papel.como_texto()), Ok(papel));
        }
        assert!(Papel::try_from("ADMIN").is_err());
    }

    #[test]
    fn so_o_dono_administra() {
        assert!(pode_administrar(&Ator::do_site("1", Papel::Dono)));
        assert!(pode_administrar(&Ator::da_equipe("2")));
        assert!(!pode_administrar(&Ator::do_site("3", Papel::Editor)));
        assert!(!pode_administrar(&Ator::do_site("4", Papel::Autor)));
    }

    #[test]
    fn slug_de_site_tem_formato_e_nomes_reservados() {
        for bom in ["oficina", "casa-do-pao", "abc", "loja2"] {
            assert_eq!(validar_slug_de_site(bom), Ok(()), "{bom}");
        }
        for ruim in [
            "", "ab", "-oficina", "oficina-", "Oficina", "a.b", "a b c", "ação",
        ] {
            assert_eq!(
                validar_slug_de_site(ruim),
                Err(SlugInvalido::Formato),
                "{ruim}"
            );
        }
        assert_eq!(
            validar_slug_de_site(&"a".repeat(41)),
            Err(SlugInvalido::Formato)
        );
        for reservado in ["www", "cms", "painel", "lojas", "admin"] {
            assert_eq!(
                validar_slug_de_site(reservado),
                Err(SlugInvalido::Reservado),
                "{reservado}"
            );
        }
    }
}
