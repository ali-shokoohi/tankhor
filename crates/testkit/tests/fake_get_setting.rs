//! T327: exact known setting identity and history with read-only actor isolation.
use admin::{
    application::get_setting::{GetSetting, GetSettingInput},
    domain::{
        auth_state::BindingDigest,
        settings::{SettingKey, SettingValue},
    },
    ports::{session_store::SessionRecord, settings_repository::SettingRecord},
};
use kernel::error::ErrorCode;
use testkit::fakes::{
    admin_repository::FakeAdminRepository, clock::FixedClock, session_store::FakeSessionStore,
    settings_repository::FakeSettingsRepository, store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
async fn get_setting_requires_exact_known_key() {
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
        for version in fixture::versions() {
            state
                .settings
                .insert(version.key(), SettingRecord::new(version.clone()));
            state.setting_history.insert(version.key(), vec![version]);
        }
    });
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let settings = FakeSettingsRepository::new(store.clone());
    let clock = FixedClock::new(fixture::now());
    let service = GetSetting {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        settings: &settings,
        clock: &clock,
    };
    let before = store.try_read_committed().unwrap();
    let output = service
        .execute(GetSettingInput {
            actor: fixture::digest(),
            key: "upload_max_mb".into(),
        })
        .await
        .unwrap();
    assert_eq!(output.key, SettingKey::UploadMaxMb);
    assert_eq!(output.value, SettingValue::Number(15));
    assert_eq!(output.history.len(), 1);
    assert_eq!(output.history[0].value, SettingValue::Number(15));
    assert_eq!(output.history[0].actor, fixture::admin().id());

    let unknown = service
        .execute(GetSettingInput {
            actor: fixture::digest(),
            key: "upload-max-mb".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), ErrorCode::InvalidValue);
    assert!(
        service
            .execute(GetSettingInput {
                actor: BindingDigest::new([24; 32]),
                key: "upload_max_mb".into()
            })
            .await
            .is_err()
    );
    let after = store.try_read_committed().unwrap();
    assert_eq!(after.settings, before.settings);
    assert_eq!(after.setting_history, before.setting_history);
    assert_eq!(after.admins, before.admins);
    assert_eq!(after.admin_sessions, before.admin_sessions);
    assert_eq!(after.audit_entries, before.audit_entries);
}
