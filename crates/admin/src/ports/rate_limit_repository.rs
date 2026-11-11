//! FR-002, FR-CON-02, Q09: transactional keyed abuse budgets, independent of SMS transport.

use crate::domain::security_policy::RateLimit;
use kernel::{error::DomainError, ports::unit_of_work::TxHandle};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RateLimitScope {
    Subject,
    Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RateLimitPurpose {
    Send,
    Verify,
}

/// Purpose-separated keyed digest; never a plaintext phone number or IP address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RateLimitKey {
    scope: RateLimitScope,
    purpose: RateLimitPurpose,
    digest: [u8; 32],
}

impl RateLimitKey {
    #[must_use]
    pub const fn new(scope: RateLimitScope, purpose: RateLimitPurpose, digest: [u8; 32]) -> Self {
        Self {
            scope,
            purpose,
            digest,
        }
    }
    #[must_use]
    pub const fn scope(self) -> RateLimitScope {
        self.scope
    }
    #[must_use]
    pub const fn purpose(self) -> RateLimitPurpose {
        self.purpose
    }
    #[must_use]
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

impl std::fmt::Debug for RateLimitKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimitKey")
            .field("scope", &self.scope)
            .field("purpose", &self.purpose)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitStatus {
    pub count: u64,
    pub started_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub blocked_until: Option<OffsetDateTime>,
    pub allowed: bool,
}

/// Increment and read under the supplied `UoW`. The first increment opens a finite
/// window; increments before expiry retain its original end and policy. Exactly
/// `max_attempts` increments are allowed, later increments are counted but blocked
/// until that end. At expiry a new window starts. Reads never create/reset state.
/// Reject backwards time/policy changes within a window and arithmetic overflow
/// without mutation. Provider switches, resends and challenge IDs are not keys.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait RateLimitRepository: Send + Sync {
    async fn increment(
        &self,
        tx: &mut TxHandle,
        key: RateLimitKey,
        limit: RateLimit,
        now: OffsetDateTime,
    ) -> Result<RateLimitStatus, DomainError>;
    async fn read(
        &self,
        tx: &mut TxHandle,
        key: RateLimitKey,
        now: OffsetDateTime,
    ) -> Result<Option<RateLimitStatus>, DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput};
    use kernel::ports::unit_of_work::{StoreId, TransactionMode, TxHandle};
    use time::OffsetDateTime;

    pub(crate) fn policy() -> crate::domain::security_policy::RateLimit {
        SecurityPolicy::try_from(SecurityPolicyInput {
            otp_ttl_seconds: Some(60),
            primary_proof_ttl_seconds: Some(60),
            ceremony_ttl_seconds: Some(60),
            enrollment_ttl_seconds: Some(60),
            session_idle_ttl_seconds: Some(60),
            session_absolute_ttl_seconds: Some(120),
            otp_max_attempts: Some(3),
            subject_rate_limit: Some(RateLimitInput {
                max_attempts: Some(2),
                window_seconds: Some(60),
            }),
            origin_rate_limit: Some(RateLimitInput {
                max_attempts: Some(3),
                window_seconds: Some(60),
            }),
            rp_id: Some("console.example.test".into()),
            origin: Some("https://console.example.test".into()),
        })
        .unwrap()
        .subject_rate_limit()
    }

    #[tokio::test]
    async fn rate_limit_repository_port_is_mockable() {
        let key = RateLimitKey::new(RateLimitScope::Subject, RateLimitPurpose::Send, [42; 32]);
        let origin = RateLimitKey::new(RateLimitScope::Origin, RateLimitPurpose::Verify, [42; 32]);
        assert_ne!(key, origin);
        assert!(!format!("{key:?}").contains("42"));
        let now = OffsetDateTime::UNIX_EPOCH;
        let status = RateLimitStatus {
            count: 1,
            started_at: now,
            expires_at: now + time::Duration::MINUTE,
            blocked_until: None,
            allowed: true,
        };
        let mut mock = MockRateLimitRepository::new();
        mock.expect_increment()
            .withf(move |_, actual, _, at| *actual == key && *at == now)
            .times(1)
            .returning(move |_, _, _, _| Ok(status));
        mock.expect_read()
            .withf(move |_, actual, at| *actual == key && *at == now)
            .times(1)
            .returning(move |_, _, _| Ok(Some(status)));
        let port: &dyn RateLimitRepository = &mock;
        let mut tx = TxHandle::new(StoreId::new(), TransactionMode::ReadWrite, ());
        assert_eq!(
            port.increment(&mut tx, key, policy(), now).await.unwrap(),
            status
        );
        assert_eq!(port.read(&mut tx, key, now).await.unwrap(), Some(status));
    }
}
