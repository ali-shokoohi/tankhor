//! T211: unchanged T137 contract against the real restricted PostgreSQL adapter.

use infra_postgres::{
    admin_repository::PostgresAdminRepository,
    otp_challenge_repository::PostgresOtpChallengeRepository, unit_of_work::PostgresUnitOfWork,
};
use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::contracts::otp_challenge_repository::{
    OtpChallengeRepositoryContractHarness, otp_challenge_repository_contract,
};
use testkit::fixtures::database::FixtureDatabase;

struct Harness {
    database: FixtureDatabase,
    uow: PostgresUnitOfWork,
    repository: PostgresOtpChallengeRepository,
    admins: PostgresAdminRepository,
    foreign_uow: PostgresUnitOfWork,
}

#[async_trait::async_trait]
impl OtpChallengeRepositoryContractHarness for Harness {
    type Uow = PostgresUnitOfWork;
    type Repository = PostgresOtpChallengeRepository;
    type Admins = PostgresAdminRepository;

    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn repository(&self) -> &Self::Repository {
        &self.repository
    }
    fn admins(&self) -> &Self::Admins {
        &self.admins
    }
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn otp_challenge_repository_contract_postgres(
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
        repository: PostgresOtpChallengeRepository::new(uow.clone()),
        admins: PostgresAdminRepository::new(uow.clone()),
        foreign_uow: PostgresUnitOfWork::new(foreign),
        uow,
    };

    otp_challenge_repository_contract(&harness).await;

    harness.uow.pool().close().await;
    harness.foreign_uow.pool().close().await;
    harness.database.close().await;
}
