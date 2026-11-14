//! T336: draft-only persistence, exact-review invalidation, CAS and atomic audit.
use admin::{
    application::save_draft::{SaveDraft, SaveDraftInput},
    domain::{
        audit::{AuditAction, ReasonCode},
        change_revision::{ChangeSubject, ChangeValue},
        settings::SettingKey,
    },
    ports::session_store::SessionRecord,
};
use testkit::fakes::{
    admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
    change_repository::FakeChangeRepository, clock::FixedClock,
    flag_repository::FakeFlagRepository, id_generator::SequenceIdGenerator,
    session_store::FakeSessionStore, settings_repository::FakeSettingsRepository, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
#[path = "support/change_fixture.rs"]
mod change;
#[path = "support/settings_fixture.rs"]
mod fixture;

struct Harness {
    store: FakeStore,
    uow: FakeUnitOfWork,
    admins: FakeAdminRepository,
    sessions: FakeSessionStore,
    settings: FakeSettingsRepository,
    flags: FakeFlagRepository,
    changes: FakeChangeRepository,
    audit: FakeAuditRepository,
    clock: FixedClock,
    ids: SequenceIdGenerator,
}
impl Harness {
    fn new() -> Self {
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
                state.settings.insert(
                    version.key(),
                    admin::ports::settings_repository::SettingRecord::new(version.clone()),
                );
                state.setting_history.insert(version.key(), vec![version]);
            }
        });
        Self {
            uow: FakeUnitOfWork::new(store.clone()),
            admins: FakeAdminRepository::new(store.clone()),
            sessions: FakeSessionStore::new(store.clone()),
            settings: FakeSettingsRepository::new(store.clone()),
            flags: FakeFlagRepository::new(store.clone()),
            changes: FakeChangeRepository::new(store.clone()),
            audit: FakeAuditRepository::new(store.clone()),
            clock: FixedClock::new(fixture::now()),
            ids: SequenceIdGenerator::new(100),
            store,
        }
    }
    fn service(&self) -> SaveDraft<'_> {
        SaveDraft {
            uow: &self.uow,
            sessions: &self.sessions,
            admins: &self.admins,
            settings: &self.settings,
            flags: &self.flags,
            changes: &self.changes,
            audit: &self.audit,
            clock: &self.clock,
            ids: &self.ids,
        }
    }
}
fn input() -> SaveDraftInput {
    SaveDraftInput {
        actor: fixture::digest(),
        change: None,
        subject: ChangeSubject::Setting(SettingKey::UploadMaxMb),
        value: ChangeValue::Number(20),
        reason: ReasonCode::DraftChange,
        expected_head: None,
        schedule: None,
    }
}

#[tokio::test]
async fn saving_setting_creates_draft_only() {
    let harness = Harness::new();
    let before = harness.store.try_read_committed().unwrap();
    let output = harness.service().execute(input()).await.unwrap();
    let after = harness.store.try_read_committed().unwrap();
    assert_eq!(after.settings, before.settings);
    assert_eq!(after.setting_history, before.setting_history);
    assert_eq!(after.flags, before.flags);
    assert_eq!(after.changes.len(), 1);
    assert_eq!(output.current().unwrap().value(), ChangeValue::Number(20));
    assert_eq!(output.creator(), fixture::admin().id());
    assert_eq!(
        output.current().unwrap().created_by(),
        fixture::admin().id()
    );
    assert_eq!(after.changes[&output.id()], output);
    assert_eq!(
        after
            .audit_entries
            .values()
            .filter(|entry| entry.action() == AuditAction::DraftSaved)
            .count(),
        1
    );
}

#[tokio::test]
async fn editing_approved_draft_invalidates_approval() {
    let harness = Harness::new();
    let draft = change::draft();
    let report = change::report(&draft);
    let approval = change::approval(&report);
    let approved_revision = draft.head().unwrap();
    harness.store.seed(|state| {
        state.changes.insert(draft.id(), draft.clone());
        state.previews.insert(report.id(), report);
        state.approvals.insert(approval.id(), approval);
    });
    let mut edit = input();
    edit.change = Some(draft.id());
    edit.expected_head = draft.head();
    edit.value = ChangeValue::Number(21);
    let output = harness.service().execute(edit).await.unwrap();
    assert_ne!(output.head(), Some(approved_revision));
    let state = harness.store.try_read_committed().unwrap();
    assert_eq!(state.approvals.len(), 1);
    assert_eq!(
        state
            .approvals
            .values()
            .next()
            .unwrap()
            .approved_report()
            .revision_id(),
        approved_revision
    );
    assert_eq!(state.changes[&draft.id()].revisions().len(), 2);
    let stored = &state.changes[&draft.id()];
    assert_ne!(change::creator(), fixture::admin().id());
    assert_eq!(stored.creator(), change::creator());
    assert_eq!(stored.revisions()[0], draft.revisions()[0]);
    assert_eq!(stored.revisions()[0].created_by(), change::creator());
    assert_eq!(
        stored.current().unwrap().created_by(),
        fixture::admin().id()
    );
}

#[tokio::test]
async fn concurrent_setting_edits_do_not_silently_overwrite() {
    let harness = Harness::new();
    let first = harness.service().execute(input()).await.unwrap();
    let mut second = input();
    second.change = Some(first.id());
    second.expected_head = None;
    assert!(harness.service().execute(second).await.is_err());
    let state = harness.store.try_read_committed().unwrap();
    assert_eq!(state.changes[&first.id()].revisions().len(), 1);
    assert_eq!(state.audit_entries.len(), 1);
}

#[tokio::test]
async fn audit_draft_has_complete_safe_fields_once() {
    let harness = Harness::new();
    let before = harness.store.try_read_committed().unwrap();
    harness.audit.fail_next_append();
    assert!(harness.service().execute(input()).await.is_err());
    let after = harness.store.try_read_committed().unwrap();
    assert!(after.changes.is_empty());
    assert!(after.audit_entries.is_empty());
    assert_eq!(after.settings, before.settings);
    assert_eq!(after.setting_history, before.setting_history);
}
