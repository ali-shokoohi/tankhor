use admin::{
    application::seed_foundation::{SeedFoundation, SeedFoundationInput, SeedOutcome},
    domain::audit::{Actor, AuditAction, ReasonCode, Subject},
    ports::{
        audit_repository::MockAuditRepository, flag_repository::MockFlagRepository,
        settings_repository::MockSettingsRepository,
    },
};
use kernel::{
    ids::{AdminId, AuditId, VersionId},
    ports::{
        clock::MockClock,
        id_generator::MockIdGenerator,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};
use time::macros::datetime;
fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-6000-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}

#[tokio::test]
async fn seed_records_defaults_flags_and_audit_once() {
    let store = StoreId::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut uow = MockUnitOfWork::new();
    uow.expect_begin().once().returning(move |mode| {
        assert_eq!(mode, TransactionMode::ReadWrite);
        Ok(TxHandle::new(store, mode, ()))
    });
    uow.expect_commit().once().returning(move |mut tx| {
        tx.take::<()>(store)?;
        Ok(())
    });
    let mut settings = MockSettingsRepository::new();
    settings.expect_list().once().return_once(|_| Ok(vec![]));
    let log = events.clone();
    settings.expect_seed().once().returning(move |_, versions| {
        assert_eq!(versions.len(), 10);
        log.lock().unwrap().push("settings");
        Ok(())
    });
    let mut flags = MockFlagRepository::new();
    flags.expect_list().once().return_once(|_| Ok(vec![]));
    let log = events.clone();
    flags.expect_seed().once().returning(move |_, versions| {
        assert_eq!(versions.len(), 6);
        assert!(versions.iter().all(|version| !version.value()));
        log.lock().unwrap().push("flags");
        Ok(())
    });
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    audit.expect_append().once().returning(move |_, entry| {
        assert_eq!(entry.id(), id::<AuditId>(100));
        assert_eq!(entry.event_key(), id::<AuditId>(100));
        assert_eq!(entry.actor(), Actor::Admin(id::<AdminId>(99)));
        assert_eq!(entry.subject(), Subject::Admin(id::<AdminId>(99)));
        assert_eq!(entry.action(), AuditAction::FoundationSeeded);
        assert_eq!(entry.reason(), ReasonCode::FoundationSeed);
        log.lock().unwrap().push("audit");
        Ok(())
    });
    let mut clock = MockClock::new();
    clock
        .expect_now()
        .once()
        .return_once(|| Ok(datetime!(2026-01-01 00:00 UTC)));
    let mut ids = MockIdGenerator::new();
    let mut next = 0_u16;
    ids.expect_next_version_id().times(16).returning(move || {
        next += 1;
        Ok(id::<VersionId>(next))
    });
    let outcome = SeedFoundation {
        uow: &uow,
        settings: &settings,
        flags: &flags,
        audit: &audit,
        clock: &clock,
        ids: &ids,
    }
    .execute(SeedFoundationInput {
        operation: id(100),
        system: id(99),
    })
    .await
    .unwrap();
    assert_eq!(outcome, SeedOutcome::Seeded);
    assert_eq!(*events.lock().unwrap(), vec!["settings", "flags", "audit"]);
}
