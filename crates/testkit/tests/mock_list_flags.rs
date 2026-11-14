//! T323: bounded flag registry reads occur only after current-admin authorization.
use admin::{
    application::list_flags::{ListFlags, ListFlagsInput},
    ports::{
        admin_repository::MockAdminRepository,
        flag_repository::{FlagRecord, MockFlagRepository},
        session_store::MockSessionStore,
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
async fn flag_registry_queries_return_safe_bounded_metadata_port_interactions() {
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
        sessions.expect_lookup().times(1).returning(move |_, _| {
            log.lock().unwrap().push("session");
            Ok(authorized.then(fixture::session))
        });
        let mut admins = MockAdminRepository::new();
        let log = events.clone();
        admins
            .expect_get()
            .times(usize::from(authorized))
            .returning(move |_, _| {
                log.lock().unwrap().push("admin");
                Ok(Some(fixture::admin()))
            });
        let mut flags = MockFlagRepository::new();
        let log = events.clone();
        flags
            .expect_list()
            .times(usize::from(authorized))
            .returning(move |_| {
                log.lock().unwrap().push("flags");
                Ok(fixture::flag_versions()
                    .into_iter()
                    .map(FlagRecord::new)
                    .collect())
            });
        let log = events.clone();
        flags
            .expect_history()
            .times(if authorized { 6 } else { 0 })
            .returning(move |_, key, limit| {
                assert_eq!(limit, 20);
                log.lock().unwrap().push("history");
                Ok(fixture::flag_versions()
                    .into_iter()
                    .filter(|version| version.key() == key)
                    .collect())
            });
        let mut clock = MockClock::new();
        clock
            .expect_now()
            .times(usize::from(authorized))
            .returning(|| Ok(fixture::now()));
        let result = ListFlags {
            uow: &uow,
            admins: &admins,
            sessions: &sessions,
            flags: &flags,
            clock: &clock,
        }
        .execute(ListFlagsInput {
            actor: fixture::digest(),
        })
        .await;
        assert_eq!(result.is_ok(), authorized);
        if authorized {
            assert_eq!(result.unwrap().entries.len(), 6);
        }
        let mut expected = vec!["begin", "session"];
        if authorized {
            expected.extend(["admin", "flags"]);
            expected.extend(std::iter::repeat_n("history", 6));
            expected.push("commit");
        } else {
            expected.push("rollback");
        }
        assert_eq!(*events.lock().unwrap(), expected);
    }
}
