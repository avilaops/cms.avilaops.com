//! Regras puras do CMS: o que é um site, como um host vira um site e quando
//! ele aparece na busca. Sem banco e sem rede.

pub mod datas;
pub mod endereco;
pub mod site;

pub use endereco::{Endereco, Host, classificar, ler_host};
pub use site::{PerfilDoSite, Situacao, SituacaoDesconhecida};
