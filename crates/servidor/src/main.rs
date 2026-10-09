//! O binário do CMS.
//!
//! - `servidor servir`: atende os sites.
//! - `servidor migrar`: aplica as migrações pendentes.
//! - `servidor migrar --conferir`: sai com 0 se não há migração pendente.
//! - `servidor semear-demonstracao`: recria o site de demonstração.

mod configuracao;
mod eventos;
mod imagens;
mod migracao;
mod semente;

use std::process::ExitCode;
use std::sync::Arc;

use cms_integracoes::ClienteN8n;
use cms_integracoes::auth::ClienteAuth;
use cms_web::Estado;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::configuracao::Ambiente;

#[derive(Debug, thiserror::Error)]
enum Erro {
    #[error("{0}")]
    Uso(String),
    #[error("configuração inválida: {0}")]
    Configuracao(String),
    #[error("falha no banco: {0}")]
    Banco(#[from] sqlx::Error),
    #[error("falha ao migrar: {0}")]
    Migracao(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Dados(#[from] cms_dados::ErroDeDados),
    #[error("falha de entrada e saída: {0}")]
    Arquivo(#[from] std::io::Error),
    #[error("falha ao semear: {0}")]
    Semente(String),
}

const USO: &str = "uso: servidor <servir | migrar [--conferir] | semear-demonstracao>";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().json().with_target(false).init();
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    match executar(&argumentos).await {
        Ok(codigo) => codigo,
        Err(erro) => {
            tracing::error!(%erro, "o comando falhou");
            ExitCode::FAILURE
        }
    }
}

async fn executar(argumentos: &[String]) -> Result<ExitCode, Erro> {
    let argumentos: Vec<&str> = argumentos.iter().map(String::as_str).collect();
    let ambiente = Ambiente::ler()?;
    match argumentos.as_slice() {
        ["servir"] => {
            servir(&ambiente).await?;
            Ok(ExitCode::SUCCESS)
        }
        ["migrar"] => {
            let pool = conectar(&ambiente).await?;
            cms_dados::MIGRADOR.run(&pool).await?;
            tracing::info!("migrações aplicadas");
            Ok(ExitCode::SUCCESS)
        }
        ["migrar", "--conferir"] => {
            let pool = conectar(&ambiente).await?;
            let pendentes = migracao::pendentes(&pool).await?;
            if pendentes.is_empty() {
                tracing::info!("nenhuma migração pendente");
                Ok(ExitCode::SUCCESS)
            } else {
                tracing::warn!(?pendentes, "há migração pendente");
                Ok(ExitCode::FAILURE)
            }
        }
        ["semear-demonstracao"] => {
            let pool = conectar(&ambiente).await?;
            let resumo =
                semente::semear_demonstracao(&pool, &ambiente.web.diretorio_de_midia).await?;
            tracing::info!(
                site = %resumo.slug,
                documentos = resumo.documentos,
                arquivos = resumo.arquivos,
                "site de demonstração recriado"
            );
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(Erro::Uso(USO.to_string())),
    }
}

async fn conectar(ambiente: &Ambiente) -> Result<PgPool, Erro> {
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .connect(&ambiente.banco)
        .await?)
}

async fn servir(ambiente: &Ambiente) -> Result<(), Erro> {
    let pool = conectar(ambiente).await?;
    imagens::agendar(pool.clone(), ambiente.web.diretorio_de_midia.clone());
    match &ambiente.n8n {
        Some(saida) => {
            let cliente = ClienteN8n::novo(&saida.url, saida.autorizacao.expor())
                .map_err(|erro| Erro::Configuracao(erro.to_string()))?;
            eventos::agendar(pool.clone(), cliente, ambiente.web.clone());
        }
        None => tracing::warn!("n8n não configurado: os eventos ficam na fila"),
    }
    let auth = match &ambiente.auth {
        Some(login) => Some(
            ClienteAuth::novo(&login.url, &login.app)
                .map_err(|erro| Erro::Configuracao(erro.to_string()))?,
        ),
        None => {
            tracing::warn!("painel não configurado: a aplicação só serve sites");
            None
        }
    };
    let estado = Estado {
        auth,
        pool,
        configuracao: Arc::new(ambiente.web.clone()),
    };
    let escuta = tokio::net::TcpListener::bind(("0.0.0.0", ambiente.porta)).await?;
    tracing::info!(porta = ambiente.porta, dominio_base = %ambiente.web.dominio_base, "servidor no ar");
    axum::serve(escuta, cms_web::roteador(estado))
        .with_graceful_shutdown(sinal_de_parada())
        .await?;
    Ok(())
}

/// Deixa os pedidos em andamento terminarem quando o container é parado.
async fn sinal_de_parada() {
    if tokio::signal::ctrl_c().await.is_err() {
        // Sem como ouvir o sinal, o servidor segue até ser encerrado de fora.
        std::future::pending::<()>().await;
    }
}
