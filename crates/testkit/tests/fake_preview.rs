//! T339: immutable exact previews, known/unknown impact and no downstream effects.
use admin::{
    application::preview::{ImpactAssessment, Preview, PreviewEvidence, PreviewInput},
    domain::{
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        flag::{FlagKey, launch_flags},
        flag_readiness::ReadinessStatus,
        formats::FormatSet,
        impact_lifetime::{LifetimeItem, LifetimePopulation, LifetimeReference},
        impact_recovery::ExistingRecovery,
        impact_uploads::UploadPolicySnapshot,
        settings::{SettingKey, SettingValue},
    },
    ports::{
        flag_repository::{FlagRecord, FlagVersion},
        session_store::SessionRecord,
        settings_repository::{SettingRecord, SettingVersion},
    },
};
use kernel::ids::{ChangeId, RevisionId};
use testkit::fakes::{
    admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
    change_repository::FakeChangeRepository, clock::FixedClock,
    flag_repository::FakeFlagRepository, id_generator::SequenceIdGenerator,
    session_store::FakeSessionStore, settings_repository::FakeSettingsRepository, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
use time::macros::datetime;
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
    fn setting(&self, key: SettingKey, previous: u32, proposed: u32) {
        let version = SettingVersion::new(
            change::id(2),
            key,
            SettingValue::Number(i128::from(previous)),
            fixture::admin().id(),
            fixture::now(),
        )
        .unwrap();
        let draft = ChangeDraft::new(
            change::id(1),
            ChangeSubject::Setting(key),
            change::id(2),
            change::creator(),
        )
        .append_revision(
            ChangeRevision::new(
                change::id(3),
                change::id(1),
                1,
                ChangeValue::Number(proposed),
                None,
                change::creator(),
                fixture::now(),
            ),
            None,
        )
        .unwrap();
        self.store.seed(|state| {
            state
                .settings
                .insert(key, SettingRecord::new(version.clone()));
            state.setting_history.insert(key, vec![version]);
            state.changes.insert(draft.id(), draft);
        });
    }
    fn flag(&self, key: FlagKey) {
        let version = FlagVersion::new(
            change::id(2),
            key,
            false,
            fixture::admin().id(),
            fixture::now(),
        );
        let draft = ChangeDraft::new(
            change::id(1),
            ChangeSubject::Flag(key),
            change::id(2),
            change::creator(),
        )
        .append_revision(
            ChangeRevision::new(
                change::id(3),
                change::id(1),
                1,
                ChangeValue::Boolean(true),
                None,
                change::creator(),
                fixture::now(),
            ),
            None,
        )
        .unwrap();
        self.store.seed(|state| {
            state.flags.insert(key, FlagRecord::new(version));
            state.changes.insert(draft.id(), draft);
        });
    }
    fn service(&self) -> Preview<'_> {
        Preview {
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
    #[allow(clippy::unused_self)]
    fn input(&self, evidence: PreviewEvidence) -> PreviewInput {
        PreviewInput {
            actor: fixture::digest(),
            change: change::id::<ChangeId>(1).to_string(),
            revision: change::id::<RevisionId>(3).to_string(),
            evidence,
        }
    }
}

#[tokio::test]
async fn preview_uses_exact_draft_without_live_effects() {
    let lifetime = Harness::new();
    lifetime.setting(SettingKey::BuyerTempPhotoDays, 14, 7);
    let before = lifetime.store.try_read_committed().unwrap();
    let output = lifetime
        .service()
        .execute(
            lifetime.input(PreviewEvidence::Lifetime(LifetimePopulation::Snapshots(
                vec![LifetimeItem::new(
                    datetime!(2025-12-20 00:00 UTC),
                    datetime!(2026-01-03 00:00 UTC),
                )],
            ))),
        )
        .await
        .unwrap();
    let ImpactAssessment::Lifetime(impact) = output.impact else {
        panic!("wrong impact")
    };
    assert_eq!(impact.reference(), LifetimeReference::OriginalUpload);
    assert_eq!(output.report.evidence()[0].known_count(), Some(1));
    assert_eq!(output.report.evidence()[1].known_count(), Some(1));
    let after = lifetime.store.try_read_committed().unwrap();
    assert_eq!(after.settings, before.settings);
    assert_eq!(after.previews.len(), 1);
    assert_eq!(after.audit_entries.len(), 1);
    assert_ne!(change::creator(), fixture::admin().id());
    assert_eq!(output.report.reviewed_by(), fixture::admin().id());
    assert_eq!(after.previews[&output.report.id()], output.report);

    let unknown = Harness::new();
    unknown.setting(SettingKey::BuyerTempPhotoDays, 14, 7);
    let output = unknown
        .service()
        .execute(unknown.input(PreviewEvidence::Lifetime(LifetimePopulation::Unavailable)))
        .await
        .unwrap();
    assert!(
        output
            .report
            .evidence()
            .iter()
            .all(admin::domain::change_preview::PopulationEvidence::is_unavailable)
    );

    let recovery = Harness::new();
    recovery.setting(SettingKey::AccountRecoveryDays, 30, 14);
    let output = recovery
        .service()
        .execute(recovery.input(PreviewEvidence::Recovery(
            admin::domain::impact_recovery::PendingRecoverySnapshot::new(
                datetime!(2026-02-01 00:00 UTC),
            ),
        )))
        .await
        .unwrap();
    let ImpactAssessment::Recovery(impact) = output.impact else {
        panic!("wrong impact")
    };
    assert_eq!(
        impact.existing_recovery(),
        ExistingRecovery::EndDatePreserved
    );
    assert_eq!(impact.pending_end_date(), datetime!(2026-02-01 00:00 UTC));
    assert_eq!(
        impact.new_request_end_date(),
        datetime!(2026-01-15 00:00 UTC)
    );

    for flag in launch_flags() {
        let harness = Harness::new();
        harness.flag(flag.key());
        let output = harness
            .service()
            .execute(harness.input(PreviewEvidence::None))
            .await
            .unwrap();
        assert_eq!(
            output.report.readiness(),
            Some(ReadinessStatus::NotDelivered)
        );
        assert!(
            !harness.store.try_read_committed().unwrap().flags[&flag.key()]
                .version()
                .value()
        );
    }
}

#[tokio::test]
async fn audit_preview_has_complete_safe_fields_once() {
    let harness = Harness::new();
    harness.setting(SettingKey::UploadMaxMb, 15, 20);
    let before = harness.store.try_read_committed().unwrap();
    harness.audit.fail_next_append();
    assert!(
        harness
            .service()
            .execute(
                harness.input(PreviewEvidence::Upload(UploadPolicySnapshot::new(
                    20,
                    512,
                    FormatSet::all()
                ),))
            )
            .await
            .is_err()
    );
    let after = harness.store.try_read_committed().unwrap();
    assert!(after.previews.is_empty());
    assert!(after.audit_entries.is_empty());
    assert_eq!(after.settings, before.settings);
}
