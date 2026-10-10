//! Domínio próprio de um site: o pedido do Dono, a conferência do DNS e a
//! resposta que o Caddy pede antes de emitir certificado.

use std::net::IpAddr;

use cms_dominio::conta::pode_administrar;
use cms_dominio::eventos::{DominioAtivado, Evento};
use cms_dominio::{Ator, ler_host};
use sqlx::PgPool;
use uuid::Uuid;

use crate::ErroDeDados;
use crate::eventos::emitir;

/// As mensagens são para quem está na tela.
#[derive(Debug, thiserror::Error)]
pub enum ErroDeDominio {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("Só o dono do site mexe no endereço.")]
    SemPermissao,
    #[error("Informe só o domínio, como padaria.com.br, sem http e sem barra.")]
    Invalido,
    #[error("Este domínio é da própria plataforma. Informe o seu.")]
    Reservado,
    #[error("Este domínio já está em uso por outro site.")]
    EmUso,
    #[error("Publique a página inicial antes de apontar um domínio para o site.")]
    SemPaginaInicial,
}

impl From<sqlx::Error> for ErroDeDominio {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

/// O domínio como o Dono digitou, já conferido: sem `www.`, sem porta, e
/// fora dos endereços da plataforma.
fn normalizar(digitado: &str, reservados: &[&str]) -> Result<String, ErroDeDominio> {
    let host = ler_host(digitado).ok_or(ErroDeDominio::Invalido)?;
    let nome = host.nome.strip_prefix("www.").unwrap_or(&host.nome);
    if host.com_porta != host.nome || !nome.contains('.') || nome.parse::<IpAddr>().is_ok() {
        return Err(ErroDeDominio::Invalido);
    }
    let da_plataforma = reservados
        .iter()
        .any(|reservado| nome == *reservado || nome.ends_with(&format!(".{reservado}")));
    if da_plataforma {
        return Err(ErroDeDominio::Reservado);
    }
    Ok(nome.to_string())
}

/// Registra o pedido de domínio próprio. Ele fica pendente até a rotina
/// conferir que o nome aponta para o servidor.
///
/// `reservados` são os endereços da plataforma: o domínio-base dos sites e o
/// host do painel.
pub async fn pedir_dominio(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    digitado: &str,
    reservados: &[&str],
) -> Result<String, ErroDeDominio> {
    if !pode_administrar(ator) {
        return Err(ErroDeDominio::SemPermissao);
    }
    let host = normalizar(digitado, reservados)?;
    let tem_home = sqlx::query_scalar!(
        r#"
        select exists(
            select 1 from documento
            where site_id = $1 and caminho = '/' and situacao = 'publicado'
        ) as "existe!"
        "#,
        site_id
    )
    .fetch_one(pool)
    .await?;
    if !tem_home {
        return Err(ErroDeDominio::SemPaginaInicial);
    }
    // Pedir de novo o mesmo domínio do mesmo site não muda nada; o de outro
    // site não é tomado.
    let gravado = sqlx::query!(
        r#"
        insert into dominio (host, site_id, pedido_por) values ($1, $2, $3)
        on conflict (host) do update set pedido_por = excluded.pedido_por
            where dominio.site_id = excluded.site_id
        "#,
        host,
        site_id,
        ator.conta
    )
    .execute(pool)
    .await?;
    if gravado.rows_affected() == 0 {
        return Err(ErroDeDominio::EmUso);
    }
    Ok(host)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DominioDoSite {
    pub host: String,
    pub ativo: bool,
}

pub async fn dominios_do_site(
    pool: &PgPool,
    site_id: Uuid,
) -> Result<Vec<DominioDoSite>, ErroDeDados> {
    let dominios = sqlx::query_as!(
        DominioDoSite,
        r#"
        select host, (situacao = 'ativo') as "ativo!" from dominio
        where site_id = $1 order by pedido_em
        "#,
        site_id
    )
    .fetch_all(pool)
    .await?;
    Ok(dominios)
}

/// Tira um domínio do site. Devolve se havia o que tirar.
pub async fn remover_dominio(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    host: &str,
) -> Result<bool, ErroDeDominio> {
    if !pode_administrar(ator) {
        return Err(ErroDeDominio::SemPermissao);
    }
    let removidos = sqlx::query!(
        "delete from dominio where host = $1 and site_id = $2",
        host,
        site_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(removidos > 0)
}

/// O Dono assume o endereço provisório como definitivo, ou volta atrás. Sem
/// isso, e sem domínio próprio, o site fica fora da busca.
pub async fn definir_provisorio_definitivo(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    definitivo: bool,
) -> Result<(), ErroDeDominio> {
    if !pode_administrar(ator) {
        return Err(ErroDeDominio::SemPermissao);
    }
    sqlx::query!(
        "update site set provisorio_definitivo = $2, atualizado_em = now() where id = $1",
        site_id,
        definitivo
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Um domínio à espera de conferência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DominioPendente {
    pub host: String,
    pub site_id: Uuid,
}

pub async fn dominios_pendentes(pool: &PgPool) -> Result<Vec<DominioPendente>, ErroDeDados> {
    let pendentes = sqlx::query_as!(
        DominioPendente,
        r#"
        select d.host, d.site_id from dominio d
        join site s on s.id = d.site_id
        where d.situacao = 'pendente' and s.situacao <> 'suspenso'
        order by d.pedido_em
        "#
    )
    .fetch_all(pool)
    .await?;
    Ok(pendentes)
}

/// O DNS confere: o domínio passa a ser o endereço do site, e o fato vira
/// evento. O domínio ativo anterior do mesmo site volta a pendente.
pub async fn confirmar_dominio(
    pool: &PgPool,
    pendente: &DominioPendente,
) -> Result<(), ErroDeDados> {
    let mut transacao = pool.begin().await?;
    sqlx::query!(
        "update dominio set situacao = 'pendente' where site_id = $1 and situacao = 'ativo'",
        pendente.site_id
    )
    .execute(&mut *transacao)
    .await?;
    let ativados = sqlx::query!(
        r#"
        update dominio set situacao = 'ativo', conferido_em = now()
        where host = $1 and site_id = $2
        "#,
        pendente.host,
        pendente.site_id
    )
    .execute(&mut *transacao)
    .await?
    .rows_affected();
    if ativados > 0 {
        let evento = Evento::DominioAtivado(DominioAtivado {
            dominio: pendente.host.clone(),
        });
        emitir(
            &mut transacao,
            pendente.site_id,
            &format!("dominio.ativado:{}:{}", pendente.host, Uuid::new_v4()),
            &evento,
        )
        .await?;
    }
    transacao.commit().await?;
    Ok(())
}

/// A pergunta do Caddy antes de emitir certificado sob demanda: este host é
/// de um site que está no ar?
///
/// Vale o domínio próprio, ativo ou pendente (o certificado precisa existir
/// para a conferência seguinte dar certo no navegador), com ou sem `www.`, e
/// o endereço provisório de site que existe. Site suspenso não ganha
/// certificado.
pub async fn dominio_permitido(
    pool: &PgPool,
    host: &str,
    dominio_base: &str,
) -> Result<bool, ErroDeDados> {
    let Some(host) = ler_host(host) else {
        return Ok(false);
    };
    let sem_www = host.nome.strip_prefix("www.").unwrap_or(&host.nome);
    let slug = host
        .nome
        .strip_suffix(dominio_base)
        .and_then(|resto| resto.strip_suffix('.'))
        .filter(|slug| !slug.is_empty() && !slug.contains('.'));
    let permitido = sqlx::query_scalar!(
        r#"
        select exists(
            select 1 from dominio d join site s on s.id = d.site_id
            where d.host = $1 and s.situacao <> 'suspenso'
        ) or exists(
            select 1 from site where slug = $2 and situacao <> 'suspenso'
        ) as "permitido!"
        "#,
        sem_www,
        slug
    )
    .fetch_one(pool)
    .await?;
    Ok(permitido)
}

#[cfg(test)]
mod testes {
    use super::*;

    const RESERVADOS: [&str; 2] = ["sites.exemplo.example", "cms.exemplo.example"];

    #[test]
    fn dominio_digitado_e_normalizado() {
        for (digitado, esperado) in [
            ("Padaria.com.br", "padaria.com.br"),
            (" www.padaria.com.br ", "padaria.com.br"),
            ("loja.padaria.com.br.", "loja.padaria.com.br"),
        ] {
            assert_eq!(normalizar(digitado, &RESERVADOS).expect(digitado), esperado);
        }
    }

    #[test]
    fn o_que_nao_e_dominio_de_cliente_e_recusado() {
        for invalido in [
            "",
            "localhost",
            "https://padaria.com.br",
            "padaria.com.br/contato",
            "padaria.com.br:8080",
            "203.0.113.7",
            "a b.com",
        ] {
            assert!(
                matches!(
                    normalizar(invalido, &RESERVADOS),
                    Err(ErroDeDominio::Invalido)
                ),
                "{invalido}"
            );
        }
        for reservado in [
            "sites.exemplo.example",
            "golpe.sites.exemplo.example",
            "cms.exemplo.example",
            "www.cms.exemplo.example",
        ] {
            assert!(
                matches!(
                    normalizar(reservado, &RESERVADOS),
                    Err(ErroDeDominio::Reservado)
                ),
                "{reservado}"
            );
        }
    }
}
