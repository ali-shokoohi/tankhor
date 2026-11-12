//! T194: bootstrap creates restricted first-admin eligibility, never recovery/access.
use admin::{
    application::bootstrap_admin::{BootstrapAdmin, BootstrapAdminInput, OperatorContext},
    domain::{
        admin_state::{
            AdminState, Eligibility, EnrollmentPurpose, EnrollmentStatus, SecurityEpoch,
        },
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
    },
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    audit_repository::FakeAuditRepository, clock::FixedClock, id_generator::SequenceIdGenerator,
    store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[allow(dead_code)]
#[path = "support/invite_fixture.rs"]
mod fixture;

#[tokio::test]
async fn bootstrap_admin_creates_restricted_eligibility_not_recovery_or_access() {
    for scenario in [
        "valid",
        "unauthorized",
        "repeated",
        "self_recovery",
        "audit",
    ] {
        drive(scenario).await;
    }
}

async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = FixedClock::new(fixture::now());
    let ids = SequenceIdGenerator::new(100);
    let policy = fixture::policy();
    if scenario == "repeated" {
        store.seed(|state| {
            state.admins.insert(fixture::actor().id(), fixture::actor());
        });
    }
    if scenario == "self_recovery" {
        store.seed(|state| {
            state.admins.insert(
                fixture::target(),
                AdminState::new(
                    fixture::target(),
                    Eligibility::Recovering,
                    SecurityEpoch::new(4),
                ),
            );
        });
    }
    if scenario == "audit" {
        audit.fail_next_append();
    }
    let before = store.try_read_committed().unwrap();
    let input = BootstrapAdminInput {
        operator: OperatorContext::new(fixture::actor().id(), scenario != "unauthorized"),
        phone: fixture::phone(),
    };
    assert!(!format!("{input:?}").contains("98912"));
    let service = BootstrapAdmin {
        uow: &uow,
        admins: &admins,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &policy,
    };
    let result = service.execute(input.clone()).await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    let after = store.try_read_committed().unwrap();
    if scenario == "valid" {
        let id = result.unwrap();
        let target = &after.admins[&id];
        assert_eq!(target.id(), id);
        assert!(!target.is_eligible());
        let authorization = target.enrollment().unwrap();
        assert_eq!(authorization.purpose(), EnrollmentPurpose::Addition);
        assert_eq!(authorization.authorizer(), fixture::actor().id());
        assert_eq!(authorization.status(), EnrollmentStatus::Pending);
        assert!(after.admin_sessions.is_empty());
        assert!(after.admin_pending.is_empty());
        assert!(after.admin_credentials.is_empty());
        assert_eq!(after.audit_entries.len(), 1);
        let entry = after.audit_entries.values().next().unwrap();
        assert_eq!(entry.actor(), Actor::Admin(fixture::actor().id()));
        assert_eq!(entry.subject(), Subject::Admin(id));
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
        assert!(service.execute(input).await.is_err());
        let repeated = store.try_read_committed().unwrap();
        assert_eq!(repeated.admins, after.admins);
        assert_eq!(repeated.audit_entries, after.audit_entries);
    } else {
        assert_eq!(after.admins, before.admins);
        assert_eq!(after.admin_identities, before.admin_identities);
        assert_eq!(after.admin_sessions, before.admin_sessions);
        assert_eq!(after.admin_pending, before.admin_pending);
        assert_eq!(after.admin_credentials, before.admin_credentials);
        assert_eq!(after.audit_entries, before.audit_entries);
    }
}
