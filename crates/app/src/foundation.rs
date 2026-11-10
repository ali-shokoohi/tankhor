//! Foundational router assembly keeping undelivered management and authentication routes unregistered.
//!
//! Wires only delivered Phase 2 surfaces:
//! - Bilingual console shell and vendored local assets (`/console`, `/static/...`)
//! - Versioned `/api/v1/health` and `/api/v1/openapi.json`
//! - Vendored `/api/docs` Swagger UI
//! - Strict CSP, bounded body limit, bounded concurrency limit, and CSRF enforcement
//!
//! Story authentication/management routes (`/console/access`, `/console/settings`,
//! `/console/flags`, `/console/audit`, etc.) remain explicitly unregistered (returning 404)
//! until their corresponding Phase 3–6 tasks deliver them.

use std::sync::Arc;

use admin::ports::health_probe::HealthProbe;
use axum::Router;
use axum::middleware;
use kernel::locale::Locale;
use web::api::docs::build_api_and_docs_router;
use web::api::openapi::SchemaAccessPolicy;
use web::i18n::{ConsoleCatalog, I18nError};
use web::middleware::body_limit::{BodyLimitState, enforce_body_limit_middleware};
use web::middleware::concurrency_limit::{
    ConcurrencyLimitState, enforce_concurrency_limit_middleware,
};
use web::middleware::csp::apply_csp_middleware;
use web::security::{CsrfProtectionState, SecurityPolicyError, enforce_csrf_middleware};
use web::shell::shell_router;

use crate::config::RuntimeEnvironment;
use crate::transport_policy::TransportPolicy;

/// Errors returned while constructing the foundational HTTP router.
#[derive(Debug, thiserror::Error)]
pub enum FoundationError {
    /// Fluent catalog initialization failed.
    #[error("failed to load Fluent console catalog: {0}")]
    Catalog(#[from] I18nError),
    /// Origin/CSRF security policy validation failed.
    #[error("invalid console origin security policy: {0}")]
    SecurityPolicy(#[from] SecurityPolicyError),
}

/// Assembles the delivered foundational Axum router without registering any unbuilt
/// authentication or management routes.
///
/// # Errors
///
/// Returns [`FoundationError`] if the Fluent catalog or origin policy fails validation.
pub fn build_foundation_router(
    environment: RuntimeEnvironment,
    transport_policy: TransportPolicy,
    expected_origin: &str,
    health_probe: Arc<dyn HealthProbe>,
) -> Result<Router, FoundationError> {
    let csrf_catalog = ConsoleCatalog::load_authoritative()?;
    let shell_catalog = ConsoleCatalog::load_authoritative()?;
    let shared_catalog = Arc::new(ConsoleCatalog::load_authoritative()?);
    let schema_policy = match environment {
        RuntimeEnvironment::Development | RuntimeEnvironment::Staging => {
            SchemaAccessPolicy::NonProductionOpen
        }
        RuntimeEnvironment::Production => SchemaAccessPolicy::ProductionAdminRequired,
    };

    let csrf_state = CsrfProtectionState::new(expected_origin, csrf_catalog, Locale::Fa)?;
    let console = shell_router(shell_catalog).layer(middleware::from_fn_with_state(
        csrf_state,
        enforce_csrf_middleware,
    ));

    let (api_and_docs, _doc) = build_api_and_docs_router(health_probe, schema_policy);

    let body_limit_state = BodyLimitState::new(
        transport_policy.max_body_bytes(),
        Arc::clone(&shared_catalog),
        Locale::Fa,
    );
    let concurrency_state =
        ConcurrencyLimitState::new(transport_policy.max_inflight(), shared_catalog, Locale::Fa);

    let router = console
        .merge(api_and_docs)
        .layer(middleware::from_fn_with_state(
            body_limit_state,
            enforce_body_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            concurrency_state,
            enforce_concurrency_limit_middleware,
        ))
        .layer(middleware::from_fn(apply_csp_middleware));

    Ok(router)
}
