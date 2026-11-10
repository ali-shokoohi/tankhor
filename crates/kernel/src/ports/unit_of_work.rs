use std::any::Any;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{DomainError, ErrorCode};

static NEXT_STORE_ID: AtomicU64 = AtomicU64::new(1);

/// Unique runtime identity of a repository / unit-of-work store instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoreId(u64);

impl StoreId {
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_STORE_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for StoreId {
    fn default() -> Self {
        Self::new()
    }
}

/// Requested transaction access mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransactionMode {
    ReadOnly,
    ReadWrite,
}

/// Opaque, backend-agnostic transaction handle passed across application services.
pub struct TxHandle {
    store_id: StoreId,
    mode: TransactionMode,
    payload: Option<Box<dyn Any + Send>>,
}

impl fmt::Debug for TxHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TxHandle")
            .field("store_id", &self.store_id)
            .field("mode", &self.mode)
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

impl TxHandle {
    /// Creates an active transaction handle bound to `store_id` and `mode`.
    #[must_use]
    pub fn new<T: Any + Send + 'static>(
        store_id: StoreId,
        mode: TransactionMode,
        payload: T,
    ) -> Self {
        Self {
            store_id,
            mode,
            payload: Some(Box::new(payload)),
        }
    }

    #[must_use]
    pub const fn store_id(&self) -> StoreId {
        self.store_id
    }

    #[must_use]
    pub const fn mode(&self) -> TransactionMode {
        self.mode
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.payload.is_none()
    }

    /// Ensures the transaction was opened in `ReadWrite` mode.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::FinishedTransactionHandle`] if the handle was already consumed, or
    /// [`ErrorCode::ReadOnlyTransaction`] if the handle is read-only.
    pub fn require_writable(&self) -> Result<(), DomainError> {
        if self.is_finished() {
            return Err(DomainError::invalid_handle(
                ErrorCode::FinishedTransactionHandle,
            ));
        }
        if self.mode != TransactionMode::ReadWrite {
            return Err(DomainError::invalid_handle(ErrorCode::ReadOnlyTransaction));
        }
        Ok(())
    }

    /// Borrows the concrete backend transaction after verifying store identity and runtime type.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::FinishedTransactionHandle`] if already consumed, or
    /// [`ErrorCode::ForeignTransactionHandle`] if `expected_store` or `T` does not match.
    pub fn downcast_ref<T: Any + Send + 'static>(
        &self,
        expected_store: StoreId,
    ) -> Result<&T, DomainError> {
        let Some(boxed) = self.payload.as_ref() else {
            return Err(DomainError::invalid_handle(
                ErrorCode::FinishedTransactionHandle,
            ));
        };
        if self.store_id != expected_store {
            return Err(DomainError::invalid_handle(
                ErrorCode::ForeignTransactionHandle,
            ));
        }
        boxed
            .downcast_ref::<T>()
            .ok_or_else(|| DomainError::invalid_handle(ErrorCode::ForeignTransactionHandle))
    }

    /// Mutably borrows the concrete backend transaction after verifying store identity and type.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::FinishedTransactionHandle`] if already consumed, or
    /// [`ErrorCode::ForeignTransactionHandle`] if `expected_store` or `T` does not match.
    pub fn downcast_mut<T: Any + Send + 'static>(
        &mut self,
        expected_store: StoreId,
    ) -> Result<&mut T, DomainError> {
        if self.payload.is_none() {
            return Err(DomainError::invalid_handle(
                ErrorCode::FinishedTransactionHandle,
            ));
        }
        if self.store_id != expected_store {
            return Err(DomainError::invalid_handle(
                ErrorCode::ForeignTransactionHandle,
            ));
        }
        let Some(boxed) = self.payload.as_mut() else {
            return Err(DomainError::invalid_handle(
                ErrorCode::FinishedTransactionHandle,
            ));
        };
        boxed
            .downcast_mut::<T>()
            .ok_or_else(|| DomainError::invalid_handle(ErrorCode::ForeignTransactionHandle))
    }

    /// Consumes the inner concrete backend transaction once and marks this handle finished.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::FinishedTransactionHandle`] if already consumed, or
    /// [`ErrorCode::ForeignTransactionHandle`] if `expected_store` or `T` does not match.
    pub fn take<T: Any + Send + 'static>(
        &mut self,
        expected_store: StoreId,
    ) -> Result<T, DomainError> {
        let Some(boxed) = self.payload.as_ref() else {
            return Err(DomainError::invalid_handle(
                ErrorCode::FinishedTransactionHandle,
            ));
        };
        if self.store_id != expected_store || !boxed.is::<T>() {
            return Err(DomainError::invalid_handle(
                ErrorCode::ForeignTransactionHandle,
            ));
        }
        let taken = self
            .payload
            .take()
            .ok_or_else(|| DomainError::invalid_handle(ErrorCode::FinishedTransactionHandle))?;
        taken
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| DomainError::invalid_handle(ErrorCode::ForeignTransactionHandle))
    }
}

/// Multi-repository transaction boundary port.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Opens a new transaction in the requested `mode`.
    async fn begin(&self, mode: TransactionMode) -> Result<TxHandle, DomainError>;

    /// Commits and consumes `tx`.
    async fn commit(&self, tx: TxHandle) -> Result<(), DomainError>;

    /// Rolls back and consumes `tx`.
    async fn rollback(&self, tx: TxHandle) -> Result<(), DomainError>;
}
