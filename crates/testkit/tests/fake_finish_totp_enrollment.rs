//! T173: confirmation commits a matched step and cannot grant unaudited access.
use admin::{
    application::enrollment::{
        BeginTotpEnrollment, BeginTotpEnrollmentInput, FinishTotpEnrollment,
        FinishTotpEnrollmentInput,
    },
    application::totp::{FinishTotp, FinishTotpInput},
    domain::{
        admin_state::{
            Eligibility, EnrollmentPurpose, EnrollmentStatus, Invalidation, SecurityEpoch,
        },
        audit::{Actor, AuditAction, AuditMetadata, Factor, ReasonCode, Subject},
        auth_state::{AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        session_store::{PendingCheck, PendingRecord, SessionStore},
    },
};
use kernel::ports::unit_of_work::{TransactionMode, UnitOfWork};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator,
    admin_credential_repository::FakeAdminCredentialRepository,
    admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
    clock::FixedClock, id_generator::SequenceIdGenerator, session_store::FakeSessionStore,
    store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/totp_enrollment_fixture.rs"]
mod fixture;
#[tokio::test]
async fn enrollment_authorization_is_not_console_access() {
    for scenario in [
        "valid",
        "invalid",
        "browser",
        "session",
        "purpose",
        "stage",
        "target",
        "epoch",
        "reference",
        "expired",
        "authorization_revoked",
        "epoch_changed",
        "credential",
        "audit",
    ] {
        drive(scenario).await;
    }
}
#[tokio::test]
async fn audit_enrollment_has_complete_safe_fields_once() {
    drive("valid").await;
    drive("audit").await;
}
#[expect(
    clippy::too_many_lines,
    reason = "independent bound confirmation and full rollback assertions"
)]
async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let credentials = FakeAdminCredentialRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = FixedClock::new(fixture::now());
    let ids = SequenceIdGenerator::new(100);
    let policy = fixture::policy();
    store.seed(|state| {
        let target = fixture::target(true);
        state.admins.insert(target.id(), target);
        state.admin_pending.insert([5; 32], fixture::pending());
    });
    let started = BeginTotpEnrollment {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &policy,
    }
    .execute(BeginTotpEnrollmentInput {
        proof: fixture::check(),
        predecessor: None,
    })
    .await
    .unwrap();
    assert!(started.no_store);
    let reference = crypto
        .token_digest(TokenPurpose::Ceremony, started.token)
        .await
        .unwrap();
    let mut proof = PendingCheck {
        reference,
        binding: started.binding,
        now: fixture::now(),
    };
    let mut tx = uow.begin(TransactionMode::ReadOnly).await.unwrap();
    let envelope = sessions
        .verification_state(&mut tx, proof.clone())
        .await
        .unwrap();
    assert!(
        sessions
            .present_provisioning(&mut tx, proof.clone())
            .await
            .is_err()
    );
    uow.commit(tx).await.unwrap();
    let (code, step) = crypto.totp_fixture(&envelope, fixture::now()).unwrap();
    let mut credential = started.credential;
    match scenario {
        "browser" => proof.binding.browser = BindingDigest::new([99; 32]),
        "session" => {
            proof.binding.session = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
        }
        "purpose" => proof.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery),
        "stage" => proof.binding.stage = AuthStage::PrimaryVerified,
        "target" => proof.binding.target = "019a53a0-0000-7000-8000-000000000099".parse().unwrap(),
        "epoch" => proof.binding.epoch = SecurityEpoch::new(1),
        "reference" => proof.reference = BindingDigest::new([99; 32]),
        "credential" => credential = "019a53a0-0000-7000-8000-000000000099".parse().unwrap(),
        "expired" => clock.advance(time::Duration::MINUTE),
        "authorization_revoked" => store.seed(|state| {
            let admin = state.admins.get_mut(&fixture::target(true).id()).unwrap();
            admin.revoke_enrollment(admin.epoch()).unwrap();
        }),
        "epoch_changed" => store.seed(|state| {
            let admin = state.admins.get_mut(&fixture::target(true).id()).unwrap();
            admin
                .invalidate(
                    admin.epoch(),
                    admin.epoch().advance().unwrap(),
                    Invalidation::Recover,
                )
                .unwrap();
        }),
        "audit" => audit.fail_next_append(),
        _ => {}
    }
    let before = store.try_read_committed().unwrap();
    let service = FinishTotpEnrollment {
        uow: &uow,
        admins: &admins,
        credentials: &credentials,
        sessions: &sessions,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &policy,
    };
    let input = FinishTotpEnrollmentInput {
        proof,
        credential,
        code: if scenario == "invalid" {
            fixture::secret("000000")
        } else {
            code.clone()
        },
    };
    let result = service.execute(input.clone()).await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    let after = store.try_read_committed().unwrap();
    if scenario == "valid" {
        let target = &after.admins[&fixture::target(true).id()];
        assert!(target.is_eligible());
        assert_eq!(
            target.enrollment().unwrap().status(),
            EnrollmentStatus::Consumed
        );
        assert_eq!(after.admin_credentials.len(), 1);
        let saved = &after.admin_credentials[&credential];
        assert_eq!(saved.state().last_totp_step(), Some(step));
        assert_eq!(saved.state().epoch(), target.epoch());
        assert_eq!(after.admin_sessions.len(), 1);
        let session = after.admin_sessions.values().next().unwrap();
        assert_eq!(session.state.factor(), Some(Factor::Totp));
        assert!(session.state.check_access(target, fixture::now()).is_ok());
        assert_eq!(
            session.digest,
            crypto
                .token_digest(TokenPurpose::Session, result.unwrap().token)
                .await
                .unwrap()
        );
        let events: Vec<_> = after
            .audit_entries
            .values()
            .filter(|entry| entry.action() == AuditAction::EnrollmentConfirmed)
            .collect();
        assert_eq!(events.len(), 1);
        let entry = events[0];
        assert_eq!(entry.actor(), Actor::Admin(target.id()));
        assert_eq!(entry.subject(), Subject::Admin(target.id()));
        assert_eq!(entry.occurred_at(), fixture::now());
        assert_eq!(entry.reason(), ReasonCode::Authentication);
        assert_eq!(
            entry.metadata(),
            &AuditMetadata::Enrollment {
                before: Eligibility::Invited,
                after: Eligibility::Active,
                factor: Factor::Totp
            }
        );
        assert!(!format!("{entry:?}").contains("synthetic"));
        assert!(!format!("{entry:?}").contains("654321"));
        assert!(service.execute(input).await.is_err());
        let repeated = store.try_read_committed().unwrap();
        assert_eq!(repeated.admin_sessions, after.admin_sessions);
        assert_eq!(repeated.audit_entries, after.audit_entries);
        assert_eq!(repeated.admin_credentials, after.admin_credentials);
        // A fresh primary proof cannot reuse the enrollment confirmation's accepted step.
        let mut fresh = fixture::check();
        fresh.reference = BindingDigest::new([77; 32]);
        fresh.binding.purpose = AuthPurpose::SignIn;
        store.seed(|state| {
            state.admin_pending.insert(
                [77; 32],
                PendingRecord::new(
                    fresh.reference.clone(),
                    AuthState::try_from(AuthInput {
                        id: "019a53a0-0000-7000-8000-000000000077".parse().unwrap(),
                        binding: fresh.binding.clone(),
                        primary_verified_at: Some(fixture::now()),
                        created_at: fixture::now(),
                        expires_at: fixture::now() + time::Duration::MINUTE,
                    })
                    .unwrap(),
                    None,
                    None,
                )
                .unwrap(),
            );
        });
        let before_replay = store.try_read_committed().unwrap();
        assert!(
            FinishTotp {
                uow: &uow,
                admins: &admins,
                credentials: &credentials,
                sessions: &sessions,
                audit: &audit,
                authenticator: &crypto,
                clock: &clock,
                ids: &ids,
                policy: &policy
            }
            .execute(FinishTotpInput {
                proof: fresh,
                credential,
                code
            })
            .await
            .is_err()
        );
        let replay = store.try_read_committed().unwrap();
        assert_eq!(replay.admin_sessions, before_replay.admin_sessions);
        assert_eq!(replay.audit_entries, before_replay.audit_entries);
        assert_eq!(replay.admin_pending, before_replay.admin_pending);
        assert_eq!(replay.admin_credentials, before_replay.admin_credentials);
    } else {
        assert_eq!(after.admins, before.admins);
        assert_eq!(after.admin_pending, before.admin_pending);
        assert_eq!(after.admin_credentials, before.admin_credentials);
        assert_eq!(after.admin_sessions, before.admin_sessions);
        assert_eq!(after.audit_entries, before.audit_entries);
    }
}
