//! FR-002/Q09: serialized, transaction-local abuse budgets with no transport identity.

use super::store::FakeStore;
use admin::{
    domain::security_policy::RateLimit,
    ports::rate_limit_repository::{RateLimitKey, RateLimitRepository, RateLimitStatus},
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::unit_of_work::TxHandle,
};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy)]
pub struct RateLimitBucket {
    limit: RateLimit,
    status: RateLimitStatus,
}

#[derive(Debug, Clone)]
pub struct FakeRateLimitRepository {
    store: FakeStore,
}

impl FakeRateLimitRepository {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }
}

fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

#[async_trait::async_trait]
impl RateLimitRepository for FakeRateLimitRepository {
    async fn increment(
        &self,
        tx: &mut TxHandle,
        key: RateLimitKey,
        limit: RateLimit,
        now: OffsetDateTime,
    ) -> Result<RateLimitStatus, DomainError> {
        self.store.with_write(tx, |state| {
            let previous = state.rate_limits.get(&key);
            if previous.is_some_and(|bucket| now < bucket.status.started_at) {
                return Err(invalid());
            }
            let mut next =
                if let Some(bucket) = previous.filter(|bucket| now < bucket.status.expires_at) {
                    if bucket.limit != limit {
                        return Err(invalid());
                    }
                    *bucket
                } else {
                    let window = Duration::try_from(limit.window()).map_err(|_| invalid())?;
                    let expires_at = now.checked_add(window).ok_or_else(invalid)?;
                    RateLimitBucket {
                        limit,
                        status: RateLimitStatus {
                            count: 0,
                            started_at: now,
                            expires_at,
                            blocked_until: None,
                            allowed: true,
                        },
                    }
                };
            next.status.count = next.status.count.checked_add(1).ok_or_else(invalid)?;
            next.status.allowed = next.status.count <= next.limit.max_attempts().get();
            next.status.blocked_until = (!next.status.allowed).then_some(next.status.expires_at);
            state.rate_limits.insert(key, next);
            Ok(next.status)
        })
    }
    async fn read(
        &self,
        tx: &mut TxHandle,
        key: RateLimitKey,
        now: OffsetDateTime,
    ) -> Result<Option<RateLimitStatus>, DomainError> {
        self.store.with_read(tx, |state| {
            let Some(bucket) = state.rate_limits.get(&key) else {
                return Ok(None);
            };
            if now < bucket.status.started_at {
                return Err(invalid());
            }
            Ok((now < bucket.status.expires_at).then_some(bucket.status))
        })
    }
}
