#[path = "support/access.rs"]
mod access;

use access::{RealAccess, SmsFixture, active_admin};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn real_access_harness_is_ready(
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
    let sms = SmsFixture::start().await;
    let admin = active_admin();
    access.seed_admin(&admin).await;

    let (_, code) = access.request_otp(&sms, &admin).await;
    assert_eq!(code.expose().len(), 6);
    assert!(code.expose().iter().all(u8::is_ascii_digit));
    assert_eq!(access.session_count().await, 0);

    let credential = access.register_passkey(&admin).await;
    access.insert_credential(&credential).await;
    assert_eq!(access.active_credential_count(admin.id()).await, 1);

    access.close().await;
}
