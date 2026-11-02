use cms_dados::{
    ErroDeEstudio, NovoArquivo, criar_arquivo, listar_arquivos, mover_no, previa_por_token,
    publicar_previa,
};
use cms_dominio::estudio::{Medida, TIPO_TEXTO, medida_do_no, posicao_inicial};
use cms_dominio::{PerfilDoSite, Situacao};
use sqlx::PgPool;
use uuid::Uuid;

fn novo(site_id: Uuid, nome: &str) -> NovoArquivo {
    let pagina = Medida {
        largura: 1280,
        altura: 800,
    };
    let texto = medida_do_no(TIPO_TEXTO, "", pagina).expect("medida");
    let (x, y) = posicao_inicial(0, texto, pagina).expect("posição");
    NovoArquivo {
        site_id,
        nome: nome.to_string(),
        pedido: nome.to_string(),
        criado_por: "conta-a".to_string(),
        sistema: "tinta".to_string(),
        largura: pagina.largura,
        altura: pagina.altura,
        texto: nome.to_string(),
        texto_x: x,
        texto_y: y,
        texto_largura: texto.largura,
        texto_altura: texto.altura,
    }
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn estudio_isola_por_site_e_a_versao_nao_muda(pool: PgPool) {
    let sitio_a = cms_dados::criar_site(
        &pool,
        "oficina-a",
        Situacao::EmMontagem,
        false,
        &PerfilDoSite::inicial("Oficina A"),
    )
    .await
    .expect("site");
    let sitio_b = cms_dados::criar_site(
        &pool,
        "oficina-b",
        Situacao::EmMontagem,
        false,
        &PerfilDoSite::inicial("Oficina B"),
    )
    .await
    .expect("site");
    let arquivo = criar_arquivo(&pool, novo(sitio_a, "Cartaz"))
        .await
        .expect("arquivo");
    assert!(
        listar_arquivos(&pool, sitio_b)
            .await
            .expect("lista")
            .is_empty()
    );

    let aberto = cms_dados::abrir_arquivo(&pool, sitio_a, arquivo)
        .await
        .expect("leitura")
        .expect("arquivo do sítio A");
    let no = aberto.paginas[0].nos[0].id;
    mover_no(&pool, sitio_a, arquivo, no, 40, 48)
        .await
        .expect("move");
    let recusado = mover_no(&pool, sitio_b, arquivo, no, 40, 48).await;
    assert!(matches!(recusado, Err(ErroDeEstudio::NoNaoEncontrado)));

    let primeira = publicar_previa(&pool, sitio_a, arquivo, "conta-a")
        .await
        .expect("prévia");
    let segunda = publicar_previa(&pool, sitio_a, arquivo, "conta-a")
        .await
        .expect("republicar");
    assert_eq!(primeira.token, segunda.token);
    assert_eq!(segunda.numero, 2);
    let erro = sqlx::query("update estudio_versao set numero = 9 where arquivo_id = $1")
        .bind(arquivo)
        .execute(&pool)
        .await;
    assert!(erro.is_err(), "versão publicada não se reescreve");

    let definicao = previa_por_token(&pool, &primeira.token)
        .await
        .expect("consulta")
        .expect("prévia");
    assert_eq!(definicao.nome, "Cartaz");
    assert_eq!(definicao.sistema, "tinta");
    assert!(
        previa_por_token(&pool, &"ab".repeat(32))
            .await
            .expect("consulta")
            .is_none()
    );
}
