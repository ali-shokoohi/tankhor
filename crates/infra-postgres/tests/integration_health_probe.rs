use admin::application::seed_foundation::{SeedFoundation, SeedFoundationInput};
use infra_postgres::{
    audit_repository::PostgresAuditRepository, flag_repository::PostgresFlagRepository,
    health_probe::PostgresHealthProbe, settings_repository::PostgresSettingsRepository,
    unit_of_work::PostgresUnitOfWork,
};
use kernel::ids::{AdminId, AuditId};
use sqlx::{
    Connection, PgConnection,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{str::FromStr, time::Duration as StdDuration};
use testkit::{
    contracts::health_probe::{HealthProbeContractHarness, health_probe_contract},
    fakes::{clock::FixedClock, id_generator::SequenceIdGenerator},
    fixtures::database::FixtureDatabase,
};
use time::macros::datetime;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn health_probe_contract_postgres(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    database
        .verify_migrated_and_role_separation()
        .await
        .expect("restricted runtime role and migrations must be ready");

    let missing_name = format!("probe_missing_{}", uuid::Uuid::now_v7().simple());
    let seeded_name = format!("probe_seeded_{}", uuid::Uuid::now_v7().simple());
    let server_opts = database.server_admin_options();
    let mut server_conn = PgConnection::connect_with(&server_opts)
        .await
        .expect("connect server admin connection");
    for name in [&missing_name, &seeded_name] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE {name} OWNER tankhor_fixture_migration"
        )))
        .execute(&mut server_conn)
        .await
        .expect("create auxiliary probe database");
    }

    let seeded_migration_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(database.migration_options().database(&seeded_name))
        .await
        .expect("connect seeded migration pool");
    infra_postgres::MIGRATOR
        .run(&seeded_migration_pool)
        .await
        .expect("migrate seeded database");
    seeded_migration_pool.close().await;

    let missing_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(database.runtime_options().database(&missing_name))
        .await
        .expect("connect missing-registry runtime pool");
    let unseeded_pool = database
        .connect_runtime_pool(1)
        .await
        .expect("connect unseeded runtime pool");
    let seeded_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(database.runtime_options().database(&seeded_name))
        .await
        .expect("connect seeded runtime pool");

    let seeded_uow = PostgresUnitOfWork::new(seeded_pool.clone());
    let settings = PostgresSettingsRepository::new(seeded_uow.clone());
    let flags = PostgresFlagRepository::new(seeded_uow.clone());
    let audit = PostgresAuditRepository::new(seeded_uow.clone());
    let clock = FixedClock::new(datetime!(2026-01-01 00:00 UTC));
    let ids = SequenceIdGenerator::new(100);
    SeedFoundation {
        uow: &seeded_uow,
        settings: &settings,
        flags: &flags,
        audit: &audit,
        clock: &clock,
        ids: &ids,
    }
    .execute(SeedFoundationInput {
        operation: AuditId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
        system: AdminId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap(),
    })
    .await
    .expect("seed foundation registry in seeded database");

    let unavailable_pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(StdDuration::from_millis(100))
        .connect_lazy_with(
            database
                .runtime_options()
                .host("127.0.0.1")
                .port(1)
                .password("secret-probe-password"),
        );

    let missing_probe = PostgresHealthProbe::new(missing_pool.clone());
    let harness = HealthProbeContractHarness {
        unseeded_probe: PostgresHealthProbe::new(unseeded_pool.clone()),
        seeded_probe: PostgresHealthProbe::new(seeded_pool.clone()),
        unavailable_probe: PostgresHealthProbe::new(unavailable_pool.clone()),
    };

    health_probe_contract(&missing_probe, &harness).await;

    missing_pool.close().await;
    unseeded_pool.close().await;
    seeded_pool.close().await;
    unavailable_pool.close().await;

    for name in [&missing_name, &seeded_name] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE {name}")))
            .execute(&mut server_conn)
            .await
            .expect("drop auxiliary probe database");
    }
    server_conn.close().await.expect("close server connection");
    database.close().await;
}
