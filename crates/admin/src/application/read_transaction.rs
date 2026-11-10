use kernel::error::DomainError;
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};

/// Read-only application operation executed inside a single `UnitOfWork` transaction.
#[async_trait::async_trait]
pub trait ReadInTransactionOp<T: Send>: Send + Sync {
    async fn execute(&self, tx: &mut TxHandle) -> Result<T, DomainError>;
}

/// Opens a single `ReadOnly` transaction, executes `op`, commits on success, and rolls back
/// cleanly on failure without leaking the transaction handle.
///
/// # Errors
///
/// Returns the error from `begin`, `op.execute` (rolling back first), or `commit`.
pub async fn read_in_transaction<T: Send>(
    uow: &dyn UnitOfWork,
    op: &(dyn ReadInTransactionOp<T> + Sync),
) -> Result<T, DomainError> {
    let mut tx = uow.begin(TransactionMode::ReadOnly).await?;
    match op.execute(&mut tx).await {
        Ok(value) => {
            uow.commit(tx).await?;
            Ok(value)
        }
        Err(err) => {
            let _ = uow.rollback(tx).await;
            Err(err)
        }
    }
}

/// Non-generic `u32` specialization of [`read_in_transaction`] for unit-of-work health probes
/// and mock verification.
///
/// # Errors
///
/// Propagates errors from [`read_in_transaction`].
pub async fn read_uow_once(
    uow: &dyn UnitOfWork,
    op: &(dyn ReadInTransactionOp<u32> + Sync),
) -> Result<u32, DomainError> {
    let mut tx = uow.begin(TransactionMode::ReadOnly).await?;
    match op.execute(&mut tx).await {
        Ok(value) => {
            uow.commit(tx).await?;
            Ok(value)
        }
        Err(err) => {
            let _ = uow.rollback(tx).await;
            Err(err)
        }
    }
}
