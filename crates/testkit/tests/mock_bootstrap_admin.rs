//! T193: initial operator provisioning is one audited, restricted transition.
use admin::{
    application::bootstrap_admin::{BootstrapAdmin, BootstrapAdminInput, OperatorContext},
    domain::{
        admin_state::{Eligibility, EnrollmentPurpose, EnrollmentStatus},
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
    },
    ports::{
        admin_authenticator::MockAdminAuthenticator,
        admin_repository::{AdminPage, IdentityDigest, MockAdminRepository},
        audit_repository::MockAuditRepository,
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
#[allow(dead_code)]
#[path = "support/invite_fixture.rs"]
mod fixture;

#[tokio::test]
async fn bootstrap_admin_uses_ports_and_one_audited_uow() {
    for scenario in ["valid", "unauthorized", "repeated", "self", "audit"] {
        drive(scenario).await;
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "complete operator precondition, audit and rollback interaction checks"
)]
async fn drive(scenario: &str) {
    let authorized = scenario != "unauthorized";
    let first = scenario != "repeated";
    let distinct = scenario != "self";
    let success = scenario == "valid";
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut crypto = MockAdminAuthenticator::new();
    crypto
        .expect_identity_digest()
        .times(usize::from(authorized))
        .returning(|phone| {
            assert_eq!(phone, fixture::phone());
            Ok(IdentityDigest::new([2; 32]))
        });
    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin()
        .times(usize::from(authorized))
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
        .times(usize::from(authorized && !success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut admins = MockAdminRepository::new();
    let log = events.clone();
    admins
        .expect_list()
        .times(usize::from(authorized))
        .returning(move |_, query| {
            assert_eq!(query.after(), None);
            assert_eq!(query.limit(), 1);
            log.lock().unwrap().push("precondition");
            Ok(AdminPage {
                entries: if first {
                    vec![]
                } else {
                    vec![fixture::actor()]
                },
                next: None,
            })
        });
    let log = events.clone();
    admins
        .expect_invite()
        .times(usize::from(authorized && first && distinct))
        .returning(move |_, state, identity| {
            assert_eq!(state.id(), fixture::target());
            assert!(!state.is_eligible());
            assert_eq!(identity, IdentityDigest::new([2; 32]));
            let authorization = state.enrollment().unwrap();
            assert_eq!(authorization.purpose(), EnrollmentPurpose::Addition);
            assert_eq!(authorization.authorizer(), fixture::actor().id());
            assert_eq!(authorization.status(), EnrollmentStatus::Pending);
            assert_eq!(authorization.authorized_at(), fixture::now());
            assert_eq!(
                authorization.expires_at(),
                fixture::now() + time::Duration::minutes(5)
            );
            log.lock().unwrap().push("create");
            Ok(())
        });
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    audit
        .expect_append()
        .times(usize::from(authorized && first && distinct))
        .returning(move |_, entry| {
            assert_eq!(entry.actor(), Actor::Admin(fixture::actor().id()));
            assert_eq!(entry.subject(), Subject::Admin(fixture::target()));
            assert_eq!(entry.action(), AuditAction::Invited);
            assert_eq!(entry.reason(), ReasonCode::AdminAddition);
            assert_eq!(
                entry.metadata(),
                &AuditMetadata::Invitation {
                    before: None,
                    after: Eligibility::Invited,
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
        .times(usize::from(authorized))
        .returning(|| Ok(fixture::target()));
    ids.expect_next_audit_id()
        .times(usize::from(authorized))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000010".parse().unwrap()));
    let operator = if distinct {
        fixture::actor().id()
    } else {
        fixture::target()
    };
    let result = BootstrapAdmin {
        uow: &uow,
        admins: &admins,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &fixture::policy(),
    }
    .execute(BootstrapAdminInput {
        operator: OperatorContext::new(operator, authorized),
        phone: fixture::phone(),
    })
    .await;
    assert_eq!(result.is_ok(), success, "{scenario}");
    let mut expected = Vec::new();
    if authorized {
        expected.extend(["begin", "precondition"]);
        if first && distinct {
            expected.extend(["create", "audit"]);
        }
        expected.push(if success { "commit" } else { "rollback" });
    }
    assert_eq!(*events.lock().unwrap(), expected);
}
