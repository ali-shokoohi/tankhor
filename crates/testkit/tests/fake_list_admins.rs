//! T179: distinct safe attribution, equal permissions, bounded keysets, no writes.
use admin::{
    application::list_admins::{ListAdmins, ListAdminsInput},
    domain::admin_state::{AdminState, Eligibility, EnrollmentPurpose, Invalidation, Permissions},
    ports::session_store::SessionRecord,
};
use testkit::fakes::{
    admin_repository::FakeAdminRepository, clock::FixedClock, session_store::FakeSessionStore,
    store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/list_admins_fixture.rs"]
mod fixture;
#[tokio::test]
async fn admins_have_equal_permissions_and_distinct_attribution() {
    for scenario in [
        "valid",
        "incomplete",
        "expired",
        "epoch",
        "ineligible",
        "missing_admin",
        "missing_session",
        "cursor",
        "limit",
    ] {
        drive(scenario).await;
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "independent access refusals, complete page checks and unchanged committed collections"
)]
async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let clock = FixedClock::new(fixture::now());
    store.seed(|state| {
        for n in 1..=3 {
            state
                .admins
                .insert(fixture::admin(n).id(), fixture::admin(n));
            state
                .admin_identities
                .insert([n; 32], fixture::admin(n).id());
        }
        let mut target = AdminState::new(
            fixture::admin(4).id(),
            Eligibility::Invited,
            fixture::admin(4).epoch(),
        );
        target
            .authorize_enrollment(
                &fixture::admin(1),
                EnrollmentPurpose::Addition,
                target.epoch(),
                fixture::now(),
                fixture::now() + time::Duration::minutes(5),
            )
            .unwrap();
        state.admins.insert(target.id(), target);
        if scenario != "missing_session" {
            state.admin_sessions.insert(
                *fixture::digest().as_bytes(),
                SessionRecord {
                    digest: fixture::digest(),
                    state: fixture::session(scenario != "incomplete"),
                },
            );
        }
        if matches!(scenario, "epoch" | "ineligible") {
            let actor = state.admins.get_mut(&fixture::admin(1).id()).unwrap();
            actor
                .invalidate(
                    actor.epoch(),
                    actor.epoch().advance().unwrap(),
                    Invalidation::Deactivate,
                )
                .unwrap();
            if scenario == "epoch" {
                actor.restore(actor.epoch()).unwrap();
            }
        }
        if scenario == "missing_admin" {
            state.admins.remove(&fixture::admin(1).id());
        }
    });
    if scenario == "expired" {
        clock.advance(time::Duration::MINUTE);
    }
    let before = store.try_read_committed().unwrap();
    let service = ListAdmins {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        clock: &clock,
    };
    let mut input = ListAdminsInput {
        actor: fixture::digest(),
        cursor: (scenario == "cursor").then(|| "not-a-cursor".into()),
        limit: if scenario == "limit" { 0 } else { 2 },
    };
    let result = service.execute(input.clone()).await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    if scenario == "valid" {
        let first = result.unwrap();
        assert_eq!(
            first.entries.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![fixture::admin(1).id(), fixture::admin(2).id()]
        );
        assert!(
            first
                .entries
                .iter()
                .all(|row| row.permissions == Some(Permissions::Full))
        );
        assert_eq!(first.next, Some(fixture::admin(2).id().to_string()));
        input.cursor = first.next.clone();
        let second = service.execute(input.clone()).await.unwrap();
        assert_eq!(
            second.entries.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![fixture::admin(3).id(), fixture::admin(4).id()]
        );
        assert_eq!(second.entries[0].permissions, Some(Permissions::Full));
        assert_eq!(second.entries[1].permissions, None);
        let enrollment = second.entries[1].enrollment.as_ref().unwrap();
        assert_eq!(enrollment.purpose, EnrollmentPurpose::Addition);
        assert_eq!(enrollment.authorizer, fixture::admin(1).id());
        assert!(second.next.is_none());
        input.cursor = Some(fixture::admin(99).id().to_string());
        assert!(service.execute(input).await.unwrap().entries.is_empty());
        let safe = format!("{first:?}{second:?}");
        for forbidden in [
            "IdentityDigest",
            "CredentialMaterial",
            "SecretBytes",
            "98912",
        ] {
            assert!(!safe.contains(forbidden));
        }
    }
    let after = store.try_read_committed().unwrap();
    assert_eq!(after.admins, before.admins);
    assert_eq!(after.admin_identities, before.admin_identities);
    assert_eq!(after.admin_sessions, before.admin_sessions);
    assert_eq!(after.admin_pending, before.admin_pending);
    assert_eq!(after.admin_credentials, before.admin_credentials);
    assert_eq!(after.audit_entries, before.audit_entries);
}
