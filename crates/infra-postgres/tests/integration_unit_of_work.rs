use std::path::Path;

use infra_postgres::unit_of_work::{PostgresUnitOfWork, map_commit_error};
use kernel::error::{DomainError, ErrorCode};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use testkit::contracts::unit_of_work::{UnitOfWorkContractHarness, unit_of_work_contract};
use testkit::fixtures::roles::provision_roles;

struct PostgresScratchHarness {
    primary_uow: PostgresUnitOfWork,
    foreign_uow: PostgresUnitOfWork,
    reader_pool: PgPool,
}

#[async_trait::async_trait]
impl UnitOfWorkContractHarness for PostgresScratchHarness {
    async fn write_entry(
        &self,
        tx: &mut TxHandle,
        key: &str,
        value: &str,
    ) -> Result<(), DomainError> {
        let conn = self.primary_uow.writable_connection(tx)?;
        sqlx::query(
            "INSERT INTO test_uow_scratch (entry_key, entry_value) \
             VALUES ($1, $2) \
             ON CONFLICT (entry_key) DO UPDATE SET entry_value = EXCLUDED.entry_value",
        )
        .bind(key)
        .bind(value)
        .execute(conn)
        .await
        .map_err(|_| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))?;
        Ok(())
    }

    async fn read_in_tx(
        &self,
        tx: &mut TxHandle,
        key: &str,
    ) -> Result<Option<String>, DomainError> {
        let conn = self.primary_uow.readable_connection(tx)?;
        sqlx::query_scalar::<_, String>(
            "SELECT entry_value FROM test_uow_scratch WHERE entry_key = $1",
        )
        .bind(key)
        .fetch_optional(conn)
        .await
        .map_err(|_| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
    }

    async fn read_committed(&self, key: &str) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar::<_, String>(
            "SELECT entry_value FROM test_uow_scratch WHERE entry_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.reader_pool)
        .await
        .map_err(|_| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
    }

    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }

    async fn simulate_unknown_commit(&self) -> Result<(), DomainError> {
        Err(map_commit_error(&sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "connection reset during COMMIT",
        ))))
    }
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn postgres_uow_passes_shared_contract(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let admin_pool = admin_pool_options
        .max_connections(1)
        .connect_with(admin_options.clone())
        .await
        .expect("connect admin pool");

    let passfile = std::env::var_os("PGPASSFILE").expect("PGPASSFILE must be set by fixture");
    let credential_dir = Path::new(&passfile)
        .parent()
        .expect("PGPASSFILE must live inside fixture credential directory");
    let roles = provision_roles(&admin_options, credential_dir)
        .await
        .expect("fixture roles must be provisioned");

    sqlx::query(
        "CREATE TABLE test_uow_scratch (
            entry_key TEXT PRIMARY KEY,
            entry_value TEXT NOT NULL
        )",
    )
    .execute(&admin_pool)
    .await
    .expect("create test-owned scratch table");
    sqlx::query(
        "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE test_uow_scratch TO tankhor_fixture_runtime",
    )
    .execute(&admin_pool)
    .await
    .expect("grant runtime privileges on scratch table");
    admin_pool.close().await;

    let runtime_options = roles.runtime(&admin_options);
    let primary_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(runtime_options.clone())
        .await
        .expect("connect primary runtime pool");
    let reader_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(runtime_options.clone())
        .await
        .expect("connect independent reader runtime pool");
    let foreign_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(runtime_options)
        .await
        .expect("connect foreign runtime pool");

    let store_id = kernel::ports::unit_of_work::StoreId::new();
    let harness = PostgresScratchHarness {
        primary_uow: PostgresUnitOfWork::with_store_id(primary_pool.clone(), store_id),
        foreign_uow: PostgresUnitOfWork::new(foreign_pool.clone()),
        reader_pool: reader_pool.clone(),
    };

    unit_of_work_contract(
        &PostgresUnitOfWork::with_store_id(primary_pool.clone(), store_id),
        &harness,
    )
    .await;

    // Close extra runtime pools so SQLx test teardown can drop the isolated database cleanly.
    primary_pool.close().await;
    reader_pool.close().await;
    foreign_pool.close().await;
}
