//! O catálogo de rotinas: o que roda sozinho, disparado pelo próprio
//! servidor a cada minuto.
//!
//! A trava é a linha da rotina no banco, então várias instâncias não
//! duplicam trabalho. Rotina nova é entrada neste arquivo, não agendamento
//! em serviço de fora.

use std::future::Future;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use cms_dados::ErroDeDados;
use cms_web::Cache;
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;

const CADENCIA: Duration = Duration::from_secs(60);

/// Os endereços para onde um nome aponta hoje.
async fn resolver(host: String) -> Vec<IpAddr> {
    match tokio::net::lookup_host((host.as_str(), 443)).await {
        Ok(enderecos) => enderecos.map(|endereco| endereco.ip()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Confere os domínios pendentes: o que já aponta para este servidor passa a
/// ser o endereço do site. Devolve os sites que ganharam domínio.
pub async fn conferir_dominios<F, Fut>(
    pool: &PgPool,
    ips_do_servidor: &[IpAddr],
    resolver: F,
) -> Result<Vec<uuid::Uuid>, ErroDeDados>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Vec<IpAddr>>,
{
    let mut ativados = Vec::new();
    // Sem saber o próprio endereço, o servidor não confirma domínio nenhum.
    if ips_do_servidor.is_empty() {
        return Ok(ativados);
    }
    for pendente in cms_dados::dominios_pendentes(pool).await? {
        let enderecos = resolver(pendente.host.clone()).await;
        // Todos os endereços do nome precisam ser deste servidor: um nome que
        // também aponta para outro lugar ainda não é do site.
        let aponta_para_ca =
            !enderecos.is_empty() && enderecos.iter().all(|ip| ips_do_servidor.contains(ip));
        if aponta_para_ca {
            cms_dados::confirmar_dominio(pool, &pendente).await?;
            ativados.push(pendente.site_id);
        }
    }
    Ok(ativados)
}

async fn rodada(pool: &PgPool, cache: &Cache, ips: &[IpAddr]) -> Result<(), ErroDeDados> {
    if cms_dados::reivindicar_rotina(pool, "publicacao.agendada", 50).await? {
        let agendados = cms_dados::fluxo::publicar_agendados(pool, Utc::now()).await?;
        for site_id in &agendados.sites_publicados {
            cache.invalidar_site(*site_id);
        }
        if !agendados.sites_publicados.is_empty() || agendados.recusados > 0 {
            tracing::info!(
                publicados = agendados.sites_publicados.len(),
                recusados = agendados.recusados,
                "publicacao.agendada"
            );
        }
    }
    if cms_dados::reivindicar_rotina(pool, "dominios.conferir", 600).await? {
        let ativados = conferir_dominios(pool, ips, resolver).await?;
        for site_id in &ativados {
            cache.invalidar_site(*site_id);
        }
        if !ativados.is_empty() {
            tracing::info!(ativados = ativados.len(), "dominios.conferir");
        }
    }
    if cms_dados::reivindicar_rotina(pool, "historico.limpar", 86_400).await? {
        let apagadas = cms_dados::limpar_historico(pool).await?;
        tracing::info!(apagadas, "historico.limpar");
    }
    Ok(())
}

/// Dispara o catálogo a cada minuto, enquanto o servidor estiver no ar.
pub fn agendar(pool: PgPool, cache: Arc<Cache>, ips_do_servidor: Vec<IpAddr>) {
    tokio::spawn(async move {
        let mut relogio = tokio::time::interval(CADENCIA);
        relogio.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            relogio.tick().await;
            if let Err(erro) = rodada(&pool, &cache, &ips_do_servidor).await {
                tracing::error!(%erro, "rotina falhou");
            }
        }
    });
}

#[cfg(test)]
mod testes {
    use cms_dados::fluxo;
    use cms_dominio::{Ator, Papel, PerfilDoSite, Situacao};
    use motor_web::demonstracao;
    use uuid::Uuid;

    use super::*;

    const SERVIDOR: &str = "203.0.113.7";

    fn ips() -> Vec<IpAddr> {
        vec![SERVIDOR.parse().expect("ip")]
    }

    /// Um site com a página inicial no ar, que é o que libera domínio próprio.
    async fn site_com_home(pool: &PgPool, slug: &str) -> (Uuid, Ator) {
        let perfil = PerfilDoSite::from(demonstracao::site());
        let site_id = cms_dados::criar_site(pool, slug, Situacao::EmMontagem, false, &perfil)
            .await
            .expect("site criado");
        let dono = Ator::do_site("dono", Papel::Dono);
        let home = demonstracao::documentos()
            .into_iter()
            .find(|documento| documento.caminho == "/")
            .expect("o exemplo tem página inicial")
            .conteudo;
        let salvo = fluxo::salvar_rascunho(pool, site_id, &dono, None, &home)
            .await
            .expect("rascunho salvo");
        fluxo::publicar(pool, site_id, &dono, salvo.documento_id, Utc::now())
            .await
            .expect("publicado");
        (site_id, dono)
    }

    #[sqlx::test(migrator = "cms_dados::MIGRADOR")]
    async fn dominio_so_ativa_quando_o_nome_aponta_para_o_servidor(pool: PgPool) {
        let (site_id, dono) = site_com_home(&pool, "oficina").await;
        // Publicar a página inicial tirou o site da montagem.
        let situacao: String = sqlx::query_scalar("select situacao from site where id = $1")
            .bind(site_id)
            .fetch_one(&pool)
            .await
            .expect("situação");
        assert_eq!(situacao, "ativo");

        let reservados = ["sites.teste", "cms.teste"];
        for host in ["oficina.example", "outra.example", "dividido.example"] {
            cms_dados::pedir_dominio(&pool, site_id, &dono, host, &reservados)
                .await
                .expect("domínio pedido");
        }
        let dns = |host: String| async move {
            match host.as_str() {
                "oficina.example" => vec![SERVIDOR.parse().expect("ip")],
                "dividido.example" => vec![
                    SERVIDOR.parse().expect("ip"),
                    "198.51.100.9".parse().expect("ip"),
                ],
                "outra.example" => vec!["198.51.100.9".parse().expect("ip")],
                _ => Vec::new(),
            }
        };

        // Sem saber o próprio IP, nada é ativado.
        assert!(
            conferir_dominios(&pool, &[], dns)
                .await
                .expect("conferência")
                .is_empty()
        );
        let ativados = conferir_dominios(&pool, &ips(), dns)
            .await
            .expect("conferência");
        assert_eq!(ativados, vec![site_id]);

        let dominios = cms_dados::dominios_do_site(&pool, site_id)
            .await
            .expect("domínios");
        let ativo: Vec<&str> = dominios
            .iter()
            .filter(|dominio| dominio.ativo)
            .map(|dominio| dominio.host.as_str())
            .collect();
        assert_eq!(ativo, ["oficina.example"]);
        let evento: String = sqlx::query_scalar(
            "select dados ->> 'dominio' from evento where tipo = 'dominio.ativado'",
        )
        .fetch_one(&pool)
        .await
        .expect("evento do domínio");
        assert_eq!(evento, "oficina.example");

        // O Caddy só emite certificado para endereço de site que existe.
        for (host, esperado) in [
            ("oficina.example", true),
            ("www.oficina.example", true),
            ("outra.example", true),
            ("oficina.sites.teste", true),
            ("desconhecido.example", false),
            ("naoexiste.sites.teste", false),
            ("a.b.sites.teste", false),
        ] {
            assert_eq!(
                cms_dados::dominio_permitido(&pool, host, "sites.teste")
                    .await
                    .expect("consulta"),
                esperado,
                "{host}"
            );
        }
    }

    #[sqlx::test(migrator = "cms_dados::MIGRADOR")]
    async fn cada_rotina_roda_uma_vez_por_intervalo(pool: PgPool) {
        assert!(
            cms_dados::reivindicar_rotina(&pool, "historico.limpar", 3600)
                .await
                .expect("trava")
        );
        // A segunda instância, no mesmo intervalo, não pega a vez.
        assert!(
            !cms_dados::reivindicar_rotina(&pool, "historico.limpar", 3600)
                .await
                .expect("trava")
        );
        assert!(
            !cms_dados::reivindicar_rotina(&pool, "rotina.que.nao.existe", 0)
                .await
                .expect("trava")
        );
    }
}
