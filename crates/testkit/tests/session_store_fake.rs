use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::session_store::{SessionStoreContractHarness, session_store_contract},
    fakes::{
        admin_repository::FakeAdminRepository, session_store::FakeSessionStore, store::FakeStore,
        unit_of_work::FakeUnitOfWork,
    },
};

struct Harness {
    uow: FakeUnitOfWork,
    sessions: FakeSessionStore,
    admins: FakeAdminRepository,
}

#[async_trait::async_trait]
impl SessionStoreContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Sessions = FakeSessionStore;
    type Admins = FakeAdminRepository;
    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn sessions(&self) -> &Self::Sessions {
        &self.sessions
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
async fn session_store_fake_passes_contract() {
    let store = FakeStore::new();
    session_store_contract(&Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        sessions: FakeSessionStore::new(store.clone()),
        admins: FakeAdminRepository::new(store),
    })
    .await;
}
