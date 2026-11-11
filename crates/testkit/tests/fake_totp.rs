//! T161: committed-state assertions independent of the interaction driver.
use admin::{
    application::totp::{FinishTotp, FinishTotpInput},
    domain::{
        admin_state::{EnrollmentPurpose, Invalidation},
        audit::{Actor, AuditAction, AuditMetadata, Factor, ReasonCode, Subject},
        auth_state::{AuthPurpose, AuthStage, BindingDigest},
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, MockAdminAuthenticator, TokenPurpose},
        admin_credential_repository::CredentialRecord,
    },
};
use kernel::error::{DomainError, ErrorCode};
use std::sync::Arc;
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator,
    admin_credential_repository::FakeAdminCredentialRepository,
    admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
    clock::FixedClock, id_generator::SequenceIdGenerator, session_store::FakeSessionStore,
    store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/totp_fixture.rs"]
mod fixture;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Success,
    Invalid,
    Expired,
    WrongBrowser,
    WrongSession,
    WrongPurpose,
    WrongStage,
    InvalidPrimary,
    Revoked,
    ForeignAdminEnvelope,
    ForeignCredentialEnvelope,
    Epoch,
    ReplayedStep,
    Audit,
    StepRace,
    ExpiryRace,
}

#[expect(
    clippy::too_many_lines,
    reason = "independent security scenarios compare complete committed access state"
)]
async fn drive(outcome: Outcome) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let credentials = FakeAdminCredentialRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = Arc::new(FixedClock::new(fixture::now()));
    let ids = SequenceIdGenerator::new(100);
    let policy = fixture::policy();
    // Enrollment A is not the independently authorized sign-in proof B.
    let mut enrollment_context = fixture::context();
    enrollment_context.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery);
    enrollment_context.binding.browser = BindingDigest::new([71; 32]);
    enrollment_context.binding.session = "019a53a0-0000-7000-8000-000000000071".parse().unwrap();
    enrollment_context.challenge = "019a53a0-0000-7000-8000-000000000072".parse().unwrap();
    enrollment_context.primary_verified_at -= time::Duration::seconds(30);
    enrollment_context.created_at -= time::Duration::seconds(30);
    enrollment_context.expires_at = fixture::now() - time::Duration::SECOND;
    if outcome == Outcome::ForeignAdminEnvelope {
        enrollment_context.binding.target = "019a53a0-0000-7000-8000-000000000073".parse().unwrap();
    }
    if outcome == Outcome::ForeignCredentialEnvelope {
        enrollment_context.credential =
            Some("019a53a0-0000-7000-8000-000000000074".parse().unwrap());
    }
    let enrollment = crypto.begin_totp(enrollment_context).await.unwrap();
    let (code, step) = crypto
        .totp_fixture(&enrollment.envelope, fixture::now())
        .unwrap();
    let mut credential = fixture::credential(enrollment.envelope);
    if outcome == Outcome::Revoked {
        let mut state = credential.state().clone();
        state.revoke(fixture::now());
        credential = CredentialRecord::new(state, credential.material().clone()).unwrap();
    }
    if outcome == Outcome::ReplayedStep {
        let mut state = credential.state().clone();
        // Enrollment confirmation consumes the same step as subsequent sign-in.
        state
            .accept_totp_step(fixture::admin().epoch(), 1, step)
            .unwrap();
        credential = CredentialRecord::new(state, credential.material().clone()).unwrap();
    }
    store.seed(|state| {
        let mut admin = fixture::admin();
        if outcome == Outcome::Epoch {
            admin
                .invalidate(
                    admin.epoch(),
                    admin.epoch().advance().unwrap(),
                    Invalidation::Recover,
                )
                .unwrap();
        }
        state.admins.insert(admin.id(), admin);
        state
            .admin_credentials
            .insert(fixture::credential_id(), credential);
        state.admin_pending.insert([5; 32], fixture::pending());
    });
    if outcome == Outcome::Expired {
        clock.advance(time::Duration::MINUTE);
    }
    if outcome == Outcome::Audit {
        audit.fail_next_append();
    }
    let mut proof = fixture::check();
    if outcome == Outcome::WrongBrowser {
        proof.binding.browser = BindingDigest::new([99; 32]);
    }
    if outcome == Outcome::WrongSession {
        proof.binding.session = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
    }
    if outcome == Outcome::WrongPurpose {
        proof.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery);
    }
    if outcome == Outcome::WrongStage {
        proof.binding.stage = AuthStage::Totp;
    }
    if outcome == Outcome::InvalidPrimary {
        proof.reference = BindingDigest::new([99; 32]);
    }
    let input = FinishTotpInput {
        proof,
        credential: fixture::credential_id(),
        code: if outcome == Outcome::Invalid {
            fixture::secret("000000")
        } else {
            code
        },
    };
    assert!(!format!("{input:?}").contains("654321"));
    let before = store.try_read_committed().unwrap();
    let mut racing = MockAdminAuthenticator::new();
    let racing_store = store.clone();
    let racing_clock = clock.clone();
    if matches!(outcome, Outcome::StepRace | Outcome::ExpiryRace) {
        racing.expect_verify_totp().times(1).returning(move |_| {
            if outcome == Outcome::StepRace {
                racing_store.seed(|state| {
                    let record = &state.admin_credentials[&fixture::credential_id()];
                    let mut current = record.state().clone();
                    current
                        .accept_totp_step(fixture::admin().epoch(), 1, step)
                        .unwrap();
                    let material = record.material().clone();
                    state.admin_credentials.insert(
                        fixture::credential_id(),
                        CredentialRecord::new(current, material).unwrap(),
                    );
                });
            } else {
                racing_clock.advance(time::Duration::MINUTE);
            }
            Ok(step)
        });
        racing
            .expect_issue_token()
            .times(1)
            .returning(|_| Ok(fixture::secret("synthetic-token")));
        racing
            .expect_token_digest()
            .times(1)
            .returning(|_, _| Ok(BindingDigest::new([8; 32])));
    }
    let authenticator: &dyn AdminAuthenticator =
        if matches!(outcome, Outcome::StepRace | Outcome::ExpiryRace) {
            &racing
        } else {
            &crypto
        };
    let service = FinishTotp {
        uow: &uow,
        admins: &admins,
        credentials: &credentials,
        sessions: &sessions,
        audit: &audit,
        authenticator,
        clock: clock.as_ref(),
        ids: &ids,
        policy: &policy,
    };
    let result = service.execute(input.clone()).await;
    assert_eq!(result.is_ok(), outcome == Outcome::Success, "{outcome:?}");
    let after = store.try_read_committed().unwrap();
    if outcome == Outcome::Success {
        assert_eq!(after.admin_sessions.len(), 1);
        let session = after.admin_sessions.values().next().unwrap();
        session
            .state
            .check_access(&fixture::admin(), fixture::now())
            .unwrap();
        assert_eq!(session.state.factor(), Some(Factor::Totp));
        assert_eq!(
            after.admin_credentials[&fixture::credential_id()]
                .state()
                .last_totp_step(),
            Some(step)
        );
        assert_eq!(after.audit_entries.len(), 1);
        let entry = after.audit_entries.values().next().unwrap();
        assert_eq!(entry.actor(), Actor::Admin(fixture::admin().id()));
        assert_eq!(entry.subject(), Subject::Admin(fixture::admin().id()));
        assert_eq!(entry.action(), AuditAction::SignedIn);
        assert_eq!(entry.reason(), ReasonCode::Authentication);
        assert_eq!(entry.occurred_at(), fixture::now());
        assert_eq!(entry.metadata(), &AuditMetadata::Factor(Factor::Totp));
        assert!(!format!("{entry:?}").contains("synthetic"));
        let token = result.unwrap().token;
        assert_eq!(
            crypto
                .token_digest(TokenPurpose::Session, token)
                .await
                .unwrap(),
            session.digest
        );
        assert!(service.execute(input).await.is_err());
        let repeated = store.try_read_committed().unwrap();
        assert_eq!(repeated.admin_sessions, after.admin_sessions);
        assert_eq!(repeated.audit_entries, after.audit_entries);
        assert_eq!(repeated.admin_credentials, after.admin_credentials);
    } else {
        assert_eq!(after.admin_sessions, before.admin_sessions);
        assert_eq!(after.audit_entries, before.audit_entries);
        assert_eq!(after.admin_pending, before.admin_pending);
        if outcome == Outcome::StepRace {
            assert_eq!(
                after.admin_credentials[&fixture::credential_id()]
                    .state()
                    .last_totp_step(),
                Some(step)
            );
        } else {
            assert_eq!(after.admin_credentials, before.admin_credentials);
        }
        if outcome == Outcome::Audit {
            // Failure did not consume proof or step: retry commits the complete boundary.
            assert!(service.execute(input).await.is_ok());
        }
    }
    let safe_error = DomainError::unauthorised(ErrorCode::AuthenticationRequired);
    assert!(!format!("{safe_error:?}").contains("654321"));
}
#[tokio::test]
async fn enrolled_totp_works_with_independent_fresh_primary_proof() {
    drive(Outcome::Success).await;
}
#[tokio::test]
async fn totp_fallback_rejects_replayed_or_expired_proof() {
    for outcome in [
        Outcome::Success,
        Outcome::Invalid,
        Outcome::Expired,
        Outcome::WrongBrowser,
        Outcome::WrongSession,
        Outcome::WrongPurpose,
        Outcome::WrongStage,
        Outcome::InvalidPrimary,
        Outcome::Revoked,
        Outcome::ForeignAdminEnvelope,
        Outcome::ForeignCredentialEnvelope,
        Outcome::Epoch,
        Outcome::ReplayedStep,
        Outcome::Audit,
    ] {
        drive(outcome).await;
    }
}
#[tokio::test]
async fn otp_and_totp_consumption_are_single_use_under_race_state_contract() {
    drive(Outcome::StepRace).await;
    drive(Outcome::ExpiryRace).await;
}
#[tokio::test]
async fn audit_totp_signin_has_complete_safe_fields_once() {
    drive(Outcome::Success).await;
}
