//! T187: one current-session revocation after locked current authorization.
use admin::{
    application::sign_out::SignOut,
    domain::secrets::SecretBytes,
    ports::{
        admin_authenticator::{MockAdminAuthenticator, TokenPurpose},
        admin_repository::MockAdminRepository,
        session_store::MockSessionStore,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::{
        clock::MockClock,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
use std::sync::{Arc, Mutex};
#[path = "support/list_admins_fixture.rs"]
mod fixture;

#[tokio::test]
async fn signout_revokes_current_admin_session_only_port_interactions() {
    for scenario in ["valid", "missing", "incomplete", "epoch", "write"] {
        drive(scenario).await;
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "exact authorization ordering, target-only write counts and rollback scenarios"
)]
async fn drive(scenario: &str) {
    let success = scenario == "valid";
    let present = scenario != "missing";
    let authorized = matches!(scenario, "valid" | "write");
    let log = Arc::new(Mutex::new(Vec::new()));
    let store = StoreId::new();
    let mut crypto = MockAdminAuthenticator::new();
    let events = log.clone();
    crypto
        .expect_token_digest()
        .times(1)
        .returning(move |purpose, _| {
            assert_eq!(purpose, TokenPurpose::Session);
            events.lock().unwrap().push("digest");
            Ok(fixture::digest())
        });
    let mut uow = MockUnitOfWork::new();
    let events = log.clone();
    uow.expect_begin().times(1).returning(move |mode| {
        assert_eq!(mode, TransactionMode::ReadWrite);
        events.lock().unwrap().push("begin");
        Ok(TxHandle::new(store, mode, ()))
    });
    let events = log.clone();
    uow.expect_commit()
        .times(usize::from(success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            events.lock().unwrap().push("commit");
            Ok(())
        });
    let events = log.clone();
    uow.expect_rollback()
        .times(usize::from(!success))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            events.lock().unwrap().push("rollback");
            Ok(())
        });
    let mut sessions = MockSessionStore::new();
    let events = log.clone();
    let complete = scenario != "incomplete";
    sessions
        .expect_lookup()
        .times(1)
        .returning(move |tx, digest| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(digest, fixture::digest());
            events.lock().unwrap().push("session");
            Ok(present.then(|| fixture::session(complete)))
        });
    let mut admins = MockAdminRepository::new();
    let events = log.clone();
    let stale = scenario == "epoch";
    admins
        .expect_lock()
        .times(usize::from(present))
        .returning(move |tx, ids| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(ids, vec![fixture::admin(1).id()]);
            events.lock().unwrap().push("lock");
            let mut admin = fixture::admin(1);
            if stale {
                admin
                    .invalidate(
                        admin.epoch(),
                        admin.epoch().advance().unwrap(),
                        admin::domain::admin_state::Invalidation::Recover,
                    )
                    .unwrap();
            }
            Ok(vec![admin])
        });
    let mut clock = MockClock::new();
    let events = log.clone();
    clock
        .expect_now()
        .times(usize::from(present))
        .returning(move || {
            events.lock().unwrap().push("clock");
            Ok(fixture::now())
        });
    let events = log.clone();
    let fail = scenario == "write";
    sessions
        .expect_revoke_one()
        .times(usize::from(authorized))
        .returning(move |tx, id, now| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(id, fixture::session(true).id());
            assert_eq!(now, fixture::now());
            events.lock().unwrap().push("revoke");
            if fail {
                Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
            } else {
                Ok(())
            }
        });
    let result = SignOut {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        authenticator: &crypto,
        clock: &clock,
    }
    .execute(SecretBytes::new(b"synthetic-cookie".to_vec()))
    .await;
    assert_eq!(result.is_ok(), success);
    let mut expected = vec!["digest", "begin", "session"];
    if present {
        expected.extend(["lock", "clock"]);
    }
    if authorized {
        expected.push("revoke");
    }
    expected.push(if success { "commit" } else { "rollback" });
    assert_eq!(*log.lock().unwrap(), expected);
}
