use kernel::{
    error::DomainError,
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use testkit::{
    contracts::otp_challenge_repository::{
        OtpChallengeRepositoryContractHarness, otp_challenge_repository_contract,
    },
    fakes::{
        admin_repository::FakeAdminRepository,
        otp_challenge_repository::FakeOtpChallengeRepository, store::FakeStore,
        unit_of_work::FakeUnitOfWork,
    },
};
struct Harness {
    uow: FakeUnitOfWork,
    repository: FakeOtpChallengeRepository,
    admins: FakeAdminRepository,
}
#[async_trait::async_trait]
impl OtpChallengeRepositoryContractHarness for Harness {
    type Uow = FakeUnitOfWork;
    type Repository = FakeOtpChallengeRepository;
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
async fn otp_challenge_repository_fake_passes_contract() {
    let store = FakeStore::new();
    otp_challenge_repository_contract(&Harness {
        uow: FakeUnitOfWork::new(store.clone()),
        repository: FakeOtpChallengeRepository::new(store.clone()),
        admins: FakeAdminRepository::new(store),
    })
    .await;
}
