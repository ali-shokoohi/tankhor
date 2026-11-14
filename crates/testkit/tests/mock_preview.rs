//! T338: exact-revision pure assessment and one atomic preview/audit write.
use admin::{
    application::preview::{ImpactAssessment, Preview, PreviewEvidence, PreviewInput},
    domain::{
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
        formats::FormatSet,
        impact_uploads::UploadPolicySnapshot,
        settings::{SettingKey, SettingValue},
    },
    ports::{
        admin_repository::MockAdminRepository,
        audit_repository::MockAuditRepository,
        change_repository::MockChangeRepository,
        flag_repository::MockFlagRepository,
        session_store::MockSessionStore,
        settings_repository::{MockSettingsRepository, SettingRecord, SettingVersion},
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

#[tokio::test]
async fn preview_reads_exact_revision_assesses_once_and_never_executes_effects() {
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
    changes.expect_draft().once().returning(|_, id| {
        assert_eq!(id, change::id(1));
        Ok(Some(change::draft()))
    });
    changes.expect_save_preview().once().returning(|_, report| {
        assert_eq!(report.id(), change::id(30));
        assert_eq!(report.revision_id(), change::id(3));
        assert_eq!(report.baseline_version(), change::id(2));
        assert_eq!(report.reviewed_by(), fixture::admin().id());
        assert_ne!(report.reviewed_by(), change::creator());
        assert_eq!(report.provenance().policy(), "upload-validation/v1");
        assert_eq!(
            report.effect_rule().as_str(),
            "preserve-stored/validate-new-only"
        );
        Ok(())
    });
    let baseline = SettingVersion::new(
        change::id(2),
        SettingKey::UploadMaxMb,
        SettingValue::Number(15),
        fixture::admin().id(),
        fixture::now(),
    )
    .unwrap();
    let mut settings = MockSettingsRepository::new();
    settings.expect_get().once().return_once(move |_, key| {
        assert_eq!(key, SettingKey::UploadMaxMb);
        Ok(Some(SettingRecord::new(baseline)))
    });
    let flags = MockFlagRepository::new();
    let mut ids = MockIdGenerator::new();
    ids.expect_next_preview_id()
        .once()
        .returning(|| Ok(change::id(30)));
    ids.expect_next_audit_id()
        .once()
        .returning(|| Ok(change::id(31)));
    let mut audit = MockAuditRepository::new();
    audit.expect_append().once().returning(|_, entry| {
        assert_eq!(entry.actor(), Actor::Admin(fixture::admin().id()));
        assert_eq!(entry.action(), AuditAction::Previewed);
        assert_eq!(entry.subject(), Subject::Change(change::id(1)));
        assert_eq!(entry.reason(), ReasonCode::PreviewAssessment);
        assert_eq!(entry.occurred_at(), fixture::now());
        let AuditMetadata::Preview(metadata) = entry.metadata() else {
            panic!("wrong metadata")
        };
        assert_eq!(metadata.revision_id(), change::id(3));
        assert_eq!(metadata.baseline_version(), change::id(2));
        assert_eq!(
            metadata.digest(),
            admin::domain::change_hash::draft_hash(&change::draft()).unwrap()
        );
        Ok(())
    });
    let output = Preview {
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
    .execute(PreviewInput {
        actor: fixture::digest(),
        change: change::id::<kernel::ids::ChangeId>(1).to_string(),
        revision: change::id::<kernel::ids::RevisionId>(3).to_string(),
        evidence: PreviewEvidence::Upload(UploadPolicySnapshot::new(20, 512, FormatSet::all())),
    })
    .await
    .unwrap();
    assert!(matches!(output.impact, ImpactAssessment::Upload(_)));
    assert_eq!(output.report.id(), change::id(30));
}

#[tokio::test]
async fn audit_preview_has_complete_safe_fields_once() {
    let store = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    uow.expect_begin().once().returning(move |mode| {
        assert_eq!(mode, TransactionMode::ReadWrite);
        Ok(TxHandle::new(store, mode, ()))
    });
    uow.expect_commit().never();
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
    changes.expect_save_preview().once().returning(|_, report| {
        assert_eq!(report.reviewed_by(), fixture::admin().id());
        Ok(())
    });
    let baseline = SettingVersion::new(
        change::id(2),
        SettingKey::UploadMaxMb,
        SettingValue::Number(15),
        fixture::admin().id(),
        fixture::now(),
    )
    .unwrap();
    let mut settings = MockSettingsRepository::new();
    settings
        .expect_get()
        .once()
        .return_once(move |_, _| Ok(Some(SettingRecord::new(baseline))));
    let flags = MockFlagRepository::new();
    let mut ids = MockIdGenerator::new();
    ids.expect_next_preview_id()
        .once()
        .returning(|| Ok(change::id(30)));
    ids.expect_next_audit_id()
        .once()
        .returning(|| Ok(change::id(31)));
    let mut audit = MockAuditRepository::new();
    audit.expect_append().once().returning(|_, entry| {
        assert_eq!(entry.actor(), Actor::Admin(fixture::admin().id()));
        assert_eq!(entry.action(), AuditAction::Previewed);
        assert_eq!(entry.subject(), Subject::Change(change::id(1)));
        assert_eq!(entry.reason(), ReasonCode::PreviewAssessment);
        assert_eq!(entry.occurred_at(), fixture::now());
        Err(kernel::error::DomainError::unavailable(
            kernel::error::ErrorCode::PrerequisiteUnavailable,
        ))
    });
    assert!(
        Preview {
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
        .execute(PreviewInput {
            actor: fixture::digest(),
            change: change::id::<kernel::ids::ChangeId>(1).to_string(),
            revision: change::id::<kernel::ids::RevisionId>(3).to_string(),
            evidence: PreviewEvidence::Upload(
                UploadPolicySnapshot::new(20, 512, FormatSet::all(),)
            ),
        })
        .await
        .is_err()
    );
}
