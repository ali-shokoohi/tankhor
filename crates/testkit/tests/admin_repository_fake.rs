use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::admin_repository::{AdminRepositoryContractHarness, admin_repository_contract},
    fakes::{
        admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
        store::FakeStore, unit_of_work::FakeUnitOfWork,
    },
};

struct Harness {
    uow: FakeUnitOfWork,
    repository: FakeAdminRepository,
    audit: FakeAuditRepository,
}

#[async_trait::async_trait]
impl AdminRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeAdminRepository;
    type Audit = FakeAuditRepository;

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
        self.audit.fail_next_append();
        Ok(())
    }
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        FakeUnitOfWork::new(FakeStore::new())
            .begin(TransactionMode::ReadWrite)
            .await
    }
}

#[tokio::test]
async fn admin_repository_fake_passes_contract() {
    let store = FakeStore::new();
    let harness = Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeAdminRepository::new(store.clone()),
        audit: FakeAuditRepository::new(store),
    };
    admin_repository_contract(&harness).await;
}
