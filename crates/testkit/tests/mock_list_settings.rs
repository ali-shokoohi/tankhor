//! T320: authorized, bounded registry reads with no settings writes.
use admin::{
    application::list_settings::{ListSettings, ListSettingsInput},
    ports::{
        admin_repository::MockAdminRepository,
        session_store::MockSessionStore,
        settings_repository::{MockSettingsRepository, SettingRecord},
    },
};
use kernel::ports::{
    clock::MockClock,
    unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
};
use std::sync::{Arc, Mutex};
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn initial_settings_have_decided_values_and_complete_metadata_port_interactions() {
    for authorized in [true, false] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let store = StoreId::new();
        let mut uow = MockUnitOfWork::new();
        let log = events.clone();
        uow.expect_begin().times(1).returning(move |mode| {
            assert_eq!(mode, TransactionMode::ReadOnly);
            log.lock().unwrap().push("begin");
            Ok(TxHandle::new(store, mode, ()))
        });
        let log = events.clone();
        uow.expect_commit()
            .times(usize::from(authorized))
            .returning(move |mut tx| {
                tx.take::<()>(store)?;
                log.lock().unwrap().push("commit");
                Ok(())
            });
        let log = events.clone();
        uow.expect_rollback()
            .times(usize::from(!authorized))
            .returning(move |mut tx| {
                tx.take::<()>(store)?;
                log.lock().unwrap().push("rollback");
                Ok(())
            });
        let mut sessions = MockSessionStore::new();
        let log = events.clone();
        sessions
            .expect_lookup()
            .times(1)
            .returning(move |_, digest| {
                assert_eq!(digest, fixture::digest());
                log.lock().unwrap().push("session");
                Ok(authorized.then(fixture::session))
            });
        let mut admins = MockAdminRepository::new();
        let log = events.clone();
        admins
            .expect_get()
            .times(usize::from(authorized))
            .returning(move |_, target| {
                assert_eq!(target, fixture::admin().id());
                log.lock().unwrap().push("admin");
                Ok(Some(fixture::admin()))
            });
        let mut settings = MockSettingsRepository::new();
        let log = events.clone();
        settings
            .expect_list()
            .times(usize::from(authorized))
            .returning(move |_| {
                log.lock().unwrap().push("settings");
                Ok(fixture::versions()
                    .into_iter()
                    .map(SettingRecord::new)
                    .collect())
            });
        let log = events.clone();
        settings
            .expect_history()
            .times(if authorized { 10 } else { 0 })
            .returning(move |_, key, limit| {
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
            .times(usize::from(authorized))
            .returning(|| Ok(fixture::now()));
        let result = ListSettings {
            uow: &uow,
            admins: &admins,
            sessions: &sessions,
            settings: &settings,
            clock: &clock,
        }
        .execute(ListSettingsInput {
            actor: fixture::digest(),
        })
        .await;
        assert_eq!(result.is_ok(), authorized);
        if authorized {
            let output = result.unwrap();
            assert_eq!(output.entries.len(), 10);
            assert!(output.entries.iter().all(|entry| entry.history.len() == 1));
        }
        let mut expected = vec!["begin", "session"];
        if authorized {
            expected.extend(["admin", "settings"]);
            expected.extend(std::iter::repeat_n("history", 10));
            expected.push("commit");
        } else {
            expected.push("rollback");
        }
        assert_eq!(*events.lock().unwrap(), expected);
    }
}
