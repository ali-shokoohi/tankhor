//! T321: complete owned setting presentation metadata and read-only actor isolation.
use admin::{
    application::list_settings::{ListSettings, ListSettingsInput},
    domain::{
        auth_state::BindingDigest,
        settings::{SettingKey, SettingUnit, SettingValue},
    },
    ports::{session_store::SessionRecord, settings_repository::SettingRecord},
};
use testkit::fakes::{
    admin_repository::FakeAdminRepository, clock::FixedClock, session_store::FakeSessionStore,
    settings_repository::FakeSettingsRepository, store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn initial_settings_have_decided_values_and_complete_metadata() {
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
    let service = ListSettings {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        settings: &settings,
        clock: &clock,
    };
    let before = store.try_read_committed().unwrap();
    let output = service
        .execute(ListSettingsInput {
            actor: fixture::digest(),
        })
        .await
        .unwrap();
    let expected = [
        (
            SettingKey::BuyerTempPhotoDays,
            14,
            1,
            30,
            SettingUnit::DaysFromOriginalUpload,
        ),
        (
            SettingKey::StudioTempUploadDays,
            30,
            7,
            90,
            SettingUnit::DaysFromOriginalUpload,
        ),
        (
            SettingKey::StudioAutoDraftDays,
            30,
            1,
            90,
            SettingUnit::DaysFromLastActivity,
        ),
        (
            SettingKey::AccountRecoveryDays,
            30,
            14,
            90,
            SettingUnit::DaysFromDeletionRequest,
        ),
        (SettingKey::StudioMaxCards, 50, 1, 100, SettingUnit::Cards),
        (
            SettingKey::BuyerMaxPermanentPhotos,
            5,
            1,
            10,
            SettingUnit::Photos,
        ),
        (SettingKey::UploadMaxMb, 15, 1, 25, SettingUnit::Megabytes),
        (
            SettingKey::UploadMinShortestEdgePx,
            512,
            256,
            1024,
            SettingUnit::Pixels,
        ),
        (
            SettingKey::GenerationMaxAttempts,
            3,
            1,
            3,
            SettingUnit::TotalAttempts,
        ),
    ];
    assert_eq!(output.entries.len(), 10);
    for (key, value, minimum, maximum, unit) in expected {
        let entry = output
            .entries
            .iter()
            .find(|entry| entry.key == key)
            .unwrap();
        assert_eq!(entry.value, SettingValue::Number(i128::from(value)));
        assert_eq!(entry.minimum, minimum);
        assert_eq!(entry.maximum, maximum);
        assert_eq!(entry.unit, unit);
        assert_eq!(entry.label_key, format!("setting-{}-label", key.as_str()));
        assert_eq!(entry.effective_at, fixture::now());
        assert_eq!(entry.history.len(), 1);
        assert_eq!(entry.history[0].value, entry.value);
        assert_eq!(entry.history[0].actor, fixture::admin().id());
    }
    let formats = output
        .entries
        .iter()
        .find(|entry| entry.key == SettingKey::UploadFormats)
        .unwrap();
    assert_eq!(formats.unit, SettingUnit::FormatMask);
    assert_eq!(formats.minimum, 1);
    assert_eq!(formats.maximum, 15);
    assert!(matches!(formats.value, SettingValue::Formats(_)));

    assert!(
        service
            .execute(ListSettingsInput {
                actor: BindingDigest::new([24; 32])
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
