use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use tokio::sync::{Mutex, OwnedMutexGuard};

use kernel::error::DomainError;
use kernel::ports::unit_of_work::{StoreId, TxHandle};

/// Transactional working-set snapshot shared by all in-memory fake repositories.
#[derive(Clone, Debug, Default)]
pub struct FakeState {
    pub scratch: BTreeMap<String, String>,
    pub repositories_scratch: BTreeMap<String, BTreeMap<String, String>>,
    pub health_ready: bool,
}

/// Active serialized transaction payload stored inside an opaque `TxHandle`.
pub(crate) struct FakeTxPayload {
    pub(crate) working: FakeState,
    pub(crate) _tx_guard: OwnedMutexGuard<()>,
}

/// Shared serialized in-memory store backing `FakeUnitOfWork` and all fake repositories.
#[derive(Clone)]
pub struct FakeStore {
    store_id: StoreId,
    committed: Arc<RwLock<FakeState>>,
    tx_lock: Arc<Mutex<()>>,
}

impl std::fmt::Debug for FakeStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeStore")
            .field("store_id", &self.store_id)
            .finish_non_exhaustive()
    }
}

impl Default for FakeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store_id: StoreId::new(),
            committed: Arc::new(RwLock::new(FakeState::default())),
            tx_lock: Arc::new(Mutex::new(())),
        }
    }

    #[must_use]
    pub const fn store_id(&self) -> StoreId {
        self.store_id
    }

    pub(crate) async fn acquire_tx_guard(&self) -> OwnedMutexGuard<()> {
        Arc::clone(&self.tx_lock).lock_owned().await
    }

    /// Reads the currently committed snapshot outside any open transaction.
    #[must_use]
    pub fn try_read_committed(&self) -> Option<FakeState> {
        self.committed.read().ok().map(|guard| guard.clone())
    }

    /// Replaces or mutates committed state directly for test seeding outside transactions.
    pub fn seed<F>(&self, f: F)
    where
        F: FnOnce(&mut FakeState),
    {
        if let Ok(mut guard) = self.committed.write() {
            f(&mut guard);
        }
    }

    pub(crate) fn replace_committed(&self, next: FakeState) {
        if let Ok(mut guard) = self.committed.write() {
            *guard = next;
        }
    }

    /// Executes a read closure against the transaction's isolated working state.
    ///
    /// # Errors
    ///
    /// Propagates handle validation errors (`ForeignTransactionHandle`, `FinishedTransactionHandle`)
    /// or errors returned by `f`.
    pub fn with_read<R, F>(&self, tx: &mut TxHandle, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&FakeState) -> Result<R, DomainError>,
    {
        let payload = tx.downcast_ref::<FakeTxPayload>(self.store_id)?;
        f(&payload.working)
    }

    /// Executes a write closure against the transaction's isolated working state after verifying
    /// `ReadWrite` transaction mode.
    ///
    /// # Errors
    ///
    /// Propagates handle validation errors (`ReadOnlyTransaction`, `ForeignTransactionHandle`,
    /// `FinishedTransactionHandle`) or errors returned by `f`.
    pub fn with_write<R, F>(&self, tx: &mut TxHandle, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&mut FakeState) -> Result<R, DomainError>,
    {
        tx.require_writable()?;
        let payload = tx.downcast_mut::<FakeTxPayload>(self.store_id)?;
        f(&mut payload.working)
    }
}
