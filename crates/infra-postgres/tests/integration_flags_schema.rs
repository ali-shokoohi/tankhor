use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn flag_schema_rejects_unknown_keys_and_cross_subject_versions(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    let pool = database.admin_pool();
    let actor = "019a53a0-0000-7000-8000-000000000001";
    let first = "019a53a0-0000-7000-8000-000000000201";
    let second = "019a53a0-0000-7000-8000-000000000202";

    let mut tx = pool.begin().await.expect("begin valid flag seed");
    sqlx::query(
        "INSERT INTO flags (key, label_key, description_key) VALUES
             ('ticketing', 'flag-ticketing-label', 'flag-ticketing-description'),
             ('partner_widget_api', 'flag-partner_widget_api-label', 'flag-partner_widget_api-description')",
    )
    .execute(&mut *tx)
    .await
    .expect("insert canonical flag registry");
    sqlx::query(
        "INSERT INTO flag_versions (
             id, flag_key, enabled, publication_id, actor_ref, reason_code, applied_at
         ) VALUES
             ($1::uuid, 'ticketing', FALSE, NULL, $3::uuid, 'foundation_seed', '2026-01-01T00:00:00Z'),
             ($2::uuid, 'partner_widget_api', FALSE, NULL, $3::uuid, 'foundation_seed', '2026-01-01T00:00:00Z')",
    )
    .bind(first)
    .bind(second)
    .bind(actor)
    .execute(&mut *tx)
    .await
    .expect("insert OFF versions");
    sqlx::query(
        "UPDATE flags SET current_version_id = CASE key
             WHEN 'ticketing' THEN $1::uuid WHEN 'partner_widget_api' THEN $2::uuid END",
    )
    .bind(first)
    .bind(second)
    .execute(&mut *tx)
    .await
    .expect("attach live flags");
    tx.commit().await.expect("commit valid flag seed");

    let mut tx = pool.begin().await.expect("begin unknown flag check");
    let unknown = sqlx::query(
        "INSERT INTO flags (key, label_key, description_key)
         VALUES ('security_bypass', 'unsafe', 'unsafe')",
    )
    .execute(&mut *tx)
    .await;
    assert!(unknown.is_err(), "unregistered flags must be rejected");
    tx.rollback().await.expect("rollback unknown flag");

    let mut tx = pool.begin().await.expect("begin cross-subject flag check");
    let crossed =
        sqlx::query("UPDATE flags SET current_version_id = $1::uuid WHERE key = 'ticketing'")
            .bind(second)
            .execute(&mut *tx)
            .await;
    if crossed.is_err() {
        tx.rollback().await.expect("rollback crossed flag");
    } else {
        assert!(tx.commit().await.is_err(), "cross-subject flag pointer");
    }

    database.close().await;
}
