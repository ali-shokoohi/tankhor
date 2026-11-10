//! Asynchronous UTC clock port.

use time::OffsetDateTime;

use crate::error::DomainError;

/// Asynchronous source of UTC instants supplied by the composition root or test fixture.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait Clock: Send + Sync {
    /// Return the current UTC instant, or a sanitized [`DomainError`] if unavailable.
    async fn now(&self) -> Result<OffsetDateTime, DomainError>;
}
