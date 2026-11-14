use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;

async fn seed_admin_and_setting(pool: &PgPool) {
    let mut tx = pool.begin().await.expect("begin review fixture");
    sqlx::raw_sql(
        "INSERT INTO admins (
             id, phone_key_id, phone_nonce, phone_ciphertext, phone_lookup_digest,
             eligibility, security_epoch, created_by, created_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000001', 'test', '\\x01', '\\x01',
             '\\x0000000000000000000000000000000000000000000000000000000000000001',
             'active', 0, '019a53a0-0000-7000-8000-000000000001', '2026-01-01T00:00:00Z'
         );
         INSERT INTO settings (key, label_key, unit, min_value, max_value) VALUES
             ('upload_max_mb', 'setting-upload_max_mb-label', 'megabytes', 1, 25);
         INSERT INTO setting_versions (
             id, setting_key, numeric_value, publication_id, actor_ref, reason_code, applied_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000401', 'upload_max_mb', 15, NULL,
             '019a53a0-0000-7000-8000-000000000001', 'foundation_seed', '2026-01-01T00:00:00Z'
         );
         UPDATE settings SET current_version_id = '019a53a0-0000-7000-8000-000000000401'
         WHERE key = 'upload_max_mb';",
    )
    .execute(&mut *tx)
    .await
    .expect("seed admin and setting");
    tx.commit().await.expect("commit review fixture");
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn review_schema_enforces_immutable_exact_version_links(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    let pool = database.admin_pool();
    seed_admin_and_setting(pool).await;

    let mut tx = pool.begin().await.expect("begin valid review");
    sqlx::raw_sql(
        "INSERT INTO change_drafts (
             id, setting_key, flag_key, baseline_setting_version, baseline_flag_version,
             edit_counter, creator_id, created_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000501', 'upload_max_mb', NULL,
             '019a53a0-0000-7000-8000-000000000401', NULL, 0,
             '019a53a0-0000-7000-8000-000000000001', '2026-01-01T00:00:00Z'
         );
         INSERT INTO change_revisions (
             id, change_id, setting_key, flag_key, revision_number,
             numeric_value, format_mask, boolean_value, reason_code,
             scheduled_at, scheduled_zone, version_hash, created_by, created_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000502',
             '019a53a0-0000-7000-8000-000000000501', 'upload_max_mb', NULL, 1,
             12, NULL, NULL, 'draft_change', NULL, NULL, '\\x0000000000000000000000000000000000000000000000000000000000000000',
             '019a53a0-0000-7000-8000-000000000001', '2026-01-01T00:00:00Z'
         );
         UPDATE change_drafts SET head_revision_id = '019a53a0-0000-7000-8000-000000000502', edit_counter = 1
         WHERE id = '019a53a0-0000-7000-8000-000000000501';
         INSERT INTO change_previews (
             id, change_id, revision_id, setting_key, flag_key,
             baseline_setting_version, baseline_flag_version, policy_identity, source_identity,
             effect_rule, assessed_at, reviewed_by, version_hash, readiness
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000503',
             '019a53a0-0000-7000-8000-000000000501',
             '019a53a0-0000-7000-8000-000000000502', 'upload_max_mb', NULL,
             '019a53a0-0000-7000-8000-000000000401', NULL, 'upload/v1', 'upload-ledger',
             'preserve-stored/validate-new-only', '2026-01-01T00:30:00Z',
             '019a53a0-0000-7000-8000-000000000001',
             '\\x0000000000000000000000000000000000000000000000000000000000000000', NULL
         );
         INSERT INTO change_preview_evidence (preview_id, evidence_index, population, known_count)
         VALUES ('019a53a0-0000-7000-8000-000000000503', 0, 'stored', 0);
         INSERT INTO change_approvals (
             id, preview_id, change_id, revision_id, baseline_setting_version,
             baseline_flag_version, approver_id, approved_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000504',
             '019a53a0-0000-7000-8000-000000000503',
             '019a53a0-0000-7000-8000-000000000501',
             '019a53a0-0000-7000-8000-000000000502',
             '019a53a0-0000-7000-8000-000000000401', NULL,
             '019a53a0-0000-7000-8000-000000000001', '2026-01-01T01:00:00Z'
         );",
    )
    .execute(&mut *tx)
    .await
    .expect("insert exact review chain");
    tx.commit().await.expect("commit exact review chain");

    let mut tx = pool.begin().await.expect("begin wrong preview revision");
    let wrong = sqlx::query(
        "INSERT INTO change_previews (
             id, change_id, revision_id, setting_key, flag_key,
             baseline_setting_version, baseline_flag_version, policy_identity, source_identity,
             effect_rule, assessed_at, reviewed_by, version_hash, readiness
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000505',
             '019a53a0-0000-7000-8000-000000000501',
             '019a53a0-0000-7000-8000-000000000599', 'upload_max_mb', NULL,
             '019a53a0-0000-7000-8000-000000000401', NULL, 'upload/v1', 'upload-ledger',
             'preserve-stored/validate-new-only', '2026-01-01T00:30:00Z',
             '019a53a0-0000-7000-8000-000000000001',
             '\\x0000000000000000000000000000000000000000000000000000000000000000', NULL
         )",
    )
    .execute(&mut *tx)
    .await;
    assert!(
        wrong.is_err(),
        "preview must reference the exact persisted revision"
    );
    tx.rollback().await.expect("rollback wrong preview");

    let mut tx = pool.begin().await.expect("begin immutable revision check");
    let update = sqlx::query(
        "UPDATE change_revisions SET numeric_value = 25
         WHERE id = '019a53a0-0000-7000-8000-000000000502'",
    )
    .execute(&mut *tx)
    .await;
    assert!(update.is_err(), "review history must be immutable");
    tx.rollback().await.expect("rollback immutable revision");

    database.close().await;
}
