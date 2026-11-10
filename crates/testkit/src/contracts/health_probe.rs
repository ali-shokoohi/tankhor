//! Shared generic contract suite for [`admin::ports::health_probe::HealthProbe`].

use admin::ports::health_probe::{HealthProbe, ProbeReadiness};
use kernel::error::{ErrorCategory, ErrorCode};

/// Fixture constructors exercising unseeded, seeded, and unavailable storage states
/// against a concrete [`HealthProbe`] implementation.
pub struct HealthProbeContractHarness<P> {
    pub unseeded_probe: P,
    pub seeded_probe: P,
    pub unavailable_probe: P,
}

/// Asserts that any [`HealthProbe`] implementation distinguishes unseeded storage,
/// explicitly seeded readiness, and unavailable storage using sanitized domain errors.
///
/// # Panics
///
/// Panics if readiness transitions or error classifications violate the port contract.
pub async fn health_probe_contract<P: HealthProbe>(
    probe: &P,
    harness: &HealthProbeContractHarness<P>,
) {
    let default_state = probe.check().await.expect("default reachable check");
    assert_eq!(default_state, ProbeReadiness::Unseeded);

    let unseeded = harness
        .unseeded_probe
        .check()
        .await
        .expect("unseeded check");
    assert_eq!(unseeded, ProbeReadiness::Unseeded);

    let ready = harness.seeded_probe.check().await.expect("seeded check");
    assert_eq!(ready, ProbeReadiness::Ready);

    let err = harness
        .unavailable_probe
        .check()
        .await
        .expect_err("unavailable probe must fail closed");
    assert_eq!(err.category(), ErrorCategory::Unavailable);
    assert_eq!(err.code(), ErrorCode::PrerequisiteUnavailable);
    let rendered = format!("{err} {err:?}");
    assert!(!rendered.contains("postgres://"));
    assert!(!rendered.contains("password"));
}
