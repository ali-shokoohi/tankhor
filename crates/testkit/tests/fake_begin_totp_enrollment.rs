//! T170: only a bound, authenticated ceremony may present provisioning once.
use admin::{
    application::enrollment::{BeginTotpEnrollment, BeginTotpEnrollmentInput},
    domain::{
        admin_state::{EnrollmentPurpose, SecurityEpoch},
        auth_state::{AuthPurpose, AuthStage, BindingDigest},
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        session_store::{PendingCheck, SessionStore},
    },
};
use kernel::ports::unit_of_work::{TransactionMode, UnitOfWork};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    audit_repository::FakeAuditRepository, clock::FixedClock, id_generator::SequenceIdGenerator,
    session_store::FakeSessionStore, store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/enrollment_fixture.rs"]
mod fixture;

#[tokio::test]
async fn totp_enrollment_secret_is_bound_and_presented_once() {
    for scenario in [
        "valid",
        "absent",
        "authorization_expired",
        "primary_expired",
        "browser",
        "session",
        "purpose",
        "stage",
        "target",
        "epoch",
        "reference",
        "audit",
    ] {
        drive(scenario).await;
    }
}
#[tokio::test]
async fn totp_provisioning_is_once_authenticated_and_no_store() {
    drive("valid").await;
}

#[expect(
    clippy::too_many_lines,
    reason = "independent binding refusals and one-time presentation assertions"
)]
async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = FixedClock::new(fixture::now());
    let ids = SequenceIdGenerator::new(100);
    store.seed(|state| {
        let target = if scenario == "authorization_expired" {
            fixture::target_until(true, fixture::now() + time::Duration::seconds(30))
        } else {
            fixture::target(scenario != "absent")
        };
        state.admins.insert(target.id(), target);
        state.admin_pending.insert([5; 32], fixture::pending());
    });
    if scenario == "authorization_expired" {
        clock.advance(time::Duration::seconds(30));
    }
    if scenario == "primary_expired" {
        clock.advance(time::Duration::MINUTE);
    }
    if scenario == "audit" {
        audit.fail_next_append();
    }
    let mut proof = fixture::check();
    match scenario {
        "browser" => proof.binding.browser = BindingDigest::new([99; 32]),
        "session" => {
            proof.binding.session = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
        }
        "purpose" => proof.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery),
        "stage" => proof.binding.stage = AuthStage::Totp,
        "target" => proof.binding.target = "019a53a0-0000-7000-8000-000000000099".parse().unwrap(),
        "epoch" => proof.binding.epoch = SecurityEpoch::new(1),
        "reference" => proof.reference = BindingDigest::new([99; 32]),
        _ => {}
    }
    let before = store.try_read_committed().unwrap();
    let service = BeginTotpEnrollment {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &fixture::policy(),
    };
    let result = service
        .execute(BeginTotpEnrollmentInput {
            proof: proof.clone(),
            predecessor: None,
        })
        .await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    let after = store.try_read_committed().unwrap();
    assert!(after.admin_sessions.is_empty());
    assert!(after.admin_credentials.is_empty());
    assert_eq!(after.admins, before.admins);
    if scenario != "valid" {
        assert_eq!(after.admin_pending, before.admin_pending);
        assert_eq!(after.audit_entries, before.audit_entries);
        return;
    }
    let result = result.unwrap();
    assert_ne!(result.provisioning, fixture::secret(""));
    assert!(result.no_store);
    assert_eq!(result.binding.stage, AuthStage::Totp);
    let debug = format!("{result:?}");
    assert!(!debug.contains("otpauth://"));
    assert!(!debug.contains("synthetic-provisioning-secret"));
    let reference = crypto
        .token_digest(TokenPurpose::Ceremony, result.token)
        .await
        .unwrap();
    let check = PendingCheck {
        reference,
        binding: result.binding,
        now: fixture::now(),
    };
    let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(
        sessions
            .read_pending(&mut tx, check.clone())
            .await
            .unwrap()
            .provisioning_presented
    );
    assert!(
        sessions
            .verification_state(&mut tx, check.clone())
            .await
            .is_ok()
    );
    assert!(
        sessions
            .present_provisioning(&mut tx, check.clone())
            .await
            .is_err()
    );
    uow.rollback(tx).await.unwrap();
    assert!(
        service
            .execute(BeginTotpEnrollmentInput {
                proof,
                predecessor: None
            })
            .await
            .is_err()
    );
    let repeated = store.try_read_committed().unwrap();
    assert_eq!(repeated.admin_pending, after.admin_pending);
    assert_eq!(repeated.audit_entries, after.audit_entries);
    assert_eq!(after.audit_entries.len(), 1);
    // A lost response is not redisplayed: a fresh phone proof creates fresh material
    // and supersedes the old ceremony in the same transaction.
    let mut fresh = fixture::check();
    fresh.reference = BindingDigest::new([77; 32]);
    let primary_input = admin::domain::auth_state::AuthInput {
        id: "019a53a0-0000-7000-8000-000000000077".parse().unwrap(),
        binding: fresh.binding.clone(),
        primary_verified_at: Some(fixture::now()),
        created_at: fixture::now(),
        expires_at: fixture::now() + time::Duration::MINUTE,
    };
    let fresh_pending = admin::ports::session_store::PendingRecord::new(
        fresh.reference.clone(),
        admin::domain::auth_state::AuthState::try_from(primary_input.clone()).unwrap(),
        None,
        None,
    )
    .unwrap();
    store.seed(|state| {
        state.admin_pending.insert([77; 32], fresh_pending);
    });
    let replacement = service
        .execute(BeginTotpEnrollmentInput {
            proof: fresh,
            predecessor: Some(check.clone()),
        })
        .await
        .unwrap();
    assert_ne!(replacement.provisioning, result.provisioning);
    let mut tx = uow.begin(TransactionMode::ReadOnly).await.unwrap();
    assert!(sessions.read_pending(&mut tx, check).await.is_err());
    uow.rollback(tx).await.unwrap();
    // Keep this fixture explicitly primary-only; no completed factor was required.
    assert!(
        store
            .try_read_committed()
            .unwrap()
            .admin_sessions
            .is_empty()
    );
}
