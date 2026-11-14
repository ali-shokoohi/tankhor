use infra_postgres::settings_repository::PostgresSettingsRepository;
use infra_postgres::unit_of_work::PostgresUnitOfWork;
use kernel::error::DomainError;
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::contracts::settings_repository::{
    SettingsRepositoryContractHarness, settings_repository_contract,
};
use testkit::fixtures::database::FixtureDatabase;

struct Harness {
    uow: PostgresUnitOfWork,
    foreign_uow: PostgresUnitOfWork,
    repository: PostgresSettingsRepository,
}

#[async_trait::async_trait]
impl SettingsRepositoryContractHarness for Harness {
    type Uow = PostgresUnitOfWork;
    type Repository = PostgresSettingsRepository;

    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn repository(&self) -> &Self::Repository {
        &self.repository
    }
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn settings_repository_contract_postgres(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .unwrap();
    database
        .verify_migrated_and_role_separation()
        .await
        .unwrap();
    sqlx::raw_sql(
        "INSERT INTO admins (
             id, phone_key_id, phone_nonce, phone_ciphertext, phone_lookup_digest,
             eligibility, security_epoch, created_by, created_at
         ) VALUES
         ('019a53a0-1000-7000-8000-000000000001', 'test', '\\x01', '\\x01',
          '\\x0000000000000000000000000000000000000000000000000000000000000011',
          'active', 0, '019a53a0-1000-7000-8000-000000000001', '1970-01-01T00:00:00Z'),
         ('019a53a0-1000-7000-8000-000000000002', 'test', '\\x01', '\\x01',
          '\\x0000000000000000000000000000000000000000000000000000000000000012',
          'active', 0, '019a53a0-1000-7000-8000-000000000001', '1970-01-01T00:00:00Z')",
    )
    .execute(database.admin_pool())
    .await
    .unwrap();
    let uow = PostgresUnitOfWork::new(database.connect_runtime_pool(2).await.unwrap());
    let foreign_uow = PostgresUnitOfWork::new(database.connect_runtime_pool(1).await.unwrap());
    let repository = PostgresSettingsRepository::new(uow.clone());
    let harness = Harness {
        uow,
        foreign_uow,
        repository,
    };
    settings_repository_contract(&harness).await;
    harness.uow.pool().close().await;
    harness.foreign_uow.pool().close().await;
}
