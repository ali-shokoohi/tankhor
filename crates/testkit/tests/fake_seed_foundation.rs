use admin::{
    application::seed_foundation::{SeedFoundation, SeedFoundationInput, SeedOutcome},
    domain::{
        audit::AuditAction,
        settings::{SettingKey, SettingValue},
    },
    ports::settings_repository::{SettingVersion, SettingsRepository},
};
use kernel::{
    ids::{AdminId, AuditId, VersionId},
    ports::unit_of_work::{TransactionMode, UnitOfWork},
};
use std::str::FromStr;
use testkit::fakes::{
    audit_repository::FakeAuditRepository, clock::FixedClock, flag_repository::FakeFlagRepository,
    id_generator::SequenceIdGenerator, settings_repository::FakeSettingsRepository,
    store::FakeStore, unit_of_work::FakeUnitOfWork,
};
use time::{OffsetDateTime, macros::datetime};
fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-6100-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}

#[tokio::test]
async fn seed_rerun_preserves_published_values() {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let settings = FakeSettingsRepository::new(store.clone());
    let flags = FakeFlagRepository::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let clock = FixedClock::new(datetime!(2026-01-01 00:00 UTC));
    let ids = SequenceIdGenerator::new(1);
    let service = SeedFoundation {
        uow: &uow,
        settings: &settings,
        flags: &flags,
        audit: &audit,
        clock: &clock,
        ids: &ids,
    };
    let input = SeedFoundationInput {
        operation: id::<AuditId>(100),
        system: id::<AdminId>(99),
    };
    assert_eq!(service.execute(input).await.unwrap(), SeedOutcome::Seeded);
    let changed = SettingVersion::new(
        id::<VersionId>(200),
        SettingKey::UploadMaxMb,
        SettingValue::Number(25),
        id::<AdminId>(98),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
    let baseline = settings
        .get(&mut tx, SettingKey::UploadMaxMb)
        .await
        .unwrap()
        .unwrap()
        .version()
        .version();
    settings
        .append_version(&mut tx, changed.clone())
        .await
        .unwrap();
    settings
        .set_live(
            &mut tx,
            SettingKey::UploadMaxMb,
            Some(baseline),
            changed.version(),
        )
        .await
        .unwrap();
    uow.commit(tx).await.unwrap();
    assert_eq!(
        service.execute(input).await.unwrap(),
        SeedOutcome::AlreadyApplied
    );
    let state = store.try_read_committed().unwrap();
    assert_eq!(state.settings.len(), 10);
    assert_eq!(state.flags.len(), 6);
    assert!(state.flags.values().all(|flag| !flag.version().value()));
    assert_eq!(
        state
            .settings
            .get(&SettingKey::UploadMaxMb)
            .unwrap()
            .version()
            .value(),
        &SettingValue::Number(25)
    );
    assert!(state.admins.is_empty());
    assert_eq!(
        state
            .audit_entries
            .values()
            .filter(|entry| entry.action() == AuditAction::FoundationSeeded)
            .count(),
        1
    );
}
