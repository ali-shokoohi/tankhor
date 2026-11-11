//! T175: another current administrator authorizes an audited identity, never a bearer grant.
use admin::{
    application::invite_admin::{InviteAdmin, InviteAdminInput},
    domain::{
        admin_state::{Eligibility, EnrollmentPurpose},
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
    },
    ports::{
        admin_authenticator::MockAdminAuthenticator,
        admin_repository::{IdentityDigest, MockAdminRepository},
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
#[path = "support/invite_fixture.rs"]
mod fixture;
#[tokio::test]
async fn admin_addition_requires_another_admin_and_audit_port_interactions() {
    for scenario in ["valid", "invalid_actor", "self", "reason", "audit"] {
        drive(scenario).await;
    }
}
#[tokio::test]
async fn audit_invitation_has_complete_safe_fields_once() {
    drive("valid").await;
    drive("audit").await;
}
#[expect(
    clippy::too_many_lines,
    reason = "strict complete invitation ordering and every abort boundary"
)]
async fn drive(scenario: &str) {
    let reason_valid = scenario != "reason";
    let actor_valid = scenario != "invalid_actor";
    let writable = reason_valid && actor_valid && scenario != "self";
    let success = scenario == "valid";
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut crypto = MockAdminAuthenticator::new();
    crypto
        .expect_identity_digest()
        .times(usize::from(reason_valid))
        .returning(|phone| {
            assert_eq!(phone, fixture::phone());
            Ok(IdentityDigest::new([2; 32]))
        });
    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin()
        .times(usize::from(reason_valid))
        .returning(move |mode| {
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
        .times(usize::from(reason_valid && !success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut sessions = MockSessionStore::new();
    let log = events.clone();
    sessions
        .expect_check_current()
        .times(usize::from(reason_valid))
        .returning(move |_, digest, now| {
            assert_eq!(digest, fixture::digest());
            assert_eq!(now, fixture::now());
            log.lock().unwrap().push("actor");
            if actor_valid {
                Ok(fixture::session(true))
            } else {
                Err(DomainError::unauthorised(ErrorCode::AuthenticationRequired))
            }
        });
    let mut admins = MockAdminRepository::new();
    let log = events.clone();
    admins
        .expect_find_by_identity()
        .times(usize::from(reason_valid && actor_valid))
        .returning(move |_, _| {
            log.lock().unwrap().push("identity");
            Ok((!writable).then(fixture::actor))
        });
    let log = events.clone();
    admins
        .expect_lock()
        .times(usize::from(writable))
        .returning(move |_, ids| {
            assert_eq!(ids, vec![fixture::actor().id()]);
            log.lock().unwrap().push("lock");
            Ok(vec![fixture::actor()])
        });
    let log = events.clone();
    admins
        .expect_invite()
        .times(usize::from(writable))
        .returning(move |_, state, identity| {
            assert_eq!(state.id(), fixture::target());
            assert!(!state.is_eligible());
            assert!(state.enrollment().is_none());
            assert_eq!(identity, IdentityDigest::new([2; 32]));
            log.lock().unwrap().push("create");
            Ok(())
        });
    let log = events.clone();
    admins
        .expect_authorize_enrollment()
        .times(usize::from(writable))
        .returning(move |_, command| {
            assert_eq!(command.actor, fixture::actor().id());
            assert_eq!(command.target, fixture::target());
            assert_eq!(command.purpose, EnrollmentPurpose::Addition);
            assert!(command.expected_authorization.is_none());
            assert_eq!(command.authorized_at, fixture::now());
            assert_eq!(
                command.expires_at,
                fixture::now() + time::Duration::minutes(5)
            );
            log.lock().unwrap().push("authorization");
            Ok(())
        });
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    audit
        .expect_append()
        .times(usize::from(writable))
        .returning(move |_, entry| {
            assert_eq!(entry.actor(), Actor::Admin(fixture::actor().id()));
            assert_eq!(entry.subject(), Subject::Admin(fixture::target()));
            assert_eq!(entry.action(), AuditAction::Invited);
            assert_eq!(entry.reason(), ReasonCode::AdminAddition);
            assert_eq!(entry.occurred_at(), fixture::now());
            assert_eq!(
                entry.metadata(),
                &AuditMetadata::Invitation {
                    before: None,
                    after: Eligibility::Invited
                }
            );
            assert!(!format!("{entry:?}").contains("98912"));
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
    ids.expect_next_admin_id()
        .times(usize::from(reason_valid))
        .returning(|| Ok(fixture::target()));
    ids.expect_next_audit_id()
        .times(usize::from(reason_valid))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000010".parse().unwrap()));
    let result = InviteAdmin {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &fixture::policy(),
    }
    .execute(InviteAdminInput {
        actor: fixture::digest(),
        phone: fixture::phone(),
        reason: if reason_valid {
            Some(ReasonCode::AdminAddition)
        } else {
            None
        },
    })
    .await;
    assert_eq!(result.is_ok(), success);
    let mut expected = Vec::new();
    if reason_valid {
        expected.extend(["begin", "actor"]);
        if actor_valid {
            expected.push("identity");
            if writable {
                expected.extend(["lock", "create", "authorization", "audit"]);
            }
        }
        expected.push(if success { "commit" } else { "rollback" });
    }
    assert_eq!(*events.lock().unwrap(), expected);
}
