use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::rate_limit_repository::{
        RateLimitRepositoryContractHarness, rate_limit_repository_contract,
    },
    fakes::{
        clock::FixedClock, rate_limit_repository::FakeRateLimitRepository, store::FakeStore,
        unit_of_work::FakeUnitOfWork,
    },
};
use time::OffsetDateTime;

struct Harness {
    uow: FakeUnitOfWork,
    repository: FakeRateLimitRepository,
    clock: FixedClock,
}
#[async_trait::async_trait]
impl RateLimitRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeRateLimitRepository;
    type Clock = FixedClock;
    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn repository(&self) -> &Self::Repository {
        &self.repository
    }
    fn clock(&self) -> &Self::Clock {
        &self.clock
    }
    fn set_time(&self, now: OffsetDateTime) {
        self.clock.set(now);
    }
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        FakeUnitOfWork::new(FakeStore::new())
            .begin(TransactionMode::ReadWrite)
            .await
    }
}
#[tokio::test]
async fn rate_limit_repository_fake_passes_contract() {
    let store = FakeStore::new();
    rate_limit_repository_contract(&Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeRateLimitRepository::new(store),
        clock: FixedClock::new(OffsetDateTime::UNIX_EPOCH),
    })
    .await;
}
