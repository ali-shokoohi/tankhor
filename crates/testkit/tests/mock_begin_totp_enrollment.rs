//! T169: commit the one-use presentation gate before decrypting provisioning.
use admin::{
    application::enrollment::{BeginTotpEnrollment, BeginTotpEnrollmentInput},
    domain::{
        audit::{Actor, AuditAction, AuditMetadata, Factor, Subject},
        auth_state::{AuthStage, BindingDigest},
    },
    ports::{
        admin_authenticator::{MockAdminAuthenticator, ProtectionContext, TotpEnrollment},
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
async fn totp_enrollment_secret_is_bound_and_presented_once_port_interactions() {
    for scenario in ["absent", "audit", "success", "decrypt", "unknown"] {
        drive(scenario).await;
    }
}

#[tokio::test]
async fn totp_provisioning_is_once_authenticated_and_no_store() {
    drive("success").await;
    drive("decrypt").await;
}

#[expect(
    clippy::too_many_lines,
    reason = "strict provisioning transaction and crypto order"
)]
async fn drive(scenario: &'static str) {
    let authorized = scenario != "absent";
    let committed = authorized && scenario != "audit";
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
        .times(usize::from(authorized) + usize::from(committed))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("commit");
            if scenario == "unknown" && log.lock().unwrap().contains(&"present") {
                Err(DomainError::unknown_commit(ErrorCode::CommitOutcomeUnknown))
            } else {
                Ok(())
            }
        });
    let log = events.clone();
    uow.expect_rollback()
        .times(usize::from(!committed))
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
        .times(usize::from(authorized))
        .returning(|_, ids| {
            assert_eq!(ids, vec![fixture::target(true).id()]);
            Ok(vec![fixture::target(true)])
        });
    let mut sessions = MockSessionStore::new();
    sessions
        .expect_read_pending()
        .times(if authorized { 2 } else { 0 })
        .returning(|_, check| {
            assert_eq!(check, fixture::check());
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
            assert_eq!(pending.state().binding().stage, AuthStage::Totp);
            assert_eq!(
                pending.state().binding().purpose,
                fixture::check().binding.purpose
            );
            assert_eq!(
                pending.state().expires_at(),
                fixture::pending().state().expires_at()
            );
            log.lock().unwrap().push("create");
            Ok(())
        });
    let log = events.clone();
    sessions
        .expect_present_provisioning()
        .times(usize::from(authorized))
        .returning(move |_, check| {
            assert_eq!(check.binding.stage, AuthStage::Totp);
            assert_eq!(check.binding.browser, fixture::check().binding.browser);
            assert_eq!(check.binding.session, fixture::check().binding.session);
            assert_eq!(check.binding.target, fixture::target(true).id());
            log.lock().unwrap().push("present");
            Ok(fixture::secret("protected-provisioning"))
        });
    let mut crypto = MockAdminAuthenticator::new();
    let log = events.clone();
    crypto
        .expect_begin_totp()
        .times(usize::from(authorized))
        .returning(move |context| {
            assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
            assert_eq!(context.binding.stage, AuthStage::Totp);
            assert!(context.credential.is_some());
            log.lock().unwrap().push("generate");
            Ok(TotpEnrollment {
                envelope: fixture::secret("protected-verifier"),
                protected_provisioning: fixture::secret("protected-provisioning"),
            })
        });
    crypto
        .expect_issue_token()
        .times(usize::from(authorized))
        .returning(|_| Ok(fixture::secret("opaque-reference")));
    crypto
        .expect_token_digest()
        .times(usize::from(authorized))
        .returning(|_, _| Ok(BindingDigest::new([8; 32])));
    let log = events.clone();
    crypto
        .expect_unprotect()
        .times(usize::from(committed && scenario != "unknown"))
        .returning(move |context, envelope| {
            assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
            let ProtectionContext::Ceremony(context) = context else {
                panic!("ceremony protection required")
            };
            assert_eq!(context.binding.stage, AuthStage::Totp);
            assert_eq!(envelope, fixture::secret("protected-provisioning"));
            log.lock().unwrap().push("decrypt");
            if scenario == "decrypt" {
                Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
            } else {
                Ok(fixture::secret("provisioning-canary"))
            }
        });
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    audit
        .expect_append()
        .times(usize::from(authorized))
        .returning(move |_, entry| {
            assert_eq!(entry.action(), AuditAction::EnrollmentStarted);
            assert_eq!(entry.actor(), Actor::Admin(fixture::target(true).id()));
            assert_eq!(entry.subject(), Subject::Admin(fixture::target(true).id()));
            assert_eq!(entry.metadata(), &AuditMetadata::Factor(Factor::Totp));
            assert!(!format!("{entry:?}").contains("provisioning-canary"));
            log.lock().unwrap().push("audit");
            if scenario == "audit" {
                Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
            } else {
                Ok(())
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
    let result = BeginTotpEnrollment {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &fixture::policy(),
    }
    .execute(BeginTotpEnrollmentInput {
        proof: fixture::check(),
        predecessor: None,
    })
    .await;
    assert_eq!(result.is_ok(), scenario == "success");
    if let Ok(result) = result {
        assert!(result.no_store);
        assert_eq!(result.provisioning, fixture::secret("provisioning-canary"));
        assert!(!format!("{result:?}").contains("provisioning-canary"));
    }
    let mut expected = vec!["begin"];
    if authorized {
        expected.extend([
            "commit",
            "generate",
            "begin",
            "consume",
            "create",
            "present",
            "audit",
            if committed { "commit" } else { "rollback" },
        ]);
        if committed && scenario != "unknown" {
            expected.push("decrypt");
        }
    } else {
        expected.push("rollback");
    }
    assert_eq!(*events.lock().unwrap(), expected);
}
