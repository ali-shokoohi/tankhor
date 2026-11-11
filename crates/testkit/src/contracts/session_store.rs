//! FR-002/003/004, AC-ADM-003/043: shared fake/real session and ceremony contract.

use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, EnrollmentPurpose, Invalidation, SecurityEpoch},
        audit::Factor,
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        secrets::SecretBytes,
        session_state::{SessionInput, SessionState},
    },
    ports::{
        admin_repository::{AdminRepository, AuthorizeEnrollment, IdentityDigest},
        session_store::{PendingCheck, PendingRecord, SessionRecord, SessionStore},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ChallengeId, SessionId},
    ports::unit_of_work::{StoreId, TransactionMode, TxHandle, UnitOfWork},
};
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

#[async_trait::async_trait]
pub trait SessionStoreContractHarness: Sync {
    type Uow: UnitOfWork;
    type Sessions: SessionStore;
    type Admins: AdminRepository;
    fn uow(&self) -> &Self::Uow;
    fn sessions(&self) -> &Self::Sessions;
    fn admins(&self) -> &Self::Admins;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn admin(n: u8) -> AdminId {
    AdminId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap()
}

fn session_id(n: u8) -> SessionId {
    SessionId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap()
}

fn digest(n: u8) -> BindingDigest {
    BindingDigest::new([n; 32])
}
fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}
fn epoch() -> SecurityEpoch {
    SecurityEpoch::new(4)
}

fn session(n: u8, target: u8, absolute: i64, idle: i64) -> SessionRecord {
    SessionRecord {
        digest: digest(n),
        state: SessionState::try_from(SessionInput {
            id: session_id(n),
            admin_id: admin(target),
            epoch: epoch(),
            primary_verified: true,
            factor: Some(Factor::Passkey),
            issued_at: now(),
            expires_at: now() + Duration::minutes(absolute),
            idle_timeout: Duration::minutes(idle),
        })
        .unwrap(),
    }
}

fn pending(n: u8, target: u8, stage: AuthStage) -> (PendingRecord, PendingCheck) {
    let binding = AuthBinding {
        target: admin(target),
        browser: digest(201),
        session: session_id(202),
        purpose: if target == 3 {
            AuthPurpose::Enrollment(EnrollmentPurpose::Recovery)
        } else {
            AuthPurpose::SignIn
        },
        stage,
        epoch: epoch(),
        credential_revision: (stage == AuthStage::Passkey).then_some(7),
    };
    let auth = AuthState::try_from(AuthInput {
        id: ChallengeId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap(),
        binding: binding.clone(),
        primary_verified_at: Some(now()),
        created_at: now(),
        expires_at: now() + Duration::minutes(2),
    })
    .unwrap();
    let verification = (stage != AuthStage::PrimaryVerified)
        .then(|| SecretBytes::new(b"synthetic-verifier-envelope".to_vec()));
    let provisioning = (target == 3 && stage == AuthStage::Totp)
        .then(|| SecretBytes::new(b"synthetic-provisioning-envelope".to_vec()));
    (
        PendingRecord::new(digest(n), auth, verification, provisioning).unwrap(),
        PendingCheck {
            reference: digest(n),
            binding,
            now: now(),
        },
    )
}

/// # Panics
/// Panics on expiry/authority, replay, transaction or secret-presentation violations.
pub async fn session_store_contract<H: SessionStoreContractHarness>(h: &H) {
    seed(h).await;
    verify_sessions(h).await;
    verify_pending(h).await;
    verify_presentation(h).await;
    verify_races(h).await;
    verify_recovery(h).await;
    verify_handles(h).await;
}

async fn seed<H: SessionStoreContractHarness>(h: &H) {
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    for n in [1, 2, 3] {
        let eligibility = if n == 3 {
            Eligibility::Recovering
        } else {
            Eligibility::Active
        };
        h.admins()
            .invite(
                &mut tx,
                AdminState::new(admin(n), eligibility, epoch()),
                IdentityDigest::new([n; 32]),
            )
            .await
            .unwrap();
    }
    h.admins()
        .authorize_enrollment(
            &mut tx,
            AuthorizeEnrollment {
                actor: admin(1),
                target: admin(3),
                purpose: EnrollmentPurpose::Recovery,
                expected_epoch: epoch(),
                expected_authorization: None,
                authorized_at: now(),
                expires_at: now() + Duration::minutes(2),
            },
        )
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
}

async fn verify_sessions<H: SessionStoreContractHarness>(h: &H) {
    let s = h.sessions();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(s.lookup(&mut tx, digest(99)).await.unwrap(), None);
    for (n, target, absolute, idle) in [
        (11, 1, 10, 2),
        (12, 2, 10, 2),
        (13, 1, 3, 10),
        (14, 1, 10, 2),
    ] {
        s.insert(&mut tx, session(n, target, absolute, idle))
            .await
            .unwrap();
    }
    assert_eq!(
        s.lookup(&mut tx, digest(11)).await.unwrap(),
        Some(session(11, 1, 10, 2).state)
    );
    assert_eq!(
        s.insert(&mut tx, session(11, 1, 10, 2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::IdempotencyConflict
    );
    let mut duplicate = session(11, 1, 10, 2);
    duplicate.digest = digest(90);
    assert_eq!(
        s.insert(&mut tx, duplicate).await.unwrap_err().code(),
        ErrorCode::IdempotencyConflict
    );
    let mut duplicate = session(90, 1, 10, 2);
    duplicate.digest = digest(11);
    assert_eq!(
        s.insert(&mut tx, duplicate).await.unwrap_err().code(),
        ErrorCode::IdempotencyConflict
    );
    assert_eq!(
        s.insert(&mut tx, session(99, 99, 10, 2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        s.check_current(&mut tx, digest(99), now())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::AuthenticationRequired
    );
    assert_eq!(
        s.check_current(&mut tx, digest(11), now() + Duration::MINUTE)
            .await
            .unwrap()
            .last_activity_at(),
        now() + Duration::MINUTE
    );
    assert!(s.check_current(&mut tx, digest(11), now()).await.is_err());
    assert!(
        s.check_current(&mut tx, digest(11), now() + Duration::minutes(3))
            .await
            .is_err()
    );
    assert!(
        s.check_current(&mut tx, digest(13), now() + Duration::minutes(3))
            .await
            .is_err()
    );
    s.revoke_one(&mut tx, session_id(14), now()).await.unwrap();
    s.revoke_one(&mut tx, session_id(14), now()).await.unwrap();
    s.revoke_one(&mut tx, session_id(99), now()).await.unwrap();
    assert!(s.check_current(&mut tx, digest(14), now()).await.is_err());
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    s.insert(&mut tx, session(15, 1, 10, 2)).await.unwrap();
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(s.lookup(&mut tx, digest(15)).await.unwrap(), None);
    s.insert(&mut tx, session(15, 1, 10, 2)).await.unwrap();
    drop(tx);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(s.lookup(&mut tx, digest(15)).await.unwrap(), None);
    h.uow().rollback(tx).await.unwrap();
}

async fn verify_pending<H: SessionStoreContractHarness>(h: &H) {
    for (n, stage) in [
        (21, AuthStage::PrimaryVerified),
        (22, AuthStage::Passkey),
        (23, AuthStage::Totp),
    ] {
        let (record, check) = pending(n, 1, stage);
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.sessions()
            .create_pending(&mut tx, record.clone())
            .await
            .unwrap();
        assert_eq!(
            h.sessions()
                .create_pending(&mut tx, record.clone())
                .await
                .unwrap_err()
                .code(),
            ErrorCode::IdempotencyConflict
        );
        let (same_digest, _) = pending(n, 2, stage);
        assert_eq!(
            h.sessions()
                .create_pending(&mut tx, same_digest)
                .await
                .unwrap_err()
                .code(),
            ErrorCode::IdempotencyConflict
        );
        let view = h
            .sessions()
            .read_pending(&mut tx, check.clone())
            .await
            .unwrap();
        assert_eq!(view.reference, check.reference);
        assert_eq!(view.state, record.state().clone());
        assert!(!format!("{view:?}").contains("synthetic-"));
        assert!(!format!("{record:?}").contains("synthetic-"));
        if stage == AuthStage::PrimaryVerified {
            assert!(
                h.sessions()
                    .verification_state(&mut tx, check.clone())
                    .await
                    .is_err()
            );
        } else {
            assert_eq!(
                h.sessions()
                    .verification_state(&mut tx, check.clone())
                    .await
                    .unwrap()
                    .expose(),
                b"synthetic-verifier-envelope"
            );
        }
        verify_wrong_pending(h, &mut tx, &check).await;
        h.sessions()
            .consume_pending(&mut tx, check.clone())
            .await
            .unwrap();
        assert!(
            h.sessions()
                .consume_pending(&mut tx, check.clone())
                .await
                .is_err()
        );
        assert!(
            h.sessions()
                .read_pending(&mut tx, check.clone())
                .await
                .is_err()
        );
        h.uow().rollback(tx).await.unwrap();
        verify_pending_rollback(h, record, check).await;
    }
}

async fn verify_wrong_pending<H: SessionStoreContractHarness>(
    h: &H,
    tx: &mut TxHandle,
    check: &PendingCheck,
) {
    for wrong in wrong_checks(check) {
        assert_eq!(
            h.sessions()
                .read_pending(tx, wrong.clone())
                .await
                .unwrap_err()
                .code(),
            ErrorCode::PermissionDenied
        );
        assert!(
            h.sessions()
                .consume_pending(tx, wrong.clone())
                .await
                .is_err()
        );
        assert!(
            h.sessions()
                .supersede_pending(tx, wrong.clone())
                .await
                .is_err()
        );
        assert!(
            h.sessions()
                .verification_state(tx, wrong.clone())
                .await
                .is_err()
        );
        assert!(h.sessions().present_provisioning(tx, wrong).await.is_err());
    }
}

async fn verify_pending_rollback<H: SessionStoreContractHarness>(
    h: &H,
    record: PendingRecord,
    check: PendingCheck,
) {
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(
        h.sessions()
            .read_pending(&mut tx, check.clone())
            .await
            .is_err()
    );
    h.sessions().create_pending(&mut tx, record).await.unwrap();
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.sessions()
        .consume_pending(&mut tx, check.clone())
        .await
        .unwrap();
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.sessions()
        .read_pending(&mut tx, check.clone())
        .await
        .unwrap();
    h.sessions()
        .supersede_pending(&mut tx, check.clone())
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(h.sessions().read_pending(&mut tx, check).await.is_err());
    h.uow().rollback(tx).await.unwrap();
}

fn wrong_checks(check: &PendingCheck) -> Vec<PendingCheck> {
    (0..10)
        .map(|field| {
            let mut wrong = check.clone();
            match field {
                0 => wrong.reference = digest(99),
                1 => wrong.binding.browser = digest(99),
                2 => wrong.binding.session = session_id(99),
                3 => wrong.binding.target = admin(2),
                4 => {
                    wrong.binding.stage = if check.binding.stage == AuthStage::Passkey {
                        AuthStage::Totp
                    } else {
                        AuthStage::Passkey
                    }
                }
                5 => wrong.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Addition),
                6 => wrong.binding.epoch = SecurityEpoch::new(99),
                7 => wrong.binding.credential_revision = Some(99),
                8 => wrong.now += Duration::minutes(2),
                _ => wrong.now -= Duration::SECOND,
            }
            wrong
        })
        .collect()
}

async fn verify_presentation<H: SessionStoreContractHarness>(h: &H) {
    let s = h.sessions();
    let (record, check) = pending(31, 3, AuthStage::Totp);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    s.create_pending(&mut tx, record).await.unwrap();
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    for wrong in wrong_checks(&check) {
        assert!(s.present_provisioning(&mut tx, wrong).await.is_err());
    }
    assert!(
        !s.read_pending(&mut tx, check.clone())
            .await
            .unwrap()
            .provisioning_presented
    );
    assert_eq!(
        s.present_provisioning(&mut tx, check.clone())
            .await
            .unwrap()
            .expose(),
        b"synthetic-provisioning-envelope"
    );
    assert!(
        s.present_provisioning(&mut tx, check.clone())
            .await
            .is_err()
    );
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(
        !s.read_pending(&mut tx, check.clone())
            .await
            .unwrap()
            .provisioning_presented
    );
    s.present_provisioning(&mut tx, check.clone())
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(
        s.read_pending(&mut tx, check.clone())
            .await
            .unwrap()
            .provisioning_presented
    );
    assert!(
        s.present_provisioning(&mut tx, check.clone())
            .await
            .is_err()
    );
    assert_eq!(
        s.verification_state(&mut tx, check.clone())
            .await
            .unwrap()
            .expose(),
        b"synthetic-verifier-envelope"
    );
    s.consume_pending(&mut tx, check.clone()).await.unwrap();
    assert!(s.verification_state(&mut tx, check.clone()).await.is_err());
    h.uow().commit(tx).await.unwrap();
    for n in [32, 33] {
        let (record, check) = pending(n, 3, AuthStage::Totp);
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        s.create_pending(&mut tx, record).await.unwrap();
        if n == 32 {
            s.supersede_pending(&mut tx, check.clone()).await.unwrap();
        } else {
            s.revoke_pending(&mut tx, admin(3)).await.unwrap();
        }
        assert!(
            s.present_provisioning(&mut tx, check.clone())
                .await
                .is_err()
        );
        assert!(s.verification_state(&mut tx, check).await.is_err());
        h.uow().commit(tx).await.unwrap();
    }
}

async fn verify_races<H: SessionStoreContractHarness>(h: &H) {
    for (n, target, stage) in [(41, 1, AuthStage::Passkey), (42, 3, AuthStage::Totp)] {
        let (record, check) = pending(n, target, stage);
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.sessions().create_pending(&mut tx, record).await.unwrap();
        h.uow().commit(tx).await.unwrap();
        let attempt = || async {
            let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
            let result = if n == 41 {
                h.sessions().consume_pending(&mut tx, check.clone()).await
            } else {
                h.sessions()
                    .present_provisioning(&mut tx, check.clone())
                    .await
                    .map(|_| ())
            };
            h.uow().commit(tx).await.unwrap();
            result
        };
        let (first, second) = tokio::join!(attempt(), attempt());
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        assert_eq!(
            first.err().or_else(|| second.err()).unwrap().code(),
            ErrorCode::PermissionDenied
        );
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        if n == 42 {
            assert!(
                h.sessions()
                    .read_pending(&mut tx, check.clone())
                    .await
                    .unwrap()
                    .provisioning_presented
            );
            h.sessions()
                .verification_state(&mut tx, check)
                .await
                .unwrap();
        } else {
            assert!(h.sessions().read_pending(&mut tx, check).await.is_err());
        }
        h.uow().rollback(tx).await.unwrap();
    }
}

async fn verify_recovery<H: SessionStoreContractHarness>(h: &H) {
    let s = h.sessions();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    for (n, target) in [(51, 1), (52, 2)] {
        let (record, _) = pending(n, target, AuthStage::PrimaryVerified);
        s.create_pending(&mut tx, record).await.unwrap();
    }
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    s.revoke_all(&mut tx, admin(1), now()).await.unwrap();
    s.revoke_pending(&mut tx, admin(1)).await.unwrap();
    assert!(s.check_current(&mut tx, digest(13), now()).await.is_err());
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    s.check_current(&mut tx, digest(13), now()).await.unwrap();
    s.read_pending(&mut tx, pending(51, 1, AuthStage::PrimaryVerified).1)
        .await
        .unwrap();
    h.admins()
        .invalidate_epoch(
            &mut tx,
            admin(1),
            epoch(),
            SecurityEpoch::new(5),
            Invalidation::Recover,
        )
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(s.check_current(&mut tx, digest(13), now()).await.is_err());
    assert!(
        s.read_pending(&mut tx, pending(51, 1, AuthStage::PrimaryVerified).1)
            .await
            .is_err()
    );
    s.check_current(&mut tx, digest(12), now()).await.unwrap();
    s.read_pending(&mut tx, pending(52, 2, AuthStage::PrimaryVerified).1)
        .await
        .unwrap();
    for _ in 0..2 {
        s.revoke_all(&mut tx, admin(1), now()).await.unwrap();
        s.revoke_pending(&mut tx, admin(1)).await.unwrap();
    }
    h.admins()
        .restore(&mut tx, admin(1), SecurityEpoch::new(5))
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(s.check_current(&mut tx, digest(13), now()).await.is_err());
    assert!(
        s.read_pending(&mut tx, pending(51, 1, AuthStage::PrimaryVerified).1)
            .await
            .is_err()
    );
    s.check_current(&mut tx, digest(12), now()).await.unwrap();
    h.uow().rollback(tx).await.unwrap();
}

async fn verify_handles<H: SessionStoreContractHarness>(h: &H) {
    let s = h.sessions();
    let (record, check) = pending(61, 2, AuthStage::PrimaryVerified);
    let mut foreign = h.begin_foreign().await.unwrap();
    assert_eq!(
        s.lookup(&mut foreign, digest(12)).await.unwrap_err().code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        s.insert(&mut foreign, session(61, 2, 10, 2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        s.create_pending(&mut foreign, record.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        s.read_pending(&mut foreign, check.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    drop(foreign);
    verify_readonly(h, record, check).await;
}

async fn verify_readonly<H: SessionStoreContractHarness>(
    h: &H,
    record: PendingRecord,
    check: PendingCheck,
) {
    let s = h.sessions();
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        s.insert(&mut tx, session(61, 2, 10, 2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.check_current(&mut tx, digest(12), now())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.revoke_one(&mut tx, session_id(12), now())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.revoke_all(&mut tx, admin(2), now())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.create_pending(&mut tx, record).await.unwrap_err().code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.consume_pending(&mut tx, check.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.supersede_pending(&mut tx, check.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.revoke_pending(&mut tx, admin(2))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        s.present_provisioning(&mut tx, check.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    h.uow().rollback(tx).await.unwrap();
    let store = StoreId::new();
    let mut finished = TxHandle::new(store, TransactionMode::ReadWrite, ());
    finished.take::<()>(store).unwrap();
    assert_eq!(
        s.lookup(&mut finished, digest(12))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::FinishedTransactionHandle
    );
}
