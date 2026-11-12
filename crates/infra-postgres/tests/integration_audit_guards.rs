use sqlx::{PgPool, postgres::PgConnectOptions, postgres::PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;

const AUDIT_ID: &str = "019a53a0-0000-7000-8000-000000000051";
const OTHER_AUDIT_ID: &str = "019a53a0-0000-7000-8000-000000000052";
const EVENT_KEY: &str = "019a53a0-0000-7000-8000-000000000053";
const ADMIN_ID: &str = "019a53a0-0000-7000-8000-000000000001";

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn runtime_cannot_update_delete_or_truncate_audit(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let db = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    assert_audit_shape(db.admin_pool()).await;
    let runtime = db
        .connect_runtime_pool(1)
        .await
        .expect("connect restricted runtime role");

    for statement in [
        "UPDATE audit_log SET reason_code = 'admin_revoked' WHERE id = '019a53a0-0000-7000-8000-000000000051'",
        "DELETE FROM audit_log WHERE id = '019a53a0-0000-7000-8000-000000000051'",
        "TRUNCATE audit_log",
    ] {
        let error = sqlx::query(statement)
            .execute(&runtime)
            .await
            .expect_err("runtime mutation of immutable audit history must fail");
        assert!(error.as_database_error().is_some());
    }
    assert_eq!(audit_count(&runtime).await, 1);
    runtime.close().await;
    db.close().await;
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn audit_guards_reject_privileged_dml(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let db = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    assert_audit_shape(db.admin_pool()).await;
    let migration = db.admin_pool().clone();

    for statement in [
        "UPDATE audit_log SET reason_code = 'admin_revoked' WHERE id = '019a53a0-0000-7000-8000-000000000051'",
        "DELETE FROM audit_log WHERE id = '019a53a0-0000-7000-8000-000000000051'",
        "TRUNCATE audit_log",
    ] {
        let error = sqlx::query(statement)
            .execute(&migration)
            .await
            .expect_err("guard must reject migration-owner audit mutation");
        let database = error
            .as_database_error()
            .expect("guard rejection must be a database error");
        assert_eq!(database.code().as_deref(), Some("P0001"), "{statement}");
    }
    assert_eq!(audit_count(&migration).await, 1);
    migration.close().await;
    db.close().await;
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn runtime_cannot_disable_guards_or_assume_owner(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let db = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    assert_audit_shape(db.admin_pool()).await;

    let owner: String = sqlx::query_scalar(
        "SELECT pg_get_userbyid(relowner) FROM pg_class WHERE oid = 'audit_log'::regclass",
    )
    .fetch_one(db.admin_pool())
    .await
    .expect("read audit table owner");
    assert_ne!(owner, "tankhor_fixture_runtime");

    let trigger_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_trigger
         WHERE tgrelid = 'audit_log'::regclass AND NOT tgisinternal
           AND tgname IN (
               'audit_log_reject_update_delete',
               'audit_log_reject_truncate'
           )",
    )
    .fetch_one(db.admin_pool())
    .await
    .expect("read audit guard triggers");
    assert_eq!(trigger_count, 2);

    let runtime = db
        .connect_runtime_pool(1)
        .await
        .expect("connect restricted runtime role");
    for statement in [
        "ALTER TABLE audit_log DISABLE TRIGGER audit_log_reject_update_delete",
        "ALTER TABLE audit_log DISABLE TRIGGER audit_log_reject_truncate",
        "DROP TRIGGER audit_log_reject_update_delete ON audit_log",
        "SET ROLE tankhor_fixture_migration",
    ] {
        sqlx::query(statement)
            .execute(&runtime)
            .await
            .expect_err("runtime must not disable guards or assume migration owner");
    }

    let privileges: (bool, bool, bool, bool, bool, bool) = sqlx::query_as(
        "SELECT
             has_table_privilege(current_user, 'audit_log', 'SELECT'),
             has_table_privilege(current_user, 'audit_log', 'INSERT'),
             has_table_privilege(current_user, 'audit_log', 'UPDATE'),
             has_table_privilege(current_user, 'audit_log', 'DELETE'),
             has_table_privilege(current_user, 'audit_log', 'TRUNCATE'),
             has_table_privilege(current_user, 'audit_log', 'TRIGGER')",
    )
    .fetch_one(&runtime)
    .await
    .expect("inspect runtime audit privileges");
    assert_eq!(privileges, (true, true, false, false, false, false));

    runtime.close().await;
    db.close().await;
}

async fn assert_audit_shape(pool: &PgPool) {
    let table: Option<String> = sqlx::query_scalar("SELECT to_regclass('public.audit_log')::text")
        .fetch_one(pool)
        .await
        .expect("inspect audit table");
    assert_eq!(table.as_deref(), Some("audit_log"), "missing audit table");

    let definitions: Vec<String> = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes WHERE schemaname = 'public' AND tablename = 'audit_log'",
    )
    .fetch_all(pool)
    .await
    .expect("read audit indexes");
    for columns in [
        "(occurred_at, id)",
        "(actor_id, occurred_at, id)",
        "(action, occurred_at, id)",
        "(subject_id, occurred_at, id)",
    ] {
        assert!(
            definitions
                .iter()
                .any(|definition| definition.contains(columns)),
            "missing audit search index {columns}"
        );
    }

    insert_audit(pool, AUDIT_ID).await;
    let duplicate = sqlx::query(
        "INSERT INTO audit_log (
             id, event_key, actor_type, actor_id, action, subject_type, subject_id,
             occurred_at, reason_code, factor
         ) VALUES (
             $1::uuid, $2::uuid, 'admin', $3::uuid, 'signed_in', 'admin', $3::uuid,
             now(), 'authentication', 'passkey'
         )",
    )
    .bind(OTHER_AUDIT_ID)
    .bind(EVENT_KEY)
    .bind(ADMIN_ID)
    .execute(pool)
    .await
    .expect_err("event-key uniqueness must prevent duplicate audit events");
    assert_eq!(
        duplicate
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("23505")
    );
}

async fn insert_audit(pool: &PgPool, id: &str) {
    sqlx::query(
        "INSERT INTO audit_log (
             id, event_key, actor_type, actor_id, action, subject_type, subject_id,
             occurred_at, reason_code, factor
         ) VALUES (
             $1::uuid, $2::uuid, 'admin', $3::uuid, 'signed_in', 'admin', $3::uuid,
             now(), 'authentication', 'passkey'
         )",
    )
    .bind(id)
    .bind(EVENT_KEY)
    .bind(ADMIN_ID)
    .execute(pool)
    .await
    .expect("append safe audit event");
}

async fn audit_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(pool)
        .await
        .expect("read retained audit history")
}
