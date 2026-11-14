//! T330: exact historical subject/version values and actors, never live substitutes.
use admin::{
    application::get_version::{GetVersion, GetVersionInput, RegistrySubject, VersionValue},
    domain::{
        flag::FlagKey,
        settings::{SettingKey, SettingValue},
    },
    ports::{
        flag_repository::{FlagRecord, FlagVersion},
        session_store::SessionRecord,
        settings_repository::{SettingRecord, SettingVersion},
    },
};
use kernel::{error::ErrorCode, ids::VersionId};
use testkit::fakes::{
    admin_repository::FakeAdminRepository, clock::FixedClock, flag_repository::FakeFlagRepository,
    session_store::FakeSessionStore, settings_repository::FakeSettingsRepository, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn get_version_requires_exact_subject_version_identity() {
    let historical_setting = SettingVersion::new(
        fixture::id::<VersionId>(90),
        SettingKey::UploadMaxMb,
        SettingValue::Number(20),
        fixture::id(91),
        fixture::now() - time::Duration::DAY,
    )
    .unwrap();
    let historical_flag = FlagVersion::new(
        fixture::id(92),
        FlagKey::Ticketing,
        true,
        fixture::id(91),
        fixture::now() - time::Duration::DAY,
    );
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
        state
            .setting_history
            .get_mut(&SettingKey::UploadMaxMb)
            .unwrap()
            .insert(0, historical_setting.clone());
        for version in fixture::flag_versions() {
            state
                .flags
                .insert(version.key(), FlagRecord::new(version.clone()));
            state.flag_history.insert(version.key(), vec![version]);
        }
        state
            .flag_history
            .get_mut(&FlagKey::Ticketing)
            .unwrap()
            .insert(0, historical_flag.clone());
    });
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let settings = FakeSettingsRepository::new(store.clone());
    let flags = FakeFlagRepository::new(store.clone());
    let clock = FixedClock::new(fixture::now());
    let service = GetVersion {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        settings: &settings,
        flags: &flags,
        clock: &clock,
    };
    let before = store.try_read_committed().unwrap();

    let setting = service
        .execute(GetVersionInput {
            actor: fixture::digest(),
            subject: RegistrySubject::Setting(SettingKey::UploadMaxMb),
            version: fixture::id(90),
        })
        .await
        .unwrap();
    assert_eq!(
        setting.value,
        VersionValue::Setting(SettingValue::Number(20))
    );
    assert_eq!(setting.actor, fixture::id(91));
    let flag = service
        .execute(GetVersionInput {
            actor: fixture::digest(),
            subject: RegistrySubject::Flag(FlagKey::Ticketing),
            version: fixture::id(92),
        })
        .await
        .unwrap();
    assert_eq!(flag.value, VersionValue::Flag(true));
    assert_eq!(flag.actor, fixture::id(91));

    let wrong_subject = service
        .execute(GetVersionInput {
            actor: fixture::digest(),
            subject: RegistrySubject::Flag(FlagKey::Ticketing),
            version: fixture::id(90),
        })
        .await
        .unwrap_err();
    assert_eq!(wrong_subject.code(), ErrorCode::NotFound);
    let after = store.try_read_committed().unwrap();
    assert_eq!(after.settings, before.settings);
    assert_eq!(after.setting_history, before.setting_history);
    assert_eq!(after.flags, before.flags);
    assert_eq!(after.flag_history, before.flag_history);
    assert_eq!(after.audit_entries, before.audit_entries);
}
