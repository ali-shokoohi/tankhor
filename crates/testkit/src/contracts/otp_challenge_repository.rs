//! FR-002, FR-CON-02, AC-ADM-002, Q09: shared OTP persistence contract.

use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthPurpose, BindingDigest},
        otp_state::{OtpBinding, OtpDigest, OtpInput, OtpState},
    },
    ports::{
        admin_repository::{AdminRepository, IdentityDigest},
        otp_challenge_repository::{OtpChallengeRepository, OtpCheck, SubmissionCategory},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ChallengeId},
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

#[async_trait::async_trait]
pub trait OtpChallengeRepositoryContractHarness: Sync {
    type Uow: UnitOfWork;
    type Repository: OtpChallengeRepository;
    type Admins: AdminRepository;
    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;
    fn admins(&self) -> &Self::Admins;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn target() -> AdminId {
    AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap()
}
fn id(n: u8) -> ChallengeId {
    ChallengeId::from_str(&format!("019a53a0-0000-7000-8000-{n:012x}")).unwrap()
}
fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}
fn input(n: u8, failures: u32) -> OtpInput {
    OtpInput {
        id: id(n),
        binding: OtpBinding {
            target: target(),
            browser: BindingDigest::new([2; 32]),
            purpose: AuthPurpose::SignIn,
            epoch: SecurityEpoch::new(4),
        },
        digest: OtpDigest::new([n; 32]),
        issued_at: now(),
        expires_at: now() + Duration::MINUTE,
        max_attempts: 3,
        failed_attempts: failures,
    }
}
fn check(n: u8, failures: u32) -> OtpCheck {
    OtpCheck {
        id: id(n),
        binding: input(n, failures).binding,
        expected_failed_attempts: failures,
        now: now(),
    }
}

/// # Panics
/// Panics on lost budgets, changed retry identity, replay or transaction violations.
pub async fn otp_challenge_repository_contract<H: OtpChallengeRepositoryContractHarness>(h: &H) {
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.admins()
        .invite(
            &mut tx,
            AdminState::new(target(), Eligibility::Active, SecurityEpoch::new(4)),
            IdentityDigest::new([1; 32]),
        )
        .await
        .unwrap();
    assert_eq!(h.repository().lock(&mut tx, id(99)).await.unwrap(), None);
    assert_eq!(
        h.repository().submission(&mut tx, id(99)).await.unwrap(),
        None
    );
    h.repository()
        .issue(&mut tx, OtpState::try_from(input(11, 0)).unwrap(), None)
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    verify_attempts(h).await;
    verify_resend(h).await;
    verify_consumption_race(h).await;
    verify_handles(h).await;
}

async fn verify_attempts<H: OtpChallengeRepositoryContractHarness>(h: &H) {
    let r = h.repository();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    let original = r.lock(&mut tx, id(11)).await.unwrap().unwrap();
    assert_eq!(
        r.issue(&mut tx, original.clone(), None)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::IdempotencyConflict
    );
    for category in [
        SubmissionCategory::Accepted,
        SubmissionCategory::Rejected,
        SubmissionCategory::Unavailable,
        SubmissionCategory::Unknown,
    ] {
        r.record_submission(&mut tx, id(11), category)
            .await
            .unwrap();
        assert_eq!(r.submission(&mut tx, id(11)).await.unwrap(), Some(category));
        assert_eq!(
            r.lock(&mut tx, id(11)).await.unwrap(),
            Some(original.clone())
        );
    }
    for field in 0..7 {
        let mut wrong = check(11, 0);
        match field {
            0 => wrong.binding.browser = BindingDigest::new([99; 32]),
            1 => wrong.binding.epoch = SecurityEpoch::new(99),
            2 => {
                wrong.binding.purpose = AuthPurpose::Enrollment(
                    admin::domain::admin_state::EnrollmentPurpose::Addition,
                );
            }
            3 => wrong.now += Duration::MINUTE,
            4 => wrong.now -= Duration::SECOND,
            5 => wrong.expected_failed_attempts = 1,
            _ => wrong.id = id(99),
        }
        assert!(r.record_failure(&mut tx, wrong.clone()).await.is_err());
        assert!(
            r.consume_once(&mut tx, wrong, OtpDigest::new([11; 32]))
                .await
                .is_err()
        );
        assert_eq!(
            r.lock(&mut tx, id(11)).await.unwrap(),
            Some(original.clone())
        );
    }
    assert!(
        r.consume_once(&mut tx, check(11, 0), OtpDigest::new([99; 32]))
            .await
            .is_err()
    );
    assert_eq!(r.lock(&mut tx, id(11)).await.unwrap(), Some(original));
    r.record_failure(&mut tx, check(11, 0)).await.unwrap();
    assert!(r.record_failure(&mut tx, check(11, 0)).await.is_err());
    assert_eq!(
        r.lock(&mut tx, id(11))
            .await
            .unwrap()
            .unwrap()
            .failed_attempts(),
        1
    );
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(
        r.lock(&mut tx, id(11))
            .await
            .unwrap()
            .unwrap()
            .failed_attempts(),
        0
    );
    r.record_failure(&mut tx, check(11, 0)).await.unwrap();
    h.uow().commit(tx).await.unwrap();
}

async fn verify_resend<H: OtpChallengeRepositoryContractHarness>(h: &H) {
    let r = h.repository();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    for predecessor in [None, Some(id(99))] {
        assert!(
            r.issue(
                &mut tx,
                OtpState::try_from(input(12, 1)).unwrap(),
                predecessor
            )
            .await
            .is_err()
        );
    }
    for changed in 0..4 {
        let mut next = input(12, 1);
        match changed {
            0 => next.failed_attempts = 0,
            1 => next.max_attempts = 4,
            2 => next.binding.browser = BindingDigest::new([99; 32]),
            _ => next.binding.epoch = SecurityEpoch::new(99),
        }
        assert!(
            r.issue(&mut tx, OtpState::try_from(next).unwrap(), Some(id(11)))
                .await
                .is_err()
        );
        assert_eq!(r.lock(&mut tx, id(12)).await.unwrap(), None);
    }
    r.issue(
        &mut tx,
        OtpState::try_from(input(12, 1)).unwrap(),
        Some(id(11)),
    )
    .await
    .unwrap();
    assert!(
        r.consume_once(&mut tx, check(11, 1), OtpDigest::new([11; 32]))
            .await
            .is_err()
    );
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    r.issue(
        &mut tx,
        OtpState::try_from(input(12, 1)).unwrap(),
        Some(id(11)),
    )
    .await
    .unwrap();
    assert!(
        r.issue(
            &mut tx,
            OtpState::try_from(input(13, 1)).unwrap(),
            Some(id(11))
        )
        .await
        .is_err()
    );
    r.record_failure(&mut tx, check(12, 1)).await.unwrap();
    r.record_submission(&mut tx, id(12), SubmissionCategory::Unknown)
        .await
        .unwrap();
    r.record_submission(&mut tx, id(12), SubmissionCategory::Accepted)
        .await
        .unwrap();
    assert_eq!(
        r.lock(&mut tx, id(12))
            .await
            .unwrap()
            .unwrap()
            .failed_attempts(),
        2
    );
    r.issue(
        &mut tx,
        OtpState::try_from(input(13, 2)).unwrap(),
        Some(id(12)),
    )
    .await
    .unwrap();
    r.record_failure(&mut tx, check(13, 2)).await.unwrap();
    assert!(r.record_failure(&mut tx, check(13, 3)).await.is_err());
    assert!(
        r.consume_once(&mut tx, check(13, 3), OtpDigest::new([13; 32]))
            .await
            .is_err()
    );
    assert_eq!(
        r.lock(&mut tx, id(13)).await.unwrap().unwrap().expires_at(),
        now() + Duration::MINUTE
    );
    h.uow().commit(tx).await.unwrap();
}

async fn verify_consumption_race<H: OtpChallengeRepositoryContractHarness>(h: &H) {
    let r = h.repository();
    let mut next = input(21, 0);
    next.binding.browser = BindingDigest::new([21; 32]);
    let mut proof = check(21, 0);
    proof.binding = next.binding.clone();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    r.issue(&mut tx, OtpState::try_from(next).unwrap(), None)
        .await
        .unwrap();
    h.uow().commit(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    r.consume_once(&mut tx, proof.clone(), OtpDigest::new([21; 32]))
        .await
        .unwrap();
    h.uow().rollback(tx).await.unwrap();
    let attempt = || async {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        let result = r
            .consume_once(&mut tx, proof.clone(), OtpDigest::new([21; 32]))
            .await;
        h.uow().commit(tx).await.unwrap();
        result
    };
    let (first, second) = tokio::join!(attempt(), attempt());
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert!(
        r.consume_once(&mut tx, proof.clone(), OtpDigest::new([21; 32]))
            .await
            .is_err()
    );
    assert!(r.record_failure(&mut tx, proof).await.is_err());
    h.uow().rollback(tx).await.unwrap();
}

async fn verify_handles<H: OtpChallengeRepositoryContractHarness>(h: &H) {
    let r = h.repository();
    let mut tx = h.begin_foreign().await.unwrap();
    assert_eq!(
        r.lock(&mut tx, id(11)).await.unwrap_err().code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        r.issue(&mut tx, OtpState::try_from(input(99, 0)).unwrap(), None)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    drop(tx);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    r.lock(&mut tx, id(11)).await.unwrap();
    r.submission(&mut tx, id(11)).await.unwrap();
    assert_eq!(
        r.record_failure(&mut tx, check(11, 0))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        r.consume_once(&mut tx, check(11, 0), OtpDigest::new([11; 32]))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        r.issue(&mut tx, OtpState::try_from(input(99, 0)).unwrap(), None)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    assert_eq!(
        r.record_submission(&mut tx, id(11), SubmissionCategory::Unknown)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    h.uow().rollback(tx).await.unwrap();
}
