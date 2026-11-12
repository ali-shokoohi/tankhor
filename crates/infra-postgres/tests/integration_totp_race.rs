#[path = "support/access.rs"]
mod access;

use std::sync::Arc;

use access::{CoordinatedAuthenticator, RealAccess, SmsFixture, credential_id, secret};
use admin::{
    application::{
        totp::{FinishTotp, FinishTotpInput},
        verify_admin_otp::{VerifyAdminOtp, VerifyOtp},
    },
    domain::auth_state::{AuthPurpose, BindingDigest},
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;
use tokio::sync::Barrier;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn otp_and_totp_consumption_are_single_use_under_race(
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
    let (totp_input, step) = access.prepare_totp_login().await;
    let barrier = Arc::new(Barrier::new(2));
    let first_crypto = CoordinatedAuthenticator::totp_race(&access.authenticator, &barrier);
    let second_crypto = CoordinatedAuthenticator::totp_race(&access.authenticator, &barrier);
    let first = FinishTotp {
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
    let second = FinishTotp {
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
        first.execute(FinishTotpInput::clone(&totp_input)),
        second.execute(totp_input)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert_eq!(access.session_count().await, 1);
    assert_eq!(access.audit_count().await, 1);
    assert_eq!(access.last_totp_step(credential_id()).await, Some(step));

    let sms = SmsFixture::start().await;
    let admin = access::active_admin();
    let (challenge, code) = access.request_otp(&sms, &admin).await;
    let otp_input = VerifyOtp {
        phone: secret("+989120000001"),
        challenge,
        code,
        browser: BindingDigest::new([7; 32]),
        origin: BindingDigest::new([8; 32]),
        purpose: AuthPurpose::SignIn,
    };
    let first_crypto = CoordinatedAuthenticator::otp_race(&access.authenticator, &barrier);
    let second_crypto = CoordinatedAuthenticator::otp_race(&access.authenticator, &barrier);
    let first = VerifyAdminOtp {
        uow: &access.uow,
        admins: &access.admins,
        rates: &access.rates,
        otps: &access.otps,
        sessions: &access.sessions,
        authenticator: &first_crypto,
        clock: &access.clock,
        ids: &access.ids,
        policy: &access.policy,
    };
    let second = VerifyAdminOtp {
        uow: &access.uow,
        admins: &access.admins,
        rates: &access.rates,
        otps: &access.otps,
        sessions: &access.sessions,
        authenticator: &second_crypto,
        clock: &access.clock,
        ids: &access.ids,
        policy: &access.policy,
    };
    let (first, second) = tokio::join!(
        first.execute(VerifyOtp::clone(&otp_input)),
        second.execute(otp_input)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert_eq!(access.live_pending_count().await, 1);
    assert_eq!(access.session_count().await, 1);
    assert_eq!(access.audit_count().await, 1);

    access.close().await;
}
