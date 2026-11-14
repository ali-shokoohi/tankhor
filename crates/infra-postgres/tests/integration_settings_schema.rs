use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Postgres, Transaction};
use testkit::fixtures::database::FixtureDatabase;

const ACTOR: &str = "019a53a0-0000-7000-8000-000000000001";
const NUMERIC_VERSION: &str = "019a53a0-0000-7000-8000-000000000101";
const FORMAT_VERSION: &str = "019a53a0-0000-7000-8000-000000000102";

async fn seed_pair(pool: &PgPool) -> Transaction<'static, Postgres> {
    let mut tx = pool.begin().await.expect("begin registry seed");
    sqlx::query(
        "INSERT INTO settings (key, label_key, unit, min_value, max_value)
         VALUES
           ('upload_max_mb', 'setting-upload_max_mb-label', 'megabytes', 1, 25),
           ('upload_formats', 'setting-upload_formats-label', 'format_mask', NULL, NULL)",
    )
    .execute(&mut *tx)
    .await
    .expect("insert fixed registry policy");
    sqlx::query(
        "INSERT INTO setting_versions (
             id, setting_key, numeric_value, format_mask, publication_id,
             actor_ref, reason_code, applied_at
         ) VALUES
           ($1::uuid, 'upload_max_mb', 15, NULL, NULL, $3::uuid, 'foundation_seed', '2026-01-01T00:00:00Z'),
           ($2::uuid, 'upload_formats', NULL, 15, NULL, $3::uuid, 'foundation_seed', '2026-01-01T00:00:00Z')",
    )
    .bind(NUMERIC_VERSION)
    .bind(FORMAT_VERSION)
    .bind(ACTOR)
    .execute(&mut *tx)
    .await
    .expect("insert typed initial versions");
    sqlx::query(
        "UPDATE settings SET current_version_id = CASE key
             WHEN 'upload_max_mb' THEN $1::uuid
             WHEN 'upload_formats' THEN $2::uuid
         END WHERE key IN ('upload_max_mb', 'upload_formats')",
    )
    .bind(NUMERIC_VERSION)
    .bind(FORMAT_VERSION)
    .execute(&mut *tx)
    .await
    .expect("attach matching live versions");
    tx
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn registry_schema_rejects_invalid_values_and_cross_subject_pointers(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    let pool = database.admin_pool();
    seed_pair(pool)
        .await
        .commit()
        .await
        .expect("commit valid seed");

    let mut tx = pool.begin().await.expect("begin unknown key check");
    let unknown = sqlx::query(
        "INSERT INTO settings (key, label_key, unit, min_value, max_value)
         VALUES ('session_timeout', 'unsafe', 'seconds', 0, 0)",
    )
    .execute(&mut *tx)
    .await;
    assert!(unknown.is_err(), "unknown registry keys must be rejected");
    tx.rollback().await.expect("rollback unknown key");

    for (numeric, mask) in [(Some(0), None), (None, Some(0)), (Some(15), Some(1))] {
        let mut tx = pool.begin().await.expect("begin typed-value check");
        let result = sqlx::query(
            "INSERT INTO setting_versions (
                 id, setting_key, numeric_value, format_mask, publication_id,
                 actor_ref, reason_code, applied_at
             ) VALUES ($1::uuid, $2, $3, $4, NULL, $5::uuid, 'foundation_seed', '2026-01-01T00:00:00Z')",
        )
        .bind("019a53a0-0000-7000-8000-000000000199")
        .bind(if mask == Some(0) {
            "upload_formats"
        } else {
            "upload_max_mb"
        })
        .bind(numeric)
        .bind(mask)
        .bind(ACTOR)
        .execute(&mut *tx)
        .await;
        assert!(result.is_err(), "invalid typed value must be rejected");
        tx.rollback().await.expect("rollback invalid value");
    }

    let mut tx = pool.begin().await.expect("begin cross-subject check");
    let crossed =
        sqlx::query("UPDATE settings SET current_version_id = $1 WHERE key = 'upload_formats'")
            .bind(NUMERIC_VERSION)
            .execute(&mut *tx)
            .await;
    if crossed.is_err() {
        tx.rollback().await.expect("rollback crossed pointer");
    } else {
        assert!(
            tx.commit().await.is_err(),
            "a live pointer cannot reference another subject's version"
        );
    }

    let mut tx = pool.begin().await.expect("begin missing pointer check");
    sqlx::query(
        "INSERT INTO settings (key, label_key, unit, min_value, max_value)
         VALUES ('generation_max_attempts', 'setting-generation_max_attempts-label', 'total_attempts', 1, 3)",
    )
    .execute(&mut *tx)
    .await
    .expect("defer cyclic seed pointer");
    assert!(
        tx.commit().await.is_err(),
        "a committed registry row must have a live pointer"
    );

    database.close().await;
}
