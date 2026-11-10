//! Real UTC system clock adapter implementing [`Clock`].

use kernel::error::DomainError;
use kernel::ports::clock::Clock;
use time::OffsetDateTime;

/// Real operating-system UTC clock adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl SystemClock {
    /// Construct a new [`SystemClock`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Clock for SystemClock {
    async fn now(&self) -> Result<OffsetDateTime, DomainError> {
        Ok(OffsetDateTime::now_utc())
    }
}
