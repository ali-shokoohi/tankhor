//! T333: exact draft head, baseline and matching review without live or historical substitution.
use admin::{
    application::get_change::{GetChange, GetChangeInput},
    domain::{auth_state::BindingDigest, change_approval::ApprovalEligibility},
    ports::session_store::SessionRecord,
};
use kernel::error::ErrorCode;
use testkit::fakes::{
    admin_repository::FakeAdminRepository, change_repository::FakeChangeRepository,
    clock::FixedClock, session_store::FakeSessionStore, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
#[path = "support/change_fixture.rs"]
mod change;
#[path = "support/settings_fixture.rs"]
mod fixture;

#[tokio::test]
async fn get_change_returns_exact_head_review_and_baseline() {
    let draft = change::draft();
    let report = change::report(&draft);
    let approval = change::approval(&report);
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
        state.changes.insert(draft.id(), draft.clone());
        state.previews.insert(report.id(), report.clone());
        state.approvals.insert(approval.id(), approval.clone());
    });
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let changes = FakeChangeRepository::new(store.clone());
    let clock = FixedClock::new(fixture::now());
    let service = GetChange {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        changes: &changes,
        clock: &clock,
    };
    let before = store.try_read_committed().unwrap();
    let output = service
        .execute(GetChangeInput {
            actor: fixture::digest(),
            change: draft.id().to_string(),
        })
        .await
        .unwrap();
    assert_eq!(output.id, draft.id());
    assert_eq!(output.subject, draft.subject());
    assert_eq!(output.baseline_version, draft.baseline_version());
    assert_eq!(output.head, draft.head());
    assert_eq!(
        output.current.as_ref().unwrap().value,
        draft.current().unwrap().value()
    );
    assert_eq!(output.preview.as_ref().unwrap().id(), report.id());
    assert_eq!(output.approval.as_ref().unwrap().id(), approval.id());
    assert_eq!(output.eligibility, Some(ApprovalEligibility::Eligible));

    let missing = service
        .execute(GetChangeInput {
            actor: fixture::digest(),
            change: change::id::<kernel::ids::ChangeId>(99).to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(missing.code(), ErrorCode::NotFound);
    assert!(
        service
            .execute(GetChangeInput {
                actor: BindingDigest::new([24; 32]),
                change: draft.id().to_string()
            })
            .await
            .is_err()
    );
    let after = store.try_read_committed().unwrap();
    assert_eq!(after.changes, before.changes);
    assert_eq!(after.previews, before.previews);
    assert_eq!(after.approvals, before.approvals);
    assert_eq!(after.audit_entries, before.audit_entries);
}
