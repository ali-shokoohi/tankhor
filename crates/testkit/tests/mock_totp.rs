//! T160: strict interaction driver for the TOTP sign-in security boundary.
use admin::{
    application::totp::{FinishTotp, FinishTotpInput},
    domain::audit::{Actor, AuditAction, AuditMetadata, Factor, ReasonCode, Subject},
    ports::{
        admin_authenticator::MockAdminAuthenticator,
        admin_credential_repository::MockAdminCredentialRepository,
        admin_repository::MockAdminRepository, audit_repository::MockAuditRepository,
        session_store::MockSessionStore,
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
#[path = "support/totp_fixture.rs"]
mod fixture;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Success,
    Invalid,
    CasFailure,
    AuditFailure,
}

#[expect(
    clippy::too_many_lines,
    reason = "strict cross-port order and independent failure injection"
)]
async fn drive(outcome: Outcome) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let valid = outcome != Outcome::Invalid;
    let writes = matches!(outcome, Outcome::Success | Outcome::AuditFailure);
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
        .times(if outcome == Outcome::Success { 2 } else { 1 })
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("commit");
            Ok(())
        });
    let log = events.clone();
    uow.expect_rollback()
        .times(usize::from(matches!(
            outcome,
            Outcome::CasFailure | Outcome::AuditFailure
        )))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut admins = MockAdminRepository::new();
    admins.expect_get().times(1).returning(|_, id| {
        assert_eq!(id, fixture::admin().id());
        Ok(Some(fixture::admin()))
    });
    admins
        .expect_lock()
        .times(usize::from(valid))
        .returning(|_, ids| {
            assert_eq!(ids, vec![fixture::admin().id()]);
            Ok(vec![fixture::admin()])
        });
    let mut credentials = MockAdminCredentialRepository::new();
    credentials
        .expect_lock()
        .times(if valid { 2 } else { 1 })
        .returning(|_, id| {
            assert_eq!(id, fixture::credential_id());
            Ok(Some(fixture::credential(fixture::secret(
                "synthetic-envelope",
            ))))
        });
    let log = events.clone();
    credentials
        .expect_accept_totp_step()
        .times(usize::from(valid))
        .returning(move |_, id, epoch, revision, step| {
            assert_eq!(
                (id, epoch, revision, step),
                (fixture::credential_id(), fixture::admin().epoch(), 1, 42)
            );
            log.lock().unwrap().push("step-cas");
            if outcome == Outcome::CasFailure {
                Err(DomainError::conflict(ErrorCode::StaleRevision))
            } else {
                Ok(())
            }
        });
    let mut sessions = MockSessionStore::new();
    sessions
        .expect_read_pending()
        .times(if valid { 2 } else { 1 })
        .returning(|_, check| {
            assert_eq!(check, fixture::check());
            fixture::pending().view(&check, &fixture::admin())
        });
    let log = events.clone();
    sessions
        .expect_consume_pending()
        .times(usize::from(valid))
        .returning(move |_, check| {
            assert_eq!(check, fixture::check());
            log.lock().unwrap().push("proof-cas");
            Ok(())
        });
    let log = events.clone();
    sessions
        .expect_insert()
        .times(usize::from(writes))
        .returning(move |_, record| {
            assert_eq!(record.state.factor(), Some(Factor::Totp));
            record
                .state
                .check_access(&fixture::admin(), fixture::now())
                .unwrap();
            log.lock().unwrap().push("session");
            Ok(())
        });
    let mut crypto = MockAdminAuthenticator::new();
    let log = events.clone();
    crypto
        .expect_verify_totp()
        .times(1)
        .returning(move |request| {
            assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
            assert_eq!(request.context, fixture::context());
            assert_eq!(request.envelope, fixture::secret("synthetic-envelope"));
            assert_eq!(request.code, fixture::secret("654321"));
            assert!(!format!("{request:?}").contains("654321"));
            log.lock().unwrap().push("verify");
            if outcome == Outcome::Invalid {
                Err(DomainError::unauthorised(ErrorCode::AuthenticationRequired))
            } else {
                Ok(42)
            }
        });
    crypto
        .expect_issue_token()
        .times(usize::from(valid))
        .returning(|_| Ok(fixture::secret("synthetic-session")));
    crypto
        .expect_token_digest()
        .times(usize::from(valid))
        .returning(|_, _| Ok(admin::domain::auth_state::BindingDigest::new([8; 32])));
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    audit
        .expect_append()
        .times(usize::from(writes))
        .returning(move |_, entry| {
            assert_eq!(entry.actor(), Actor::Admin(fixture::admin().id()));
            assert_eq!(entry.subject(), Subject::Admin(fixture::admin().id()));
            assert_eq!(entry.action(), AuditAction::SignedIn);
            assert_eq!(entry.occurred_at(), fixture::now());
            assert_eq!(entry.reason(), ReasonCode::Authentication);
            assert_eq!(entry.metadata(), &AuditMetadata::Factor(Factor::Totp));
            assert!(!format!("{entry:?}").contains("synthetic"));
            log.lock().unwrap().push("audit");
            if outcome == Outcome::AuditFailure {
                Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
            } else {
                Ok(())
            }
        });
    let mut clock = MockClock::new();
    clock.expect_now().returning(|| Ok(fixture::now()));
    let mut ids = MockIdGenerator::new();
    ids.expect_next_session_id()
        .times(usize::from(valid))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000009".parse().unwrap()));
    ids.expect_next_audit_id()
        .times(usize::from(valid))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000010".parse().unwrap()));
    let result = FinishTotp {
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
    .execute(FinishTotpInput {
        proof: fixture::check(),
        credential: fixture::credential_id(),
        code: fixture::secret("654321"),
    })
    .await;
    assert_eq!(result.is_ok(), outcome == Outcome::Success);
    let expected: &[&str] = match outcome {
        Outcome::Invalid => &["begin", "commit", "verify"],
        Outcome::CasFailure => &[
            "begin",
            "commit",
            "verify",
            "begin",
            "proof-cas",
            "step-cas",
            "rollback",
        ],
        Outcome::AuditFailure => &[
            "begin",
            "commit",
            "verify",
            "begin",
            "proof-cas",
            "step-cas",
            "session",
            "audit",
            "rollback",
        ],
        Outcome::Success => &[
            "begin",
            "commit",
            "verify",
            "begin",
            "proof-cas",
            "step-cas",
            "session",
            "audit",
            "commit",
        ],
    };
    assert_eq!(events.lock().unwrap().as_slice(), expected);
}
#[tokio::test]
async fn totp_step_and_session_audit_commit_once() {
    drive(Outcome::Success).await;
    drive(Outcome::Invalid).await;
    drive(Outcome::AuditFailure).await;
}
#[tokio::test]
async fn otp_and_totp_consumption_are_single_use_under_race_state_contract() {
    drive(Outcome::CasFailure).await;
}
#[tokio::test]
async fn audit_totp_signin_has_complete_safe_fields_once() {
    drive(Outcome::Success).await;
}
