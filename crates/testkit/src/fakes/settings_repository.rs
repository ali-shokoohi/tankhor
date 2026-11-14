use super::store::FakeStore;
use admin::{
    domain::settings::SettingKey,
    ports::settings_repository::{
        SettingRecord, SettingVersion, SettingsRepository, validate_value,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::VersionId,
    ports::unit_of_work::TxHandle,
};

#[derive(Debug, Clone)]
pub struct FakeSettingsRepository {
    store: FakeStore,
}

impl FakeSettingsRepository {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }
}

fn stale() -> DomainError {
    DomainError::conflict(ErrorCode::StaleRevision)
}

#[async_trait::async_trait]
impl SettingsRepository for FakeSettingsRepository {
    async fn list(&self, tx: &mut TxHandle) -> Result<Vec<SettingRecord>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.settings.values().cloned().collect()))
    }

    async fn get(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
    ) -> Result<Option<SettingRecord>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.settings.get(&key).cloned()))
    }

    async fn history(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        limit: u16,
    ) -> Result<Vec<SettingVersion>, DomainError> {
        if limit == 0 {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        self.store.with_read(tx, |state| {
            Ok(state
                .setting_history
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
        key: SettingKey,
        version: VersionId,
    ) -> Result<Option<SettingVersion>, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state
                .setting_history
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
        mut keys: Vec<SettingKey>,
    ) -> Result<Vec<SettingRecord>, DomainError> {
        keys.sort_unstable();
        keys.dedup();
        self.store.with_read(tx, |state| {
            Ok(keys
                .into_iter()
                .filter_map(|key| state.settings.get(&key).cloned())
                .collect())
        })
    }

    async fn append_version(
        &self,
        tx: &mut TxHandle,
        version: SettingVersion,
    ) -> Result<(), DomainError> {
        validate_value(version.key(), version.value())?;
        self.store.with_write(tx, |state| {
            if state
                .setting_history
                .values()
                .flatten()
                .any(|item| item.version() == version.version())
            {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            state
                .setting_history
                .entry(version.key())
                .or_default()
                .push(version);
            Ok(())
        })
    }

    async fn set_live(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        expected: Option<VersionId>,
        next: VersionId,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            let current = state
                .settings
                .get(&key)
                .map(SettingRecord::version)
                .map(SettingVersion::version);
            if current != expected {
                return Err(stale());
            }
            let version = state
                .setting_history
                .get(&key)
                .into_iter()
                .flatten()
                .find(|version| version.version() == next)
                .cloned()
                .ok_or_else(stale)?;
            state.settings.insert(key, SettingRecord::new(version));
            Ok(())
        })
    }

    async fn seed(
        &self,
        tx: &mut TxHandle,
        versions: Vec<SettingVersion>,
    ) -> Result<(), DomainError> {
        for version in &versions {
            validate_value(version.key(), version.value())?;
        }
        self.store.with_write(tx, |state| {
            for version in versions {
                if state.settings.contains_key(&version.key()) {
                    continue;
                }
                state
                    .setting_history
                    .entry(version.key())
                    .or_default()
                    .push(version.clone());
                state
                    .settings
                    .insert(version.key(), SettingRecord::new(version));
            }
            Ok(())
        })
    }
}
