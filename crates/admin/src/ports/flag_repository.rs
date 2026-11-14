//! Launch-flag persistence port.

use crate::domain::flag::FlagKey;
use kernel::{
    ids::{AdminId, VersionId},
    ports::unit_of_work::TxHandle,
};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagVersion {
    version: VersionId,
    key: FlagKey,
    value: bool,
    actor: AdminId,
    effective_at: OffsetDateTime,
}

impl FlagVersion {
    #[must_use]
    pub const fn new(
        version: VersionId,
        key: FlagKey,
        value: bool,
        actor: AdminId,
        effective_at: OffsetDateTime,
    ) -> Self {
        Self {
            version,
            key,
            value,
            actor,
            effective_at,
        }
    }
    #[must_use]
    pub const fn version(&self) -> VersionId {
        self.version
    }
    #[must_use]
    pub const fn key(&self) -> FlagKey {
        self.key
    }
    #[must_use]
    pub const fn value(&self) -> bool {
        self.value
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
pub struct FlagRecord {
    current: FlagVersion,
}
impl FlagRecord {
    #[must_use]
    pub const fn new(current: FlagVersion) -> Self {
        Self { current }
    }
    #[must_use]
    pub const fn version(&self) -> &FlagVersion {
        &self.current
    }
}

#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait FlagRepository: Send + Sync {
    async fn list(&self, tx: &mut TxHandle) -> Result<Vec<FlagRecord>, kernel::error::DomainError>;
    async fn get(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
    ) -> Result<Option<FlagRecord>, kernel::error::DomainError>;
    async fn history(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        limit: u16,
    ) -> Result<Vec<FlagVersion>, kernel::error::DomainError>;
    async fn version(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        version: VersionId,
    ) -> Result<Option<FlagVersion>, kernel::error::DomainError>;
    async fn lock(
        &self,
        tx: &mut TxHandle,
        keys: Vec<FlagKey>,
    ) -> Result<Vec<FlagRecord>, kernel::error::DomainError>;
    async fn append_version(
        &self,
        tx: &mut TxHandle,
        version: FlagVersion,
    ) -> Result<(), kernel::error::DomainError>;
    async fn set_live(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        expected: Option<VersionId>,
        next: VersionId,
    ) -> Result<(), kernel::error::DomainError>;
    async fn seed(
        &self,
        tx: &mut TxHandle,
        versions: Vec<FlagVersion>,
    ) -> Result<(), kernel::error::DomainError>;
}

#[cfg(test)]
mod tests {
    use super::{FlagRecord, FlagRepository, FlagVersion, MockFlagRepository};
    use crate::domain::flag::FlagKey;
    use kernel::{
        ids::{AdminId, VersionId},
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    use time::OffsetDateTime;

    fn id<T: FromStr>(n: u16) -> T {
        format!("019a53a0-2000-7000-8000-{n:012x}")
            .parse()
            .ok()
            .unwrap()
    }

    #[tokio::test]
    async fn flag_repository_port_is_mockable() {
        let mut mock = MockFlagRepository::new();
        let mut tx = TxHandle::new(StoreId::new(), TransactionMode::ReadWrite, ());
        let key = FlagKey::PartnerWidgetApi;
        let version = FlagVersion::new(
            id::<VersionId>(1),
            key,
            false,
            id::<AdminId>(2),
            OffsetDateTime::UNIX_EPOCH,
        );
        mock.expect_list().return_once(|_| Ok(vec![]));
        assert!(mock.list(&mut tx).await.unwrap().is_empty());
        let record = FlagRecord::new(version.clone());
        mock.expect_get().return_once(|_, _| Ok(Some(record)));
        assert_eq!(
            mock.get(&mut tx, key).await.unwrap().unwrap().version(),
            &version
        );
        mock.expect_history().return_once(|_, _, _| Ok(vec![]));
        assert!(mock.history(&mut tx, key, 5).await.unwrap().is_empty());
        mock.expect_lock().return_once(|_, _| Ok(vec![]));
        assert!(mock.lock(&mut tx, vec![key]).await.unwrap().is_empty());
        mock.expect_append_version().return_once(|_, _| Ok(()));
        mock.append_version(&mut tx, version.clone()).await.unwrap();
        mock.expect_set_live().return_once(|_, _, _, _| Ok(()));
        mock.set_live(&mut tx, key, None, version.version())
            .await
            .unwrap();
        mock.expect_seed().return_once(|_, _| Ok(()));
        mock.seed(&mut tx, vec![version]).await.unwrap();
    }
}
