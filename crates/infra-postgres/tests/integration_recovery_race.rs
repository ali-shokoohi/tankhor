#[path = "support/access.rs"]
mod access;

use access::{CoordinatedAuthenticator, RealAccess, active_admin, actor_id, credential_id};
use admin::{
    application::{
        recover_admin::{RecoverAdmin, RecoverAdminInput},
        totp::FinishTotp,
    },
    domain::audit::ReasonCode,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;
use tokio::sync::Notify;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn recovery_races_with_login_safely(
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
    let (input, _) = access.prepare_totp_login().await;
    let actor = access.prepare_recovery_actor().await;
    let ready = Notify::new();
    let resume = Notify::new();
    let paused = CoordinatedAuthenticator::paused_totp(&access.authenticator, &ready, &resume);
    let login = FinishTotp {
        uow: &access.uow,
        admins: &access.admins,
        credentials: &access.credentials,
        sessions: &access.sessions,
        audit: &access.audit,
        authenticator: &paused,
        clock: &access.clock,
        ids: &access.ids,
        policy: &access.policy,
    };
    let recovery = RecoverAdmin {
        uow: &access.uow,
        admins: &access.admins,
        sessions: &access.sessions,
        credentials: &access.credentials,
        otp: &access.otps,
        audit: &access.audit,
        clock: &access.clock,
        ids: &access.ids,
        policy: &access.policy,
    };
    let recover = async {
        ready.notified().await;
        let result = recovery
            .execute(RecoverAdminInput {
                actor,
                target: active_admin().id(),
                reason: Some(ReasonCode::AdminRecovery),
            })
            .await;
        resume.notify_one();
        result
    };
    let (login, recovery) = tokio::join!(login.execute(input), recover);
    assert!(
        login.is_err(),
        "stale login must not complete after recovery"
    );
    recovery.expect("current actor must complete target recovery");
    assert_eq!(access.admin_epoch(active_admin().id()).await, 1);
    assert_eq!(access.admin_epoch(actor_id()).await, 0);
    assert!(access.credential_is_revoked(credential_id()).await);
    assert_eq!(access.active_credential_count(active_admin().id()).await, 0);
    assert_eq!(access.active_credential_count(actor_id()).await, 0);
    assert_eq!(access.live_pending_count().await, 0);
    assert_eq!(
        access.session_count().await,
        1,
        "only the actor session remains"
    );
    assert_eq!(access.audit_count().await, 1, "only recovery is audited");

    access.close().await;
}
