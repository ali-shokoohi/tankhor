//! Deterministic in-memory [`HealthProbe`] backed by [`FakeStore`].

use admin::ports::health_probe::{HealthProbe, ProbeReadiness};
use kernel::error::{DomainError, ErrorCode};

use crate::fakes::store::FakeStore;

/// In-memory [`HealthProbe`] that inspects committed [`FakeStore`] seed state
/// or simulates an explicitly unavailable backend.
#[derive(Debug, Clone)]
pub struct FakeHealthProbe {
    store: Option<FakeStore>,
}

impl FakeHealthProbe {
    /// Creates a probe bound to an explicit [`FakeStore`].
    ///
    /// A freshly constructed store has `health_ready = false`, so this probe
    /// reports [`ProbeReadiness::Unseeded`] until explicitly seeded.
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store: Some(store) }
    }

    /// Creates a probe representing an unreachable or unconfigured backend.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self { store: None }
    }
}

#[async_trait::async_trait]
impl HealthProbe for FakeHealthProbe {
    async fn check(&self) -> Result<ProbeReadiness, DomainError> {
        let Some(store) = &self.store else {
            return Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable));
        };
        let state = store
            .try_read_committed()
            .ok_or_else(|| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))?;
        if state.health_ready {
            Ok(ProbeReadiness::Ready)
        } else {
            Ok(ProbeReadiness::Unseeded)
        }
    }
}
