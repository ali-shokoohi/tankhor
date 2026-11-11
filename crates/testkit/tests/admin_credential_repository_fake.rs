use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::admin_credential_repository::{
        AdminCredentialRepositoryContractHarness, admin_credential_repository_contract,
    },
    fakes::{
        admin_credential_repository::FakeAdminCredentialRepository,
        admin_repository::FakeAdminRepository, store::FakeStore, unit_of_work::FakeUnitOfWork,
    },
};

struct Harness {
    uow: FakeUnitOfWork,
    repository: FakeAdminCredentialRepository,
    admins: FakeAdminRepository,
}

#[async_trait::async_trait]
impl AdminCredentialRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeAdminCredentialRepository;
    type Admins = FakeAdminRepository;
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
        FakeUnitOfWork::new(FakeStore::new())
            .begin(TransactionMode::ReadWrite)
            .await
    }
}

#[tokio::test]
async fn admin_credential_repository_fake_passes_contract() {
    let store = FakeStore::new();
    admin_credential_repository_contract(&Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeAdminCredentialRepository::new(store.clone()),
        admins: FakeAdminRepository::new(store),
    })
    .await;
}
