//! T184: read-only access validates purpose-separated cookie and current authority.
use admin::{
    application::console_access::VerifyConsoleAccess,
    domain::secrets::SecretBytes,
    ports::{
        admin_authenticator::{MockAdminAuthenticator, TokenPurpose},
        admin_repository::MockAdminRepository,
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
async fn console_rejects_non_admin_access_port_interactions() {
    for scenario in [
        "valid",
        "missing_session",
        "missing_admin",
        "incomplete",
        "expired",
    ] {
        drive(scenario).await;
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "exact read-only authorization order and zero-write refusal scenarios"
)]
async fn drive(scenario: &str) {
    let success = scenario == "valid";
    let present = scenario != "missing_session";
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = StoreId::new();
    let mut crypto = MockAdminAuthenticator::new();
    let log = events.clone();
    crypto
        .expect_token_digest()
        .times(1)
        .returning(move |purpose, cookie| {
            assert_eq!(purpose, TokenPurpose::Session);
            assert_eq!(cookie, SecretBytes::new(b"synthetic-cookie".to_vec()));
            log.lock().unwrap().push("digest");
            Ok(fixture::digest())
        });
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin().times(1).returning(move |mode| {
        assert_eq!(mode, TransactionMode::ReadOnly);
        log.lock().unwrap().push("begin");
        Ok(TxHandle::new(store, mode, ()))
    });
    let log = events.clone();
    uow.expect_commit()
        .times(usize::from(success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("commit");
            Ok(())
        });
    let log = events.clone();
    uow.expect_rollback()
        .times(usize::from(!success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut sessions = MockSessionStore::new();
    let log = events.clone();
    let complete = scenario != "incomplete";
    sessions
        .expect_lookup()
        .times(1)
        .returning(move |tx, digest| {
            assert_eq!(tx.mode(), TransactionMode::ReadOnly);
            assert_eq!(tx.store_id(), store);
            assert_eq!(digest, fixture::digest());
            log.lock().unwrap().push("session");
            Ok(present.then(|| fixture::session(complete)))
        });
    let mut admins = MockAdminRepository::new();
    let log = events.clone();
    let admin_present = scenario != "missing_admin";
    admins
        .expect_get()
        .times(usize::from(present))
        .returning(move |tx, id| {
            assert_eq!(tx.mode(), TransactionMode::ReadOnly);
            assert_eq!(id, fixture::admin(1).id());
            log.lock().unwrap().push("admin");
            Ok(admin_present.then(|| fixture::admin(1)))
        });
    let mut clock = MockClock::new();
    let log = events.clone();
    let expired = scenario == "expired";
    clock
        .expect_now()
        .times(usize::from(present && admin_present))
        .returning(move || {
            log.lock().unwrap().push("clock");
            Ok(fixture::now()
                + if expired {
                    time::Duration::MINUTE
                } else {
                    time::Duration::ZERO
                })
        });
    let result = VerifyConsoleAccess {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        authenticator: &crypto,
        clock: &clock,
    }
    .execute(SecretBytes::new(b"synthetic-cookie".to_vec()))
    .await;
    assert_eq!(result.is_ok(), success);
    if success {
        let actor = result.unwrap();
        assert_eq!(actor.admin, fixture::admin(1).id());
        assert_eq!(actor.session, fixture::session(true).id());
        assert_eq!(actor.epoch, fixture::admin(1).epoch());
        assert!(!format!("{actor:?}").contains("synthetic-cookie"));
    }
    let mut expected = vec!["digest", "begin", "session"];
    if present {
        expected.push("admin");
        if admin_present {
            expected.push("clock");
        }
    }
    expected.push(if success { "commit" } else { "rollback" });
    assert_eq!(*events.lock().unwrap(), expected);
}
