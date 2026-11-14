//! T335: validated draft-only writes, explicit CAS, safe audit and no publication effects.
use admin::{
    application::save_draft::{SaveDraft, SaveDraftInput, ScheduleInput},
    domain::{
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
        change_revision::{ChangeSubject, ChangeValue},
    },
    ports::{
        admin_repository::MockAdminRepository, audit_repository::MockAuditRepository,
        change_repository::MockChangeRepository, flag_repository::MockFlagRepository,
        session_store::MockSessionStore, settings_repository::MockSettingsRepository,
    },
};
use kernel::ports::{
    clock::MockClock,
    id_generator::MockIdGenerator,
    unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
};
#[path = "support/change_fixture.rs"]
mod change;
#[path = "support/settings_fixture.rs"]
mod fixture;

fn input() -> SaveDraftInput {
    SaveDraftInput {
        actor: fixture::digest(),
        change: Some(change::id(1)),
        subject: ChangeSubject::Setting(admin::domain::settings::SettingKey::UploadMaxMb),
        value: ChangeValue::Number(20),
        reason: ReasonCode::DraftChange,
        expected_head: Some(change::id(3)),
        schedule: Some(ScheduleInput {
            local: "2026-01-01T04:30".into(),
            zone: None,
        }),
    }
}

#[tokio::test]
async fn validation_does_not_invoke_publish_or_vendor() {
    let uow = MockUnitOfWork::new();
    let sessions = MockSessionStore::new();
    let admins = MockAdminRepository::new();
    let settings = MockSettingsRepository::new();
    let flags = MockFlagRepository::new();
    let changes = MockChangeRepository::new();
    let audit = MockAuditRepository::new();
    let clock = MockClock::new();
    let ids = MockIdGenerator::new();
    let mut invalid = input();
    invalid.value = ChangeValue::Number(26);
    assert!(
        SaveDraft {
            uow: &uow,
            sessions: &sessions,
            admins: &admins,
            settings: &settings,
            flags: &flags,
            changes: &changes,
            audit: &audit,
            clock: &clock,
            ids: &ids
        }
        .execute(invalid)
        .await
        .is_err()
    );

    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    uow.expect_begin()
        .once()
        .returning(move |mode| Ok(TxHandle::new(store, mode, ())));
    uow.expect_rollback().once().returning(move |mut tx| {
        tx.take::<()>(store)?;
        Ok(())
    });
    let mut sessions = MockSessionStore::new();
    sessions
        .expect_lookup()
        .once()
        .returning(|_, _| Ok(Some(fixture::session())));
    let mut admins = MockAdminRepository::new();
    admins
        .expect_get()
        .once()
        .returning(|_, _| Ok(Some(fixture::admin())));
    let mut clock = MockClock::new();
    clock.expect_now().once().returning(|| Ok(fixture::now()));
    let mut changes = MockChangeRepository::new();
    changes
        .expect_draft()
        .once()
        .returning(|_, _| Ok(Some(change::draft())));
    let settings = MockSettingsRepository::new();
    let flags = MockFlagRepository::new();
    let audit = MockAuditRepository::new();
    let ids = MockIdGenerator::new();
    let mut stale = input();
    stale.expected_head = None;
    assert!(
        SaveDraft {
            uow: &uow,
            sessions: &sessions,
            admins: &admins,
            settings: &settings,
            flags: &flags,
            changes: &changes,
            audit: &audit,
            clock: &clock,
            ids: &ids
        }
        .execute(stale)
        .await
        .is_err()
    );
}

#[tokio::test]
async fn audit_draft_has_complete_safe_fields_once() {
    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    uow.expect_begin().once().returning(move |mode| {
        assert_eq!(mode, TransactionMode::ReadWrite);
        Ok(TxHandle::new(store, mode, ()))
    });
    uow.expect_commit().once().returning(move |mut tx| {
        tx.take::<()>(store)?;
        Ok(())
    });
    let mut sessions = MockSessionStore::new();
    sessions
        .expect_lookup()
        .once()
        .returning(|_, _| Ok(Some(fixture::session())));
    let mut admins = MockAdminRepository::new();
    admins
        .expect_get()
        .once()
        .returning(|_, _| Ok(Some(fixture::admin())));
    let mut clock = MockClock::new();
    clock.expect_now().once().returning(|| Ok(fixture::now()));
    let mut changes = MockChangeRepository::new();
    changes
        .expect_draft()
        .once()
        .returning(|_, _| Ok(Some(change::draft())));
    changes
        .expect_append_revision()
        .once()
        .returning(|_, draft, expected| {
            assert_eq!(expected, Some(change::id(3)));
            assert_eq!(draft.current().unwrap().revision_number(), 2);
            assert_eq!(draft.creator(), change::creator());
            assert_eq!(draft.revisions()[0], change::draft().revisions()[0]);
            assert_eq!(draft.current().unwrap().created_by(), fixture::admin().id());
            assert_eq!(
                draft.current().unwrap().scheduled_at().unwrap().utc(),
                time::macros::datetime!(2026-01-01 01:00 UTC)
            );
            Ok(())
        });
    let mut ids = MockIdGenerator::new();
    ids.expect_next_revision_id()
        .once()
        .returning(|| Ok(change::id(30)));
    ids.expect_next_audit_id()
        .once()
        .returning(|| Ok(change::id(31)));
    let mut audit = MockAuditRepository::new();
    audit.expect_append().once().returning(|_, entry| {
        assert_eq!(entry.actor(), Actor::Admin(fixture::admin().id()));
        assert_eq!(entry.action(), AuditAction::DraftSaved);
        assert_eq!(entry.subject(), Subject::Change(change::id(1)));
        assert_eq!(entry.occurred_at(), fixture::now());
        assert_eq!(entry.reason(), ReasonCode::DraftChange);
        let AuditMetadata::Draft(metadata) = entry.metadata() else {
            panic!("wrong metadata")
        };
        assert_eq!(metadata.before(), Some(ChangeValue::Number(20)));
        assert_eq!(metadata.after(), ChangeValue::Number(20));
        assert_eq!(metadata.baseline_version(), change::id(2));
        assert_eq!(metadata.revision_id(), change::id(30));
        assert_eq!(metadata.scheduled_at().unwrap().zone(), "Asia/Tehran");
        Ok(())
    });
    let settings = MockSettingsRepository::new();
    let flags = MockFlagRepository::new();
    let output = SaveDraft {
        uow: &uow,
        sessions: &sessions,
        admins: &admins,
        settings: &settings,
        flags: &flags,
        changes: &changes,
        audit: &audit,
        clock: &clock,
        ids: &ids,
    }
    .execute(input())
    .await
    .unwrap();
    assert_eq!(output.head(), Some(change::id(30)));
}
