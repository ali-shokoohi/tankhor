//! T163: protected target authorization and primary proof precede ceremony writes.
use admin::{
    application::enrollment::{BeginPasskeyEnrollment, BeginPasskeyEnrollmentInput},
    domain::{
        audit::{Actor, AuditAction, AuditMetadata, Factor, Subject},
        auth_state::{AuthStage, BindingDigest},
    },
    ports::{
        admin_authenticator::{MockAdminAuthenticator, PasskeyChallenge},
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
#[path = "support/enrollment_fixture.rs"]
mod fixture;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "strict begin-enrollment port sequence and rollback scenarios"
)]
async fn passkey_enrollment_authorization_is_not_console_access_port_interactions() {
    for (authorized, audit_ok) in [(false, true), (true, false), (true, true)] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let store = StoreId::new();
        let mut uow = MockUnitOfWork::new();
        let log = events.clone();
        uow.expect_begin()
            .times(if authorized { 2 } else { 1 })
            .returning(move |mode| {
                log.lock().unwrap().push("begin");
                Ok(TxHandle::new(store, mode, ()))
            });
        let log = events.clone();
        uow.expect_commit()
            .times(if authorized && audit_ok {
                2
            } else {
                usize::from(authorized)
            })
            .returning(move |mut tx| {
                tx.take::<()>(store)?;
                log.lock().unwrap().push("commit");
                Ok(())
            });
        let log = events.clone();
        uow.expect_rollback()
            .times(usize::from(!authorized || !audit_ok))
            .returning(move |mut tx| {
                tx.take::<()>(store)?;
                log.lock().unwrap().push("rollback");
                Ok(())
            });
        let mut admins = MockAdminRepository::new();
        let log = events.clone();
        admins.expect_get().times(1).returning(move |_, id| {
            assert_eq!(id, fixture::target(false).id());
            log.lock().unwrap().push("authority");
            Ok(Some(fixture::target(authorized)))
        });
        let log = events.clone();
        admins
            .expect_lock()
            .times(usize::from(authorized))
            .returning(move |_, ids| {
                assert_eq!(ids, vec![fixture::target(false).id()]);
                log.lock().unwrap().push("recheck");
                Ok(vec![fixture::target(true)])
            });
        let mut sessions = MockSessionStore::new();
        let log = events.clone();
        sessions
            .expect_read_pending()
            .times(if authorized { 2 } else { 0 })
            .returning(move |_, check| {
                assert_eq!(check, fixture::check());
                log.lock().unwrap().push("proof");
                fixture::pending().view(&check, &fixture::target(true))
            });
        let log = events.clone();
        sessions
            .expect_consume_pending()
            .times(usize::from(authorized))
            .returning(move |_, check| {
                assert_eq!(check, fixture::check());
                log.lock().unwrap().push("consume");
                Ok(())
            });
        let log = events.clone();
        sessions
            .expect_create_pending()
            .times(usize::from(authorized))
            .returning(move |_, pending| {
                assert_eq!(pending.state().binding().stage, AuthStage::Passkey);
                assert_eq!(
                    pending.state().binding().purpose,
                    fixture::check().binding.purpose
                );
                assert_eq!(
                    pending.state().expires_at(),
                    fixture::pending().state().expires_at()
                );
                log.lock().unwrap().push("ceremony");
                Ok(())
            });
        let mut crypto = MockAdminAuthenticator::new();
        let log = events.clone();
        crypto
            .expect_begin_passkey()
            .times(usize::from(authorized))
            .returning(move |request| {
                assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
                assert_eq!(request.context.binding.target, fixture::target(false).id());
                assert!(request.credentials.is_empty());
                assert_eq!(request.rp_id, "console.example.test");
                assert_eq!(request.origin, "https://console.example.test");
                assert_eq!(request.context.binding.stage, AuthStage::Passkey);
                assert!(request.context.credential.is_some());
                log.lock().unwrap().push("crypto");
                Ok(PasskeyChallenge {
                    public_options: fixture::secret("public-options"),
                    protected_state: fixture::secret("protected-state"),
                })
            });
        crypto
            .expect_issue_token()
            .times(usize::from(authorized))
            .returning(|_| Ok(fixture::secret("opaque-token")));
        crypto
            .expect_token_digest()
            .times(usize::from(authorized))
            .returning(|_, _| Ok(BindingDigest::new([8; 32])));
        let mut audit = MockAuditRepository::new();
        let log = events.clone();
        audit
            .expect_append()
            .times(usize::from(authorized))
            .returning(move |_, entry| {
                assert_eq!(entry.action(), AuditAction::EnrollmentStarted);
                assert_eq!(entry.actor(), Actor::Admin(fixture::target(false).id()));
                assert_eq!(entry.subject(), Subject::Admin(fixture::target(false).id()));
                assert_eq!(entry.metadata(), &AuditMetadata::Factor(Factor::Passkey));
                log.lock().unwrap().push("audit");
                if audit_ok {
                    Ok(())
                } else {
                    Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
                }
            });
        let mut clock = MockClock::new();
        clock.expect_now().returning(|| Ok(fixture::now()));
        let mut ids = MockIdGenerator::new();
        ids.expect_next_credential_id()
            .times(usize::from(authorized))
            .returning(|| Ok("019a53a0-0000-7000-8000-000000000010".parse().unwrap()));
        ids.expect_next_challenge_id()
            .times(usize::from(authorized))
            .returning(|| Ok("019a53a0-0000-7000-8000-000000000011".parse().unwrap()));
        ids.expect_next_audit_id()
            .times(usize::from(authorized))
            .returning(|| Ok("019a53a0-0000-7000-8000-000000000012".parse().unwrap()));
        let result = BeginPasskeyEnrollment {
            uow: &uow,
            admins: &admins,
            sessions: &sessions,
            audit: &audit,
            authenticator: &crypto,
            clock: &clock,
            ids: &ids,
            policy: &fixture::policy(),
        }
        .execute(BeginPasskeyEnrollmentInput {
            proof: fixture::check(),
        })
        .await;
        assert_eq!(result.is_ok(), authorized && audit_ok);
        let mut expected = vec!["begin", "authority"];
        if authorized {
            expected.extend([
                "proof",
                "commit",
                "crypto",
                "begin",
                "recheck",
                "proof",
                "consume",
                "ceremony",
                "audit",
                if audit_ok { "commit" } else { "rollback" },
            ]);
        } else {
            expected.push("rollback");
        }
        assert_eq!(*events.lock().unwrap(), expected);
    }
}
