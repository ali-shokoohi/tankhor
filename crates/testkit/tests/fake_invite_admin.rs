//! T176: duplicate/self identities and incomplete actors cannot create console access.
use admin::{
    application::invite_admin::{InviteAdmin, InviteAdminInput},
    domain::{
        admin_state::{Eligibility, EnrollmentPurpose, EnrollmentStatus},
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
    },
    ports::session_store::SessionRecord,
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    audit_repository::FakeAuditRepository, clock::FixedClock, id_generator::SequenceIdGenerator,
    session_store::FakeSessionStore, store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/invite_fixture.rs"]
mod fixture;
#[tokio::test]
async fn admin_addition_requires_another_admin_and_audit() {
    for scenario in [
        "valid",
        "self",
        "duplicate",
        "incomplete",
        "expired",
        "reason",
        "wrong_reason",
        "audit",
    ] {
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
    reason = "complete attribution and unchanged-state checks for independent invitation refusals"
)]
async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = FixedClock::new(fixture::now());
    let ids = SequenceIdGenerator::new(100);
    let policy = fixture::policy();
    store.seed(|state| {
        state.admins.insert(fixture::actor().id(), fixture::actor());
        state
            .admin_identities
            .insert([1; 32], fixture::actor().id());
        state.admin_sessions.insert(
            *fixture::digest().as_bytes(),
            SessionRecord {
                digest: fixture::digest(),
                state: fixture::session(scenario != "incomplete"),
            },
        );
        if scenario == "duplicate" {
            state.admins.insert(
                fixture::target(),
                admin::domain::admin_state::AdminState::new(
                    fixture::target(),
                    Eligibility::Invited,
                    fixture::actor().epoch(),
                ),
            );
            state.admin_identities.insert([2; 32], fixture::target());
        }
    });
    if scenario == "expired" {
        clock.advance(time::Duration::MINUTE);
    }
    if scenario == "audit" {
        audit.fail_next_append();
    }
    let before = store.try_read_committed().unwrap();
    let input = InviteAdminInput {
        actor: fixture::digest(),
        phone: if scenario == "self" {
            admin::domain::secrets::SecretBytes::new(b"+989120000001".to_vec())
        } else {
            fixture::phone()
        },
        reason: match scenario {
            "reason" => None,
            "wrong_reason" => Some(ReasonCode::AdminRecovery),
            _ => Some(ReasonCode::AdminAddition),
        },
    };
    assert!(!format!("{input:?}").contains("98912"));
    let service = InviteAdmin {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
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
        assert!(!target.is_eligible());
        let authorization = target.enrollment().unwrap();
        assert_eq!(authorization.purpose(), EnrollmentPurpose::Addition);
        assert_eq!(authorization.status(), EnrollmentStatus::Pending);
        assert_eq!(authorization.authorizer(), fixture::actor().id());
        assert_eq!(
            authorization.expires_at(),
            fixture::now() + time::Duration::minutes(5)
        );
        assert_eq!(after.admin_sessions.len(), 1);
        assert!(after.admin_credentials.is_empty());
        assert!(after.admin_pending.is_empty());
        assert_eq!(after.audit_entries.len(), 1);
        let entry = after.audit_entries.values().next().unwrap();
        assert_eq!(entry.actor(), Actor::Admin(fixture::actor().id()));
        assert_eq!(entry.subject(), Subject::Admin(id));
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
        assert!(service.execute(input).await.is_err());
        let repeated = store.try_read_committed().unwrap();
        assert_eq!(repeated.admins, after.admins);
        assert_eq!(repeated.audit_entries, after.audit_entries);
    } else {
        assert_eq!(after.admins, before.admins);
        assert_eq!(after.admin_identities, before.admin_identities);
        assert_eq!(after.admin_sessions, before.admin_sessions);
        assert_eq!(after.audit_entries, before.audit_entries);
    }
}
