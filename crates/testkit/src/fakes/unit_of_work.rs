use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kernel::error::{DomainError, ErrorCode};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};

use crate::fakes::store::{FakeStore, FakeTxPayload};

/// Deterministic serialized in-memory `UnitOfWork` backed by [`FakeStore`].
#[derive(Clone, Debug)]
pub struct FakeUnitOfWork {
    store: FakeStore,
    fail_begin: bool,
    fail_next_commit_unknown: Arc<AtomicBool>,
}

impl FakeUnitOfWork {
    #[must_use]
    pub fn new(store: FakeStore) -> Self {
        Self {
            store,
            fail_begin: false,
            fail_next_commit_unknown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Configures a deterministic `UnitOfWork` whose `begin` fails with `DependencyUnavailable`.
    #[must_use]
    pub fn unavailable(store: FakeStore) -> Self {
        Self {
            store,
            fail_begin: true,
            fail_next_commit_unknown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Causes the next `commit` to return [`ErrorCode::CommitOutcomeUnknown`] after persisting
    /// working state so retry-identity / unknown-commit paths are deterministically testable.
    #[must_use]
    pub fn fail_next_commit_unknown(self) -> Self {
        self.fail_next_commit_unknown.store(true, Ordering::SeqCst);
        self
    }

    #[must_use]
    pub const fn store(&self) -> &FakeStore {
        &self.store
    }
}

#[async_trait::async_trait]
impl UnitOfWork for FakeUnitOfWork {
    async fn begin(&self, mode: TransactionMode) -> Result<TxHandle, DomainError> {
        if self.fail_begin {
            return Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable));
        }
        let guard = self.store.acquire_tx_guard().await;
        let working = self
            .store
            .try_read_committed()
            .ok_or_else(|| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))?;
        Ok(TxHandle::new(
            self.store.store_id(),
            mode,
            FakeTxPayload {
                working,
                _tx_guard: guard,
            },
        ))
    }

    async fn commit(&self, mut tx: TxHandle) -> Result<(), DomainError> {
        let payload = tx.take::<FakeTxPayload>(self.store.store_id())?;
        self.store.replace_committed(payload.working);
        if self.fail_next_commit_unknown.swap(false, Ordering::SeqCst) {
            return Err(DomainError::unknown_commit(ErrorCode::CommitOutcomeUnknown));
        }
        Ok(())
    }

    async fn rollback(&self, mut tx: TxHandle) -> Result<(), DomainError> {
        let _discarded = tx.take::<FakeTxPayload>(self.store.store_id())?;
        Ok(())
    }
}
