//! T185: unrelated cookies, incomplete proofs and stale epochs never grant access.
use admin::{
    application::console_access::VerifyConsoleAccess,
    domain::{admin_state::Invalidation, audit::AuditAction, secrets::SecretBytes},
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        session_store::SessionRecord,
    },
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    clock::FixedClock, session_store::FakeSessionStore, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
#[path = "support/list_admins_fixture.rs"]
mod fixture;
#[path = "support/recovery_fixture.rs"]
mod recovery_fixture;

#[tokio::test]
async fn console_rejects_non_admin_access() {
    for scenario in [
        "valid",
        "missing",
        "incomplete",
        "expired",
        "epoch",
        "revoked",
        "missing_admin",
        "revoked_session",
    ] {
        drive(scenario).await;
    }
}
#[tokio::test]
async fn end_user_session_does_not_grant_admin_access() {
    drive("end_user").await;
}
#[tokio::test]
async fn unauditable_admin_action_is_not_reported_successful() {
    use admin::application::recover_admin::{RecoverAdmin, RecoverAdminInput};
    use admin::domain::audit::ReasonCode;
    use testkit::fakes::{
        admin_credential_repository::FakeAdminCredentialRepository,
        audit_repository::FakeAuditRepository, id_generator::SequenceIdGenerator,
        otp_challenge_repository::FakeOtpChallengeRepository,
    };
    let store = FakeStore::new();
    store.seed(|state| {
        let actor = recovery_fixture::actor();
        state.admins.insert(actor.id(), actor.clone());
        state
            .admins
            .insert(recovery_fixture::target().id(), recovery_fixture::target());
        state.admin_sessions.insert(
            [7; 32],
            SessionRecord {
                digest: recovery_fixture::digest(7),
                state: recovery_fixture::session(&actor, 20, true),
            },
        );
    });
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let credentials = FakeAdminCredentialRepository::new(store.clone());
    let otp = FakeOtpChallengeRepository::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let clock = FixedClock::new(recovery_fixture::now());
    let ids = SequenceIdGenerator::new(100);
    let policy = recovery_fixture::policy();
    audit.fail_next_append();
    let before = store.try_read_committed().unwrap();
    let result = RecoverAdmin {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        credentials: &credentials,
        otp: &otp,
        audit: &audit,
        clock: &clock,
        ids: &ids,
        policy: &policy,
    }
    .execute(RecoverAdminInput {
        actor: recovery_fixture::digest(7),
        target: recovery_fixture::target().id(),
        reason: Some(ReasonCode::AdminRecovery),
    })
    .await;
    assert!(result.is_err());
    let after = store.try_read_committed().unwrap();
    assert_eq!(before.admins, after.admins);
    assert_eq!(before.admin_sessions, after.admin_sessions);
    assert!(after.audit_entries.is_empty());
}

async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = FixedClock::new(fixture::now());
    let cookie = crypto.issue_token(TokenPurpose::Session).await.unwrap();
    let digest = crypto
        .token_digest(TokenPurpose::Session, cookie.clone())
        .await
        .unwrap();
    assert_ne!(
        digest,
        fixture::digest(),
        "use the authenticator-derived session digest, not the unrelated fixture reference"
    );
    store.seed(|state| {
        let mut admin = fixture::admin(1);
        if matches!(scenario, "epoch" | "revoked") {
            admin
                .invalidate(
                    admin.epoch(),
                    admin.epoch().advance().unwrap(),
                    Invalidation::Revoke,
                )
                .unwrap();
            if scenario == "epoch" {
                admin.restore(admin.epoch()).unwrap();
            }
        }
        if scenario != "missing_admin" {
            state.admins.insert(admin.id(), admin);
        }
        if scenario != "missing" {
            let mut session = fixture::session(scenario != "incomplete");
            if scenario == "revoked_session" {
                session.revoke(fixture::now());
            }
            state.admin_sessions.insert(
                *digest.as_bytes(),
                SessionRecord {
                    digest: digest.clone(),
                    state: session,
                },
            );
        }
    });
    if scenario == "expired" {
        clock.advance(time::Duration::MINUTE);
    }
    let before = store.try_read_committed().unwrap();
    let result = VerifyConsoleAccess {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        authenticator: &crypto,
        clock: &clock,
    }
    .execute(if scenario == "end_user" {
        SecretBytes::new(b"synthetic-end-user-cookie".to_vec())
    } else {
        cookie
    })
    .await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    if scenario == "valid" {
        let actor = result.unwrap();
        assert_eq!(actor.admin, fixture::admin(1).id());
        assert_eq!(actor.epoch, fixture::admin(1).epoch());
        assert_eq!(actor.session, fixture::session(true).id());
    }
    let after = store.try_read_committed().unwrap();
    assert_eq!(before.admins, after.admins);
    assert_eq!(before.admin_sessions, after.admin_sessions);
    assert_eq!(before.admin_credentials, after.admin_credentials);
    assert_eq!(before.admin_pending, after.admin_pending);
    assert_eq!(before.admin_otps, after.admin_otps);
    assert_eq!(before.audit_entries, after.audit_entries);
    assert!(
        !after
            .audit_entries
            .values()
            .any(|entry| entry.action() == AuditAction::SignedIn)
    );
}
