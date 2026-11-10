//! Foundational readiness query service.

use kernel::error::{DomainError, ErrorCode};

use crate::ports::health_probe::{HealthProbe, ProbeReadiness};

/// Sanitized application health outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthReport {
    /// Required schema and foundational seed are verified ready.
    Ready,
}

/// Checks required storage and foundational seed readiness once, failing closed
/// if storage is unseeded or unreachable.
///
/// # Errors
///
/// Returns sanitized [`DomainError`] (`Unavailable` / `PrerequisiteUnavailable`)
/// when the probe reports [`ProbeReadiness::Unseeded`] or returns an error.
pub async fn get_health(probe: &dyn HealthProbe) -> Result<HealthReport, DomainError> {
    match probe.check().await? {
        ProbeReadiness::Ready => Ok(HealthReport::Ready),
        ProbeReadiness::Unseeded => {
            Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
        }
    }
}
