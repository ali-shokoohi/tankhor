//! Unchanged fake/real settings persistence contract.

use admin::{
    domain::settings::{
        SettingKey, SettingValue, initial_numeric_settings, initial_upload_formats,
    },
    ports::settings_repository::{SettingRecord, SettingVersion, SettingsRepository},
};
use kernel::{
    error::{DomainError, ErrorCategory},
    ids::{AdminId, VersionId},
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use std::str::FromStr;
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait SettingsRepositoryContractHarness: Send + Sync {
    type Uow: UnitOfWork;
    type Repository: SettingsRepository;
    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-1000-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}

fn versions() -> Vec<SettingVersion> {
    let actor = id::<AdminId>(1);
    let mut result = initial_numeric_settings()
        .into_iter()
        .enumerate()
        .map(|(index, setting)| {
            SettingVersion::new(
                id::<VersionId>(u16::try_from(index + 1).unwrap()),
                setting.key(),
                SettingValue::Number(i128::from(setting.value())),
                actor,
                OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let formats = initial_upload_formats();
    result.push(
        SettingVersion::new(
            id::<VersionId>(10),
            formats.key(),
            SettingValue::Formats(formats.value()),
            actor,
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap(),
    );
    result
}

/// # Panics
/// Panics if the repository or unit-of-work violates the shared contract.
#[allow(clippy::too_many_lines)]
pub async fn settings_repository_contract<H: SettingsRepositoryContractHarness>(harness: &H) {
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
    let listed = harness.repository().list(&mut tx).await.unwrap();
    assert_eq!(listed.len(), 10);
    for expected in &seeded {
        let actual = harness
            .repository()
            .get(&mut tx, expected.key())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual.version(), expected);
        let history = harness
            .repository()
            .history(&mut tx, expected.key(), 2)
            .await
            .unwrap();
        assert_eq!(history, vec![expected.clone()]);
        assert!(
            harness
                .repository()
                .history(&mut tx, expected.key(), 0)
                .await
                .is_err()
        );
    }
    harness.uow().commit(tx).await.unwrap();

    let key = SettingKey::UploadMaxMb;
    let next = SettingVersion::new(
        id::<VersionId>(20),
        key,
        SettingValue::Number(25),
        id::<AdminId>(2),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
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
        .set_live(&mut tx, key, Some(seeded[6].version()), next.version())
        .await
        .unwrap();
    assert_eq!(
        harness
            .repository()
            .set_live(&mut tx, key, Some(seeded[6].version()), next.version())
            .await
            .unwrap_err()
            .category(),
        ErrorCategory::Conflict,
    );
    harness.uow().commit(tx).await.unwrap();

    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    assert_eq!(
        harness.repository().get(&mut tx, key).await.unwrap(),
        Some(SettingRecord::new(next.clone()))
    );
    assert_eq!(
        harness.repository().history(&mut tx, key, 1).await.unwrap(),
        vec![next]
    );
    harness.uow().commit(tx).await.unwrap();

    let mut foreign = harness.begin_foreign().await.unwrap();
    assert!(harness.repository().list(&mut foreign).await.is_err());
}
