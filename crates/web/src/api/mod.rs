//! Versioned JSON API (`/api/v1`) and `OpenAPI` router composition.
//!
//! All JSON routes are registered under `/api/v1` via [`utoipa_axum::router::OpenApiRouter`];
//! HTML console and authentication routes are never exposed as JSON endpoints.

pub mod docs;
pub mod health;
pub mod openapi;
pub mod problem;

use std::sync::Arc;

use admin::ports::health_probe::HealthProbe;
use axum::Router;
use utoipa::OpenApi;
use utoipa::openapi::OpenApi as OpenApiSpec;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::api::health::{HealthApiState, HealthResponse};
use crate::api::openapi::{OpenApiDocument, OpenApiInfoSummary};
use crate::api::problem::ProblemDetails;

/// Base `OpenAPI` metadata and shared component schemas for Tankhor `/api/v1`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Tankhor API",
        version = "0.1.0",
        description = "Versioned JSON API endpoints for Tankhor.",
        license(name = "LicenseRef-Proprietary")
    ),
    components(schemas(HealthResponse, ProblemDetails, OpenApiInfoSummary, OpenApiDocument)),
    tags(
        (name = "system", description = "Foundational readiness and OpenAPI schema endpoints")
    )
)]
pub struct ApiDoc;

/// Builds the `/api/v1/health`-only router and splits its generated `OpenAPI` specification once.
pub fn health_api_router(probe: Arc<dyn HealthProbe>) -> (Router, OpenApiSpec) {
    let (router, doc) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::get_health_handler))
        .split_for_parts();
    (router.with_state(HealthApiState::new(probe)), doc)
}
