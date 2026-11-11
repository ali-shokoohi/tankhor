//! FR-004/005, AC-ADM-004/007/043: unchanged fake/real persistence contract.

use admin::{
    domain::{
        admin_state::{
            AdminState, Eligibility, EnrollmentAuthorization, EnrollmentPurpose, EnrollmentStatus,
            Invalidation, SecurityEpoch,
        },
        audit::{Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, ReasonCode, Subject},
    },
    ports::{
        admin_repository::{AdminListQuery, AdminRepository, AuthorizeEnrollment, IdentityDigest},
        audit_repository::{AuditFilter, AuditQuery, AuditRepository},
    },
};
use kernel::{
    error::{DomainError, ErrorCategory, ErrorCode},
    ids::{AdminId, AuditId},
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use std::str::FromStr;
use time::{Duration, OffsetDateTime};
use tokio::sync::Barrier;

/// Fresh isolated backend; controls are test infrastructure, not repository methods.
#[async_trait::async_trait]
pub trait AdminRepositoryContractHarness: Send + Sync {
    type Uow: UnitOfWork;
    type Repository: AdminRepository;
    type Audit: AuditRepository;

    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;
    fn audit(&self) -> &Self::Audit;
    async fn fail_next_audit(&self) -> Result<(), DomainError>;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn id(n: u16) -> AdminId {
    AdminId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap()
}

fn identity(n: u8) -> IdentityDigest {
    IdentityDigest::new([n; 32])
}

fn epoch(n: u64) -> SecurityEpoch {
    SecurityEpoch::new(n)
}

fn state(n: u16, eligibility: Eligibility, version: u64) -> AdminState {
    AdminState::new(id(n), eligibility, epoch(version))
}

fn command(target: u16, purpose: EnrollmentPurpose, version: u64) -> AuthorizeEnrollment {
    AuthorizeEnrollment {
        actor: id(1),
        target: id(target),
        purpose,
        expected_epoch: epoch(version),
        expected_authorization: None,
        authorized_at: OffsetDateTime::UNIX_EPOCH,
        expires_at: OffsetDateTime::UNIX_EPOCH + Duration::minutes(5),
    }
}

fn assert_error<T: std::fmt::Debug>(result: Result<T, DomainError>, category: ErrorCategory) {
    assert_eq!(result.unwrap_err().category(), category);
}

async fn read<H: AdminRepositoryContractHarness>(h: &H, n: u16) -> AdminState {
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    let value = h.repository().get(&mut tx, id(n)).await.unwrap().unwrap();
    h.uow().commit(tx).await.unwrap();
    value
}

async fn seed<H: AdminRepositoryContractHarness>(h: &H) {
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    for (n, eligibility, version) in [
        (4, Eligibility::Recovering, 9),
        (2, Eligibility::Active, 4),
        (1, Eligibility::Active, 8),
        (3, Eligibility::Invited, 0),
        (5, Eligibility::Active, u64::MAX),
    ] {
        h.repository()
            .invite(
                &mut tx,
                state(n, eligibility, version),
                identity(u8::try_from(n).unwrap()),
            )
            .await
            .unwrap();
    }
    h.uow().commit(tx).await.unwrap();
}

async fn verify_reads_and_uniqueness<H: AdminRepositoryContractHarness>(h: &H) {
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        h.repository()
            .find_by_identity(&mut tx, identity(2))
            .await
            .unwrap(),
        Some(state(2, Eligibility::Active, 4))
    );
    assert!(
        h.repository()
            .find_by_identity(&mut tx, identity(99))
            .await
            .unwrap()
            .is_none()
    );
    assert!(h.repository().get(&mut tx, id(99)).await.unwrap().is_none());
    let mut after = None;
    let mut found = Vec::new();
    for _ in 0..6 {
        let page = h
            .repository()
            .list(&mut tx, AdminListQuery::new(after, 2).unwrap())
            .await
            .unwrap();
        assert!(page.entries.len() <= 2);
        for value in &page.entries {
            if let Some(previous) = found.last() {
                assert!(value.id() > *previous);
            }
            found.push(value.id());
        }
        match page.next {
            Some(next) => {
                assert_eq!(Some(next), found.last().copied());
                after = Some(next);
            }
            None => break,
        }
    }
    assert_eq!(found, (1..=5).map(id).collect::<Vec<_>>());
    h.uow().commit(tx).await.unwrap();

    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    let locked = h
        .repository()
        .lock(&mut tx, vec![id(4), id(2), id(99), id(1), id(2)])
        .await
        .unwrap();
    assert_eq!(
        locked.iter().map(AdminState::id).collect::<Vec<_>>(),
        vec![id(1), id(2), id(4)]
    );
    h.uow().commit(tx).await.unwrap();
    for (value, digest) in [
        (state(9, Eligibility::Invited, 0), identity(2)),
        (state(2, Eligibility::Invited, 0), identity(9)),
    ] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        assert_error(
            h.repository().invite(&mut tx, value, digest).await,
            ErrorCategory::Conflict,
        );
        h.uow().rollback(tx).await.unwrap();
    }
    assert_eq!(read(h, 2).await, state(2, Eligibility::Active, 4));
}

async fn invalidate<H: AdminRepositoryContractHarness>(h: &H, old: u64, kind: Invalidation) {
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.repository().lock(&mut tx, vec![id(2)]).await.unwrap();
    h.repository()
        .invalidate_epoch(&mut tx, id(2), epoch(old), epoch(old + 1), kind)
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
}

async fn verify_epochs<H: AdminRepositoryContractHarness>(h: &H) {
    assert!(read(h, 2).await.matches_epoch(epoch(4)));
    assert!(
        read(h, 2).await.matches_epoch(epoch(4)),
        "unchanged reads preserve authority"
    );
    for (old, kind, eligibility) in [
        (4, Invalidation::Deactivate, Eligibility::Deactivated),
        (5, Invalidation::Revoke, Eligibility::Revoked),
        (6, Invalidation::Recover, Eligibility::Recovering),
        (7, Invalidation::Recover, Eligibility::Recovering),
    ] {
        invalidate(h, old, kind).await;
        assert_eq!(read(h, 2).await, state(2, eligibility, old + 1));
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.repository()
            .restore(&mut tx, id(2), epoch(old + 1))
            .await
            .unwrap();
        h.uow().commit(tx).await.unwrap();
        let restored = read(h, 2).await;
        assert_eq!(restored, state(2, Eligibility::Active, old + 1));
        assert!(!restored.matches_epoch(epoch(old)));
    }
    for (expected, next) in [(7, 9), (8, 8), (8, 0)] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        assert_error(
            h.repository()
                .invalidate_epoch(
                    &mut tx,
                    id(2),
                    epoch(expected),
                    epoch(next),
                    Invalidation::Revoke,
                )
                .await,
            ErrorCategory::Conflict,
        );
        h.uow().rollback(tx).await.unwrap();
        assert_eq!(read(h, 2).await, state(2, Eligibility::Active, 8));
    }
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_error(
        h.repository().restore(&mut tx, id(2), epoch(7)).await,
        ErrorCategory::Conflict,
    );
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(epoch(u64::MAX).advance().is_err());
    assert_error(
        h.repository()
            .invalidate_epoch(
                &mut tx,
                id(5),
                epoch(u64::MAX),
                epoch(0),
                Invalidation::Recover,
            )
            .await,
        ErrorCategory::Conflict,
    );
    h.uow().rollback(tx).await.unwrap();
    assert_eq!(read(h, 5).await, state(5, Eligibility::Active, u64::MAX));
    for explicit in [true, false] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.repository()
            .invalidate_epoch(&mut tx, id(2), epoch(8), epoch(9), Invalidation::Recover)
            .await
            .unwrap();
        assert_eq!(
            h.repository().get(&mut tx, id(2)).await.unwrap(),
            Some(state(2, Eligibility::Recovering, 9))
        );
        if explicit {
            h.uow().rollback(tx).await.unwrap();
        } else {
            drop(tx);
        }
        assert_eq!(read(h, 2).await, state(2, Eligibility::Active, 8));
    }
    assert_eq!(read(h, 1).await, state(1, Eligibility::Active, 8));
}

async fn authorize<H: AdminRepositoryContractHarness>(
    h: &H,
    value: AuthorizeEnrollment,
) -> EnrollmentAuthorization {
    let target = value.target;
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.repository()
        .authorize_enrollment(&mut tx, value)
        .await
        .unwrap();
    let authorization = h
        .repository()
        .enrollment(&mut tx, target)
        .await
        .unwrap()
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    authorization
}

async fn verify_authorization_refusals<H: AdminRepositoryContractHarness>(h: &H) {
    let valid = command(3, EnrollmentPurpose::Addition, 0);
    let mut invalid = Vec::new();
    let mut value = valid.clone();
    value.actor = id(3);
    invalid.push((value, ErrorCategory::Forbidden));
    let mut value = valid.clone();
    value.actor = id(4);
    invalid.push((value, ErrorCategory::Forbidden));
    let mut value = valid.clone();
    value.purpose = EnrollmentPurpose::Recovery;
    invalid.push((value, ErrorCategory::Forbidden));
    let mut value = valid.clone();
    value.expected_epoch = epoch(1);
    invalid.push((value, ErrorCategory::Conflict));
    let mut value = valid.clone();
    value.expires_at = value.authorized_at;
    invalid.push((value, ErrorCategory::Validation));
    let mut value = valid;
    value.target = id(1);
    invalid.push((value, ErrorCategory::Forbidden));
    for (value, category) in invalid {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        assert_error(
            h.repository().authorize_enrollment(&mut tx, value).await,
            category,
        );
        h.uow().rollback(tx).await.unwrap();
        assert!(read(h, 3).await.enrollment().is_none());
    }
}

async fn verify_enrollment<H: AdminRepositoryContractHarness>(h: &H) {
    verify_authorization_refusals(h).await;
    let original = authorize(h, command(3, EnrollmentPurpose::Addition, 0)).await;
    assert_eq!(original.purpose(), EnrollmentPurpose::Addition);
    assert_eq!(original.authorizer(), id(1));
    assert_eq!(original.epoch(), epoch(0));
    assert_eq!(original.status(), EnrollmentStatus::Pending);
    let pending = read(h, 3).await;
    assert!(
        !pending.is_eligible(),
        "authorization is not console access"
    );
    assert_eq!(pending.enrollment(), Some(&original));
    let other = authorize(h, command(4, EnrollmentPurpose::Recovery, 9)).await;
    assert_eq!(other.purpose(), EnrollmentPurpose::Recovery);
    for (target, expected, now) in [
        (id(4), original.clone(), original.authorized_at()),
        (id(3), other.clone(), original.authorized_at()),
        (
            id(3),
            original.clone(),
            original.authorized_at() - Duration::SECOND,
        ),
        (id(3), original.clone(), original.expires_at()),
    ] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        assert!(
            h.repository()
                .consume_enrollment(&mut tx, target, expected, now)
                .await
                .is_err()
        );
        h.uow().rollback(tx).await.unwrap();
        assert_eq!(read(h, 3).await.enrollment(), Some(&original));
    }
    let mut replacement = command(3, EnrollmentPurpose::Addition, 0);
    replacement.expected_authorization = Some(original.clone());
    replacement.expires_at += Duration::MINUTE;
    let current = authorize(h, replacement.clone()).await;
    assert_ne!(current, original);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_error(
        h.repository()
            .authorize_enrollment(&mut tx, replacement)
            .await,
        ErrorCategory::Conflict,
    );
    h.uow().rollback(tx).await.unwrap();
    for consume in [true, false] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        let result = if consume {
            h.repository()
                .consume_enrollment(&mut tx, id(3), original.clone(), original.authorized_at())
                .await
        } else {
            h.repository()
                .revoke_enrollment(&mut tx, id(3), original.clone())
                .await
        };
        assert_error(result, ErrorCategory::Conflict);
        h.uow().rollback(tx).await.unwrap();
        assert_eq!(read(h, 3).await.enrollment(), Some(&current));
    }
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.repository()
        .revoke_enrollment(&mut tx, id(4), other.clone())
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    assert_eq!(
        read(h, 4).await.enrollment().unwrap().status(),
        EnrollmentStatus::Revoked
    );
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(
        h.repository()
            .consume_enrollment(&mut tx, id(4), other, current.authorized_at())
            .await
            .is_err()
    );
    h.uow().rollback(tx).await.unwrap();
    verify_consume_race(h, current).await;
}

async fn consume_attempt<H: AdminRepositoryContractHarness>(
    h: &H,
    barrier: &Barrier,
    expected: EnrollmentAuthorization,
) -> Result<(), DomainError> {
    barrier.wait().await;
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await?;
    h.repository().lock(&mut tx, vec![id(3)]).await?;
    let result = h
        .repository()
        .consume_enrollment(&mut tx, id(3), expected.clone(), expected.authorized_at())
        .await;
    if result.is_ok() {
        h.uow().commit(tx).await?;
    } else {
        h.uow().rollback(tx).await?;
    }
    result
}

async fn verify_consume_race<H: AdminRepositoryContractHarness>(
    h: &H,
    expected: EnrollmentAuthorization,
) {
    let barrier = Barrier::new(2);
    let (first, second) = tokio::join!(
        consume_attempt(h, &barrier, expected.clone()),
        consume_attempt(h, &barrier, expected)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert_error(
        if first.is_err() { first } else { second },
        ErrorCategory::Conflict,
    );
    assert_eq!(
        read(h, 3).await.enrollment().unwrap().status(),
        EnrollmentStatus::Consumed
    );
}

async fn invalidation_attempt<H: AdminRepositoryContractHarness>(
    h: &H,
    barrier: &Barrier,
    kind: Invalidation,
) -> Result<(), DomainError> {
    barrier.wait().await;
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await?;
    h.repository().lock(&mut tx, vec![id(2)]).await?;
    let result = h
        .repository()
        .invalidate_epoch(&mut tx, id(2), epoch(8), epoch(9), kind)
        .await;
    if result.is_ok() {
        h.uow().commit(tx).await?;
    } else {
        h.uow().rollback(tx).await?;
    }
    result
}

async fn verify_invalidation_race<H: AdminRepositoryContractHarness>(h: &H) {
    let barrier = Barrier::new(2);
    let (first, second) = tokio::join!(
        invalidation_attempt(h, &barrier, Invalidation::Deactivate),
        invalidation_attempt(h, &barrier, Invalidation::Revoke)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let eligibility = if first.is_ok() {
        Eligibility::Deactivated
    } else {
        Eligibility::Revoked
    };
    assert_error(
        if first.is_err() { first } else { second },
        ErrorCategory::Conflict,
    );
    assert_eq!(read(h, 2).await, state(2, eligibility, 9));
}

fn recovery_audit() -> AuditEntry {
    let audit_id = AuditId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap();
    AuditEntry::try_from(AuditInput {
        id: audit_id,
        event_key: audit_id,
        actor: Some(Actor::Admin(id(1))),
        action: Some(AuditAction::Recovered),
        subject: Some(Subject::Admin(id(4))),
        occurred_at: Some(OffsetDateTime::UNIX_EPOCH),
        reason: Some(ReasonCode::AdminRecovery),
        metadata: AuditMetadata::Eligibility {
            before: false,
            after: false,
        },
    })
    .unwrap()
}

async fn verify_atomic_recovery<H: AdminRepositoryContractHarness>(h: &H) {
    let before = read(h, 4).await;
    for fail in [true, false] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.repository()
            .lock(&mut tx, vec![id(4), id(1)])
            .await
            .unwrap();
        h.repository()
            .invalidate_epoch(&mut tx, id(4), epoch(9), epoch(10), Invalidation::Recover)
            .await
            .unwrap();
        let mut replacement = command(4, EnrollmentPurpose::Recovery, 10);
        replacement.expected_authorization =
            h.repository().enrollment(&mut tx, id(4)).await.unwrap();
        h.repository()
            .authorize_enrollment(&mut tx, replacement)
            .await
            .unwrap();
        if fail {
            h.fail_next_audit().await.unwrap();
        }
        let result = h.audit().append(&mut tx, recovery_audit()).await;
        if fail {
            assert_error(result, ErrorCategory::Unavailable);
            h.uow().rollback(tx).await.unwrap();
            assert_eq!(read(h, 4).await, before);
        } else {
            result.unwrap();
            h.uow().commit(tx).await.unwrap();
            let recovered = read(h, 4).await;
            assert_eq!(recovered.epoch(), epoch(10));
            assert!(!recovered.is_eligible());
            assert_eq!(
                recovered.enrollment().unwrap().status(),
                EnrollmentStatus::Pending
            );
            assert_eq!(recovered.enrollment().unwrap().epoch(), epoch(10));
        }
        let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
        let page = h
            .audit()
            .search(
                &mut tx,
                AuditQuery::new(
                    AuditFilter::new(None, None, None, None, None).unwrap(),
                    None,
                    10,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            page.entries,
            if fail { vec![] } else { vec![recovery_audit()] }
        );
        h.uow().commit(tx).await.unwrap();
    }
    assert_eq!(read(h, 1).await, state(1, Eligibility::Active, 8));
}

async fn verify_handles<H: AdminRepositoryContractHarness>(h: &H) {
    let mut foreign = h.begin_foreign().await.unwrap();
    assert_error(
        h.repository().get(&mut foreign, id(1)).await,
        ErrorCategory::InvalidHandle,
    );
    assert_error(
        h.repository()
            .invite(
                &mut foreign,
                state(99, Eligibility::Invited, 0),
                identity(99),
            )
            .await,
        ErrorCategory::InvalidHandle,
    );
    drop(foreign);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        h.repository()
            .invite(&mut tx, state(99, Eligibility::Invited, 0), identity(99))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    let store_id = tx.store_id();
    h.uow().commit(tx).await.unwrap();
    // Exercise the opaque carrier's finished state without any backend downcast.
    let mut finished = TxHandle::new(store_id, TransactionMode::ReadWrite, ());
    finished.take::<()>(store_id).unwrap();
    assert_eq!(
        h.repository()
            .get(&mut finished, id(1))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::FinishedTransactionHandle
    );
    assert_eq!(
        h.repository()
            .invite(
                &mut finished,
                state(99, Eligibility::Invited, 0),
                identity(99)
            )
            .await
            .unwrap_err()
            .code(),
        ErrorCode::FinishedTransactionHandle
    );
}

async fn verify_current_authority<H: AdminRepositoryContractHarness>(h: &H) {
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.repository()
        .invite(&mut tx, state(6, Eligibility::Recovering, 0), identity(6))
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    let pending = authorize(h, command(6, EnrollmentPurpose::Recovery, 0)).await;
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.repository()
        .invalidate_epoch(&mut tx, id(6), epoch(0), epoch(1), Invalidation::Recover)
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    let invalidated = read(h, 6).await;
    assert_eq!(invalidated.epoch(), epoch(1));
    assert_eq!(
        invalidated.enrollment().unwrap().status(),
        EnrollmentStatus::Revoked
    );
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_error(
        h.repository()
            .consume_enrollment(&mut tx, id(6), pending, OffsetDateTime::UNIX_EPOCH)
            .await,
        ErrorCategory::Conflict,
    );
    h.uow().rollback(tx).await.unwrap();

    for (old, kind) in [(8, Invalidation::Deactivate), (9, Invalidation::Revoke)] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.repository()
            .invalidate_epoch(&mut tx, id(1), epoch(old), epoch(old + 1), kind)
            .await
            .unwrap();
        h.uow().commit(tx).await.unwrap();
        let mut replacement = command(6, EnrollmentPurpose::Recovery, 1);
        replacement.expected_authorization = invalidated.enrollment().cloned();
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        assert_error(
            h.repository()
                .authorize_enrollment(&mut tx, replacement)
                .await,
            ErrorCategory::Forbidden,
        );
        h.uow().rollback(tx).await.unwrap();
        assert_eq!(read(h, 6).await, invalidated);
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.repository()
            .restore(&mut tx, id(1), epoch(old + 1))
            .await
            .unwrap();
        h.uow().commit(tx).await.unwrap();
    }
    let mut replacement = command(6, EnrollmentPurpose::Recovery, 1);
    replacement.expected_authorization = invalidated.enrollment().cloned();
    let renewed = authorize(h, replacement).await;
    assert_eq!(renewed.epoch(), epoch(1));
    assert_eq!(renewed.status(), EnrollmentStatus::Pending);
}

/// Execute unchanged on a fresh fake or real adapter; no backend state inspection.
pub async fn admin_repository_contract<H: AdminRepositoryContractHarness>(h: &H) {
    seed(h).await;
    verify_reads_and_uniqueness(h).await;
    verify_epochs(h).await;
    verify_enrollment(h).await;
    verify_invalidation_race(h).await;
    verify_atomic_recovery(h).await;
    verify_current_authority(h).await;
    verify_handles(h).await;
}
