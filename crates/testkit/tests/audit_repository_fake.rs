use kernel::error::DomainError;
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use testkit::contracts::audit_repository::{
    AuditRepositoryContractHarness, audit_repository_contract,
};
use testkit::fakes::audit_repository::FakeAuditRepository;
use testkit::fakes::store::FakeStore;
use testkit::fakes::unit_of_work::FakeUnitOfWork;

struct Harness {
    store: FakeStore,
    uow: FakeUnitOfWork,
    repository: FakeAuditRepository,
}

#[async_trait::async_trait]
impl AuditRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeAuditRepository;

    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn repository(&self) -> &Self::Repository {
        &self.repository
    }

    async fn fail_next_append(&self) -> Result<(), DomainError> {
        self.repository.fail_next_append();
        Ok(())
    }

    async fn write_companion(&self, tx: &mut TxHandle) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            state
                .scratch
                .insert("audit-companion".into(), "pending".into());
            Ok(())
        })
    }

    async fn companion_exists(&self, tx: &mut TxHandle) -> Result<bool, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state.scratch.contains_key("audit-companion"))
        })
    }

    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        FakeUnitOfWork::new(FakeStore::new())
            .begin(TransactionMode::ReadWrite)
            .await
    }
}

#[tokio::test]
async fn audit_repository_fake_passes_contract() {
    let store = FakeStore::new();
    let harness = Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeAuditRepository::new(store.clone()),
        store,
    };
    audit_repository_contract(&harness).await;
}
