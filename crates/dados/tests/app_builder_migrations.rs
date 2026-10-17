use sqlx::PgPool;
use uuid::Uuid;

async fn criar_app(pool: &PgPool, slug: &str) -> Uuid {
    sqlx::query_scalar(
        r#"
        insert into app (slug, nome, bundle_id, criado_por)
        values ($1, 'App de teste', $2, 'conta-owner')
        returning id
        "#,
    )
    .bind(slug)
    .bind(format!("com.avilaops.{slug}"))
    .fetch_one(pool)
    .await
    .expect("app deve ser criado")
}

async fn criar_versao(pool: &PgPool, app_id: Uuid, numero: i32) -> Uuid {
    sqlx::query_scalar(
        r#"
        insert into app_versao (
            app_id,
            numero,
            schema_version,
            minimum_runtime_version,
            definicao,
            criado_por
        )
        values ($1, $2, '1.0.0', '1.0.0', $3, 'conta-owner')
        returning id
        "#,
    )
    .bind(app_id)
    .bind(numero)
    .bind(serde_json::json!({
        "schemaVersion": "1.0.0",
        "appId": "app-teste",
        "minimumRuntimeVersion": "1.0.0",
        "navigation": { "initialRoute": "home" },
        "screens": [{ "id": "home", "route": "home", "title": "Home", "components": [] }]
    }))
    .fetch_one(pool)
    .await
    .expect("versão deve ser criada")
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn registry_isola_participacoes_e_versoes_por_app(pool: PgPool) {
    let app_a = criar_app(&pool, "app-a").await;
    let app_b = criar_app(&pool, "app-b").await;

    sqlx::query(
        "insert into app_participacao (app_id, conta, papel) values ($1, 'conta-owner', 'owner')",
    )
    .bind(app_a)
    .execute(&pool)
    .await
    .expect("participação deve ser criada");

    let app_da_conta: Uuid = sqlx::query_scalar(
        "select app_id from app_participacao where app_id = $1 and conta = 'conta-owner'",
    )
    .bind(app_a)
    .fetch_one(&pool)
    .await
    .expect("participação deve ser localizada pelo app e pela conta");
    assert_eq!(app_da_conta, app_a);

    let versao_a = criar_versao(&pool, app_a, 1).await;
    let erro = sqlx::query(
        "insert into app_ambiente (app_id, nome, versao_id, atualizado_por) values ($1, 'production', $2, 'conta-owner')",
    )
    .bind(app_b)
    .bind(versao_a)
    .execute(&pool)
    .await
    .expect_err("ambiente não pode apontar para versão de outro app");
    assert!(erro.as_database_error().is_some());
}

#[sqlx::test(migrator = "cms_dados::MIGRADOR")]
async fn versoes_sao_imutaveis_e_ambiente_permite_rollback_logico(pool: PgPool) {
    let app_id = criar_app(&pool, "app-rollback").await;
    let versao_1 = criar_versao(&pool, app_id, 1).await;
    let versao_2 = criar_versao(&pool, app_id, 2).await;

    sqlx::query(
        "insert into app_ambiente (app_id, nome, versao_id, atualizado_por) values ($1, 'production', $2, 'conta-owner')",
    )
    .bind(app_id)
    .bind(versao_2)
    .execute(&pool)
    .await
    .expect("ambiente deve apontar para uma versão do próprio app");

    sqlx::query(
        "update app_ambiente set versao_id = $1, atualizado_por = 'conta-owner' where app_id = $2 and nome = 'production'",
    )
    .bind(versao_1)
    .bind(app_id)
    .execute(&pool)
    .await
    .expect("rollback lógico deve trocar o ponteiro para uma versão anterior");

    let versao_publicada: Uuid = sqlx::query_scalar(
        "select versao_id from app_ambiente where app_id = $1 and nome = 'production'",
    )
    .bind(app_id)
    .fetch_one(&pool)
    .await
    .expect("ambiente deve manter a versão escolhida");
    assert_eq!(versao_publicada, versao_1);

    sqlx::query("update app_versao set definicao = '{}'::jsonb where id = $1")
        .bind(versao_1)
        .execute(&pool)
        .await
        .expect_err("versão publicada deve ser imutável");

    sqlx::query("delete from app_versao where id = $1")
        .bind(versao_2)
        .execute(&pool)
        .await
        .expect_err("versão imutável não deve ser removida mesmo fora de um ambiente");
}
