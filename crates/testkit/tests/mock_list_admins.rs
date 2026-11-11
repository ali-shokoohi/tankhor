//! T178: safe list reads cannot precede current actor validation or mutate state.
use admin::{
    application::list_admins::{ListAdmins, ListAdminsInput},
    domain::admin_state::Permissions,
    ports::{
        admin_repository::{AdminPage, MockAdminRepository},
        session_store::MockSessionStore,
    },
};
use kernel::ports::{
    clock::MockClock,
    unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
};
use std::sync::{Arc, Mutex};
#[path = "support/list_admins_fixture.rs"]
mod fixture;
#[tokio::test]
async fn admins_have_equal_permissions_and_distinct_attribution_port_interactions() {
    for scenario in [
        "valid",
        "incomplete",
        "missing_session",
        "missing_admin",
        "cursor",
        "limit",
    ] {
        drive(scenario).await;
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "strict zero-write authorization and exact bounded-read ordering across refusals"
)]
async fn drive(scenario: &str) {
    let complete = scenario != "incomplete";
    let query_valid = !matches!(scenario, "cursor" | "limit");
    let session_present = scenario != "missing_session";
    let admin_present = scenario != "missing_admin";
    let authorized = scenario == "valid";
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin()
        .times(usize::from(query_valid))
        .returning(move |mode| {
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
        .times(usize::from(query_valid && !authorized))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut sessions = MockSessionStore::new();
    let log = events.clone();
    sessions
        .expect_lookup()
        .times(usize::from(query_valid))
        .returning(move |_, digest| {
            assert_eq!(digest, fixture::digest());
            log.lock().unwrap().push("session");
            Ok(session_present.then(|| fixture::session(complete)))
        });
    let mut admins = MockAdminRepository::new();
    let log = events.clone();
    admins
        .expect_get()
        .times(usize::from(query_valid && session_present))
        .returning(move |_, target| {
            assert_eq!(target, fixture::admin(1).id());
            log.lock().unwrap().push("admin");
            Ok(admin_present.then(|| fixture::admin(1)))
        });
    let log = events.clone();
    admins
        .expect_list()
        .times(usize::from(authorized))
        .returning(move |_, query| {
            assert_eq!(query.after(), Some(fixture::admin(1).id()));
            assert_eq!(query.limit(), 2);
            log.lock().unwrap().push("list");
            Ok(AdminPage {
                entries: vec![fixture::admin(2), fixture::admin(3)],
                next: Some(fixture::admin(3).id()),
            })
        });
    let mut clock = MockClock::new();
    clock.expect_now().returning(|| Ok(fixture::now()));
    let result = ListAdmins {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        clock: &clock,
    }
    .execute(ListAdminsInput {
        actor: fixture::digest(),
        cursor: Some(if scenario == "cursor" {
            "not-a-cursor".into()
        } else {
            fixture::admin(1).id().to_string()
        }),
        limit: if scenario == "limit" { 0 } else { 2 },
    })
    .await;
    assert_eq!(result.is_ok(), authorized);
    if authorized {
        let page = result.unwrap();
        assert_eq!(page.entries[0].id, fixture::admin(2).id());
        assert_eq!(page.entries[1].id, fixture::admin(3).id());
        assert!(
            page.entries
                .iter()
                .all(|row| row.permissions == Some(Permissions::Full))
        );
        assert_eq!(page.next, Some(fixture::admin(3).id().to_string()));
    }
    let mut expected = Vec::new();
    if query_valid {
        expected.extend(["begin", "session"]);
        if session_present {
            expected.push("admin");
        }
        if authorized {
            expected.push("list");
        }
        expected.push(if authorized { "commit" } else { "rollback" });
    }
    assert_eq!(*events.lock().unwrap(), expected);
}
