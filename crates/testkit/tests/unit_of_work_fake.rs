use std::collections::BTreeMap;

use kernel::error::DomainError;
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use testkit::contracts::unit_of_work::{UnitOfWorkContractHarness, unit_of_work_contract};
use testkit::fakes::store::FakeStore;
use testkit::fakes::unit_of_work::FakeUnitOfWork;

struct ScratchFakeHarness {
    store: FakeStore,
    foreign_uow: FakeUnitOfWork,
}

impl ScratchFakeHarness {
    fn new(store: FakeStore) -> Self {
        let foreign_uow = FakeUnitOfWork::new(FakeStore::new());
        Self { store, foreign_uow }
    }
}

#[async_trait::async_trait]
impl UnitOfWorkContractHarness for ScratchFakeHarness {
    async fn write_entry(
        &self,
        tx: &mut TxHandle,
        key: &str,
        value: &str,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            state.scratch.insert(key.to_owned(), value.to_owned());
            Ok(())
        })
    }

    async fn read_in_tx(
        &self,
        tx: &mut TxHandle,
        key: &str,
    ) -> Result<Option<String>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.scratch.get(key).cloned()))
    }

    async fn read_committed(&self, key: &str) -> Result<Option<String>, DomainError> {
        Ok(self
            .store
            .try_read_committed()
            .and_then(|snapshot| snapshot.scratch.get(key).cloned()))
    }

    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }

    async fn simulate_unknown_commit(&self) -> Result<(), DomainError> {
        let failing_uow = FakeUnitOfWork::new(self.store.clone()).fail_next_commit_unknown();
        let mut tx = failing_uow.begin(TransactionMode::ReadWrite).await?;
        self.write_entry(&mut tx, "ambiguous-key", "maybe-committed")
            .await?;
        failing_uow.commit(tx).await
    }
}

#[tokio::test]
async fn unit_of_work_fake_passes_contract_and_discards_across_repositories() {
    let store = FakeStore::new();
    let harness = ScratchFakeHarness::new(store.clone());
    unit_of_work_contract(&FakeUnitOfWork::new(store.clone()), &harness).await;

    // Cross-repository rollback check on shared FakeStore working state:
    let uow = FakeUnitOfWork::new(store.clone());
    let mut tx = uow
        .begin(TransactionMode::ReadWrite)
        .await
        .expect("begin cross-repo tx");
    store
        .with_write(&mut tx, |state| {
            let mut repo_a = BTreeMap::new();
            repo_a.insert(String::from("k1"), String::from("v1"));
            state
                .repositories_scratch
                .insert(String::from("repo_a"), repo_a);

            let mut repo_b = BTreeMap::new();
            repo_b.insert(String::from("k2"), String::from("v2"));
            state
                .repositories_scratch
                .insert(String::from("repo_b"), repo_b);
            Ok(())
        })
        .expect("cross-repo writes");
    uow.rollback(tx).await.expect("cross-repo rollback");

    let committed = store
        .try_read_committed()
        .expect("committed snapshot available after rollback");
    assert!(committed.repositories_scratch.is_empty());
}
