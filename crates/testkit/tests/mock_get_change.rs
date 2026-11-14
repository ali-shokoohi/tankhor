//! T332: exact change/head/review reads after authorization, with no writes or broad scans.
use admin::{
    application::get_change::{GetChange, GetChangeInput},
    domain::change_approval::ApprovalEligibility,
    ports::{
        admin_repository::MockAdminRepository, change_repository::MockChangeRepository,
        session_store::MockSessionStore,
    },
};
use kernel::{
    error::ErrorCode,
    ids::ChangeId,
    ports::{
        clock::MockClock,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
use std::sync::{Arc, Mutex};
#[path = "support/change_fixture.rs"]
mod change;
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn get_change_returns_exact_head_review_and_baseline_port_interactions() {
    for scenario in ["valid", "malformed", "missing", "denied"] {
        let valid_id = scenario != "malformed";
        let authorized = scenario != "denied";
        let found = scenario == "valid";
        let events = Arc::new(Mutex::new(Vec::new()));
        let store = StoreId::new();
        let mut uow = MockUnitOfWork::new();
        let log = events.clone();
        uow.expect_begin()
            .times(usize::from(valid_id))
            .returning(move |mode| {
                assert_eq!(mode, TransactionMode::ReadOnly);
                log.lock().unwrap().push("begin");
                Ok(TxHandle::new(store, mode, ()))
            });
        uow.expect_commit()
            .times(usize::from(found))
            .returning(|_| Ok(()));
        uow.expect_rollback()
            .times(usize::from(valid_id && !found))
            .returning(|_| Ok(()));
        let mut sessions = MockSessionStore::new();
        let log = events.clone();
        sessions
            .expect_lookup()
            .times(usize::from(valid_id))
            .returning(move |_, _| {
                log.lock().unwrap().push("session");
                Ok(authorized.then(fixture::session))
            });
        let mut admins = MockAdminRepository::new();
        admins
            .expect_get()
            .times(usize::from(valid_id && authorized))
            .returning(|_, _| Ok(Some(fixture::admin())));
        let mut changes = MockChangeRepository::new();
        let log = events.clone();
        changes
            .expect_draft()
            .times(usize::from(valid_id && authorized))
            .returning(move |_, id| {
                assert_eq!(id, change::id::<ChangeId>(1));
                log.lock().unwrap().push("draft");
                Ok((scenario != "missing").then(change::draft))
            });
        let log = events.clone();
        changes
            .expect_latest_preview()
            .times(usize::from(found))
            .returning(move |_, change_id, revision| {
                assert_eq!(change_id, change::id(1));
                assert_eq!(revision, change::draft().head().unwrap());
                log.lock().unwrap().push("preview");
                Ok(Some(change::report(&change::draft())))
            });
        let log = events.clone();
        changes
            .expect_latest_approval()
            .times(usize::from(found))
            .returning(move |_, change_id, revision| {
                assert_eq!(change_id, change::id(1));
                assert_eq!(revision, change::draft().head().unwrap());
                log.lock().unwrap().push("approval");
                Ok(Some(change::approval(&change::report(&change::draft()))))
            });
        let mut clock = MockClock::new();
        clock
            .expect_now()
            .times(usize::from(valid_id && authorized))
            .returning(|| Ok(fixture::now()));
        let result = GetChange {
            uow: &uow,
            admins: &admins,
            sessions: &sessions,
            changes: &changes,
            clock: &clock,
        }
        .execute(GetChangeInput {
            actor: fixture::digest(),
            change: if valid_id {
                change::id::<ChangeId>(1).to_string()
            } else {
                "not-a-change".into()
            },
        })
        .await;
        assert_eq!(result.is_ok(), found, "{scenario}");
        if found {
            let output = result.unwrap();
            assert_eq!(output.id, change::id(1));
            assert_eq!(output.baseline_version, change::id(2));
            assert_eq!(output.head, change::draft().head());
            assert_eq!(output.eligibility, Some(ApprovalEligibility::Eligible));
        } else if scenario == "malformed" {
            assert_eq!(result.unwrap_err().code(), ErrorCode::InvalidValue);
        } else if scenario == "missing" {
            assert_eq!(result.unwrap_err().code(), ErrorCode::NotFound);
        }
        if found {
            assert_eq!(
                *events.lock().unwrap(),
                ["begin", "session", "draft", "preview", "approval"]
            );
        }
    }
}
