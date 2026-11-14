//! T326: exact-key authorized setting reads, bounded history and zero writes.
use admin::{
    application::get_setting::{GetSetting, GetSettingInput},
    domain::settings::SettingKey,
    ports::{
        admin_repository::MockAdminRepository,
        session_store::MockSessionStore,
        settings_repository::{MockSettingsRepository, SettingRecord},
    },
};
use kernel::{
    error::ErrorCode,
    ports::{
        clock::MockClock,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
use std::sync::{Arc, Mutex};
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn get_setting_requires_exact_known_key_port_interactions() {
    for scenario in ["valid", "unknown", "missing", "denied"] {
        let valid_key = scenario != "unknown";
        let authorized = scenario != "denied";
        let found = scenario == "valid";
        let events = Arc::new(Mutex::new(Vec::new()));
        let store = StoreId::new();
        let mut uow = MockUnitOfWork::new();
        let log = events.clone();
        uow.expect_begin()
            .times(usize::from(valid_key))
            .returning(move |mode| {
                assert_eq!(mode, TransactionMode::ReadOnly);
                log.lock().unwrap().push("begin");
                Ok(TxHandle::new(store, mode, ()))
            });
        let log = events.clone();
        uow.expect_commit()
            .times(usize::from(found))
            .returning(move |mut tx| {
                tx.take::<()>(store)?;
                log.lock().unwrap().push("commit");
                Ok(())
            });
        let log = events.clone();
        uow.expect_rollback()
            .times(usize::from(valid_key && !found))
            .returning(move |mut tx| {
                tx.take::<()>(store)?;
                log.lock().unwrap().push("rollback");
                Ok(())
            });
        let mut sessions = MockSessionStore::new();
        let log = events.clone();
        sessions
            .expect_lookup()
            .times(usize::from(valid_key))
            .returning(move |_, _| {
                log.lock().unwrap().push("session");
                Ok(authorized.then(fixture::session))
            });
        let mut admins = MockAdminRepository::new();
        let log = events.clone();
        admins
            .expect_get()
            .times(usize::from(valid_key && authorized))
            .returning(move |_, _| {
                log.lock().unwrap().push("admin");
                Ok(Some(fixture::admin()))
            });
        let mut settings = MockSettingsRepository::new();
        let log = events.clone();
        settings
            .expect_get()
            .times(usize::from(valid_key && authorized))
            .returning(move |_, key| {
                assert_eq!(key, SettingKey::UploadMaxMb);
                log.lock().unwrap().push("get");
                let version = fixture::versions()
                    .into_iter()
                    .find(|version| version.key() == key)
                    .unwrap();
                Ok((scenario != "missing").then(|| SettingRecord::new(version)))
            });
        let log = events.clone();
        settings
            .expect_history()
            .times(usize::from(found))
            .returning(move |_, key, limit| {
                assert_eq!(key, SettingKey::UploadMaxMb);
                assert_eq!(limit, 20);
                log.lock().unwrap().push("history");
                Ok(fixture::versions()
                    .into_iter()
                    .filter(|version| version.key() == key)
                    .collect())
            });
        let mut clock = MockClock::new();
        clock
            .expect_now()
            .times(usize::from(valid_key && authorized))
            .returning(|| Ok(fixture::now()));
        let result = GetSetting {
            uow: &uow,
            admins: &admins,
            sessions: &sessions,
            settings: &settings,
            clock: &clock,
        }
        .execute(GetSettingInput {
            actor: fixture::digest(),
            key: if valid_key {
                "upload_max_mb".into()
            } else {
                "upload-max-mb".into()
            },
        })
        .await;
        assert_eq!(result.is_ok(), found, "{scenario}");
        if found {
            let output = result.unwrap();
            assert_eq!(output.key, SettingKey::UploadMaxMb);
            assert_eq!(output.history.len(), 1);
        } else if scenario == "unknown" {
            assert_eq!(result.unwrap_err().code(), ErrorCode::InvalidValue);
        } else if scenario == "missing" {
            assert_eq!(result.unwrap_err().code(), ErrorCode::NotFound);
        }
        let mut expected = Vec::new();
        if valid_key {
            expected.extend(["begin", "session"]);
            if authorized {
                expected.extend(["admin", "get"]);
                if found {
                    expected.push("history");
                }
            }
            expected.push(if found { "commit" } else { "rollback" });
        }
        assert_eq!(*events.lock().unwrap(), expected, "{scenario}");
    }
}
