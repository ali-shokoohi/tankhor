use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn runtime_cannot_rewrite_registry_policy_or_version_history(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    let pool = database.admin_pool();
    let version = "019a53a0-0000-7000-8000-000000000301";
    let mut tx = pool.begin().await.expect("begin seed");
    sqlx::query(
        "INSERT INTO settings (key, label_key, unit, min_value, max_value)
         VALUES ('upload_max_mb', 'setting-upload_max_mb-label', 'megabytes', 1, 25)",
    )
    .execute(&mut *tx)
    .await
    .expect("insert registry");
    sqlx::query(
        "INSERT INTO setting_versions (
             id, setting_key, numeric_value, publication_id, actor_ref, reason_code, applied_at
         ) VALUES ($1::uuid, 'upload_max_mb', 15, NULL,
                   '019a53a0-0000-7000-8000-000000000001', 'foundation_seed', '2026-01-01T00:00:00Z')",
    )
    .bind(version)
    .execute(&mut *tx)
    .await
    .expect("insert version");
    sqlx::query("UPDATE settings SET current_version_id = $1::uuid WHERE key = 'upload_max_mb'")
        .bind(version)
        .execute(&mut *tx)
        .await
        .expect("attach version");
    tx.commit().await.expect("commit seed");

    let runtime = database
        .connect_runtime_pool(2)
        .await
        .expect("connect restricted runtime");
    let visible: i64 = sqlx::query_scalar("SELECT count(*) FROM setting_versions")
        .fetch_one(&runtime)
        .await
        .expect("runtime reads safe registry history");
    assert_eq!(visible, 1);

    for statement in [
        "UPDATE settings SET max_value = 999 WHERE key = 'upload_max_mb'",
        "UPDATE setting_versions SET numeric_value = 25 WHERE setting_key = 'upload_max_mb'",
        "DELETE FROM setting_versions WHERE setting_key = 'upload_max_mb'",
        "TRUNCATE setting_versions",
    ] {
        let mut tx = runtime.begin().await.expect("begin forbidden mutation");
        let result = sqlx::query(statement).execute(&mut *tx).await;
        assert!(result.is_err(), "runtime mutation must fail: {statement}");
        tx.rollback().await.expect("rollback forbidden mutation");
    }

    runtime.close().await;
    database.close().await;
}
