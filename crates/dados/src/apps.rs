use cms_dominio::Conta;
use cms_dominio::app_registry::{AppAction, AppRole};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{ErroDeDados, app_registry::papel_lido};

const NOME_MAXIMO: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDaConta {
    pub id: Uuid,
    pub slug: String,
    pub nome: String,
    pub bundle_id: String,
    pub revisao_do_rascunho: i64,
    pub papel: AppRole,
}

#[derive(Debug, thiserror::Error)]
pub enum ErroDeApp {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    #[error("Informe um slug válido com até 64 caracteres.")]
    SlugInvalido,
    #[error("Informe um nome com até 80 caracteres.")]
    NomeInvalido,
    #[error("Informe um bundle ID válido.")]
    BundleIdInvalido,
    #[error("Já existe um aplicativo com esse slug ou bundle ID.")]
    EmUso,
    #[error("Aplicativo não encontrado.")]
    NaoEncontrado,
    #[error("A revisão do rascunho mudou. Recarregue antes de salvar.")]
    ConflitoDeRevisao,
    #[error("O aplicativo ainda não tem rascunho.")]
    SemRascunho,
    #[error("A chave de idempotência é obrigatória e deve ter até 128 caracteres.")]
    ChaveIdempotenteInvalida,
    #[error("Versão não encontrada.")]
    VersaoInexistente,
    #[error("Ambiente inválido.")]
    AmbienteInvalido,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RascunhoDoApp {
    pub slug: String,
    pub definicao: Value,
    pub revisao: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersaoDoApp {
    pub id: Uuid,
    pub numero: i32,
    pub criada: bool,
}

async fn papel_no_app(
    transacao: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    conta: &Conta,
) -> Result<Option<AppRole>, ErroDeApp> {
    if conta.equipe {
        let existe: bool = sqlx::query_scalar("select exists(select 1 from app where id = $1)")
            .bind(app_id)
            .fetch_one(&mut **transacao)
            .await?;
        return Ok(existe.then_some(AppRole::Owner));
    }
    let papel: Option<String> =
        sqlx::query_scalar("select papel from app_participacao where app_id = $1 and conta = $2")
            .bind(app_id)
            .bind(&conta.sub)
            .fetch_optional(&mut **transacao)
            .await?;
    papel
        .map(|papel| papel_lido(&papel).map_err(ErroDeApp::Dados))
        .transpose()
}

async fn exigir(
    transacao: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    conta: &Conta,
    acao: AppAction,
) -> Result<(), ErroDeApp> {
    let permitido = papel_no_app(transacao, app_id, conta)
        .await?
        .is_some_and(|papel| papel.allows(acao));
    if permitido {
        Ok(())
    } else {
        Err(ErroDeApp::NaoEncontrado)
    }
}

async fn auditar(
    transacao: &mut Transaction<'_, Postgres>,
    app_id: Uuid,
    conta: &Conta,
    acao: &str,
    dados: &Value,
    correlation_id: Uuid,
) -> Result<(), ErroDeApp> {
    sqlx::query(
        "insert into app_auditoria (app_id, conta, acao, dados, correlation_id) values ($1, $2, $3, $4, $5)",
    )
    .bind(app_id)
    .bind(&conta.sub)
    .bind(acao)
    .bind(dados)
    .bind(correlation_id)
    .execute(&mut **transacao)
    .await?;
    Ok(())
}

impl From<sqlx::Error> for ErroDeApp {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

fn slug_valido(slug: &str) -> bool {
    (1..=64).contains(&slug.len())
        && slug.as_bytes()[0].is_ascii_lowercase()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !slug.ends_with('-')
}

fn bundle_id_valido(bundle_id: &str) -> bool {
    bundle_id.contains('.')
        && bundle_id.split('.').all(|parte| {
            !parte.is_empty()
                && parte
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

pub async fn criar_app(
    pool: &PgPool,
    conta: &Conta,
    slug: &str,
    nome: &str,
    bundle_id: &str,
    correlation_id: Uuid,
) -> Result<AppDaConta, ErroDeApp> {
    if !slug_valido(slug) {
        return Err(ErroDeApp::SlugInvalido);
    }
    let nome = nome.trim();
    if nome.is_empty() || nome.chars().count() > NOME_MAXIMO {
        return Err(ErroDeApp::NomeInvalido);
    }
    if !bundle_id_valido(bundle_id) {
        return Err(ErroDeApp::BundleIdInvalido);
    }

    let mut transacao = pool.begin().await?;
    let criado = sqlx::query_as::<_, (Uuid, String, String, String, i64)>(
        r#"
        insert into app (slug, nome, bundle_id, criado_por)
        values ($1, $2, $3, $4)
        returning id, slug, nome, bundle_id, revisao_do_rascunho
        "#,
    )
    .bind(slug)
    .bind(nome)
    .bind(bundle_id)
    .bind(&conta.sub)
    .fetch_one(&mut *transacao)
    .await;
    let (id, slug, nome, bundle_id, revisao_do_rascunho) = match criado {
        Ok(app) => app,
        Err(sqlx::Error::Database(erro)) if erro.is_unique_violation() => {
            return Err(ErroDeApp::EmUso);
        }
        Err(erro) => return Err(erro.into()),
    };
    sqlx::query("insert into app_participacao (app_id, conta, papel) values ($1, $2, 'owner')")
        .bind(id)
        .bind(&conta.sub)
        .execute(&mut *transacao)
        .await?;
    auditar(
        &mut transacao,
        id,
        conta,
        "app.created",
        &json!({ "slug": slug, "bundleId": bundle_id }),
        correlation_id,
    )
    .await?;
    transacao.commit().await?;

    Ok(AppDaConta {
        id,
        slug,
        nome,
        bundle_id,
        revisao_do_rascunho,
        papel: AppRole::Owner,
    })
}

pub async fn apps_da_conta(pool: &PgPool, conta: &str) -> Result<Vec<AppDaConta>, ErroDeDados> {
    let linhas = sqlx::query_as::<_, (Uuid, String, String, String, i64, String)>(
        r#"
        select a.id, a.slug, a.nome, a.bundle_id, a.revisao_do_rascunho, p.papel
        from app a
        join app_participacao p on p.app_id = a.id
        where p.conta = $1
        order by a.nome, a.id
        "#,
    )
    .bind(conta)
    .fetch_all(pool)
    .await?;
    linhas
        .into_iter()
        .map(|(id, slug, nome, bundle_id, revisao_do_rascunho, papel)| {
            Ok(AppDaConta {
                id,
                slug,
                nome,
                bundle_id,
                revisao_do_rascunho,
                papel: papel_lido(&papel)?,
            })
        })
        .collect()
}

pub async fn salvar_rascunho_do_app(
    pool: &PgPool,
    conta: &Conta,
    app_id: Uuid,
    revisao_esperada: i64,
    definicao: &Value,
    correlation_id: Uuid,
) -> Result<i64, ErroDeApp> {
    let mut transacao = pool.begin().await?;
    exigir(&mut transacao, app_id, conta, AppAction::EditDraft).await?;
    let nova_revisao: Option<i64> = sqlx::query_scalar(
        r#"
        update app
        set rascunho = $1,
            revisao_do_rascunho = revisao_do_rascunho + 1,
            rascunho_atualizado_por = $2,
            atualizado_em = now()
        where id = $3 and revisao_do_rascunho = $4
        returning revisao_do_rascunho
        "#,
    )
    .bind(definicao)
    .bind(&conta.sub)
    .bind(app_id)
    .bind(revisao_esperada)
    .fetch_optional(&mut *transacao)
    .await?;
    let Some(nova_revisao) = nova_revisao else {
        return Err(ErroDeApp::ConflitoDeRevisao);
    };
    auditar(
        &mut transacao,
        app_id,
        conta,
        "draft.saved",
        &serde_json::json!({ "revision": nova_revisao }),
        correlation_id,
    )
    .await?;
    transacao.commit().await?;
    Ok(nova_revisao)
}

pub async fn rascunho_do_app(
    pool: &PgPool,
    conta: &Conta,
    app_id: Uuid,
) -> Result<RascunhoDoApp, ErroDeApp> {
    let mut transacao = pool.begin().await?;
    exigir(&mut transacao, app_id, conta, AppAction::Read).await?;
    let linha: Option<(String, Option<Value>, i64)> =
        sqlx::query_as("select slug, rascunho, revisao_do_rascunho from app where id = $1")
            .bind(app_id)
            .fetch_optional(&mut *transacao)
            .await?;
    let Some((slug, Some(definicao), revisao)) = linha else {
        return Err(ErroDeApp::SemRascunho);
    };
    Ok(RascunhoDoApp {
        slug,
        definicao,
        revisao,
    })
}

pub async fn auditar_validacao(
    pool: &PgPool,
    conta: &Conta,
    app_id: Uuid,
    correlation_id: Uuid,
) -> Result<(), ErroDeApp> {
    let mut transacao = pool.begin().await?;
    exigir(&mut transacao, app_id, conta, AppAction::Read).await?;
    auditar(
        &mut transacao,
        app_id,
        conta,
        "draft.validated",
        &serde_json::json!({}),
        correlation_id,
    )
    .await?;
    transacao.commit().await?;
    Ok(())
}

pub async fn criar_versao_do_app(
    pool: &PgPool,
    conta: &Conta,
    app_id: Uuid,
    chave_idempotencia: &str,
    correlation_id: Uuid,
) -> Result<VersaoDoApp, ErroDeApp> {
    if chave_idempotencia.is_empty() || chave_idempotencia.len() > 128 {
        return Err(ErroDeApp::ChaveIdempotenteInvalida);
    }
    let mut transacao = pool.begin().await?;
    exigir(&mut transacao, app_id, conta, AppAction::CreateVersion).await?;
    sqlx::query("select pg_advisory_xact_lock(hashtext($1))")
        .bind(app_id.to_string())
        .execute(&mut *transacao)
        .await?;
    if let Some((id, numero)) = sqlx::query_as::<_, (Uuid, i32)>(
        "select id, numero from app_versao where app_id = $1 and chave_idempotencia = $2",
    )
    .bind(app_id)
    .bind(chave_idempotencia)
    .fetch_optional(&mut *transacao)
    .await?
    {
        return Ok(VersaoDoApp {
            id,
            numero,
            criada: false,
        });
    }
    let rascunho: Option<Value> =
        sqlx::query_scalar("select rascunho from app where id = $1 for update")
            .bind(app_id)
            .fetch_one(&mut *transacao)
            .await?;
    let Some(rascunho) = rascunho else {
        return Err(ErroDeApp::SemRascunho);
    };
    let numero: i32 = sqlx::query_scalar(
        "select coalesce(max(numero), 0)::int + 1 from app_versao where app_id = $1",
    )
    .bind(app_id)
    .fetch_one(&mut *transacao)
    .await?;
    let id: Uuid = sqlx::query_scalar(
        r#"
        insert into app_versao (
            app_id, numero, schema_version, minimum_runtime_version,
            definicao, criado_por, chave_idempotencia
        ) values ($1, $2, $3, $4, $5, $6, $7)
        returning id
        "#,
    )
    .bind(app_id)
    .bind(numero)
    .bind(
        rascunho
            .get("schemaVersion")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )
    .bind(
        rascunho
            .get("minimumRuntimeVersion")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )
    .bind(&rascunho)
    .bind(&conta.sub)
    .bind(chave_idempotencia)
    .fetch_one(&mut *transacao)
    .await?;
    auditar(
        &mut transacao,
        app_id,
        conta,
        "version.created",
        &serde_json::json!({ "number": numero }),
        correlation_id,
    )
    .await?;
    transacao.commit().await?;
    Ok(VersaoDoApp {
        id,
        numero,
        criada: true,
    })
}

pub async fn publicar_versao_no_ambiente(
    pool: &PgPool,
    conta: &Conta,
    app_id: Uuid,
    ambiente: &str,
    numero: i32,
    correlation_id: Uuid,
) -> Result<Uuid, ErroDeApp> {
    if !matches!(ambiente, "development" | "staging" | "production") {
        return Err(ErroDeApp::AmbienteInvalido);
    }
    let mut transacao = pool.begin().await?;
    exigir(&mut transacao, app_id, conta, AppAction::Publish).await?;
    let versao_id: Option<Uuid> =
        sqlx::query_scalar("select id from app_versao where app_id = $1 and numero = $2")
            .bind(app_id)
            .bind(numero)
            .fetch_optional(&mut *transacao)
            .await?;
    let Some(versao_id) = versao_id else {
        return Err(ErroDeApp::VersaoInexistente);
    };
    sqlx::query(
        r#"
        insert into app_ambiente (app_id, nome, versao_id, atualizado_por)
        values ($1, $2, $3, $4)
        on conflict (app_id, nome) do update
        set versao_id = excluded.versao_id,
            atualizado_por = excluded.atualizado_por,
            atualizado_em = now()
        "#,
    )
    .bind(app_id)
    .bind(ambiente)
    .bind(versao_id)
    .bind(&conta.sub)
    .execute(&mut *transacao)
    .await?;
    auditar(
        &mut transacao,
        app_id,
        conta,
        "environment.published",
        &serde_json::json!({ "environment": ambiente, "version": numero }),
        correlation_id,
    )
    .await?;
    transacao.commit().await?;
    Ok(versao_id)
}
