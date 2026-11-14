//! T324: canonical OFF flag metadata without fabricated readiness and with no writes.
use admin::{
    application::list_flags::{ListFlags, ListFlagsInput},
    domain::{auth_state::BindingDigest, flag::FlagKey},
    ports::{flag_repository::FlagRecord, session_store::SessionRecord},
};
use testkit::fakes::{
    admin_repository::FakeAdminRepository, clock::FixedClock, flag_repository::FakeFlagRepository,
    session_store::FakeSessionStore, store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
async fn flag_registry_queries_return_safe_bounded_metadata() {
    let store = FakeStore::new();
    store.seed(|state| {
        state.admins.insert(fixture::admin().id(), fixture::admin());
        state.admin_sessions.insert(
            *fixture::digest().as_bytes(),
            SessionRecord {
                digest: fixture::digest(),
                state: fixture::session(),
            },
        );
        for version in fixture::flag_versions() {
            state
                .flags
                .insert(version.key(), FlagRecord::new(version.clone()));
            state.flag_history.insert(version.key(), vec![version]);
        }
    });
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let flags = FakeFlagRepository::new(store.clone());
    let clock = FixedClock::new(fixture::now());
    let service = ListFlags {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        flags: &flags,
        clock: &clock,
    };
    let before = store.try_read_committed().unwrap();
    let output = service
        .execute(ListFlagsInput {
            actor: fixture::digest(),
        })
        .await
        .unwrap();
    let keys = [
        FlagKey::PartnerWidgetApi,
        FlagKey::PercentageRollout,
        FlagKey::PerUserUsageLimits,
        FlagKey::Ticketing,
        FlagKey::ExternalNotifications,
        FlagKey::PaidShareBilling,
    ];
    assert_eq!(output.entries.len(), 6);
    for key in keys {
        let entry = output
            .entries
            .iter()
            .find(|entry| entry.key == key)
            .unwrap();
        assert!(!entry.value);
        assert_eq!(entry.label_key, format!("flag-{}-label", key.as_str()));
        assert_eq!(
            entry.description_key,
            format!("flag-{}-description", key.as_str())
        );
        assert_eq!(entry.effective_at, fixture::now());
        assert_eq!(entry.history.len(), 1);
        assert!(!entry.history[0].value);
        assert_eq!(entry.history[0].actor, fixture::admin().id());
        let safe = format!("{entry:?}");
        assert!(!safe.contains("Ready"));
        assert!(!safe.contains("readiness"));
    }
    assert!(
        service
            .execute(ListFlagsInput {
                actor: BindingDigest::new([24; 32])
            })
            .await
            .is_err()
    );
    let after = store.try_read_committed().unwrap();
    assert_eq!(after.flags, before.flags);
    assert_eq!(after.flag_history, before.flag_history);
    assert_eq!(after.admins, before.admins);
    assert_eq!(after.admin_sessions, before.admin_sessions);
    assert_eq!(after.audit_entries, before.audit_entries);
}
