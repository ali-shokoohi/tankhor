//! T196: enrollment-page reads validate proof and protected authorization without writes.
use admin::{
    application::{
        get_enrollment::{EnrollmentFactor, GetEnrollment, GetEnrollmentInput},
        verify_admin_otp::PrimaryProof,
    },
    domain::{
        auth_state::{AuthStage, BindingDigest},
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::{MockAdminAuthenticator, TokenPurpose},
        admin_repository::MockAdminRepository,
        session_store::{MockSessionStore, PendingCheck},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::{
        clock::MockClock,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
use std::sync::{Arc, Mutex};
#[path = "support/enrollment_fixture.rs"]
#[allow(dead_code)]
mod fixture;

#[tokio::test]
async fn get_enrollment_validates_authorization_and_primary_proof_without_writes() {
    for scenario in [
        "valid",
        "missing_authorization",
        "wrong_stage",
        "wrong_session",
        "wrong_browser",
    ] {
        drive(scenario).await;
    }
}

#[expect(clippy::too_many_lines)]
async fn drive(scenario: &str) {
    let success = scenario == "valid";
    let reads = !matches!(scenario, "wrong_stage" | "wrong_browser");
    let authorized = scenario != "missing_authorization";
    let store = StoreId::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut crypto = MockAdminAuthenticator::new();
    crypto
        .expect_token_digest()
        .times(if scenario == "wrong_stage" { 0 } else { 2 })
        .returning(|purpose, token| match purpose {
            TokenPurpose::PrimaryProof => {
                assert_eq!(token, fixture::secret("synthetic-primary-proof"));
                Ok(fixture::check().reference)
            }
            TokenPurpose::Browser => {
                assert_eq!(token, fixture::secret("synthetic-browser"));
                Ok(BindingDigest::new([3; 32]))
            }
            _ => panic!("unexpected token purpose"),
        });
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin()
        .times(usize::from(reads))
        .returning(move |mode| {
            assert_eq!(mode, TransactionMode::ReadOnly);
            log.lock().unwrap().push("begin");
            Ok(TxHandle::new(store, mode, ()))
        });
    let log = events.clone();
    uow.expect_commit()
        .times(usize::from(success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("commit");
            Ok(())
        });
    let log = events.clone();
    uow.expect_rollback()
        .times(usize::from(reads && !success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut admins = MockAdminRepository::new();
    let log = events.clone();
    admins
        .expect_get()
        .times(usize::from(reads))
        .returning(move |_, target| {
            assert_eq!(target, fixture::target(false).id());
            log.lock().unwrap().push("target");
            Ok(Some(fixture::target(authorized)))
        });
    let log = events.clone();
    admins
        .expect_enrollment()
        .times(usize::from(reads))
        .returning(move |_, target| {
            assert_eq!(target, fixture::target(false).id());
            log.lock().unwrap().push("authorization");
            Ok(fixture::target(authorized).enrollment().cloned())
        });
    let wrong_session = scenario == "wrong_session";
    let mut sessions = MockSessionStore::new();
    let log = events.clone();
    sessions
        .expect_read_pending()
        .times(usize::from(reads && authorized))
        .returning(move |_, check: PendingCheck| {
            log.lock().unwrap().push("proof");
            if wrong_session {
                assert_ne!(check.binding.session, fixture::check().binding.session);
                return Err(DomainError::forbidden(ErrorCode::PermissionDenied));
            }
            assert_eq!(check, fixture::check());
            fixture::pending().view(&fixture::check(), &fixture::target(true))
        });
    let mut clock = MockClock::new();
    clock.expect_now().returning(|| Ok(fixture::now()));
    let mut binding = fixture::check().binding;
    if scenario == "wrong_stage" {
        binding.stage = AuthStage::Passkey;
    }
    if scenario == "wrong_session" {
        binding.session = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
    }
    if scenario == "wrong_browser" {
        binding.browser = BindingDigest::new([99; 32]);
    }
    let result = GetEnrollment {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        authenticator: &crypto,
        clock: &clock,
    }
    .execute(GetEnrollmentInput {
        proof: PrimaryProof {
            token: SecretBytes::new(b"synthetic-primary-proof".to_vec()),
            binding,
        },
        browser: SecretBytes::new(b"synthetic-browser".to_vec()),
    })
    .await;
    assert_eq!(result.is_ok(), success, "{scenario}");
    if success {
        let view = result.unwrap();
        assert_eq!(view.target, fixture::target(false).id());
        assert_eq!(
            view.factors,
            [EnrollmentFactor::Passkey, EnrollmentFactor::Totp]
        );
        assert!(!format!("{view:?}").contains("synthetic"));
    }
    let mut expected = Vec::new();
    if reads {
        expected.extend(["begin", "target", "authorization"]);
        if authorized {
            expected.push("proof");
        }
        expected.push(if success { "commit" } else { "rollback" });
    }
    assert_eq!(*events.lock().unwrap(), expected);
}
