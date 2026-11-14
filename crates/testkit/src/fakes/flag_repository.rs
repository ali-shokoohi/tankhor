use super::store::FakeStore;
use admin::{
    domain::flag::FlagKey,
    ports::flag_repository::{FlagRecord, FlagRepository, FlagVersion},
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::VersionId,
    ports::unit_of_work::TxHandle,
};

#[derive(Debug, Clone)]
pub struct FakeFlagRepository {
    store: FakeStore,
}
impl FakeFlagRepository {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }
}
fn stale() -> DomainError {
    DomainError::conflict(ErrorCode::StaleRevision)
}

#[async_trait::async_trait]
impl FlagRepository for FakeFlagRepository {
    async fn list(&self, tx: &mut TxHandle) -> Result<Vec<FlagRecord>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.flags.values().cloned().collect()))
    }
    async fn get(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
    ) -> Result<Option<FlagRecord>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.flags.get(&key).cloned()))
    }
    async fn history(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        limit: u16,
    ) -> Result<Vec<FlagVersion>, DomainError> {
        if limit == 0 {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        self.store.with_read(tx, |state| {
            Ok(state
                .flag_history
                .get(&key)
                .into_iter()
                .flatten()
                .rev()
                .take(usize::from(limit))
                .cloned()
                .collect())
        })
    }
    async fn version(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        version: VersionId,
    ) -> Result<Option<FlagVersion>, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state
                .flag_history
                .get(&key)
                .into_iter()
                .flatten()
                .find(|item| item.version() == version)
                .cloned())
        })
    }
    async fn lock(
        &self,
        tx: &mut TxHandle,
        mut keys: Vec<FlagKey>,
    ) -> Result<Vec<FlagRecord>, DomainError> {
        keys.sort_unstable();
        keys.dedup();
        self.store.with_read(tx, |state| {
            Ok(keys
                .into_iter()
                .filter_map(|key| state.flags.get(&key).cloned())
                .collect())
        })
    }
    async fn append_version(
        &self,
        tx: &mut TxHandle,
        version: FlagVersion,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if state
                .flag_history
                .values()
                .flatten()
                .any(|item| item.version() == version.version())
            {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            state
                .flag_history
                .entry(version.key())
                .or_default()
                .push(version);
            Ok(())
        })
    }
    async fn set_live(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        expected: Option<VersionId>,
        next: VersionId,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            let current = state
                .flags
                .get(&key)
                .map(FlagRecord::version)
                .map(FlagVersion::version);
            if current != expected {
                return Err(stale());
            }
            let version = state
                .flag_history
                .get(&key)
                .into_iter()
                .flatten()
                .find(|version| version.version() == next)
                .cloned()
                .ok_or_else(stale)?;
            state.flags.insert(key, FlagRecord::new(version));
            Ok(())
        })
    }
    async fn seed(&self, tx: &mut TxHandle, versions: Vec<FlagVersion>) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            for version in versions {
                if state.flags.contains_key(&version.key()) {
                    continue;
                }
                state
                    .flag_history
                    .entry(version.key())
                    .or_default()
                    .push(version.clone());
                state.flags.insert(version.key(), FlagRecord::new(version));
            }
            Ok(())
        })
    }
}
