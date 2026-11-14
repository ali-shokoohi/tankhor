//! Unchanged fake/real launch-flag persistence contract.

use admin::{
    domain::flag::{FlagKey, launch_flags},
    ports::flag_repository::{FlagRecord, FlagRepository, FlagVersion},
};
use kernel::{
    error::{DomainError, ErrorCategory},
    ids::{AdminId, VersionId},
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use std::str::FromStr;
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait FlagRepositoryContractHarness: Send + Sync {
    type Uow: UnitOfWork;
    type Repository: FlagRepository;
    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-3000-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}
fn versions() -> Vec<FlagVersion> {
    launch_flags()
        .into_iter()
        .enumerate()
        .map(|(index, flag)| {
            FlagVersion::new(
                id::<VersionId>(u16::try_from(index + 1).unwrap()),
                flag.key(),
                false,
                id::<AdminId>(1),
                OffsetDateTime::UNIX_EPOCH,
            )
        })
        .collect()
}

/// # Panics
/// Panics if the repository or unit-of-work violates the shared contract.
#[allow(clippy::too_many_lines)]
pub async fn flag_repository_contract<H: FlagRepositoryContractHarness>(harness: &H) {
    let seeded = versions();
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    harness
        .repository()
        .seed(&mut tx, seeded.clone())
        .await
        .unwrap();
    harness
        .repository()
        .seed(&mut tx, seeded.clone())
        .await
        .unwrap();
    harness.uow().commit(tx).await.unwrap();
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    assert_eq!(harness.repository().list(&mut tx).await.unwrap().len(), 6);
    for expected in &seeded {
        assert!(!expected.value());
        assert_eq!(
            harness
                .repository()
                .get(&mut tx, expected.key())
                .await
                .unwrap()
                .unwrap()
                .version(),
            expected
        );
        assert_eq!(
            harness
                .repository()
                .history(&mut tx, expected.key(), 2)
                .await
                .unwrap(),
            vec![expected.clone()]
        );
        assert!(
            harness
                .repository()
                .history(&mut tx, expected.key(), 0)
                .await
                .is_err()
        );
    }
    harness.uow().commit(tx).await.unwrap();

    let key = FlagKey::PartnerWidgetApi;
    let next = FlagVersion::new(
        id::<VersionId>(20),
        key,
        true,
        id::<AdminId>(2),
        OffsetDateTime::UNIX_EPOCH,
    );
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    assert_eq!(
        harness
            .repository()
            .lock(&mut tx, vec![key])
            .await
            .unwrap()
            .len(),
        1
    );
    harness
        .repository()
        .append_version(&mut tx, next.clone())
        .await
        .unwrap();
    assert!(
        harness
            .repository()
            .append_version(&mut tx, next.clone())
            .await
            .is_err()
    );
    harness
        .repository()
        .set_live(&mut tx, key, Some(seeded[0].version()), next.version())
        .await
        .unwrap();
    assert_eq!(
        harness
            .repository()
            .set_live(&mut tx, key, Some(seeded[0].version()), next.version())
            .await
            .unwrap_err()
            .category(),
        ErrorCategory::Conflict
    );
    harness.uow().commit(tx).await.unwrap();
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    assert_eq!(
        harness.repository().get(&mut tx, key).await.unwrap(),
        Some(FlagRecord::new(next.clone()))
    );
    assert_eq!(
        harness.repository().history(&mut tx, key, 1).await.unwrap(),
        vec![next]
    );
    harness.uow().commit(tx).await.unwrap();
    let mut foreign = harness.begin_foreign().await.unwrap();
    assert!(harness.repository().list(&mut foreign).await.is_err());
}
