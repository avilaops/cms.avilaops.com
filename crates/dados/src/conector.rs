//! O servidor de autorização do conector: clientes, códigos com PKCE, tokens
//! e o registro de chamadas.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use cms_dominio::Conta;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::ErroDeDados;
use crate::midias::hash_de_conteudo;

/// Os escopos que uma conexão pode ter. O de publicar vem desmarcado na tela
/// de autorização: o assistente escreve, e uma pessoa publica.
pub const ESCOPOS: [(&str, &str); 7] = [
    ("sites:ler", "Ver os seus sites"),
    ("sites:criar", "Criar sites em seu nome"),
    (
        "sites:editar",
        "Mudar a identidade dos sites: nome, descrição, logo e organização",
    ),
    ("conteudo:ler", "Ler páginas, posts, autores e categorias"),
    (
        "conteudo:escrever",
        "Escrever rascunhos, enviar para revisão e cadastrar autores e categorias",
    ),
    ("conteudo:publicar", "Publicar e tirar do ar"),
    ("midia:escrever", "Ver e enviar imagens"),
];

pub const ESCOPO_DE_PUBLICAR: &str = "conteudo:publicar";

#[derive(Debug, thiserror::Error)]
pub enum ErroDeConector {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    /// Código, verificador, cliente ou token que não confere. Quem pede não
    /// fica sabendo qual.
    #[error("concessão inválida")]
    ConcessaoInvalida,
}

impl From<sqlx::Error> for ErroDeConector {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

/// Dois UUIDs aleatórios: 244 bits que ninguém adivinha.
fn segredo() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClienteMcp {
    pub id: Uuid,
    pub nome: String,
    pub redirect_uris: Vec<String>,
}

pub async fn registrar_cliente(
    pool: &PgPool,
    nome: &str,
    redirect_uris: &[String],
) -> Result<Uuid, ErroDeDados> {
    let id = sqlx::query_scalar!(
        "insert into cliente_mcp (nome, redirect_uris) values ($1, $2) returning id",
        nome,
        redirect_uris
    )
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn cliente_mcp(pool: &PgPool, id: Uuid) -> Result<Option<ClienteMcp>, ErroDeDados> {
    let cliente = sqlx::query_as!(
        ClienteMcp,
        "select id, nome, redirect_uris from cliente_mcp where id = $1",
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(cliente)
}

/// O pedido que a pessoa aprovou na tela de autorização.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autorizacao<'a> {
    pub cliente_id: Uuid,
    pub conta: &'a Conta,
    pub escopos: &'a [String],
    pub redirect_uri: &'a str,
    pub desafio: &'a str,
}

/// Grava o código de autorização e o devolve para ir no redirecionamento.
pub async fn criar_codigo(
    pool: &PgPool,
    autorizacao: &Autorizacao<'_>,
) -> Result<String, ErroDeDados> {
    let codigo = segredo();
    sqlx::query!(
        r#"
        insert into codigo_mcp
            (hash, cliente_id, conta_sub, conta_email, conta_nome, conta_equipe, escopos,
             redirect_uri, desafio, expira_em)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, now() + interval '5 minutes')
        "#,
        hash_de_conteudo(codigo.as_bytes()),
        autorizacao.cliente_id,
        autorizacao.conta.sub,
        autorizacao.conta.email,
        autorizacao.conta.nome,
        autorizacao.conta.equipe,
        autorizacao.escopos,
        autorizacao.redirect_uri,
        autorizacao.desafio
    )
    .execute(pool)
    .await?;
    Ok(codigo)
}

/// O que o cliente recebe ao trocar o código ou renovar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub acesso: String,
    pub renovacao: String,
    /// Em quantos segundos o token de acesso vence.
    pub expira_em_segundos: i64,
    pub escopos: Vec<String>,
}

const VALIDADE_DO_ACESSO_SEGUNDOS: i64 = 3600;

/// O desafio que corresponde a um verificador, no método S256.
fn desafio_de(verificador: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verificador.as_bytes()))
}

/// Troca o código pelos tokens. O código vale uma vez, para o mesmo cliente e
/// o mesmo endereço de retorno, e só com o verificador do PKCE.
pub async fn trocar_codigo(
    pool: &PgPool,
    codigo: &str,
    cliente_id: Uuid,
    redirect_uri: &str,
    verificador: &str,
) -> Result<Tokens, ErroDeConector> {
    let mut transacao = pool.begin().await?;
    let linha = sqlx::query!(
        r#"
        update codigo_mcp set usado_em = now()
        where hash = $1 and usado_em is null and expira_em > now()
        returning cliente_id, conta_sub, conta_email, conta_nome, conta_equipe, escopos,
                  redirect_uri, desafio
        "#,
        hash_de_conteudo(codigo.as_bytes())
    )
    .fetch_optional(&mut *transacao)
    .await?
    .ok_or(ErroDeConector::ConcessaoInvalida)?;
    // O código já ficou gasto: quem errou o verificador não tenta de novo.
    transacao.commit().await?;
    if linha.cliente_id != cliente_id
        || linha.redirect_uri != redirect_uri
        || linha.desafio != desafio_de(verificador)
    {
        return Err(ErroDeConector::ConcessaoInvalida);
    }

    let (acesso, renovacao) = (segredo(), segredo());
    sqlx::query!(
        r#"
        insert into conexao_mcp
            (cliente_id, conta_sub, conta_email, conta_nome, conta_equipe, escopos,
             acesso_hash, acesso_expira_em, renovacao_hash, renovacao_expira_em)
        values ($1, $2, $3, $4, $5, $6, $7, now() + make_interval(secs => $8), $9,
                now() + interval '30 days')
        "#,
        cliente_id,
        linha.conta_sub,
        linha.conta_email,
        linha.conta_nome,
        linha.conta_equipe,
        &linha.escopos,
        hash_de_conteudo(acesso.as_bytes()),
        VALIDADE_DO_ACESSO_SEGUNDOS as f64,
        hash_de_conteudo(renovacao.as_bytes())
    )
    .execute(pool)
    .await?;
    Ok(Tokens {
        acesso,
        renovacao,
        expira_em_segundos: VALIDADE_DO_ACESSO_SEGUNDOS,
        escopos: linha.escopos,
    })
}

/// Troca o token de renovação por um par novo. O antigo deixa de valer.
pub async fn renovar_tokens(
    pool: &PgPool,
    renovacao: &str,
    cliente_id: Uuid,
) -> Result<Tokens, ErroDeConector> {
    let (acesso, nova_renovacao) = (segredo(), segredo());
    let escopos = sqlx::query_scalar!(
        r#"
        update conexao_mcp
        set acesso_hash = $3, acesso_expira_em = now() + make_interval(secs => $4),
            renovacao_hash = $5, renovacao_expira_em = now() + interval '30 days'
        where renovacao_hash = $1 and cliente_id = $2 and revogada_em is null
          and renovacao_expira_em > now()
        returning escopos
        "#,
        hash_de_conteudo(renovacao.as_bytes()),
        cliente_id,
        hash_de_conteudo(acesso.as_bytes()),
        VALIDADE_DO_ACESSO_SEGUNDOS as f64,
        hash_de_conteudo(nova_renovacao.as_bytes())
    )
    .fetch_optional(pool)
    .await?
    .ok_or(ErroDeConector::ConcessaoInvalida)?;
    Ok(Tokens {
        acesso,
        renovacao: nova_renovacao,
        expira_em_segundos: VALIDADE_DO_ACESSO_SEGUNDOS,
        escopos,
    })
}

/// Uma conexão válida, achada pelo token de acesso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conexao {
    pub id: Uuid,
    pub conta: Conta,
    pub escopos: Vec<String>,
}

impl Conexao {
    pub fn tem(&self, escopo: &str) -> bool {
        self.escopos.iter().any(|e| e == escopo)
    }
}

pub async fn conexao_por_token(
    pool: &PgPool,
    acesso: &str,
) -> Result<Option<Conexao>, ErroDeDados> {
    let linha = sqlx::query!(
        r#"
        select id, conta_sub, conta_email, conta_nome, conta_equipe, escopos
        from conexao_mcp
        where acesso_hash = $1 and revogada_em is null and acesso_expira_em > now()
        "#,
        hash_de_conteudo(acesso.as_bytes())
    )
    .fetch_optional(pool)
    .await?;
    Ok(linha.map(|linha| Conexao {
        id: linha.id,
        conta: Conta {
            sub: linha.conta_sub,
            email: linha.conta_email,
            nome: linha.conta_nome,
            equipe: linha.conta_equipe,
        },
        escopos: linha.escopos,
    }))
}

/// Uma conexão na tela do conector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConexaoDaConta {
    pub id: Uuid,
    pub cliente: String,
    pub escopos: Vec<String>,
    pub criado_em: DateTime<Utc>,
    pub chamadas: i64,
}

/// As conexões em vigor de uma conta, da mais nova para a mais antiga.
pub async fn conexoes_da_conta(
    pool: &PgPool,
    conta: &str,
) -> Result<Vec<ConexaoDaConta>, ErroDeDados> {
    let conexoes = sqlx::query_as!(
        ConexaoDaConta,
        r#"
        select c.id, k.nome as cliente, c.escopos, c.criado_em,
               (select count(*) from chamada_mcp m where m.conexao_id = c.id) as "chamadas!"
        from conexao_mcp c
        join cliente_mcp k on k.id = c.cliente_id
        where c.conta_sub = $1 and c.revogada_em is null and c.renovacao_expira_em > now()
        order by c.criado_em desc
        "#,
        conta
    )
    .fetch_all(pool)
    .await?;
    Ok(conexoes)
}

/// Corta a conexão. Só a própria conta revoga as suas. Devolve se havia o
/// que revogar.
pub async fn revogar_conexao(
    pool: &PgPool,
    conta: &str,
    conexao_id: Uuid,
) -> Result<bool, ErroDeDados> {
    let revogadas = sqlx::query!(
        r#"
        update conexao_mcp set revogada_em = now()
        where id = $1 and conta_sub = $2 and revogada_em is null
        "#,
        conexao_id,
        conta
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(revogadas > 0)
}

/// Registra o que o assistente fez: ferramenta e identificadores.
pub async fn registrar_chamada(
    pool: &PgPool,
    conexao_id: Uuid,
    ferramenta: &str,
    site_id: Option<Uuid>,
    documento_id: Option<Uuid>,
    deu_certo: bool,
) -> Result<(), ErroDeDados> {
    sqlx::query!(
        r#"
        insert into chamada_mcp (conexao_id, ferramenta, site_id, documento_id, deu_certo)
        values ($1, $2, $3, $4, $5)
        "#,
        conexao_id,
        ferramenta,
        site_id,
        documento_id,
        deu_certo
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn desafio_do_pkce_bate_com_o_exemplo_da_rfc_7636() {
        assert_eq!(
            desafio_de("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
