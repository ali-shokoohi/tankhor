#[path = "support/access.rs"]
mod access;

use std::sync::Arc;

use access::{CoordinatedAuthenticator, RealAccess, credential_id};
use admin::application::passkey::FinishPasskey;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;
use tokio::sync::Barrier;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn audit_failure_aborts_access(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    database
        .verify_migrated_and_role_separation()
        .await
        .expect("restricted runtime role and migrations must be ready");
    let access = RealAccess::connect(database).await;
    let input = access.prepare_passkey_login().await;
    sqlx::raw_sql(
        "CREATE FUNCTION reject_access_audit()
         RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER AS $$
         BEGIN
             RAISE EXCEPTION 'injected audit dependency failure';
         END;
         $$;
         CREATE TRIGGER reject_access_audit
         BEFORE INSERT ON audit_log
         FOR EACH ROW EXECUTE FUNCTION reject_access_audit();",
    )
    .execute(access.database.admin_pool())
    .await
    .expect("install real audit failure trigger");
    let barrier = Arc::new(Barrier::new(2));
    let first_crypto = CoordinatedAuthenticator::passkey_race(&access.authenticator, &barrier);
    let second_crypto = CoordinatedAuthenticator::passkey_race(&access.authenticator, &barrier);
    let first = FinishPasskey {
        uow: &access.uow,
        admins: &access.admins,
        credentials: &access.credentials,
        sessions: &access.sessions,
        audit: &access.audit,
        authenticator: &first_crypto,
        clock: &access.clock,
        ids: &access.ids,
        policy: &access.policy,
    };
    let second = FinishPasskey {
        uow: &access.uow,
        admins: &access.admins,
        credentials: &access.credentials,
        sessions: &access.sessions,
        audit: &access.audit,
        authenticator: &second_crypto,
        clock: &access.clock,
        ids: &access.ids,
        policy: &access.policy,
    };
    let (first, second) = tokio::join!(first.execute(input.clone()), second.execute(input.clone()));
    assert!(first.is_err() && second.is_err());
    assert_eq!(access.session_count().await, 0);
    assert_eq!(access.audit_count().await, 0);
    assert_eq!(access.credential_revision(credential_id()).await, 1);
    assert_eq!(access.live_pending_count().await, 1);

    sqlx::raw_sql("DROP TRIGGER reject_access_audit ON audit_log")
        .execute(access.database.admin_pool())
        .await
        .expect("remove injected audit failure");
    FinishPasskey {
        uow: &access.uow,
        admins: &access.admins,
        credentials: &access.credentials,
        sessions: &access.sessions,
        audit: &access.audit,
        authenticator: &access.authenticator,
        clock: &access.clock,
        ids: &access.ids,
        policy: &access.policy,
    }
    .execute(input)
    .await
    .expect("rolled-back ceremony remains safe to retry");
    assert_eq!(access.session_count().await, 1);
    assert_eq!(access.audit_count().await, 1);
    assert_eq!(access.credential_revision(credential_id()).await, 2);
    assert_eq!(access.live_pending_count().await, 0);

    access.close().await;
}
