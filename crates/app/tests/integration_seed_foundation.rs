#![cfg(feature = "dev-fakes")]

use admin::application::health::{HealthReport, get_health};
use admin::application::seed_foundation::SeedOutcome;
use admin::domain::audit::AuditAction;
use admin::domain::settings::{SettingKey, SettingValue};
use admin::ports::audit_repository::{AuditFilter, AuditQuery};
use admin::ports::health_probe::ProbeReadiness;
use admin::ports::settings_repository::SettingVersion;
use app::composition::{
    BackendGroup, compose_fake_registry, compose_real_registry, compose_registry,
};
use app::config::AppConfig;
use app::migrate::{MigrateError, run_migrate, validate_migrate_command};
use app::seed_foundation::{SeedFoundationCommand, SeedFoundationError, run_seed_foundation};
use kernel::error::{ErrorCategory, ErrorCode};
use kernel::ids::{AdminId, AuditId, VersionId};
use kernel::ports::unit_of_work::TransactionMode;
use std::str::FromStr;
use testkit::fixtures::database::ProvisionedDatabaseUrls;
use time::OffsetDateTime;

fn admin_id(n: u16) -> AdminId {
    AdminId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap()
}

fn audit_id(n: u16) -> AuditId {
    AuditId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap()
}

fn version_id(n: u16) -> VersionId {
    VersionId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap()
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn foundation_seed_command_updates_health_and_is_idempotent() {
    // 1. Command parsing rejects secret-bearing or unexpected CLI arguments without echoing secrets.
    let system_id = admin_id(901);
    let op_id = audit_id(902);
    let second_op_id = audit_id(903);

    assert!(
        validate_migrate_command([
            "--database-url",
            "postgresql://secret-migration-pass@host/db"
        ])
        .is_err()
    );
    let migrate_err = validate_migrate_command([
        "--database-url",
        "postgresql://secret-migration-pass@host/db",
    ])
    .unwrap_err()
    .to_string();
    assert!(!migrate_err.contains("secret-migration-pass"));

    let bad_seed_arg = SeedFoundationCommand::parse([
        "--system-id".to_owned(),
        system_id.to_string(),
        "--operation-id".to_owned(),
        op_id.to_string(),
        "--database-url".to_owned(),
        "postgresql://secret-runtime-pass@host/db".to_owned(),
    ])
    .unwrap_err();
    assert!(!bad_seed_arg.to_string().contains("secret-runtime-pass"));

    // 2. Production + fake mode is rejected before any adapter or storage acquisition.
    let prod_fake =
        AppConfig::from_pairs([("TANKHOR_ENV", "production"), ("TANKHOR_ADAPTERS", "fake")]);
    assert!(prod_fake.is_err());

    // 3. Fake mode never contacts external storage (even with no DATABASE_URL) and updates health probe.
    let fake_config =
        AppConfig::from_pairs([("TANKHOR_ENV", "development"), ("TANKHOR_ADAPTERS", "fake")])
            .unwrap();
    let fake_registry = compose_registry(&fake_config).unwrap();
    assert_eq!(fake_registry.backend(), BackendGroup::Fake);
    assert_eq!(
        fake_registry.health_probe().check().await.unwrap(),
        ProbeReadiness::Unseeded
    );
    assert!(
        get_health(fake_registry.health_probe().as_ref())
            .await
            .is_err()
    );

    // Fake-mode migrate succeeds without contacting any external database.
    run_migrate(&fake_config, None).await.unwrap();

    let fake_cmd = SeedFoundationCommand::parse([
        "--system-id".to_owned(),
        system_id.to_string(),
        "--operation-id".to_owned(),
        op_id.to_string(),
    ])
    .unwrap();
    let fake_first = run_seed_foundation(fake_cmd, &fake_registry).await.unwrap();
    assert_eq!(fake_first, SeedOutcome::Seeded);
    assert_eq!(
        fake_registry.health_probe().check().await.unwrap(),
        ProbeReadiness::Ready
    );
    assert_eq!(
        get_health(fake_registry.health_probe().as_ref())
            .await
            .unwrap(),
        HealthReport::Ready
    );

    let fake_second_cmd = SeedFoundationCommand::parse([
        "--system-id".to_owned(),
        system_id.to_string(),
        "--operation-id".to_owned(),
        second_op_id.to_string(),
    ])
    .unwrap();
    let fake_second = run_seed_foundation(fake_second_cmd, &fake_registry)
        .await
        .unwrap();
    assert_eq!(fake_second, SeedOutcome::AlreadyApplied);

    // 4. Real mode against isolated PostgreSQL database (initially unmigrated).
    let db_name = format!("seed_foundation_{}", std::process::id());
    let provisioned = ProvisionedDatabaseUrls::create_from_env(&db_name)
        .await
        .expect("create isolated unmigrated database from fixture env");

    let real_config =
        AppConfig::from_pairs([("TANKHOR_ENV", "development"), ("TANKHOR_ADAPTERS", "real")])
            .unwrap();

    let migration_url = provisioned.migration_url().to_owned();
    let runtime_url = provisioned.runtime_url().to_owned();

    // Running `migrate` with the restricted runtime role MUST fail (wrong migration role).
    let wrong_role_migrate = run_migrate(&real_config, Some(&runtime_url)).await;
    assert!(
        matches!(wrong_role_migrate, Err(MigrateError::MigrationFailed)),
        "restricted runtime role must not be permitted to run DDL migrations"
    );

    // Before migration, real health probe reports Unseeded (missing tables).
    let real_registry = compose_real_registry(&runtime_url).unwrap();
    assert_eq!(real_registry.backend(), BackendGroup::Real);
    assert_eq!(
        real_registry.health_probe().check().await.unwrap(),
        ProbeReadiness::Unseeded
    );

    // Running `migrate` with the privileged migration role succeeds.
    run_migrate(&real_config, Some(&migration_url))
        .await
        .expect("privileged migration role must apply canonical migrations");

    // Migrated but unseeded database still reports Unseeded -> 503 Unavailable.
    assert_eq!(
        real_registry.health_probe().check().await.unwrap(),
        ProbeReadiness::Unseeded
    );
    let unseeded_health = get_health(real_registry.health_probe().as_ref())
        .await
        .unwrap_err();
    assert_eq!(unseeded_health.category(), ErrorCategory::Unavailable);
    assert_eq!(unseeded_health.code(), ErrorCode::PrerequisiteUnavailable);

    // Running `seed-foundation` with the privileged migration role MUST be rejected (wrong runtime role).
    let migration_role_registry = compose_real_registry(&migration_url).unwrap();
    let wrong_role_seed = run_seed_foundation(fake_cmd, &migration_role_registry).await;
    assert!(
        matches!(
            wrong_role_seed,
            Err(SeedFoundationError::PrivilegeViolation)
        ),
        "seed-foundation must refuse to run under privileged migration/superuser credentials"
    );
    migration_role_registry.close().await;

    // Running `seed-foundation` with the restricted runtime role seeds the 10 settings + 6 OFF flags + 1 audit entry.
    let seeded_outcome = run_seed_foundation(fake_cmd, &real_registry)
        .await
        .expect("restricted runtime role must seed foundation registry");
    assert_eq!(seeded_outcome, SeedOutcome::Seeded);

    // HealthProbe now transitions to Ready.
    assert_eq!(
        real_registry.health_probe().check().await.unwrap(),
        ProbeReadiness::Ready
    );
    assert_eq!(
        get_health(real_registry.health_probe().as_ref())
            .await
            .unwrap(),
        HealthReport::Ready
    );

    // Advance one published setting value (`upload_max_mb` from 15 to 24) and verify seed rerun is idempotent and does not overwrite it.
    let mut tx = real_registry
        .uow()
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    let current = real_registry
        .settings()
        .get(&mut tx, SettingKey::UploadMaxMb)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.version().value(), &SettingValue::Number(15));
    let updated_version = SettingVersion::new(
        version_id(950),
        SettingKey::UploadMaxMb,
        SettingValue::Number(24),
        system_id,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    real_registry
        .settings()
        .append_version(&mut tx, updated_version.clone())
        .await
        .unwrap();
    real_registry
        .settings()
        .set_live(
            &mut tx,
            SettingKey::UploadMaxMb,
            Some(current.version().version()),
            updated_version.version(),
        )
        .await
        .unwrap();
    real_registry.uow().commit(tx).await.unwrap();

    // Rerun `seed-foundation` -> AlreadyApplied, published value remains 24, no extra foundation_seeded audit row.
    let rerun_outcome = run_seed_foundation(fake_second_cmd, &real_registry)
        .await
        .expect("seed rerun must succeed idempotently");
    assert_eq!(rerun_outcome, SeedOutcome::AlreadyApplied);

    let mut read_tx = real_registry
        .uow()
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    let after_rerun = real_registry
        .settings()
        .get(&mut read_tx, SettingKey::UploadMaxMb)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_rerun.version().value(), &SettingValue::Number(24));
    let audit_page = real_registry
        .audit()
        .search(
            &mut read_tx,
            AuditQuery::new(
                AuditFilter::new(None, Some(AuditAction::FoundationSeeded), None, None, None)
                    .unwrap(),
                None,
                10,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(audit_page.entries.len(), 1);
    real_registry.uow().rollback(read_tx).await.unwrap();

    let fake_direct = compose_fake_registry().unwrap();
    assert_eq!(fake_direct.backend(), BackendGroup::Fake);

    real_registry.close().await;
    provisioned
        .drop_database()
        .await
        .expect("drop auxiliary seed_foundation database");
}
