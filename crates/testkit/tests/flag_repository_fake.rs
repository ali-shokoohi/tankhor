use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::flag_repository::{FlagRepositoryContractHarness, flag_repository_contract},
    fakes::{flag_repository::FakeFlagRepository, store::FakeStore, unit_of_work::FakeUnitOfWork},
};

struct Harness {
    uow: FakeUnitOfWork,
    repository: FakeFlagRepository,
    foreign: FakeUnitOfWork,
}
#[async_trait::async_trait]
impl FlagRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeFlagRepository;
    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn repository(&self) -> &Self::Repository {
        &self.repository
    }
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign.begin(TransactionMode::ReadWrite).await
    }
}
#[tokio::test]
async fn flag_repository_fake_passes_contract() {
    let store = FakeStore::new();
    let harness = Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeFlagRepository::new(store),
        foreign: FakeUnitOfWork::new(FakeStore::new()),
    };
    flag_repository_contract(&harness).await;
}
