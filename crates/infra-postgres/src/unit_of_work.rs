//! PostgreSQL `UnitOfWork` adapter and checked `TxHandle` accessors.

use std::sync::Arc;

use kernel::error::{DomainError, ErrorCode};
use kernel::ports::unit_of_work::{StoreId, TransactionMode, TxHandle, UnitOfWork};
use sqlx::{Executor, PgConnection, PgPool, Postgres, Transaction};

struct SharedStoreIdentity {
    store_id: StoreId,
}

/// Real PostgreSQL transaction owner implementing [`UnitOfWork`].
#[derive(Clone)]
pub struct PostgresUnitOfWork {
    pool: PgPool,
    identity: Arc<SharedStoreIdentity>,
}

impl std::fmt::Debug for PostgresUnitOfWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresUnitOfWork")
            .field("store_id", &self.identity.store_id)
            .finish_non_exhaustive()
    }
}

struct ActivePostgresTx {
    tx: Transaction<'static, Postgres>,
}

impl PostgresUnitOfWork {
    /// Constructs a lazy PostgreSQL pool without opening a database connection.
    ///
    /// # Errors
    /// Returns an error when the database URL or pool configuration is invalid.
    pub fn connect_lazy(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new().connect_lazy(database_url)?;
        Ok(Self::new(pool))
    }

    /// Constructs a [`PostgresUnitOfWork`] owning a new logical [`StoreId`].
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            identity: Arc::new(SharedStoreIdentity {
                store_id: StoreId::new(),
            }),
        }
    }

    /// Constructs a [`PostgresUnitOfWork`] bound to a caller-supplied [`StoreId`].
    #[must_use]
    pub fn with_store_id(pool: PgPool, store_id: StoreId) -> Self {
        Self {
            pool,
            identity: Arc::new(SharedStoreIdentity { store_id }),
        }
    }
    /// Logical store identity shared across repositories paired with this `UnitOfWork`.
    #[must_use]
    pub fn store_id(&self) -> StoreId {
        self.identity.store_id
    }

    /// Underlying PostgreSQL pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Extracts a readable `&mut PgConnection` from `tx` after checking store identity.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::FinishedTransactionHandle`] or
    /// [`ErrorCode::ForeignTransactionHandle`] if `tx` is invalid for this store.
    pub fn readable_connection<'a>(
        &self,
        tx: &'a mut TxHandle,
    ) -> Result<&'a mut PgConnection, DomainError> {
        let active = tx.downcast_mut::<ActivePostgresTx>(self.identity.store_id)?;
        Ok(&mut active.tx)
    }

    /// Extracts a writable `&mut PgConnection` from `tx` after checking store identity and mode.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::FinishedTransactionHandle`], [`ErrorCode::ForeignTransactionHandle`],
    /// or [`ErrorCode::ReadOnlyTransaction`] if `tx` cannot execute mutations on this store.
    pub fn writable_connection<'a>(
        &self,
        tx: &'a mut TxHandle,
    ) -> Result<&'a mut PgConnection, DomainError> {
        // Validate backend/store identity before mode so foreign handles report ForeignTransactionHandle.
        let _ = tx.downcast_ref::<ActivePostgresTx>(self.identity.store_id)?;
        tx.require_writable()?;
        let active = tx.downcast_mut::<ActivePostgresTx>(self.identity.store_id)?;
        Ok(&mut active.tx)
    }
}

/// Maps a PostgreSQL `COMMIT` error into the sanitized [`DomainError`] taxonomy, preserving
/// [`DomainError::unknown_commit`] whenever commit acknowledgement was lost in transport.
#[must_use]
pub fn map_commit_error(error: &sqlx::Error) -> DomainError {
    match error {
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed => {
            DomainError::unknown_commit(ErrorCode::CommitOutcomeUnknown)
        }
        sqlx::Error::Database(db_err) => match db_err.code().as_deref() {
            Some("40001" | "40P01") => DomainError::retryable(ErrorCode::TransientConflict),
            Some("25006") => DomainError::invalid_handle(ErrorCode::ReadOnlyTransaction),
            Some(code) if code.starts_with("08") => {
                DomainError::unknown_commit(ErrorCode::CommitOutcomeUnknown)
            }
            _ => DomainError::unavailable(ErrorCode::PrerequisiteUnavailable),
        },
        _ => DomainError::unavailable(ErrorCode::PrerequisiteUnavailable),
    }
}

fn map_begin_or_rollback_error(error: &sqlx::Error) -> DomainError {
    if let sqlx::Error::Database(db_err) = error
        && matches!(db_err.code().as_deref(), Some("40001" | "40P01"))
    {
        return DomainError::retryable(ErrorCode::TransientConflict);
    }
    DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)
}

#[async_trait::async_trait]
impl UnitOfWork for PostgresUnitOfWork {
    async fn begin(&self, mode: TransactionMode) -> Result<TxHandle, DomainError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| map_begin_or_rollback_error(&err))?;
        if mode == TransactionMode::ReadOnly {
            (&mut *tx)
                .execute("SET TRANSACTION READ ONLY")
                .await
                .map_err(|err| map_begin_or_rollback_error(&err))?;
        }
        Ok(TxHandle::new(
            self.identity.store_id,
            mode,
            ActivePostgresTx { tx },
        ))
    }

    async fn commit(&self, mut tx: TxHandle) -> Result<(), DomainError> {
        let active = tx.take::<ActivePostgresTx>(self.identity.store_id)?;
        active
            .tx
            .commit()
            .await
            .map_err(|err| map_commit_error(&err))
    }

    async fn rollback(&self, mut tx: TxHandle) -> Result<(), DomainError> {
        let active = tx.take::<ActivePostgresTx>(self.identity.store_id)?;
        active
            .tx
            .rollback()
            .await
            .map_err(|err| map_begin_or_rollback_error(&err))
    }
}
