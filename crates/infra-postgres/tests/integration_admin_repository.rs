use infra_postgres::admin_repository::PostgresAdminRepository;
use infra_postgres::audit_repository::PostgresAuditRepository;
use infra_postgres::unit_of_work::PostgresUnitOfWork;
use kernel::error::DomainError;
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::contracts::admin_repository::{
    AdminRepositoryContractHarness, admin_repository_contract,
};
use testkit::fixtures::database::FixtureDatabase;

struct Harness {
    database: FixtureDatabase,
    uow: PostgresUnitOfWork,
    foreign_uow: PostgresUnitOfWork,
    repository: PostgresAdminRepository,
    audit: PostgresAuditRepository,
}

#[async_trait::async_trait]
impl AdminRepositoryContractHarness for Harness {
    type Uow = PostgresUnitOfWork;
    type Repository = PostgresAdminRepository;
    type Audit = PostgresAuditRepository;

    fn uow(&self) -> &Self::Uow {
        &self.uow
    }

    fn repository(&self) -> &Self::Repository {
        &self.repository
    }

    fn audit(&self) -> &Self::Audit {
        &self.audit
    }

    async fn fail_next_audit(&self) -> Result<(), DomainError> {
        // The nontransactional sequence is armed before the caller transaction starts.
        Ok(())
    }

    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn admin_repository_contract_postgres(
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
        .connect_runtime_pool(8)
        .await
        .expect("connect restricted runtime pool");
    let foreign = database
        .connect_runtime_pool(1)
        .await
        .expect("connect foreign restricted runtime pool");
    let uow = PostgresUnitOfWork::new(runtime);
    let repository = PostgresAdminRepository::new(uow.clone());
    let audit = PostgresAuditRepository::new(uow.clone());
    sqlx::raw_sql(
        "CREATE SEQUENCE contract_admin_audit_failure MINVALUE 0 START 0;
         CREATE FUNCTION contract_fail_admin_audit_append()
         RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER AS $$
         BEGIN
             IF nextval('contract_admin_audit_failure') = 0 THEN
                 RAISE EXCEPTION 'contract-injected audit dependency failure';
             END IF;
             RETURN NEW;
         END;
         $$;
         CREATE TRIGGER contract_fail_admin_audit_append
         BEFORE INSERT ON audit_log
         FOR EACH ROW EXECUTE FUNCTION contract_fail_admin_audit_append();",
    )
    .execute(database.admin_pool())
    .await
    .expect("install one-shot audit failure fixture");
    let harness = Harness {
        database,
        foreign_uow: PostgresUnitOfWork::new(foreign),
        uow,
        repository,
        audit,
    };

    admin_repository_contract(&harness).await;

    harness.uow.pool().close().await;
    harness.foreign_uow.pool().close().await;
    harness.database.close().await;
}
