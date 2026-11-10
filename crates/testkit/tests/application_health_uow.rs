use admin::application::read_transaction::{
    ReadInTransactionOp, read_in_transaction, read_uow_once,
};
use kernel::error::{DomainError, ErrorCategory, ErrorCode};
use kernel::ports::unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle, UnitOfWork};
use testkit::fakes::store::FakeStore;
use testkit::fakes::unit_of_work::FakeUnitOfWork;

struct OkReadOp {
    store_id: StoreId,
}

#[async_trait::async_trait]
impl ReadInTransactionOp<u32> for OkReadOp {
    async fn execute(&self, tx: &mut TxHandle) -> Result<u32, DomainError> {
        let value = tx.downcast_ref::<u32>(self.store_id)?;
        Ok(*value)
    }
}

struct FailReadOp;

#[async_trait::async_trait]
impl ReadInTransactionOp<u32> for FailReadOp {
    async fn execute(&self, _tx: &mut TxHandle) -> Result<u32, DomainError> {
        Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
    }
}

struct ForbiddenWriteOp {
    store: FakeStore,
}

#[async_trait::async_trait]
impl ReadInTransactionOp<()> for ForbiddenWriteOp {
    async fn execute(&self, tx: &mut TxHandle) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            state
                .scratch
                .insert(String::from("illegal"), String::from("write"));
            Ok(())
        })
    }
}

#[tokio::test]
async fn health_transaction_reads_once_and_rolls_back_on_failure() {
    // 1. Positive mock interaction: begins exactly one ReadOnly transaction and commits/closes cleanly.
    let store_id = StoreId::new();
    let mut uow = MockUnitOfWork::new();
    uow.expect_begin()
        .withf(|mode| *mode == TransactionMode::ReadOnly)
        .times(1)
        .returning(move |mode| Ok(TxHandle::new(store_id, mode, 19_u32)));
    uow.expect_commit().times(1).returning(move |mut tx| {
        let _ = tx.take::<u32>(store_id)?;
        Ok(())
    });
    uow.expect_rollback().times(0);

    let out = read_uow_once(&uow, &OkReadOp { store_id })
        .await
        .expect("read-only transaction succeeds");
    assert_eq!(out, 19);

    // 2. Failure mock interaction: begins once, rolls back once on error, never commits or leaks handle.
    let mut failing_uow = MockUnitOfWork::new();
    failing_uow
        .expect_begin()
        .withf(|mode| *mode == TransactionMode::ReadOnly)
        .times(1)
        .returning(move |mode| Ok(TxHandle::new(store_id, mode, 0_u32)));
    failing_uow.expect_commit().times(0);
    failing_uow
        .expect_rollback()
        .times(1)
        .returning(move |mut tx| {
            let _ = tx.take::<u32>(store_id)?;
            Ok(())
        });

    let err = read_in_transaction(&failing_uow, &FailReadOp)
        .await
        .expect_err("failed read must propagate domain error after rollback");
    assert_eq!(err.category(), ErrorCategory::Unavailable);
    assert_eq!(err.code(), ErrorCode::PrerequisiteUnavailable);

    // 3. Fake store verification: read-only service rejects writes, rolls back cleanly, and does not deadlock next begin.
    let store = FakeStore::new();
    let fake_uow = FakeUnitOfWork::new(store.clone());
    let write_err = read_in_transaction(
        &fake_uow,
        &ForbiddenWriteOp {
            store: store.clone(),
        },
    )
    .await
    .expect_err("write inside read-only service must fail");
    assert_eq!(write_err.category(), ErrorCategory::InvalidHandle);
    assert_eq!(write_err.code(), ErrorCode::ReadOnlyTransaction);

    assert!(
        store
            .try_read_committed()
            .expect("committed snapshot")
            .scratch
            .is_empty()
    );

    // Confirm no handle/lock leaked on FakeUnitOfWork:
    let next_tx = fake_uow
        .begin(TransactionMode::ReadOnly)
        .await
        .expect("next begin succeeds immediately");
    fake_uow.rollback(next_tx).await.expect("cleanup");
}
