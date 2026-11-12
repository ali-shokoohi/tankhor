#[path = "support/access.rs"]
mod access;

use std::sync::Arc;

use access::{CoordinatedAuthenticator, RealAccess, credential_id};
use admin::application::passkey::{FinishPasskey, FinishPasskeyInput};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;
use tokio::sync::Barrier;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn passkey_consumption_is_single_use_under_race(
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

    let (first, second) = tokio::join!(
        first.execute(FinishPasskeyInput::clone(&input)),
        second.execute(input)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert_eq!(access.session_count().await, 1);
    assert_eq!(access.audit_count().await, 1);
    assert_eq!(access.credential_revision(credential_id()).await, 2);

    access.close().await;
}
