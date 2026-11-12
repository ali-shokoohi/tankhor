use infra_postgres::audit_repository::PostgresAuditRepository;
use infra_postgres::unit_of_work::PostgresUnitOfWork;
use kernel::error::DomainError;
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::contracts::audit_repository::{
    AuditRepositoryContractHarness, audit_repository_contract,
};
use testkit::fixtures::database::FixtureDatabase;

struct Harness {
    database: FixtureDatabase,
    uow: PostgresUnitOfWork,
    foreign_uow: PostgresUnitOfWork,
    repository: PostgresAuditRepository,
}

#[async_trait::async_trait]
impl AuditRepositoryContractHarness for Harness {
    type Uow = PostgresUnitOfWork;
    type Repository = PostgresAuditRepository;

    fn uow(&self) -> &Self::Uow {
        &self.uow
    }

    fn repository(&self) -> &Self::Repository {
        &self.repository
    }

    async fn fail_next_append(&self) -> Result<(), DomainError> {
        // The fixture's nontransactional sequence is armed for this contract call.
        // Avoid changing database objects while the caller's transaction holds locks.
        Ok(())
    }

    async fn write_companion(&self, tx: &mut TxHandle) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        sqlx::query(
            "INSERT INTO audit_log (
                 id, event_key, actor_type, actor_id, action, subject_type, subject_id,
                 occurred_at, reason_code, factor
             ) VALUES (
                 '019a53a0-0000-7000-8000-00000000aa01',
                 '019a53a0-0000-7000-8000-00000000aa02',
                 'admin', '019a53a0-0000-7000-8000-000000000001',
                 'signed_in', 'admin', '019a53a0-0000-7000-8000-000000000001',
                 '1970-01-01T00:00:00Z', 'authentication', 'passkey'
             )",
        )
        .execute(connection)
        .await
        .map_err(|_| unavailable())?;
        Ok(())
    }

    async fn companion_exists(&self, tx: &mut TxHandle) -> Result<bool, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM audit_log
                 WHERE id = '019a53a0-0000-7000-8000-00000000aa01'
             )",
        )
        .fetch_one(connection)
        .await
        .map_err(|_| unavailable())
    }

    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }
}

fn unavailable() -> DomainError {
    DomainError::unavailable(kernel::error::ErrorCode::PrerequisiteUnavailable)
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn audit_repository_contract_postgres(
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
        .connect_runtime_pool(4)
        .await
        .expect("connect restricted runtime pool");
    let foreign = database
        .connect_runtime_pool(1)
        .await
        .expect("connect foreign restricted runtime pool");
    let uow = PostgresUnitOfWork::new(runtime);
    let repository = PostgresAuditRepository::new(uow.clone());
    sqlx::raw_sql(
        "CREATE SEQUENCE contract_audit_failure MINVALUE -10 START -10;
         CREATE FUNCTION contract_fail_next_audit_append()
         RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER AS $$
         BEGIN
             IF nextval('contract_audit_failure') = 1 THEN
                 RAISE EXCEPTION 'contract-injected audit dependency failure';
             END IF;
             RETURN NEW;
         END;
         $$;
         CREATE TRIGGER contract_fail_next_audit_append
         BEFORE INSERT ON audit_log
         FOR EACH ROW EXECUTE FUNCTION contract_fail_next_audit_append();",
    )
    .execute(database.admin_pool())
    .await
    .expect("install one-shot audit failure fixture");
    let harness = Harness {
        database,
        foreign_uow: PostgresUnitOfWork::new(foreign),
        uow,
        repository,
    };

    audit_repository_contract(&harness).await;

    harness.uow.pool().close().await;
    harness.foreign_uow.pool().close().await;
    harness.database.close().await;
}
