//! Sanitized store-and-seed readiness port.

use kernel::error::DomainError;

/// Sanitized readiness status returned by [`HealthProbe`].
///
/// Contains zero connection strings, hostnames, SQL errors, or provider identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeReadiness {
    /// Required schema and foundational registry seed are present and reachable.
    Ready,
    /// Storage is reachable, but foundational registry seed has not been applied.
    Unseeded,
}

/// Outbound port probing required store migration and foundational seed readiness.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait HealthProbe: Send + Sync {
    /// Check sanitized readiness of required storage and foundational seed state.
    async fn check(&self) -> Result<ProbeReadiness, DomainError>;
}

#[cfg(test)]
mod tests {
    use kernel::error::{DomainError, ErrorCode};

    use super::{HealthProbe, MockHealthProbe, ProbeReadiness};

    #[tokio::test]
    async fn health_probe_is_an_async_mockable_port() {
        let mut mock = MockHealthProbe::new();
        mock.expect_check()
            .times(1)
            .returning(|| Ok(ProbeReadiness::Ready));

        let probe: &dyn HealthProbe = &mock;
        let ready = probe.check().await.expect("ready check");
        assert_eq!(ready, ProbeReadiness::Ready);
        assert!(!format!("{ready:?}").contains("postgres://"));

        let mut unseeded_mock = MockHealthProbe::new();
        unseeded_mock
            .expect_check()
            .times(1)
            .returning(|| Ok(ProbeReadiness::Unseeded));
        assert_eq!(
            unseeded_mock.check().await.unwrap(),
            ProbeReadiness::Unseeded
        );

        let mut failed_mock = MockHealthProbe::new();
        failed_mock
            .expect_check()
            .times(1)
            .returning(|| Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)));
        let err = failed_mock.check().await.unwrap_err();
        assert_eq!(err.code(), ErrorCode::PrerequisiteUnavailable);
    }
}
