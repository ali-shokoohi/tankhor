//! Immutable setting definitions, published versions and caller-owned transactions.

use crate::domain::settings::{SettingKey, SettingValue};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, VersionId},
    ports::unit_of_work::TxHandle,
};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingVersion {
    version: VersionId,
    key: SettingKey,
    value: SettingValue,
    actor: AdminId,
    effective_at: OffsetDateTime,
}

impl SettingVersion {
    /// # Errors
    /// Rejects values that violate the setting policy.
    pub fn new(
        version: VersionId,
        key: SettingKey,
        value: SettingValue,
        actor: AdminId,
        effective_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        validate_value(key, &value)?;
        Ok(Self {
            version,
            key,
            value,
            actor,
            effective_at,
        })
    }

    #[must_use]
    pub const fn version(&self) -> VersionId {
        self.version
    }
    #[must_use]
    pub const fn key(&self) -> SettingKey {
        self.key
    }
    #[must_use]
    pub const fn value(&self) -> &SettingValue {
        &self.value
    }
    #[must_use]
    pub const fn actor(&self) -> AdminId {
        self.actor
    }
    #[must_use]
    pub const fn effective_at(&self) -> OffsetDateTime {
        self.effective_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRecord {
    current: SettingVersion,
}

impl SettingRecord {
    #[must_use]
    pub const fn new(current: SettingVersion) -> Self {
        Self { current }
    }
    #[must_use]
    pub const fn version(&self) -> &SettingVersion {
        &self.current
    }
}

/// # Errors
/// Rejects values that violate the setting policy.
pub fn validate_value(key: SettingKey, value: &SettingValue) -> Result<(), DomainError> {
    let numeric = crate::domain::settings::initial_numeric_settings()
        .into_iter()
        .find(|setting| setting.key() == key);
    if let Some(setting) = numeric {
        setting.validate(value.clone()).map(|_| ())
    } else if key == SettingKey::UploadFormats {
        crate::domain::settings::initial_upload_formats()
            .validate(value.clone())
            .map(|_| ())
    } else {
        Err(DomainError::validation(ErrorCode::InvalidValue))
    }
}

#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn list(&self, tx: &mut TxHandle) -> Result<Vec<SettingRecord>, DomainError>;
    async fn get(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
    ) -> Result<Option<SettingRecord>, DomainError>;
    async fn history(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        limit: u16,
    ) -> Result<Vec<SettingVersion>, DomainError>;
    async fn version(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        version: VersionId,
    ) -> Result<Option<SettingVersion>, DomainError>;
    async fn lock(
        &self,
        tx: &mut TxHandle,
        keys: Vec<SettingKey>,
    ) -> Result<Vec<SettingRecord>, DomainError>;
    async fn append_version(
        &self,
        tx: &mut TxHandle,
        version: SettingVersion,
    ) -> Result<(), DomainError>;
    async fn set_live(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        expected: Option<VersionId>,
        next: VersionId,
    ) -> Result<(), DomainError>;
    async fn seed(
        &self,
        tx: &mut TxHandle,
        versions: Vec<SettingVersion>,
    ) -> Result<(), DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::settings::{SettingKey, SettingValue};
    use kernel::{
        ids::{AdminId, VersionId},
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    use time::OffsetDateTime;

    fn id<T: FromStr>(n: u16) -> T {
        format!("019a53a0-0000-7000-8000-{n:012x}")
            .parse()
            .ok()
            .unwrap()
    }

    #[tokio::test]
    async fn settings_repository_port_is_mockable() {
        let mut mock = MockSettingsRepository::new();
        let store = StoreId::new();
        let mut tx = TxHandle::new(store, TransactionMode::ReadWrite, ());
        let key = SettingKey::UploadMaxMb;
        let version = SettingVersion::new(
            id::<VersionId>(1),
            key,
            SettingValue::Number(15),
            id::<AdminId>(2),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        mock.expect_list().times(1).return_once(|_| Ok(Vec::new()));
        assert!(mock.list(&mut tx).await.unwrap().is_empty());
        let result = SettingRecord::new(version.clone());
        mock.expect_get()
            .times(1)
            .return_once(move |_, _| Ok(Some(result)));
        assert_eq!(
            mock.get(&mut tx, key).await.unwrap().unwrap().version(),
            &version
        );
        mock.expect_history()
            .times(1)
            .return_once(|_, _, _| Ok(Vec::new()));
        assert!(mock.history(&mut tx, key, 10).await.unwrap().is_empty());
        mock.expect_lock()
            .times(1)
            .return_once(|_, _| Ok(Vec::new()));
        assert!(mock.lock(&mut tx, vec![key]).await.unwrap().is_empty());
        mock.expect_append_version()
            .times(1)
            .return_once(|_, _| Ok(()));
        mock.append_version(&mut tx, version.clone()).await.unwrap();
        mock.expect_set_live()
            .times(1)
            .return_once(|_, _, _, _| Ok(()));
        mock.set_live(&mut tx, key, None, version.version())
            .await
            .unwrap();
        mock.expect_seed().times(1).return_once(|_, _| Ok(()));
        mock.seed(&mut tx, vec![version]).await.unwrap();
    }
}
