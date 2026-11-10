use kernel::error::{DomainError, ErrorCategory, ErrorCode};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};

/// Backend-neutral scratch harness supplied by fake and real contract runners to prove
/// transactional commit, rollback, drop-rollback, read-only enforcement, wrong-store refusal,
/// and unknown-commit outcome handling on `UnitOfWork`.
#[async_trait::async_trait]
pub trait UnitOfWorkContractHarness: Send + Sync {
    /// Writes `(key, value)` inside `tx`.
    async fn write_entry(
        &self,
        tx: &mut TxHandle,
        key: &str,
        value: &str,
    ) -> Result<(), DomainError>;

    /// Reads `key` inside `tx`.
    async fn read_in_tx(&self, tx: &mut TxHandle, key: &str)
    -> Result<Option<String>, DomainError>;

    /// Reads committed state for `key` outside any open transaction.
    async fn read_committed(&self, key: &str) -> Result<Option<String>, DomainError>;

    /// Creates a foreign `TxHandle` belonging to a distinct store instance.
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;

    /// Executes a commit attempt whose outcome is ambiguous (`ErrorCode::CommitOutcomeUnknown`)
    /// without masquerading as a confirmed rollback.
    async fn simulate_unknown_commit(&self) -> Result<(), DomainError>;
}

async fn verify_handle_and_outcome_guards<U, H>(uow: &U, harness: &H)
where
    U: UnitOfWork,
    H: UnitOfWorkContractHarness,
{
    let mut tx_ro = uow
        .begin(TransactionMode::ReadOnly)
        .await
        .expect("begin read-only transaction");
    let ro_err = harness
        .write_entry(&mut tx_ro, "ro-key", "forbidden")
        .await
        .expect_err("write in read-only tx must fail");
    assert_eq!(ro_err.category(), ErrorCategory::InvalidHandle);
    assert_eq!(ro_err.code(), ErrorCode::ReadOnlyTransaction);
    uow.rollback(tx_ro).await.expect("rollback read-only tx");

    let mut foreign_for_write = harness.begin_foreign().await.expect("begin foreign 1");
    let foreign_write_err = harness
        .write_entry(&mut foreign_for_write, "foreign-key", "bad")
        .await
        .expect_err("foreign handle write must fail");
    assert_eq!(foreign_write_err.category(), ErrorCategory::InvalidHandle);
    assert_eq!(
        foreign_write_err.code(),
        ErrorCode::ForeignTransactionHandle
    );
    drop(foreign_for_write);

    let foreign_for_commit = harness.begin_foreign().await.expect("begin foreign 2");
    let foreign_commit_err = uow
        .commit(foreign_for_commit)
        .await
        .expect_err("commit of foreign handle must fail");
    assert_eq!(foreign_commit_err.category(), ErrorCategory::InvalidHandle);
    assert_eq!(
        foreign_commit_err.code(),
        ErrorCode::ForeignTransactionHandle
    );

    let unknown_err = harness
        .simulate_unknown_commit()
        .await
        .expect_err("ambiguous commit outcome must return UnknownCommit error");
    assert_eq!(unknown_err.category(), ErrorCategory::UnknownCommit);
    assert_eq!(unknown_err.code(), ErrorCode::CommitOutcomeUnknown);
}

/// Shared contract suite executed against both `FakeUnitOfWork` and `PostgresUnitOfWork`.
///
/// # Panics
///
/// Panics if `uow` or `harness` violates any `UnitOfWork` contract invariant.
pub async fn unit_of_work_contract<U, H>(uow: &U, harness: &H)
where
    U: UnitOfWork,
    H: UnitOfWorkContractHarness,
{
    let mut tx_commit = uow
        .begin(TransactionMode::ReadWrite)
        .await
        .expect("begin read-write transaction");
    harness
        .write_entry(&mut tx_commit, "commit-key", "committed-v1")
        .await
        .expect("write inside read-write tx");
    assert_eq!(
        harness
            .read_in_tx(&mut tx_commit, "commit-key")
            .await
            .expect("read inside tx"),
        Some(String::from("committed-v1"))
    );
    assert_eq!(
        harness
            .read_committed("commit-key")
            .await
            .expect("read committed before commit"),
        None,
        "uncommitted write must not leak to outside readers"
    );
    uow.commit(tx_commit).await.expect("commit succeeds");
    assert_eq!(
        harness
            .read_committed("commit-key")
            .await
            .expect("read committed after commit"),
        Some(String::from("committed-v1"))
    );

    let mut tx_rb = uow
        .begin(TransactionMode::ReadWrite)
        .await
        .expect("begin rollback transaction");
    harness
        .write_entry(&mut tx_rb, "rollback-key", "discarded")
        .await
        .expect("write before rollback");
    harness
        .write_entry(&mut tx_rb, "commit-key", "overwritten-then-rolled-back")
        .await
        .expect("overwrite before rollback");
    uow.rollback(tx_rb).await.expect("rollback succeeds");
    assert_eq!(
        harness
            .read_committed("rollback-key")
            .await
            .expect("rollback-key absent"),
        None
    );
    assert_eq!(
        harness
            .read_committed("commit-key")
            .await
            .expect("commit-key preserved after rollback"),
        Some(String::from("committed-v1"))
    );

    {
        let mut tx_drop = uow
            .begin(TransactionMode::ReadWrite)
            .await
            .expect("begin dropped transaction");
        harness
            .write_entry(&mut tx_drop, "dropped-key", "never-committed")
            .await
            .expect("write before drop");
    }
    let mut tx_after_drop = uow
        .begin(TransactionMode::ReadWrite)
        .await
        .expect("begin after drop must not deadlock or fail");
    assert_eq!(
        harness
            .read_in_tx(&mut tx_after_drop, "dropped-key")
            .await
            .expect("read after drop"),
        None,
        "dropped transaction working state must be discarded"
    );
    uow.rollback(tx_after_drop)
        .await
        .expect("cleanup after-drop tx");

    verify_handle_and_outcome_guards(uow, harness).await;
}
