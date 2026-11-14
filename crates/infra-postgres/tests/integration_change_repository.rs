use admin::domain::change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue};
use admin::domain::settings::{SettingKey, SettingValue};
use admin::ports::change_repository::ChangeRepository;
use admin::ports::settings_repository::{SettingVersion, SettingsRepository};
use infra_postgres::change_repository::PostgresChangeRepository;
use infra_postgres::settings_repository::PostgresSettingsRepository;
use infra_postgres::unit_of_work::PostgresUnitOfWork;
use kernel::error::DomainError;
use kernel::ids::{AdminId, ChangeId, RevisionId, VersionId};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::str::FromStr;
use testkit::contracts::change_repository::{
    ChangeRepositoryContractHarness, change_repository_contract, change_repository_contract_admins,
};
use testkit::fixtures::database::FixtureDatabase;
use time::macros::datetime;

struct Harness {
    uow: PostgresUnitOfWork,
    foreign_uow: PostgresUnitOfWork,
    repository: PostgresChangeRepository,
}

#[async_trait::async_trait]
impl ChangeRepositoryContractHarness for Harness {
    type Uow = PostgresUnitOfWork;
    type Repository = PostgresChangeRepository;
    fn uow(&self) -> &Self::Uow {
        &self.uow
    }
    fn repository(&self) -> &Self::Repository {
        &self.repository
    }
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError> {
        self.foreign_uow.begin(TransactionMode::ReadWrite).await
    }
}

fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-5000-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}

async fn fixture(admin_pool_options: PgPoolOptions, admin_options: PgConnectOptions) -> Harness {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .unwrap();
    database
        .verify_migrated_and_role_separation()
        .await
        .unwrap();
    let admins = change_repository_contract_admins();
    for (index, admin) in admins.iter().enumerate() {
        let mut digest = [0_u8; 32];
        digest[31] = 0x31 + u8::try_from(index).unwrap();
        sqlx::query(
            "INSERT INTO admins (
                 id, phone_key_id, phone_nonce, phone_ciphertext, phone_lookup_digest,
                 eligibility, security_epoch, created_by, created_at
             ) VALUES ($1, 'test', '\\x01', '\\x01', $2, 'active', 0, $3, '1970-01-01T00:00:00Z')",
        )
        .bind(admin.as_uuid())
        .bind(digest.as_slice())
        .bind(admins[0].as_uuid())
        .execute(database.admin_pool())
        .await
        .unwrap();
    }
    let uow = PostgresUnitOfWork::new(database.connect_runtime_pool(4).await.unwrap());
    let foreign_uow = PostgresUnitOfWork::new(database.connect_runtime_pool(1).await.unwrap());
    let settings = PostgresSettingsRepository::new(uow.clone());
    let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
    settings
        .seed(
            &mut tx,
            vec![
                SettingVersion::new(
                    id::<VersionId>(2),
                    SettingKey::UploadMaxMb,
                    SettingValue::Number(15),
                    id::<AdminId>(6),
                    datetime!(2025-01-01 00:00 UTC),
                )
                .unwrap(),
            ],
        )
        .await
        .unwrap();
    uow.commit(tx).await.unwrap();
    let repository = PostgresChangeRepository::new(uow.clone());
    Harness {
        uow,
        foreign_uow,
        repository,
    }
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn change_repository_contract_postgres(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let harness = fixture(admin_pool_options, admin_options).await;
    change_repository_contract(&harness).await;
    harness.uow.pool().close().await;
    harness.foreign_uow.pool().close().await;
}

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn concurrent_stale_revision_saves_cannot_both_advance_head(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let harness = fixture(admin_pool_options, admin_options).await;
    let admins = change_repository_contract_admins();
    let creator = admins[1];
    let change = id::<ChangeId>(20);
    let first = ChangeDraft::new(
        change,
        ChangeSubject::Setting(SettingKey::UploadMaxMb),
        id(2),
        creator,
    )
    .append_revision(
        ChangeRevision::new(
            id(21),
            change,
            1,
            ChangeValue::Number(12),
            None,
            creator,
            datetime!(2026-01-01 00:00 UTC),
        ),
        None,
    )
    .unwrap();
    let mut tx = harness.uow.begin(TransactionMode::ReadWrite).await.unwrap();
    harness
        .repository
        .create_draft(&mut tx, first.clone())
        .await
        .unwrap();
    harness.uow.commit(tx).await.unwrap();

    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut workers = Vec::new();
    for (revision_number, author) in [(22, admins[2]), (23, admins[3])] {
        let uow = harness.uow.clone();
        let repository = harness.repository.clone();
        let barrier = barrier.clone();
        let original = first.clone();
        workers.push(tokio::spawn(async move {
            let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
            let stored = repository.draft(&mut tx, change).await.unwrap().unwrap();
            assert_eq!(stored.head(), original.head());
            barrier.wait().await;
            let next = stored
                .append_revision(
                    ChangeRevision::new(
                        id::<RevisionId>(revision_number),
                        change,
                        2,
                        ChangeValue::Number(12),
                        None,
                        author,
                        datetime!(2026-01-02 00:00 UTC),
                    ),
                    stored.head(),
                )
                .unwrap();
            let saved = repository
                .append_revision(&mut tx, next, original.head())
                .await;
            match saved {
                Ok(()) => uow.commit(tx).await.map(|()| author),
                Err(error) => {
                    uow.rollback(tx).await.unwrap();
                    Err(error)
                }
            }
        }));
    }
    let mut results = Vec::new();
    for worker in workers {
        results.push(worker.await.unwrap());
    }
    let winners: Vec<AdminId> = results.iter().filter_map(|result| (*result).ok()).collect();
    assert_eq!(winners.len(), 1);
    let mut tx = harness.uow.begin(TransactionMode::ReadOnly).await.unwrap();
    let stored = harness
        .repository
        .draft(&mut tx, change)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.revisions().len(), 2);
    assert_eq!(&stored.revisions()[..1], first.revisions());
    assert_eq!(stored.creator(), creator);
    assert_eq!(stored.revisions()[0].created_by(), creator);
    assert_eq!(stored.current().unwrap().created_by(), winners[0]);
    harness.uow.commit(tx).await.unwrap();
    harness.uow.pool().close().await;
    harness.foreign_uow.pool().close().await;
}
