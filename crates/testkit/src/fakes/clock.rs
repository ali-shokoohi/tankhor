//! Deterministic controllable UTC clock fake for tests and dev-fakes.

use std::sync::Mutex;

use kernel::error::{DomainError, ErrorCode};
use kernel::ports::clock::Clock;
use time::{Duration, OffsetDateTime, UtcOffset};

/// Deterministic in-memory [`Clock`] with explicit [`set`](Self::set) and [`advance`](Self::advance) controls.
#[derive(Debug)]
pub struct FixedClock {
    state: Mutex<Result<OffsetDateTime, DomainError>>,
}

impl FixedClock {
    /// Construct a healthy [`FixedClock`] normalized to UTC.
    #[must_use]
    pub fn new(initial: OffsetDateTime) -> Self {
        Self {
            state: Mutex::new(Ok(initial.to_offset(UtcOffset::UTC))),
        }
    }

    /// Construct an unavailable [`FixedClock`] that returns a sanitized [`DomainError`].
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            state: Mutex::new(Err(DomainError::unavailable(ErrorCode::SourceExhausted))),
        }
    }

    /// Replace the current instant, normalizing any offset to UTC.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set(&self, instant: OffsetDateTime) {
        *self.state.lock().expect("FixedClock mutex poisoned") =
            Ok(instant.to_offset(UtcOffset::UTC));
    }

    /// Advance (or rewind) the current instant by `delta`.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned or the clock was configured unavailable.
    pub fn advance(&self, delta: Duration) {
        let mut guard = self.state.lock().expect("FixedClock mutex poisoned");
        let current = guard.expect("cannot advance an unavailable FixedClock");
        *guard = Ok(current + delta);
    }
}

#[async_trait::async_trait]
impl Clock for FixedClock {
    async fn now(&self) -> Result<OffsetDateTime, DomainError> {
        *self.state.lock().expect("FixedClock mutex poisoned")
    }
}
