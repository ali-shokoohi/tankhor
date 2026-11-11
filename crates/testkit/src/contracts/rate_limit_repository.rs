//! FR-002, FR-CON-02, AC-ADM-002, Q09: unchanged fake/PostgreSQL budget contract.

use admin::{
    domain::security_policy::{RateLimit, RateLimitInput, SecurityPolicy, SecurityPolicyInput},
    ports::rate_limit_repository::{
        RateLimitKey, RateLimitPurpose, RateLimitRepository, RateLimitScope,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
    },
};
use time::{Duration, OffsetDateTime};

#[async_trait::async_trait]
pub trait RateLimitRepositoryContractHarness: Sync {
    type Uow: UnitOfWork;
    type Repository: RateLimitRepository;
    type Clock: Clock;
    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;
    fn clock(&self) -> &Self::Clock;
    fn set_time(&self, now: OffsetDateTime);
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn limit(attempts: i64) -> RateLimit {
    SecurityPolicy::try_from(SecurityPolicyInput {
        otp_ttl_seconds: Some(60),
        primary_proof_ttl_seconds: Some(60),
        ceremony_ttl_seconds: Some(60),
        enrollment_ttl_seconds: Some(60),
        session_idle_ttl_seconds: Some(60),
        session_absolute_ttl_seconds: Some(120),
        otp_max_attempts: Some(3),
        subject_rate_limit: Some(RateLimitInput {
            max_attempts: Some(attempts),
            window_seconds: Some(60),
        }),
        origin_rate_limit: Some(RateLimitInput {
            max_attempts: Some(attempts),
            window_seconds: Some(60),
        }),
        rp_id: Some("console.example.test".into()),
        origin: Some("https://console.example.test".into()),
    })
    .unwrap()
    .subject_rate_limit()
}
fn key(n: u8) -> RateLimitKey {
    RateLimitKey::new(RateLimitScope::Subject, RateLimitPurpose::Send, [n; 32])
}

/// # Panics
/// Panics on lost increments, reset budgets, invalid windows or transaction leakage.
#[allow(clippy::too_many_lines)]
#[allow(clippy::many_single_char_names)]
pub async fn rate_limit_repository_contract<H: RateLimitRepositoryContractHarness>(h: &H) {
    let start = OffsetDateTime::UNIX_EPOCH;
    h.set_time(start);
    let now = h.clock().now().await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(
        h.repository().read(&mut tx, key(1), now).await.unwrap(),
        None
    );
    let first = h
        .repository()
        .increment(&mut tx, key(1), limit(2), now)
        .await
        .unwrap();
    assert_eq!(first.count, 1);
    assert!(first.allowed);
    assert_eq!(first.started_at, start);
    assert_eq!(first.expires_at, start + Duration::MINUTE);
    assert_eq!(first.blocked_until, None);
    h.uow().commit(tx).await.unwrap();

    // Resend and fallback use the same key, never a provider or challenge identity.
    h.set_time(start + Duration::seconds(30));
    let now = h.clock().now().await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    let resend = h
        .repository()
        .increment(&mut tx, key(1), limit(2), now)
        .await
        .unwrap();
    assert_eq!(resend.count, 2);
    assert!(resend.allowed);
    let fallback = h
        .repository()
        .increment(&mut tx, key(1), limit(2), now)
        .await
        .unwrap();
    assert_eq!(fallback.count, 3);
    assert!(!fallback.allowed);
    assert_eq!(fallback.expires_at, first.expires_at);
    assert_eq!(fallback.blocked_until, Some(first.expires_at));
    assert_eq!(
        h.repository()
            .increment(&mut tx, key(1), limit(3), now)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidValue
    );
    assert_eq!(
        h.repository().read(&mut tx, key(1), now).await.unwrap(),
        Some(fallback)
    );
    assert_eq!(
        h.repository()
            .increment(&mut tx, key(1), limit(2), start - Duration::SECOND)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidValue
    );
    assert_eq!(
        h.repository()
            .read(&mut tx, key(1), start - Duration::SECOND)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidValue
    );
    for isolated in [
        key(2),
        RateLimitKey::new(RateLimitScope::Origin, RateLimitPurpose::Send, [1; 32]),
        RateLimitKey::new(RateLimitScope::Subject, RateLimitPurpose::Verify, [1; 32]),
    ] {
        assert_eq!(
            h.repository()
                .increment(&mut tx, isolated, limit(2), now)
                .await
                .unwrap()
                .count,
            1
        );
    }
    h.uow().commit(tx).await.unwrap();

    h.set_time(first.expires_at - Duration::NANOSECOND);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        h.repository()
            .read(&mut tx, key(1), h.clock().now().await.unwrap())
            .await
            .unwrap(),
        Some(fallback)
    );
    assert_eq!(
        h.repository()
            .increment(&mut tx, key(1), limit(2), now)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ReadOnlyTransaction
    );
    h.uow().rollback(tx).await.unwrap();
    h.set_time(first.expires_at);
    let boundary = h.clock().now().await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    assert_eq!(
        h.repository()
            .read(&mut tx, key(1), boundary)
            .await
            .unwrap(),
        None
    );
    let renewed = h
        .repository()
        .increment(&mut tx, key(1), limit(2), boundary)
        .await
        .unwrap();
    assert_eq!(renewed.count, 1);
    assert!(renewed.allowed);
    assert_eq!(renewed.started_at, boundary);
    assert_eq!(renewed.expires_at, boundary + Duration::MINUTE);
    h.uow().rollback(tx).await.unwrap();
    h.set_time(now);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        h.repository().read(&mut tx, key(1), now).await.unwrap(),
        Some(fallback)
    );
    h.uow().rollback(tx).await.unwrap();

    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.repository()
        .increment(&mut tx, key(3), limit(2), now)
        .await
        .unwrap();
    drop(tx);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        h.repository().read(&mut tx, key(3), now).await.unwrap(),
        None
    );
    h.uow().rollback(tx).await.unwrap();
    let mut foreign = h.begin_foreign().await.unwrap();
    assert_eq!(
        h.repository()
            .read(&mut foreign, key(1), now)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    assert_eq!(
        h.repository()
            .increment(&mut foreign, key(1), limit(2), now)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::ForeignTransactionHandle
    );
    drop(foreign);

    // Independent callers share one atomic bucket; no lost increments or extra allowances.
    let increment = || async {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        tokio::task::yield_now().await;
        let result = h
            .repository()
            .increment(&mut tx, key(4), limit(2), now)
            .await
            .unwrap();
        h.uow().commit(tx).await.unwrap();
        result
    };
    let (a, b, c, d, e, f, g, i) = tokio::join!(
        increment(),
        increment(),
        increment(),
        increment(),
        increment(),
        increment(),
        increment(),
        increment()
    );
    let results = [a, b, c, d, e, f, g, i];
    assert_eq!(results.iter().filter(|result| result.allowed).count(), 2);
    let mut counts: Vec<_> = results.iter().map(|result| result.count).collect();
    counts.sort_unstable();
    assert_eq!(counts, (1..=8).collect::<Vec<_>>());
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    let final_status = h
        .repository()
        .read(&mut tx, key(4), now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_status.count, 8);
    assert!(!final_status.allowed);
    h.uow().rollback(tx).await.unwrap();
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    let end = time::macros::datetime!(9999-12-31 23:59:59 UTC);
    assert_eq!(
        h.repository()
            .increment(&mut tx, key(5), limit(2), end)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidValue
    );
    assert_eq!(
        h.repository().read(&mut tx, key(5), now).await.unwrap(),
        None
    );
    h.uow().rollback(tx).await.unwrap();
}
