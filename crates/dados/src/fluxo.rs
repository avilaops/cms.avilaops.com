//! O fluxo de publicação contra o banco: salvar rascunho, pedir revisão,
//! devolver, publicar e despublicar.
//!
//! Painel, API e conector chamam estas funções. A permissão e a validação do
//! motor são conferidas aqui, dentro da transação, e não em quem chama.

use chrono::{DateTime, Utc};
use cms_dominio::eventos::{
    ConteudoDespublicado, ConteudoEnviadoParaRevisao, ConteudoPublicado, Evento,
};
use cms_dominio::fluxo::{self, Acao, Ator, Papel};
use motor_web::tipos::{Conteudo, Documento};
use motor_web::validacao::{Contexto, Problema, pode_publicar, validar};
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::ErroDeDados;
use crate::documentos::especie_e_slug;
use crate::eventos::emitir;
use crate::midias::{atualizar_usos, hidratar, problemas_de_midia};

#[derive(Debug, thiserror::Error)]
pub enum ErroDeFluxo {
    #[error(transparent)]
    Dados(#[from] ErroDeDados),
    /// Não existe, ou é de outro site: para quem pede, é a mesma coisa.
    #[error("documento não encontrado neste site")]
    NaoEncontrado,
    #[error("a conta não pode fazer isto neste documento")]
    SemPermissao,
    /// O motor barrou. A lista volta inteira, com os avisos junto.
    #[error("o documento tem problemas que impedem a ação")]
    Recusado(Vec<Problema>),
    #[error("o documento não tem rascunho")]
    SemRascunho,
    #[error("o rascunho não está em revisão")]
    ForaDeRevisao,
    #[error("o documento não está no ar")]
    ForaDoAr,
}

impl From<sqlx::Error> for ErroDeFluxo {
    fn from(erro: sqlx::Error) -> Self {
        Self::Dados(ErroDeDados::Banco(erro))
    }
}

/// O resultado de salvar: o rascunho sempre é gravado, e os problemas do
/// motor voltam para quem edita ver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Salvo {
    pub documento_id: Uuid,
    pub problemas: Vec<Problema>,
}

/// A linha do documento, travada até o fim da transação.
struct Linha {
    id: Uuid,
    especie: String,
    slug: Option<String>,
    caminho: Option<String>,
    situacao: String,
    versao_publicada: Option<Uuid>,
    versao_rascunho: Option<Uuid>,
    revisao_pedida_em: Option<DateTime<Utc>>,
    publicado_em: Option<DateTime<Utc>>,
    criado_por: Option<String>,
}

impl Linha {
    fn no_ar(&self) -> bool {
        self.situacao == "publicado"
    }

    /// O slug com que o visitante acha o documento hoje.
    fn slug_no_ar(&self) -> Option<&str> {
        self.slug.as_deref().filter(|_| self.no_ar())
    }
}

/// Trava o documento do site. Duas ações sobre o mesmo documento andam uma
/// de cada vez.
async fn travar(
    conexao: &mut PgConnection,
    site_id: Uuid,
    documento_id: Uuid,
) -> Result<Linha, ErroDeFluxo> {
    sqlx::query_as!(
        Linha,
        r#"
        select id, especie, slug, caminho, situacao, versao_publicada, versao_rascunho,
               revisao_pedida_em, publicado_em, criado_por
        from documento
        where id = $1 and site_id = $2
        for update
        "#,
        documento_id,
        site_id
    )
    .fetch_optional(&mut *conexao)
    .await?
    .ok_or(ErroDeFluxo::NaoEncontrado)
}

fn exigir(ator: &Ator, acao: Acao, linha: &Linha) -> Result<(), ErroDeFluxo> {
    if fluxo::pode(ator, acao, linha.criado_por.as_deref()) {
        Ok(())
    } else {
        Err(ErroDeFluxo::SemPermissao)
    }
}

fn exigir_sem_bloqueio(problemas: Vec<Problema>) -> Result<(), ErroDeFluxo> {
    if pode_publicar(&problemas) {
        Ok(())
    } else {
        Err(ErroDeFluxo::Recusado(problemas))
    }
}

fn caminho_de(conteudo: &Conteudo) -> Result<String, ErroDeDados> {
    fluxo::caminho_de(conteudo)
        .ok_or_else(|| ErroDeDados::DadoInvalido("o CMS não guarda produto".into()))
}

fn titulo_de(conteudo: &Conteudo) -> &str {
    match conteudo {
        Conteudo::Pagina(pagina) => &pagina.titulo,
        Conteudo::Post(post) => &post.titulo,
        Conteudo::Produto(produto) => &produto.nome,
    }
}

async fn conteudo_da_versao(
    conexao: &mut PgConnection,
    versao_id: Uuid,
) -> Result<Conteudo, ErroDeFluxo> {
    let conteudo = sqlx::query_scalar!(
        r#"select conteudo as "conteudo: Json<Conteudo>" from versao where id = $1"#,
        versao_id
    )
    .fetch_one(&mut *conexao)
    .await?;
    Ok(conteudo.0)
}

async fn registrar(
    conexao: &mut PgConnection,
    site_id: Uuid,
    documento_id: Uuid,
    ator: &Ator,
    acao: Acao,
) -> Result<i64, ErroDeFluxo> {
    let id = sqlx::query_scalar!(
        r#"
        insert into historico (site_id, documento_id, conta, equipe, acao)
        values ($1, $2, $3, $4, $5)
        returning id
        "#,
        site_id,
        documento_id,
        ator.conta,
        ator.equipe,
        acao.como_texto()
    )
    .fetch_one(&mut *conexao)
    .await?;
    Ok(id)
}

/// Registra no histórico e grava o evento do mesmo fato. A linha do histórico
/// é a chave do fato: uma ação gera um evento só.
async fn registrar_com_evento(
    conexao: &mut PgConnection,
    site_id: Uuid,
    documento_id: Uuid,
    ator: &Ator,
    acao: Acao,
    evento: &Evento,
) -> Result<(), ErroDeFluxo> {
    let historico_id = registrar(conexao, site_id, documento_id, ator, acao).await?;
    emitir(
        conexao,
        site_id,
        &format!("historico:{historico_id}"),
        evento,
    )
    .await?;
    Ok(())
}

/// Roda o motor com o contexto do site montado a partir do banco, e soma as
/// travas de endereço que só o CMS conhece.
async fn problemas_de(
    conexao: &mut PgConnection,
    site_id: Uuid,
    documento_id: Uuid,
    slug_no_ar: Option<&str>,
    conteudo: &Conteudo,
) -> Result<Vec<Problema>, ErroDeFluxo> {
    let caminho = caminho_de(conteudo)?;
    let outros = sqlx::query!(
        r#"
        select coalesce(v.conteudo -> 'dados' -> 'seo' ->> 'titulo', '') as "titulo!",
               coalesce(v.conteudo -> 'dados' -> 'seo' ->> 'descricao', '') as "descricao!"
        from documento d
        join versao v on v.id = d.versao_publicada
        where d.site_id = $1 and d.situacao = 'publicado' and d.id <> $2
        "#,
        site_id,
        documento_id
    )
    .fetch_all(&mut *conexao)
    .await?;
    let (titulos, descricoes): (Vec<String>, Vec<String>) = outros
        .into_iter()
        .map(|linha| (linha.titulo, linha.descricao))
        .unzip();
    // Trocar o slug de documento no ar cria o redirecionamento na mesma
    // transação da publicação: o endereço antigo sempre fica coberto.
    let redirecionados: Vec<String> = slug_no_ar.map(str::to_string).into_iter().collect();
    let contexto = Contexto {
        titulos_em_uso: &titulos,
        descricoes_em_uso: &descricoes,
        slug_publicado: slug_no_ar,
        redirecionados: &redirecionados,
        ..Contexto::vazio()
    };
    let documento = Documento {
        caminho,
        conteudo: conteudo.clone(),
    };
    let mut problemas = validar(&documento, &contexto);
    problemas.extend(fluxo::problema_de_slug_reservado(conteudo));
    problemas.extend(problemas_de_midia(conexao, site_id, conteudo).await?);

    let ocupado = sqlx::query_scalar!(
        r#"
        select exists(
            select 1 from documento where site_id = $1 and caminho = $2 and id <> $3
        ) as "ocupado!"
        "#,
        site_id,
        documento.caminho,
        documento_id
    )
    .fetch_one(&mut *conexao)
    .await?;
    if ocupado {
        problemas.push(fluxo::problema_de_slug_em_uso());
    }
    Ok(problemas)
}

/// Cria a versão em edição de um documento que não tem uma.
async fn abrir_rascunho(
    conexao: &mut PgConnection,
    documento_id: Uuid,
    ator: &Ator,
    conteudo: &Conteudo,
) -> Result<(), ErroDeFluxo> {
    let versao_id = sqlx::query_scalar!(
        r#"
        insert into versao (documento_id, numero, conteudo, gravado_por)
        values ($1, (select coalesce(max(numero), 0) + 1 from versao where documento_id = $1), $2, $3)
        returning id
        "#,
        documento_id,
        Json(conteudo) as _,
        ator.conta
    )
    .fetch_one(&mut *conexao)
    .await?;
    sqlx::query!(
        "update documento set versao_rascunho = $2 where id = $1",
        documento_id,
        versao_id
    )
    .execute(&mut *conexao)
    .await?;
    Ok(())
}

/// Grava o rascunho de um documento novo (`documento_id` vazio) ou de um que
/// já existe. Nunca é barrado pela validação: os problemas voltam junto.
///
/// O rascunho é uma versão só, regravada a cada salvamento. Editar um
/// documento no ar abre uma versão nova, e o site segue servindo a publicada.
pub async fn salvar_rascunho(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Option<Uuid>,
    conteudo: &Conteudo,
) -> Result<Salvo, ErroDeFluxo> {
    let (especie, _) = especie_e_slug(conteudo)?;
    let mut transacao = pool.begin().await?;

    let (documento_id, slug_no_ar) = match documento_id {
        None => {
            if !fluxo::pode(ator, Acao::CriarRascunho, None) {
                return Err(ErroDeFluxo::SemPermissao);
            }
            let id = sqlx::query_scalar!(
                r#"
                insert into documento (site_id, especie, criado_por)
                values ($1, $2, $3)
                returning id
                "#,
                site_id,
                especie,
                ator.conta
            )
            .fetch_one(&mut *transacao)
            .await?;
            abrir_rascunho(&mut transacao, id, ator, conteudo).await?;
            registrar(&mut transacao, site_id, id, ator, Acao::CriarRascunho).await?;
            (id, None)
        }
        Some(id) => {
            let linha = travar(&mut transacao, site_id, id).await?;
            exigir(ator, Acao::EditarRascunho, &linha)?;
            if linha.especie != especie {
                return Err(
                    ErroDeDados::DadoInvalido("um documento não muda de espécie".into()).into(),
                );
            }
            match linha.versao_rascunho {
                Some(versao_id) => {
                    sqlx::query!(
                        r#"
                        update versao set conteudo = $2, gravado_por = $3, criado_em = now()
                        where id = $1
                        "#,
                        versao_id,
                        Json(conteudo) as _,
                        ator.conta
                    )
                    .execute(&mut *transacao)
                    .await?;
                }
                None => abrir_rascunho(&mut transacao, id, ator, conteudo).await?,
            }
            (id, linha.slug_no_ar().map(str::to_string))
        }
    };

    let problemas = problemas_de(
        &mut transacao,
        site_id,
        documento_id,
        slug_no_ar.as_deref(),
        conteudo,
    )
    .await?;
    atualizar_usos(&mut transacao, site_id, documento_id).await?;
    transacao.commit().await?;
    Ok(Salvo {
        documento_id,
        problemas,
    })
}

/// Os problemas do rascunho como está gravado, para o painel mostrar ao abrir
/// o editor. Documento sem rascunho não tem o que conferir.
pub async fn problemas_do_rascunho(
    pool: &PgPool,
    site_id: Uuid,
    documento_id: Uuid,
) -> Result<Vec<Problema>, ErroDeFluxo> {
    let mut transacao = pool.begin().await?;
    let linha = travar(&mut transacao, site_id, documento_id).await?;
    let Some(versao_id) = linha.versao_rascunho else {
        return Ok(Vec::new());
    };
    let conteudo = conteudo_da_versao(&mut transacao, versao_id).await?;
    problemas_de(
        &mut transacao,
        site_id,
        linha.id,
        linha.slug_no_ar(),
        &conteudo,
    )
    .await
}

/// Pede a revisão do rascunho. Com problema que bloqueia, é recusado e a
/// lista volta. Pedir de novo o que já está em revisão não muda nada.
pub async fn enviar_para_revisao(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Uuid,
) -> Result<(), ErroDeFluxo> {
    let mut transacao = pool.begin().await?;
    let linha = travar(&mut transacao, site_id, documento_id).await?;
    exigir(ator, Acao::EnviarParaRevisao, &linha)?;
    let versao_id = linha.versao_rascunho.ok_or(ErroDeFluxo::SemRascunho)?;
    if linha.revisao_pedida_em.is_some() {
        return Ok(());
    }

    let conteudo = conteudo_da_versao(&mut transacao, versao_id).await?;
    let problemas = problemas_de(
        &mut transacao,
        site_id,
        linha.id,
        linha.slug_no_ar(),
        &conteudo,
    )
    .await?;
    exigir_sem_bloqueio(problemas)?;

    sqlx::query!(
        "update documento set revisao_pedida_em = now() where id = $1",
        linha.id
    )
    .execute(&mut *transacao)
    .await?;
    let evento = Evento::ConteudoEnviadoParaRevisao(ConteudoEnviadoParaRevisao {
        documento_id: linha.id,
        especie: linha.especie.clone(),
        titulo: titulo_de(&conteudo).to_string(),
        pedido_por: ator.conta.clone(),
    });
    registrar_com_evento(
        &mut transacao,
        site_id,
        linha.id,
        ator,
        Acao::EnviarParaRevisao,
        &evento,
    )
    .await?;
    transacao.commit().await?;
    Ok(())
}

/// Devolve o rascunho a quem escreveu, sem publicar.
pub async fn devolver(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Uuid,
) -> Result<(), ErroDeFluxo> {
    let mut transacao = pool.begin().await?;
    let linha = travar(&mut transacao, site_id, documento_id).await?;
    exigir(ator, Acao::Devolver, &linha)?;
    if linha.revisao_pedida_em.is_none() {
        return Err(ErroDeFluxo::ForaDeRevisao);
    }
    sqlx::query!(
        "update documento set revisao_pedida_em = null where id = $1",
        linha.id
    )
    .execute(&mut *transacao)
    .await?;
    registrar(&mut transacao, site_id, linha.id, ator, Acao::Devolver).await?;
    transacao.commit().await?;
    Ok(())
}

/// Aponta o endereço antigo para o novo. Quem já apontava para o antigo passa
/// a apontar direto para o novo, e o novo deixa de redirecionar: sem corrente
/// e sem volta.
async fn redirecionar(
    conexao: &mut PgConnection,
    site_id: Uuid,
    antigo: Option<&str>,
    novo: &str,
) -> Result<(), ErroDeFluxo> {
    sqlx::query!(
        "delete from redirecionamento where site_id = $1 and de = $2",
        site_id,
        novo
    )
    .execute(&mut *conexao)
    .await?;
    let Some(antigo) = antigo.filter(|antigo| *antigo != novo) else {
        return Ok(());
    };
    sqlx::query!(
        "update redirecionamento set para = $3 where site_id = $1 and para = $2",
        site_id,
        antigo,
        novo
    )
    .execute(&mut *conexao)
    .await?;
    sqlx::query!(
        r#"
        insert into redirecionamento (site_id, de, para)
        values ($1, $2, $3)
        on conflict (site_id, de) do update set para = excluded.para
        "#,
        site_id,
        antigo,
        novo
    )
    .execute(&mut *conexao)
    .await?;
    Ok(())
}

/// Põe o rascunho no ar. Tudo na mesma transação: valida com o motor, grava
/// as datas do servidor na versão, cria o redirecionamento se o endereço
/// mudou e registra no histórico.
///
/// Documento despublicado sem rascunho volta ao ar como saiu.
pub async fn publicar(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Uuid,
    agora: DateTime<Utc>,
) -> Result<(), ErroDeFluxo> {
    let mut transacao = pool.begin().await?;
    let linha = travar(&mut transacao, site_id, documento_id).await?;
    exigir(ator, Acao::Publicar, &linha)?;
    let versao_id = linha
        .versao_rascunho
        .or(linha
            .versao_publicada
            .filter(|_| linha.situacao == "despublicado"))
        .ok_or(ErroDeFluxo::SemRascunho)?;

    let mut conteudo = conteudo_da_versao(&mut transacao, versao_id).await?;
    let problemas = problemas_de(
        &mut transacao,
        site_id,
        linha.id,
        linha.slug_no_ar(),
        &conteudo,
    )
    .await?;
    exigir_sem_bloqueio(problemas)?;
    // As variantes podem ter ficado prontas depois do último salvamento.
    hidratar(&mut transacao, site_id, &mut conteudo).await?;

    let ultima_no_ar = match linha.versao_publicada {
        Some(id) => Some(Documento {
            caminho: linha.caminho.clone().unwrap_or_default(),
            conteudo: conteudo_da_versao(&mut transacao, id).await?,
        }),
        None => None,
    };
    let mut novo = Documento {
        caminho: caminho_de(&conteudo)?,
        conteudo,
    };
    let datas = fluxo::datas_da_publicacao(linha.publicado_em, ultima_no_ar.as_ref(), &novo, agora);
    fluxo::carimbar(&mut novo.conteudo, datas);

    sqlx::query!(
        "update versao set conteudo = $2 where id = $1",
        versao_id,
        Json(&novo.conteudo) as _
    )
    .execute(&mut *transacao)
    .await?;
    redirecionar(
        &mut transacao,
        site_id,
        linha.caminho.as_deref(),
        &novo.caminho,
    )
    .await?;

    let gravado = sqlx::query!(
        r#"
        update documento
        set slug = $2,
            caminho = $3,
            situacao = 'publicado',
            versao_publicada = $4,
            versao_rascunho = null,
            revisao_pedida_em = null,
            agendado_para = null,
            agendado_por = null,
            publicado_em = $5,
            atualizado_em = $6
        where id = $1
        "#,
        linha.id,
        novo.slug(),
        novo.caminho,
        versao_id,
        datas.publicado_em,
        datas.atualizado_em
    )
    .execute(&mut *transacao)
    .await;
    match gravado {
        Ok(_) => {}
        // Outro documento tomou o endereço entre a conferência e a gravação.
        Err(sqlx::Error::Database(erro)) if erro.is_unique_violation() => {
            return Err(ErroDeFluxo::Recusado(
                vec![fluxo::problema_de_slug_em_uso()],
            ));
        }
        Err(erro) => return Err(erro.into()),
    }

    atualizar_usos(&mut transacao, site_id, linha.id).await?;
    // Com a página inicial no ar, o site sai da montagem.
    if novo.caminho == "/" {
        sqlx::query!(
            r#"
            update site set situacao = 'ativo', atualizado_em = now()
            where id = $1 and situacao = 'em-montagem'
            "#,
            site_id
        )
        .execute(&mut *transacao)
        .await?;
    }
    let evento = Evento::ConteudoPublicado(ConteudoPublicado {
        documento_id: linha.id,
        especie: linha.especie.clone(),
        titulo: novo.titulo().to_string(),
        caminho_anterior: linha.caminho.filter(|antigo| *antigo != novo.caminho),
        caminho: novo.caminho,
    });
    registrar_com_evento(
        &mut transacao,
        site_id,
        linha.id,
        ator,
        Acao::Publicar,
        &evento,
    )
    .await?;
    transacao.commit().await?;
    Ok(())
}

/// Tira o documento do ar. O endereço passa a responder 410, e a última
/// versão publicada fica guardada.
pub async fn despublicar(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Uuid,
) -> Result<(), ErroDeFluxo> {
    let mut transacao = pool.begin().await?;
    let linha = travar(&mut transacao, site_id, documento_id).await?;
    exigir(ator, Acao::Despublicar, &linha)?;
    if !linha.no_ar() {
        return Err(ErroDeFluxo::ForaDoAr);
    }
    sqlx::query!(
        "update documento set situacao = 'despublicado' where id = $1",
        linha.id
    )
    .execute(&mut *transacao)
    .await?;
    let evento = Evento::ConteudoDespublicado(ConteudoDespublicado {
        documento_id: linha.id,
        especie: linha.especie.clone(),
        caminho: linha.caminho.clone().unwrap_or_default(),
    });
    registrar_com_evento(
        &mut transacao,
        site_id,
        linha.id,
        ator,
        Acao::Despublicar,
        &evento,
    )
    .await?;
    transacao.commit().await?;
    Ok(())
}

/// Marca a publicação do rascunho para depois. A permissão e a validação
/// valem agora; a validação roda de novo na hora marcada.
pub async fn agendar(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Uuid,
    quando: DateTime<Utc>,
) -> Result<(), ErroDeFluxo> {
    let mut transacao = pool.begin().await?;
    let linha = travar(&mut transacao, site_id, documento_id).await?;
    exigir(ator, Acao::Agendar, &linha)?;
    let versao_id = linha.versao_rascunho.ok_or(ErroDeFluxo::SemRascunho)?;
    let conteudo = conteudo_da_versao(&mut transacao, versao_id).await?;
    let problemas = problemas_de(
        &mut transacao,
        site_id,
        linha.id,
        linha.slug_no_ar(),
        &conteudo,
    )
    .await?;
    exigir_sem_bloqueio(problemas)?;
    sqlx::query!(
        "update documento set agendado_para = $2, agendado_por = $3 where id = $1",
        linha.id,
        quando,
        ator.conta
    )
    .execute(&mut *transacao)
    .await?;
    registrar(&mut transacao, site_id, linha.id, ator, Acao::Agendar).await?;
    transacao.commit().await?;
    Ok(())
}

/// Desmarca a publicação agendada. O rascunho continua como está.
pub async fn cancelar_agendamento(
    pool: &PgPool,
    site_id: Uuid,
    ator: &Ator,
    documento_id: Uuid,
) -> Result<(), ErroDeFluxo> {
    let mut transacao = pool.begin().await?;
    let linha = travar(&mut transacao, site_id, documento_id).await?;
    exigir(ator, Acao::Agendar, &linha)?;
    sqlx::query!(
        "update documento set agendado_para = null, agendado_por = null where id = $1",
        linha.id
    )
    .execute(&mut *transacao)
    .await?;
    transacao.commit().await?;
    Ok(())
}

/// O resultado de uma rodada da rotina `publicacao.agendada`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RodadaDeAgendados {
    /// Os sites em que algo foi ao ar: quem chama invalida o cache deles.
    pub sites_publicados: Vec<Uuid>,
    pub recusados: usize,
}

/// Publica o que estava agendado e já venceu, pelo mesmo caminho de qualquer
/// publicação. O que o motor barrar na hora perde o agendamento e fica como
/// rascunho, para alguém corrigir.
pub async fn publicar_agendados(
    pool: &PgPool,
    agora: DateTime<Utc>,
) -> Result<RodadaDeAgendados, ErroDeDados> {
    let mut rodada = RodadaDeAgendados::default();
    for agendado in crate::rotinas::agendados_vencidos(pool).await? {
        // Quem agendou tinha permissão de publicar; a rotina age em nome dele.
        let ator = Ator::do_site(agendado.agendado_por.as_str(), Papel::Editor);
        match publicar(pool, agendado.site_id, &ator, agendado.documento_id, agora).await {
            Ok(()) => rodada.sites_publicados.push(agendado.site_id),
            Err(ErroDeFluxo::Dados(erro)) => return Err(erro),
            Err(_) => {
                sqlx::query!(
                    "update documento set agendado_para = null, agendado_por = null where id = $1",
                    agendado.documento_id
                )
                .execute(pool)
                .await?;
                rodada.recusados += 1;
            }
        }
    }
    Ok(rodada)
}
