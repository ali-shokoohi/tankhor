//! T172: accepted enrollment step, activation, session and audit are one transaction.
use admin::{
    application::enrollment::{FinishTotpEnrollment, FinishTotpEnrollmentInput},
    domain::{
        admin_state::Eligibility,
        audit::{Actor, AuditAction, AuditMetadata, Factor, ReasonCode, Subject},
        auth_state::{AuthInput, AuthStage, AuthState, BindingDigest},
    },
    ports::{
        admin_authenticator::MockAdminAuthenticator,
        admin_credential_repository::{CredentialMaterial, MockAdminCredentialRepository},
        admin_repository::MockAdminRepository,
        audit_repository::MockAuditRepository,
        session_store::{MockSessionStore, PendingRecord},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::{
        clock::MockClock,
        id_generator::MockIdGenerator,
        unit_of_work::{MockUnitOfWork, StoreId, TxHandle},
    },
};
use std::sync::{Arc, Mutex};
#[path = "support/totp_enrollment_fixture.rs"]
mod fixture;
fn pending() -> PendingRecord {
    let mut binding = fixture::check().binding;
    binding.stage = AuthStage::Totp;
    binding.credential_revision = Some(1);
    PendingRecord::new(
        fixture::check().reference,
        AuthState::try_from(AuthInput {
            id: fixture::pending().state().id(),
            binding,
            primary_verified_at: Some(fixture::now()),
            created_at: fixture::now(),
            expires_at: fixture::pending().state().expires_at(),
        })
        .unwrap(),
        Some(fixture::secret("protected-verifier")),
        None,
    )
    .unwrap()
}
#[tokio::test]
async fn enrollment_authorization_is_not_console_access_port_interactions() {
    for outcome in ["valid", "invalid", "absent", "step", "audit"] {
        drive(outcome).await;
    }
}
#[tokio::test]
async fn audit_enrollment_has_complete_safe_fields_once() {
    drive("valid").await;
    drive("audit").await;
}
#[expect(
    clippy::too_many_lines,
    reason = "complete strict enrollment ordering and abort driver"
)]
async fn drive(outcome: &str) {
    let authorized = outcome != "absent";
    let valid = authorized && outcome != "invalid";
    let after_step = valid && outcome != "step";
    let success = outcome == "valid";
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin()
        .times(if valid { 2 } else { 1 })
        .returning(move |mode| {
            log.lock().unwrap().push("begin");
            Ok(TxHandle::new(store, mode, ()))
        });
    let log = events.clone();
    uow.expect_commit()
        .times(usize::from(authorized) + usize::from(success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("commit");
            Ok(())
        });
    let log = events.clone();
    uow.expect_rollback()
        .times(usize::from(
            !authorized || outcome == "step" || outcome == "audit",
        ))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut admins = MockAdminRepository::new();
    admins
        .expect_get()
        .times(1)
        .returning(move |_, _| Ok(Some(fixture::target(authorized))));
    admins
        .expect_lock()
        .times(usize::from(valid))
        .returning(|_, _| Ok(vec![fixture::target(true)]));
    let log = events.clone();
    admins
        .expect_consume_enrollment()
        .times(usize::from(valid))
        .returning(move |_, id, auth, now| {
            assert_eq!(id, fixture::target(true).id());
            assert_eq!(&auth, fixture::target(true).enrollment().unwrap());
            assert_eq!(now, fixture::now());
            log.lock().unwrap().push("authorization");
            Ok(())
        });
    let log = events.clone();
    admins
        .expect_restore()
        .times(usize::from(after_step))
        .returning(move |_, _, _| {
            log.lock().unwrap().push("activate");
            Ok(())
        });
    let mut sessions = MockSessionStore::new();
    sessions
        .expect_read_pending()
        .times(usize::from(authorized) + usize::from(valid))
        .returning(|_, check| pending().view(&check, &fixture::target(true)));
    sessions
        .expect_verification_state()
        .times(usize::from(authorized))
        .returning(|_, _| Ok(fixture::secret("protected-verifier")));
    let log = events.clone();
    sessions
        .expect_consume_pending()
        .times(usize::from(valid))
        .returning(move |_, _| {
            log.lock().unwrap().push("proof");
            Ok(())
        });
    let log = events.clone();
    sessions
        .expect_insert()
        .times(usize::from(after_step))
        .returning(move |_, record| {
            let mut active = fixture::target(true);
            active.restore(active.epoch()).unwrap();
            assert!(record.state.check_access(&active, fixture::now()).is_ok());
            assert_eq!(record.state.factor(), Some(Factor::Totp));
            log.lock().unwrap().push("session");
            Ok(())
        });
    let mut credentials = MockAdminCredentialRepository::new();
    let log = events.clone();
    credentials
        .expect_insert()
        .times(usize::from(valid))
        .returning(move |_, record| {
            assert_eq!(record.state().admin_id(), fixture::target(true).id());
            assert_eq!(record.state().revision(), 1);
            assert_eq!(
                record.material(),
                &CredentialMaterial::Totp {
                    envelope: fixture::secret("protected-verifier")
                }
            );
            log.lock().unwrap().push("credential");
            Ok(())
        });
    let log = events.clone();
    credentials
        .expect_accept_totp_step()
        .times(usize::from(valid))
        .returning(move |_, id, epoch, revision, step| {
            assert_eq!(id, "019a53a0-0000-7000-8000-000000000010".parse().unwrap());
            assert_eq!(epoch, fixture::target(true).epoch());
            assert_eq!(revision, 1);
            assert_eq!(step, 42);
            log.lock().unwrap().push("step");
            if after_step {
                Ok(())
            } else {
                Err(DomainError::unauthorised(ErrorCode::AuthenticationRequired))
            }
        });
    let mut crypto = MockAdminAuthenticator::new();
    let log = events.clone();
    crypto
        .expect_verify_totp()
        .times(usize::from(authorized))
        .returning(move |request| {
            assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
            assert_eq!(&request.context.binding, pending().state().binding());
            assert_eq!(request.envelope, fixture::secret("protected-verifier"));
            assert_eq!(request.code, fixture::secret("654321"));
            assert_eq!(request.now, fixture::now());
            assert!(!format!("{request:?}").contains("654321"));
            log.lock().unwrap().push("verify");
            if valid {
                Ok(42)
            } else {
                Err(DomainError::unauthorised(ErrorCode::AuthenticationRequired))
            }
        });
    crypto
        .expect_issue_token()
        .times(usize::from(valid))
        .returning(|_| Ok(fixture::secret("opaque-session")));
    crypto
        .expect_token_digest()
        .times(usize::from(valid))
        .returning(|_, _| Ok(BindingDigest::new([8; 32])));
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    audit
        .expect_append()
        .times(usize::from(after_step))
        .returning(move |_, entry| {
            assert_eq!(entry.action(), AuditAction::EnrollmentConfirmed);
            assert_eq!(entry.actor(), Actor::Admin(fixture::target(true).id()));
            assert_eq!(entry.subject(), Subject::Admin(fixture::target(true).id()));
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
            assert!(!format!("{entry:?}").contains("protected-verifier"));
            log.lock().unwrap().push("audit");
            if success {
                Ok(())
            } else {
                Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
            }
        });
    let mut clock = MockClock::new();
    clock.expect_now().returning(|| Ok(fixture::now()));
    let mut ids = MockIdGenerator::new();
    ids.expect_next_session_id()
        .times(usize::from(valid))
        .returning(|| Ok(fixture::check().binding.session));
    ids.expect_next_audit_id()
        .times(usize::from(valid))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000012".parse().unwrap()));
    let mut proof = fixture::check();
    proof.binding = pending().state().binding().clone();
    let result = FinishTotpEnrollment {
        uow: &uow,
        admins: &admins,
        credentials: &credentials,
        sessions: &sessions,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &fixture::policy(),
    }
    .execute(FinishTotpEnrollmentInput {
        proof,
        credential: "019a53a0-0000-7000-8000-000000000010".parse().unwrap(),
        code: fixture::secret("654321"),
    })
    .await;
    assert_eq!(result.is_ok(), success);
    let mut expected = vec!["begin"];
    if authorized {
        expected.extend(["commit", "verify"]);
        if valid {
            expected.extend(["begin", "proof", "authorization", "credential", "step"]);
            if after_step {
                expected.extend(["activate", "session", "audit"]);
            }
            expected.push(if success { "commit" } else { "rollback" });
        }
    } else {
        expected.push("rollback");
    }
    assert_eq!(*events.lock().unwrap(), expected);
}
