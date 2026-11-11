//! T157: final CAS and the session/audit transaction are one security boundary.
use admin::{
    application::passkey::{FinishPasskey, FinishPasskeyInput},
    domain::{
        audit::{Actor, AuditAction, AuditMetadata, Factor, ReasonCode, Subject},
        auth_state::AuthStage,
    },
    ports::{
        admin_authenticator::{MockAdminAuthenticator, VerifiedPasskey},
        admin_credential_repository::MockAdminCredentialRepository,
        admin_repository::MockAdminRepository,
        audit_repository::MockAuditRepository,
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
#[path = "support/passkey_fixture.rs"]
mod fixture;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Invalid,
    CasFailure,
    AuditFailure,
    Success,
}

#[expect(
    clippy::too_many_lines,
    reason = "one exact cross-port finalization sequence with independent failure injections"
)]
async fn drive(outcome: Outcome) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin()
        .times(if outcome == Outcome::Invalid { 1 } else { 2 })
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
        .times(usize::from(outcome != Outcome::Invalid))
        .returning(|_, ids| {
            assert_eq!(ids, vec![fixture::admin().id()]);
            Ok(vec![fixture::admin()])
        });
    let mut credentials = MockAdminCredentialRepository::new();
    credentials
        .expect_lock()
        .times(if outcome == Outcome::Invalid { 1 } else { 2 })
        .returning(|_, id| {
            assert_eq!(id, fixture::credential_id());
            Ok(Some(fixture::credential()))
        });
    let log = events.clone();
    credentials
        .expect_compare_revision()
        .times(usize::from(outcome != Outcome::Invalid))
        .returning(move |_, update| {
            assert_eq!(update.id, fixture::credential_id());
            assert_eq!(update.expected_revision, 1);
            assert_eq!(update.next_revision, 2);
            assert_eq!(update.epoch, fixture::admin().epoch());
            assert_eq!(
                update.credential,
                fixture::secret("synthetic-updated-credential")
            );
            log.lock().unwrap().push("credential-cas");
            if outcome == Outcome::CasFailure {
                Err(DomainError::conflict(ErrorCode::StaleRevision))
            } else {
                Ok(())
            }
        });
    let mut sessions = MockSessionStore::new();
    sessions
        .expect_read_pending()
        .times(if outcome == Outcome::Invalid { 1 } else { 2 })
        .returning(|_, check| {
            assert_eq!(check, fixture::check(AuthStage::Passkey));
            fixture::pending(AuthStage::Passkey).view(&check, &fixture::admin())
        });
    sessions
        .expect_verification_state()
        .times(1)
        .returning(|_, _| Ok(fixture::secret("synthetic-server-state")));
    let log = events.clone();
    sessions
        .expect_consume_pending()
        .times(usize::from(outcome != Outcome::Invalid))
        .returning(move |_, check| {
            assert_eq!(check, fixture::check(AuthStage::Passkey));
            log.lock().unwrap().push("state-cas");
            Ok(())
        });
    let log = events.clone();
    sessions
        .expect_insert()
        .times(usize::from(matches!(
            outcome,
            Outcome::Success | Outcome::AuditFailure
        )))
        .returning(move |_, record| {
            assert_eq!(record.state.factor(), Some(Factor::Passkey));
            assert!(
                record
                    .state
                    .check_access(&fixture::admin(), fixture::now())
                    .is_ok()
            );
            log.lock().unwrap().push("session");
            Ok(())
        });
    let mut crypto = MockAdminAuthenticator::new();
    let log = events.clone();
    crypto
        .expect_finish_passkey()
        .times(1)
        .returning(move |request| {
            assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
            assert_eq!(request.request, fixture::start());
            assert_eq!(
                request.protected_state,
                fixture::secret("synthetic-server-state")
            );
            log.lock().unwrap().push("verify");
            if outcome == Outcome::Invalid {
                Err(DomainError::unauthorised(ErrorCode::AuthenticationRequired))
            } else {
                Ok(VerifiedPasskey {
                    credential_id: fixture::secret("synthetic-credential-id"),
                    credential: fixture::secret("synthetic-updated-credential"),
                })
            }
        });
    crypto
        .expect_issue_token()
        .times(usize::from(outcome != Outcome::Invalid))
        .returning(|_| Ok(fixture::secret("synthetic-session")));
    crypto
        .expect_token_digest()
        .times(usize::from(outcome != Outcome::Invalid))
        .returning(|_, _| Ok(admin::domain::auth_state::BindingDigest::new([8; 32])));
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    audit
        .expect_append()
        .times(usize::from(matches!(
            outcome,
            Outcome::Success | Outcome::AuditFailure
        )))
        .returning(move |_, entry| {
            assert_eq!(entry.actor(), Actor::Admin(fixture::admin().id()));
            assert_eq!(entry.subject(), Subject::Admin(fixture::admin().id()));
            assert_eq!(entry.action(), AuditAction::SignedIn);
            assert_eq!(entry.occurred_at(), fixture::now());
            assert_eq!(entry.reason(), ReasonCode::Authentication);
            assert_eq!(entry.metadata(), &AuditMetadata::Factor(Factor::Passkey));
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
        .times(usize::from(outcome != Outcome::Invalid))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000009".parse().unwrap()));
    ids.expect_next_audit_id()
        .times(usize::from(outcome != Outcome::Invalid))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000010".parse().unwrap()));
    let service = FinishPasskey {
        uow: &uow,
        admins: &admins,
        credentials: &credentials,
        sessions: &sessions,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &fixture::policy(),
    };
    let result = service
        .execute(FinishPasskeyInput {
            proof: fixture::check(AuthStage::Passkey),
            credential: fixture::credential_id(),
            response: fixture::secret("synthetic-response"),
        })
        .await;
    assert_eq!(result.is_ok(), outcome == Outcome::Success);
    let log = events.lock().unwrap();
    let expected: &[&str] = match outcome {
        Outcome::Invalid => &["begin", "commit", "verify"],
        Outcome::CasFailure => &[
            "begin",
            "commit",
            "verify",
            "begin",
            "state-cas",
            "credential-cas",
            "rollback",
        ],
        Outcome::AuditFailure => &[
            "begin",
            "commit",
            "verify",
            "begin",
            "state-cas",
            "credential-cas",
            "session",
            "audit",
            "rollback",
        ],
        Outcome::Success => &[
            "begin",
            "commit",
            "verify",
            "begin",
            "state-cas",
            "credential-cas",
            "session",
            "audit",
            "commit",
        ],
    };
    assert_eq!(log.as_slice(), expected);
}
#[tokio::test]
async fn failed_second_factor_never_issues_session() {
    drive(Outcome::Invalid).await;
}
#[tokio::test]
async fn passkey_consumption_is_single_use_under_race_state_contract() {
    drive(Outcome::CasFailure).await;
}
#[tokio::test]
async fn audit_failure_aborts_access_state_contract() {
    drive(Outcome::AuditFailure).await;
}
#[tokio::test]
async fn audit_passkey_signin_has_complete_safe_fields_once() {
    drive(Outcome::Success).await;
}
