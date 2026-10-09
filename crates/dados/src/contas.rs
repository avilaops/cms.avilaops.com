//! Criação de site, participação por conta e convites.

use chrono::{DateTime, Utc};
use cms_dominio::conta::{SlugInvalido, pode_administrar, validar_slug_de_site};
use cms_dominio::eventos::{ConviteCriado as EventoDeConvite, Evento, SiteCriado};
use cms_dominio::{Ator, Conta, LimitesDeCriacao, Papel, PerfilDoSite, Situacao};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ErroDeDados;
use crate::eventos::emitir;

const NOME_MAXIMO: usize = 80;
const EMAIL_MAXIMO: usize = 254;

/// As mensagens são para quem está na tela: dizem o que fazer.
#[derive(Debug, thiserror::Error)]
pub enum ErroDeConta {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("{}", .0.mensagem())]
    Slug(SlugInvalido),
    #[error("Já existe um site com este endereço. Escolha outro.")]
    SlugEmUso,
    #[error("Dê um nome ao site, com até 80 caracteres.")]
    NomeInvalido,
    #[error("Sua conta já tem o número máximo de sites. Fale com a Ávila Ops para liberar mais.")]
    LimiteDeSites,
    #[error("Você já criou sites demais hoje. Tente de novo amanhã.")]
    LimiteDiario,
    #[error("Só o dono do site pode fazer isto.")]
    SemPermissao,
    #[error("Informe um e-mail válido.")]
    EmailInvalido,
    #[error("Este convite não existe.")]
    ConviteInexistente,
    #[error("Este convite venceu. Peça um novo ao dono do site.")]
    ConviteExpirado,
    #[error("Este convite já foi usado.")]
    ConviteUsado,
    #[error("Este convite foi enviado para outro e-mail. Entre com a conta do e-mail convidado.")]
    ConviteDeOutroEmail,
}

impl From<sqlx::Error> for ErroDeConta {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

fn papel_lido(texto: &str) -> Result<Papel, ErroDeDados> {
    Papel::try_from(texto).map_err(|erro| ErroDeDados::DadoInvalido(erro.to_string()))
}

/// Cria o site da conta, que passa a ser o Dono. O site nasce em montagem:
/// responde no endereço provisório, fora da busca.
///
/// A criação é aberta, então os limites valem aqui, na mesma transação. A
/// equipe da Ávila Ops não tem limite.
pub async fn criar_site_para(
    pool: &PgPool,
    conta: &Conta,
    slug: &str,
    nome: &str,
    limites: LimitesDeCriacao,
) -> Result<Uuid, ErroDeConta> {
    validar_slug_de_site(slug).map_err(ErroDeConta::Slug)?;
    let nome = nome.trim();
    if nome.is_empty() || nome.chars().count() > NOME_MAXIMO {
        return Err(ErroDeConta::NomeInvalido);
    }

    let mut transacao = pool.begin().await?;
    // Duas criações da mesma conta andam uma de cada vez: sem isso, pedidos
    // simultâneos passariam juntos pela contagem.
    sqlx::query("select pg_advisory_xact_lock(hashtext($1))")
        .bind(&conta.sub)
        .execute(&mut *transacao)
        .await?;

    if !conta.equipe {
        let como_dono = sqlx::query_scalar!(
            r#"select count(*) as "total!" from participacao where conta = $1 and papel = 'dono'"#,
            conta.sub
        )
        .fetch_one(&mut *transacao)
        .await?;
        if como_dono >= limites.sites_por_conta {
            return Err(ErroDeConta::LimiteDeSites);
        }
        let criados_hoje = sqlx::query_scalar!(
            r#"
            select count(*) as "total!" from site
            where criado_por = $1 and criado_em > now() - interval '24 hours'
            "#,
            conta.sub
        )
        .fetch_one(&mut *transacao)
        .await?;
        if criados_hoje >= limites.criacoes_por_dia {
            return Err(ErroDeConta::LimiteDiario);
        }
    }

    let criado = sqlx::query_scalar!(
        "insert into site (slug, perfil, criado_por) values ($1, $2, $3) returning id",
        slug,
        Json(PerfilDoSite::inicial(nome)) as _,
        conta.sub
    )
    .fetch_one(&mut *transacao)
    .await;
    let site_id = match criado {
        Ok(id) => id,
        Err(sqlx::Error::Database(erro)) if erro.is_unique_violation() => {
            return Err(ErroDeConta::SlugEmUso);
        }
        Err(erro) => return Err(erro.into()),
    };

    sqlx::query!(
        "insert into participacao (site_id, conta, email, papel) values ($1, $2, $3, 'dono')",
        site_id,
        conta.sub,
        conta.email
    )
    .execute(&mut *transacao)
    .await?;
    let evento = Evento::SiteCriado(SiteCriado {
        criado_por: conta.email.clone(),
    });
    emitir(
        &mut transacao,
        site_id,
        &format!("site.criado:{site_id}"),
        &evento,
    )
    .await?;
    transacao.commit().await?;
    Ok(site_id)
}

/// O papel da conta no site, se ela participa dele.
pub async fn papel_no_site(
    pool: &PgPool,
    site_id: Uuid,
    conta: &str,
) -> Result<Option<Papel>, ErroDeDados> {
    let papel = sqlx::query_scalar!(
        "select papel from participacao where site_id = $1 and conta = $2",
        site_id,
        conta
    )
    .fetch_optional(pool)
    .await?;
    papel.as_deref().map(papel_lido).transpose()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteDaConta {
    pub id: Uuid,
    pub slug: String,
    pub nome: String,
    pub situacao: Situacao,
    pub papel: Papel,
}

/// Os sites de que a conta participa, em ordem de nome.
pub async fn sites_da_conta(pool: &PgPool, conta: &str) -> Result<Vec<SiteDaConta>, ErroDeDados> {
    let linhas = sqlx::query!(
        r#"
        select s.id, s.slug, coalesce(s.perfil ->> 'nome', '') as "nome!", s.situacao, p.papel
        from participacao p
        join site s on s.id = p.site_id
        where p.conta = $1
        order by 3, s.slug
        "#,
        conta
    )
    .fetch_all(pool)
    .await?;
    linhas
        .into_iter()
        .map(|linha| {
            Ok(SiteDaConta {
                id: linha.id,
                slug: linha.slug,
                nome: linha.nome,
                situacao: Situacao::try_from(linha.situacao.as_str())
                    .map_err(|erro| ErroDeDados::DadoInvalido(erro.to_string()))?,
                papel: papel_lido(&linha.papel)?,
            })
        })
        .collect()
}

fn hash_do_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn email_normalizado(email: &str) -> Result<String, ErroDeConta> {
    let email = email.trim().to_lowercase();
    let valido = email.len() <= EMAIL_MAXIMO
        && !email.chars().any(char::is_whitespace)
        && email
            .split_once('@')
            .is_some_and(|(nome, dominio)| !nome.is_empty() && dominio.contains('.'));
    if valido {
        Ok(email)
    } else {
        Err(ErroDeConta::EmailInvalido)
    }
}

/// O convite recém-criado. O token só existe aqui e no link: o banco guarda o
/// hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConviteCriado {
    pub id: Uuid,
    pub link: String,
    pub expira_em: DateTime<Utc>,
}

/// Convida um e-mail para o site. Vale 48 horas e uma vez só.
///
/// O link volta para o Dono copiar: o convite funciona sem e-mail. O evento
/// `convite.criado` leva o mesmo link para o n8n mandar a mensagem.
pub async fn criar_convite(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    email: &str,
    papel: Papel,
    origem_do_painel: &str,
) -> Result<ConviteCriado, ErroDeConta> {
    if !pode_administrar(ator) {
        return Err(ErroDeConta::SemPermissao);
    }
    let email = email_normalizado(email)?;
    // Dois UUIDs aleatórios: 244 bits que ninguém adivinha.
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let link = format!("{origem_do_painel}/convite/{token}");

    let mut transacao = pool.begin().await?;
    let convite = sqlx::query!(
        r#"
        insert into convite (site_id, email, papel, token_hash, expira_em, criado_por)
        values ($1, $2, $3, $4, now() + interval '48 hours', $5)
        returning id, expira_em
        "#,
        site_id,
        email,
        papel.como_texto(),
        hash_do_token(&token),
        ator.conta
    )
    .fetch_one(&mut *transacao)
    .await?;
    let evento = Evento::ConviteCriado(EventoDeConvite {
        email,
        papel: papel.rotulo().to_string(),
        link: link.clone(),
        expira_em: convite.expira_em,
    });
    emitir(
        &mut transacao,
        site_id,
        &format!("convite.criado:{}", convite.id),
        &evento,
    )
    .await?;
    transacao.commit().await?;
    Ok(ConviteCriado {
        id: convite.id,
        link,
        expira_em: convite.expira_em,
    })
}

/// Aceita o convite em nome da conta que entrou pelo Auth. Devolve o site.
///
/// Só a conta do e-mail convidado aceita: link repassado não abre o site
/// para outra pessoa. Quem já participa mantém o papel que tem.
pub async fn aceitar_convite(
    pool: &PgPool,
    token: &str,
    conta: &Conta,
) -> Result<Uuid, ErroDeConta> {
    let mut transacao = pool.begin().await?;
    let convite = sqlx::query!(
        r#"
        select id, site_id, email, papel, aceito_em, (expira_em < now()) as "expirado!"
        from convite
        where token_hash = $1
        for update
        "#,
        hash_do_token(token)
    )
    .fetch_optional(&mut *transacao)
    .await?
    .ok_or(ErroDeConta::ConviteInexistente)?;
    if convite.aceito_em.is_some() {
        return Err(ErroDeConta::ConviteUsado);
    }
    if convite.expirado {
        return Err(ErroDeConta::ConviteExpirado);
    }
    if convite.email != conta.email {
        return Err(ErroDeConta::ConviteDeOutroEmail);
    }

    sqlx::query!(
        r#"
        insert into participacao (site_id, conta, email, papel)
        values ($1, $2, $3, $4)
        on conflict (site_id, conta) do nothing
        "#,
        convite.site_id,
        conta.sub,
        conta.email,
        convite.papel
    )
    .execute(&mut *transacao)
    .await?;
    sqlx::query!(
        "update convite set aceito_em = now() where id = $1",
        convite.id
    )
    .execute(&mut *transacao)
    .await?;
    transacao.commit().await?;
    Ok(convite.site_id)
}
