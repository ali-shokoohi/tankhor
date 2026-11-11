//! Application-wide adapter composition for delivered foundational services.
//!
//! Resolves [`AdapterMode`] (`real` or `fake`, never mixed) strictly after
//! [`AppConfig`](crate::config::AppConfig) has rejected `TANKHOR_ADAPTERS=fake` in `production`.

use std::sync::Arc;

use admin::ports::health_probe::HealthProbe;

use crate::config::{AdapterMode, AppConfig};

/// Errors returned when constructing runtime service adapters for `serve`.
#[derive(Debug, thiserror::Error)]
pub enum CompositionError {
    /// `TANKHOR_ADAPTERS=fake` was requested in a build compiled without `dev-fakes`.
    #[error(
        "TANKHOR_ADAPTERS=fake requires debug feature `dev-fakes` and is unavailable in default/release builds"
    )]
    FakeAdaptersNotCompiled,
    /// `TANKHOR_ADAPTERS=real` was requested before real `HealthProbe` adapter delivery (T356).
    #[error(
        "TANKHOR_ADAPTERS=real health probe and database seeding are not yet delivered in Phase 2"
    )]
    RealHealthProbeNotDelivered,
}

/// Assembled foundational runtime services ready for HTTP serving.
pub struct FoundationComposition {
    health_probe: Arc<dyn HealthProbe>,
}

impl FoundationComposition {
    /// Shared [`HealthProbe`] wired for `/api/v1/health`.
    #[must_use]
    pub fn health_probe(&self) -> Arc<dyn HealthProbe> {
        Arc::clone(&self.health_probe)
    }
}

/// Constructs foundational adapters according to the single application-wide [`AdapterMode`].
///
/// In `dev-fakes` builds with `TANKHOR_ADAPTERS=fake`, wires an unseeded in-memory
/// `FakeHealthProbe` (which honestly reports `ProbeReadiness::Unseeded` -> HTTP 503 until
/// seeded, never fabricating readiness from an absent backend).
///
/// # Errors
///
/// Returns [`CompositionError`] if fake adapters were not compiled in or if real storage
/// readiness is requested before its Phase 4 adapter task.
pub fn compose_foundation(config: &AppConfig) -> Result<FoundationComposition, CompositionError> {
    match config.adapter_mode() {
        AdapterMode::Fake => compose_fake_foundation(),
        AdapterMode::Real => Err(CompositionError::RealHealthProbeNotDelivered),
    }
}

#[cfg(feature = "dev-fakes")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Fake-enabled and fake-disabled builds share the same fallible composition signature"
)]
fn compose_fake_foundation() -> Result<FoundationComposition, CompositionError> {
    use testkit::fakes::health_probe::FakeHealthProbe;
    use testkit::fakes::store::FakeStore;

    let store = FakeStore::new();
    let health_probe: Arc<dyn HealthProbe> = Arc::new(FakeHealthProbe::new(store));
    Ok(FoundationComposition { health_probe })
}

#[cfg(not(feature = "dev-fakes"))]
fn compose_fake_foundation() -> Result<FoundationComposition, CompositionError> {
    Err(CompositionError::FakeAdaptersNotCompiled)
}
