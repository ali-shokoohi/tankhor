use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::change_repository::{ChangeRepositoryContractHarness, change_repository_contract},
    fakes::{
        change_repository::FakeChangeRepository, store::FakeStore, unit_of_work::FakeUnitOfWork,
    },
};
struct Harness {
    uow: FakeUnitOfWork,
    repository: FakeChangeRepository,
    foreign: FakeUnitOfWork,
}
#[async_trait::async_trait]
impl ChangeRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeChangeRepository;
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
async fn change_repository_fake_passes_contract() {
    let store = FakeStore::new();
    let harness = Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeChangeRepository::new(store),
        foreign: FakeUnitOfWork::new(FakeStore::new()),
    };
    change_repository_contract(&harness).await;
}
