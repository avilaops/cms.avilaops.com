//! Acesso ao banco. SQLx é o dono do esquema: as migrações moram aqui, e as
//! consultas são conferidas na compilação.

mod catalogo;
mod conector;
mod contas;
mod documentos;
mod dominios;
mod eventos;
pub mod fluxo;
mod midias;
mod rotinas;
mod sites;

use sqlx::PgPool;
use sqlx::migrate::Migrator;

pub use catalogo::{
    AutorDoSite, DadosDoAutor, DocumentoAberto, DocumentoDoSite, ErroDeCatalogo, abrir_documento,
    autor_para_conteudo, autores_do_site, categorias_do_site, documentos_do_site, midia_vazia,
    salvar_autor, salvar_categoria,
};
pub use conector::{
    Autorizacao, ClienteMcp, Conexao, ConexaoDaConta, ESCOPO_DE_PUBLICAR, ESCOPOS, ErroDeConector,
    Tokens, cliente_mcp, conexao_por_token, conexoes_da_conta, criar_codigo, registrar_chamada,
    registrar_cliente, renovar_tokens, revogar_conexao, trocar_codigo,
};
pub use contas::{
    ConviteCriado, ConvitePendente, ErroDeConta, Membro, SiteDaConta, aceitar_convite,
    convites_pendentes, criar_convite, criar_site_para, equipe_do_site, papel_no_site,
    sites_da_conta,
};
pub use documentos::{
    Ausencia, ItemDeNavegacao, ausencia, documento_publicado, documentos_publicados, navegacao,
    publicar,
};
pub use dominios::{
    DominioDoSite, DominioPendente, ErroDeDominio, confirmar_dominio,
    definir_provisorio_definitivo, dominio_permitido, dominios_do_site, dominios_pendentes,
    pedir_dominio, remover_dominio,
};
pub use eventos::{
    Encerramento, EventoAEntregar, encerrar_evento, marcar_evento_entregue, reivindicar_eventos,
};
pub use midias::{
    ErroDeMidia, MidiaAProcessar, MidiaDoSite, NovaMidia, VarianteGravada, apagar_midia,
    concluir_midia, falhar_midia, hash_de_conteudo, hidratar_conteudo, midia_para_conteudo,
    midias_do_site, registrar_midia, reivindicar_midia,
};
pub use rotinas::{Agendado, limpar_historico, reivindicar_rotina};
pub use sites::{
    LinhaDoHistorico, SiteGravado, apagar_site, ativar_dominio, atualizar_perfil, criar_site,
    historico_do_site, mudar_situacao, site_por_dominio, site_por_slug,
};

pub static MIGRADOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum ErroDeDados {
    #[error("falha no banco: {0}")]
    Banco(#[from] sqlx::Error),
    #[error("dado gravado não pôde ser lido: {0}")]
    DadoInvalido(String),
}

/// Usado pela rota de saúde: o serviço só está apto se o banco responde.
pub async fn banco_responde(pool: &PgPool) -> bool {
    sqlx::query_scalar!("select 1 as \"um!\"")
        .fetch_one(pool)
        .await
        .is_ok()
}
