// Contract harness for the shared in-memory store.
use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::settings_repository::{
        SettingsRepositoryContractHarness, settings_repository_contract,
    },
    fakes::{
        settings_repository::FakeSettingsRepository, store::FakeStore, unit_of_work::FakeUnitOfWork,
    },
};

struct Harness {
    uow: FakeUnitOfWork,
    repository: FakeSettingsRepository,
    foreign: FakeUnitOfWork,
}

#[async_trait::async_trait]
impl SettingsRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeSettingsRepository;
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
async fn settings_repository_fake_passes_contract() {
    let store = FakeStore::new();
    let foreign = FakeUnitOfWork::new(FakeStore::new());
    let harness = Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeSettingsRepository::new(store),
        foreign,
    };
    settings_repository_contract(&harness).await;
}
