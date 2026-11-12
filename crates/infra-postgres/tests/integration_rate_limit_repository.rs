//! T213: unchanged T141 contract against real atomic PostgreSQL budgets.

use infra_postgres::{
    rate_limit_repository::PostgresRateLimitRepository, unit_of_work::PostgresUnitOfWork,
};
use kernel::{
    error::DomainError,
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
    },
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::sync::Mutex;
use testkit::contracts::rate_limit_repository::{
    RateLimitRepositoryContractHarness, rate_limit_repository_contract,
};
use testkit::fixtures::database::FixtureDatabase;
use time::OffsetDateTime;

struct TestClock(Mutex<OffsetDateTime>);

impl TestClock {
    fn new(now: OffsetDateTime) -> Self {
        Self(Mutex::new(now))
    }
    fn set(&self, now: OffsetDateTime) {
        *self.0.lock().expect("clock lock") = now;
    }
}

#[async_trait::async_trait]
impl Clock for TestClock {
    async fn now(&self) -> Result<OffsetDateTime, DomainError> {
        Ok(*self.0.lock().expect("clock lock"))
    }
}

struct Harness {
    database: FixtureDatabase,
    uow: PostgresUnitOfWork,
    repository: PostgresRateLimitRepository,
    clock: TestClock,
    foreign_uow: PostgresUnitOfWork,
}

#[async_trait::async_trait]
impl RateLimitRepositoryContractHarness for Harness {
    type Uow = PostgresUnitOfWork;
    type Repository = PostgresRateLimitRepository;
    type Clock = TestClock;

    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn repository(&self) -> &Self::Repository {
        &self.repository
    }
    fn clock(&self) -> &Self::Clock {
        &self.clock
    }
    fn set_time(&self, now: OffsetDateTime) {
        self.clock.set(now);
    }
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn rate_limit_repository_contract_postgres(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    database
        .verify_migrated_and_role_separation()
        .await
        .expect("fixture roles and migration ownership must be ready");
    let runtime = database
        .connect_runtime_pool(12)
        .await
        .expect("connect restricted runtime pool");
    let foreign = database
        .connect_runtime_pool(1)
        .await
        .expect("connect foreign restricted runtime pool");
    let uow = PostgresUnitOfWork::new(runtime);
    let harness = Harness {
        database,
        repository: PostgresRateLimitRepository::new(uow.clone()),
        foreign_uow: PostgresUnitOfWork::new(foreign),
        uow,
        clock: TestClock::new(OffsetDateTime::UNIX_EPOCH),
    };

    rate_limit_repository_contract(&harness).await;

    harness.uow.pool().close().await;
    harness.foreign_uow.pool().close().await;
    harness.database.close().await;
}
