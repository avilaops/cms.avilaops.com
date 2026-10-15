//! Regras puras do CMS: o que é um site, como um host vira um site e quando
//! ele aparece na busca. Sem banco e sem rede.

pub mod cardapio;
pub mod conta;
pub mod datas;
pub mod endereco;
pub mod eventos;
pub mod fluxo;
pub mod modelos;
pub mod redirecionamento;
pub mod site;

pub use conta::{Conta, LimitesDeCriacao};
pub use endereco::{Endereco, Host, classificar, ler_host};
pub use fluxo::{Acao, Ator, Papel};
pub use site::{EntradaDoMenu, PerfilDoSite, Situacao, SituacaoDesconhecida, ler_entrada_do_menu};
