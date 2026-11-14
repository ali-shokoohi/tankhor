//! T329: exact typed registry-version reads after authorization, without list/history scans.
use admin::{
    application::get_version::{GetVersion, GetVersionInput, RegistrySubject, VersionValue},
    domain::{
        flag::FlagKey,
        settings::{SettingKey, SettingValue},
    },
    ports::{
        admin_repository::MockAdminRepository, flag_repository::MockFlagRepository,
        session_store::MockSessionStore, settings_repository::MockSettingsRepository,
    },
};
use kernel::{
    error::ErrorCode,
    ids::VersionId,
    ports::{
        clock::MockClock,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
async fn get_version_requires_exact_subject_version_identity_port_interactions() {
    for scenario in ["setting", "flag", "missing", "denied"] {
        let setting = scenario == "setting";
        let flag = scenario == "flag";
        let found = setting || flag;
        let authorized = scenario != "denied";
        let store = StoreId::new();
        let mut uow = MockUnitOfWork::new();
        uow.expect_begin().times(1).returning(move |mode| {
            assert_eq!(mode, TransactionMode::ReadOnly);
            Ok(TxHandle::new(store, mode, ()))
        });
        uow.expect_commit()
            .times(usize::from(found))
            .returning(|_| Ok(()));
        uow.expect_rollback()
            .times(usize::from(!found))
            .returning(|_| Ok(()));
        let mut sessions = MockSessionStore::new();
        sessions
            .expect_lookup()
            .times(1)
            .returning(move |_, _| Ok(authorized.then(fixture::session)));
        let mut admins = MockAdminRepository::new();
        admins
            .expect_get()
            .times(usize::from(authorized))
            .returning(|_, _| Ok(Some(fixture::admin())));
        let mut settings = MockSettingsRepository::new();
        settings
            .expect_version()
            .times(usize::from(setting || scenario == "missing"))
            .returning(move |_, key, version| {
                assert_eq!(key, SettingKey::UploadMaxMb);
                assert_eq!(version, fixture::id::<VersionId>(10));
                Ok((scenario == "setting").then(|| {
                    fixture::versions()
                        .into_iter()
                        .find(|row| row.key() == key)
                        .unwrap()
                }))
            });
        let mut flags = MockFlagRepository::new();
        flags
            .expect_version()
            .times(usize::from(flag))
            .returning(|_, key, version| {
                assert_eq!(key, FlagKey::Ticketing);
                assert_eq!(version, fixture::id::<VersionId>(30));
                Ok(fixture::flag_versions()
                    .into_iter()
                    .find(|row| row.key() == key))
            });
        let mut clock = MockClock::new();
        clock
            .expect_now()
            .times(usize::from(authorized))
            .returning(|| Ok(fixture::now()));
        let subject = if flag {
            RegistrySubject::Flag(FlagKey::Ticketing)
        } else {
            RegistrySubject::Setting(SettingKey::UploadMaxMb)
        };
        let version = if flag {
            fixture::id(30)
        } else {
            fixture::id(10)
        };
        let result = GetVersion {
            uow: &uow,
            admins: &admins,
            sessions: &sessions,
            settings: &settings,
            flags: &flags,
            clock: &clock,
        }
        .execute(GetVersionInput {
            actor: fixture::digest(),
            subject,
            version,
        })
        .await;
        assert_eq!(result.is_ok(), found, "{scenario}");
        if setting {
            assert_eq!(
                result.unwrap().value,
                VersionValue::Setting(SettingValue::Number(15))
            );
        } else if flag {
            assert_eq!(result.unwrap().value, VersionValue::Flag(false));
        } else if scenario == "missing" {
            assert_eq!(result.unwrap_err().code(), ErrorCode::NotFound);
        }
    }
}
