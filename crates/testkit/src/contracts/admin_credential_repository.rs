//! FR-002/004, AC-ADM-002/043: unchanged fake/real credential persistence contract.

use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        credential_state::{CredentialKind, CredentialState},
        secrets::SecretBytes,
    },
    ports::{
        admin_credential_repository::{
            AdminCredentialRepository, CredentialMaterial, CredentialRecord, PasskeyRevision,
        },
        admin_repository::{AdminRepository, IdentityDigest},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, CredentialId},
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use std::str::FromStr;
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait AdminCredentialRepositoryContractHarness: Sync {
    type Uow: UnitOfWork;
    type Repository: AdminCredentialRepository;
    type Admins: AdminRepository;
    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;
    fn admins(&self) -> &Self::Admins;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn admin(number: u16) -> AdminId {
    AdminId::from_str(&format!("019a53a0-0000-7000-8000-{number:012x}")).unwrap()
}

fn id(number: u16) -> CredentialId {
    CredentialId::from_str(&format!("019a53a0-0000-7000-8000-{number:012x}")).unwrap()
}

fn record(number: u16, target: u16, kind: CredentialKind) -> CredentialRecord {
    let material = match kind {
        CredentialKind::Passkey => CredentialMaterial::Passkey {
            credential_id: SecretBytes::new(number.to_be_bytes().to_vec()),
            credential: SecretBytes::new(b"synthetic-library-state".to_vec()),
        },
        CredentialKind::Totp => CredentialMaterial::Totp {
            envelope: SecretBytes::new(b"synthetic-encrypted-envelope".to_vec()),
        },
    };
    CredentialRecord::new(
        CredentialState::new(id(number), admin(target), kind, SecurityEpoch::new(4), 1).unwrap(),
        material,
    )
    .unwrap()
}

fn update(number: u16, expected: u64, next: u64) -> PasskeyRevision {
    PasskeyRevision {
        id: id(number),
        epoch: SecurityEpoch::new(4),
        expected_revision: expected,
        next_revision: next,
        credential: SecretBytes::new(b"updated-counter-backup-state".to_vec()),
    }
}

/// # Panics
/// Panics on an adapter violating uniqueness, CAS, isolation or target-only recovery.
pub async fn admin_credential_repository_contract<H: AdminCredentialRepositoryContractHarness>(
    h: &H,
) {
    verify_insert(h).await;
    verify_revision(h).await;
    verify_steps_and_races(h).await;
    verify_revocation(h).await;
    verify_handles(h).await;
}

async fn verify_insert<H: AdminCredentialRepositoryContractHarness>(h: &H) {
    let repo = h.repository();
    let epoch = SecurityEpoch::new(4);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    for target in [1, 2] {
        h.admins()
            .invite(
                &mut tx,
                AdminState::new(admin(target), Eligibility::Active, epoch),
                IdentityDigest::new([u8::try_from(target).unwrap(); 32]),
            )
            .await
            .unwrap();
    }
    assert!(
        repo.list_active(&mut tx, admin(1))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(repo.lock(&mut tx, id(99)).await.unwrap(), None);
    let passkey = record(11, 1, CredentialKind::Passkey);
    let totp = record(12, 1, CredentialKind::Totp);
    let other = record(13, 2, CredentialKind::Passkey);
    repo.insert(&mut tx, totp.clone()).await.unwrap();
    repo.insert(&mut tx, passkey.clone()).await.unwrap();
    repo.insert(&mut tx, other.clone()).await.unwrap();
    assert_eq!(
        repo.list_active(&mut tx, admin(1)).await.unwrap(),
        vec![passkey.clone(), totp.clone()]
    );
    assert_eq!(
        repo.lock(&mut tx, id(11)).await.unwrap(),
        Some(passkey.clone())
    );
    assert_eq!(
        repo.insert(&mut tx, passkey.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::IdempotencyConflict
    );
    let duplicate = CredentialRecord::new(
        CredentialState::new(id(14), admin(2), CredentialKind::Passkey, epoch, 1).unwrap(),
        passkey.material().clone(),
    )
    .unwrap();
    assert_eq!(
        repo.insert(&mut tx, duplicate.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::IdempotencyConflict
    );
    assert_eq!(
        repo.insert(&mut tx, record(99, 99, CredentialKind::Totp))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    h.uow().commit(tx).await.unwrap();
}

async fn verify_revision<H: AdminCredentialRepositoryContractHarness>(h: &H) {
    let repo = h.repository();
    let epoch = SecurityEpoch::new(4);
    let passkey = record(11, 1, CredentialKind::Passkey);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    // All refusals preserve both policy state and serialized library material.
    for change in [
        update(11, 0, 2),
        update(11, 1, 1),
        update(11, 1, 0),
        PasskeyRevision {
            epoch: SecurityEpoch::new(3),
            ..update(11, 1, 2)
        },
    ] {
        assert_eq!(
            repo.compare_revision(&mut tx, change)
                .await
                .unwrap_err()
                .code(),
            ErrorCode::StaleRevision
        );
        assert_eq!(
            repo.lock(&mut tx, id(11)).await.unwrap(),
            Some(passkey.clone())
        );
    }
    assert_eq!(
        repo.compare_revision(
            &mut tx,
            PasskeyRevision {
                credential: SecretBytes::new(vec![]),
                ..update(11, 1, 2)
            }
        )
        .await
        .unwrap_err()
        .code(),
        ErrorCode::InvalidValue
    );
    assert!(
        repo.compare_revision(&mut tx, update(12, 1, 2))
            .await
            .is_err()
    );
    assert!(
        repo.accept_totp_step(&mut tx, id(11), epoch, 1, 10)
            .await
            .is_err()
    );
    assert_eq!(
        repo.compare_revision(&mut tx, update(99, 1, 2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    repo.compare_revision(&mut tx, update(11, 1, 2))
        .await
        .unwrap();
    let changed = repo.lock(&mut tx, id(11)).await.unwrap().unwrap();
    assert_eq!(changed.state().revision(), 2);
    assert_eq!(
        changed.material(),
        &CredentialMaterial::Passkey {
            credential_id: SecretBytes::new(11_u16.to_be_bytes().to_vec()),
            credential: SecretBytes::new(b"updated-counter-backup-state".to_vec())
        }
    );
    assert!(
        repo.compare_revision(&mut tx, update(11, 1, 3))
            .await
            .is_err()
    );
    h.uow().rollback(tx).await.unwrap();
}

async fn verify_steps_and_races<H: AdminCredentialRepositoryContractHarness>(h: &H) {
    let repo = h.repository();
    let epoch = SecurityEpoch::new(4);
    let passkey = record(11, 1, CredentialKind::Passkey);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(repo.lock(&mut tx, id(11)).await.unwrap(), Some(passkey));
    repo.compare_revision(&mut tx, update(11, 1, 2))
        .await
        .unwrap();
    // Enrollment consumes the matched step; sign-in cannot reuse it.
    repo.accept_totp_step(&mut tx, id(12), epoch, 1, 100)
        .await
        .unwrap();
    for (current, revision, step) in [
        (epoch, 1, 100),
        (epoch, 1, 99),
        (epoch, 2, 101),
        (SecurityEpoch::new(5), 1, 101),
    ] {
        assert_eq!(
            repo.accept_totp_step(&mut tx, id(12), current, revision, step)
                .await
                .unwrap_err()
                .code(),
            ErrorCode::StaleRevision
        );
        assert_eq!(
            repo.lock(&mut tx, id(12))
                .await
                .unwrap()
                .unwrap()
                .state()
                .last_totp_step(),
            Some(100)
        );
    }
    h.uow().commit(tx).await.unwrap();

    // Two independent finalization transactions can have only one replay/CAS winner.
    let accept = || async {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        let result = repo.accept_totp_step(&mut tx, id(12), epoch, 1, 101).await;
        if result.is_ok() {
            h.uow().commit(tx).await.unwrap();
        } else {
            h.uow().rollback(tx).await.unwrap();
        }
        result
    };
    let (a, b) = tokio::join!(accept(), accept());
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let revise = || async {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        let result = repo.compare_revision(&mut tx, update(11, 2, 3)).await;
        if result.is_ok() {
            h.uow().commit(tx).await.unwrap();
        } else {
            h.uow().rollback(tx).await.unwrap();
        }
        result
    };
    let (a, b) = tokio::join!(revise(), revise());
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
}

async fn verify_revocation<H: AdminCredentialRepositoryContractHarness>(h: &H) {
    let repo = h.repository();
    let epoch = SecurityEpoch::new(4);
    let other = record(13, 2, CredentialKind::Passkey);
    let duplicate = CredentialRecord::new(
        CredentialState::new(id(14), admin(2), CredentialKind::Passkey, epoch, 1).unwrap(),
        record(11, 1, CredentialKind::Passkey).material().clone(),
    )
    .unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    repo.revoke_all(&mut tx, admin(1), OffsetDateTime::UNIX_EPOCH)
        .await
        .unwrap();
    assert!(
        repo.list_active(&mut tx, admin(1))
            .await
            .unwrap()
            .is_empty()
    );
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(repo.list_active(&mut tx, admin(1)).await.unwrap().len(), 2);
    repo.revoke_all(&mut tx, admin(1), OffsetDateTime::UNIX_EPOCH)
        .await
        .unwrap();
    repo.revoke_all(&mut tx, admin(1), OffsetDateTime::UNIX_EPOCH)
        .await
        .unwrap();
    assert!(
        repo.list_active(&mut tx, admin(1))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        repo.list_active(&mut tx, admin(2)).await.unwrap(),
        vec![other]
    );
    assert!(
        repo.compare_revision(&mut tx, update(11, 3, 4))
            .await
            .is_err()
    );
    assert!(
        repo.accept_totp_step(&mut tx, id(12), epoch, 1, 102)
            .await
            .is_err()
    );
    assert_eq!(
        repo.insert(&mut tx, duplicate).await.unwrap_err().code(),
        ErrorCode::IdempotencyConflict
    );
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(
        repo.list_active(&mut tx, admin(1))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        repo.lock(&mut tx, id(12))
            .await
            .unwrap()
            .unwrap()
            .state()
            .last_totp_step(),
        Some(101)
    );
    // Failed transactions and dropped writes never escape the shared store.
    repo.insert(&mut tx, record(15, 2, CredentialKind::Totp))
        .await
        .unwrap();
    drop(tx);
}

async fn verify_handles<H: AdminCredentialRepositoryContractHarness>(h: &H) {
    let repo = h.repository();
    let epoch = SecurityEpoch::new(4);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(repo.lock(&mut tx, id(15)).await.unwrap(), None);
    assert_eq!(
        repo.insert(&mut tx, record(15, 2, CredentialKind::Totp))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        repo.revoke_all(&mut tx, admin(2), OffsetDateTime::UNIX_EPOCH)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        repo.compare_revision(&mut tx, update(13, 1, 2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        repo.accept_totp_step(&mut tx, id(12), epoch, 1, 102)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    h.uow().rollback(tx).await.unwrap();
    let mut foreign = h.begin_foreign().await.unwrap();
    assert_eq!(
        repo.list_active(&mut foreign, admin(1))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        repo.insert(&mut foreign, record(15, 2, CredentialKind::Totp))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        repo.lock(&mut foreign, id(11)).await.unwrap_err().code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        repo.compare_revision(&mut foreign, update(13, 1, 2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        repo.accept_totp_step(&mut foreign, id(12), epoch, 1, 102)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        repo.revoke_all(&mut foreign, admin(1), OffsetDateTime::UNIX_EPOCH)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    drop(foreign);
    let tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    let store_id = tx.store_id();
    h.uow().rollback(tx).await.unwrap();
    let mut finished = TxHandle::new(store_id, TransactionMode::ReadWrite, ());
    finished.take::<()>(store_id).unwrap();
    assert_eq!(
        repo.lock(&mut finished, id(11)).await.unwrap_err().code(),
        ErrorCode::FinishedTransactionHandle
    );
}
