//! Clientes HTTP do que fica fora do CMS. Sem banco: recebem o que enviar e
//! dizem se chegou.

pub mod n8n;

pub use n8n::{ClienteN8n, ErroDeEntrega};
