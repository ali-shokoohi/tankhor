//! Vendored Swagger UI (`/api/docs`) pointing at `/api/v1/openapi.json` with external
//! validation disabled and zero extra undocumented schema endpoints.

use std::sync::Arc;

use admin::ports::health_probe::HealthProbe;
use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use utoipa::openapi::OpenApi as OpenApiSpec;
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::api::openapi::{OPENAPI_JSON_PATH, SchemaAccessPolicy, build_api_router};
use crate::api::problem::{ProblemDetails, ProblemResponse};

/// Canonical Swagger UI path prefix.
pub const SWAGGER_DOCS_PATH: &str = "/api/docs";

/// Builds the vendored `/api/docs` router configured to fetch `/api/v1/openapi.json`
/// locally with `validatorUrl` set to `"none"`.
///
/// No `.url(...)` registration is added on [`SwaggerUi`], preventing any second
/// undocumented `OpenAPI` endpoint from being exposed.
pub fn swagger_docs_router(schema_policy: SchemaAccessPolicy) -> Router {
    let config = Config::new([OPENAPI_JSON_PATH])
        .validator_url("none")
        .query_config_enabled(false);
    let router = Router::from(SwaggerUi::new(SWAGGER_DOCS_PATH).config(config));

    match schema_policy {
        SchemaAccessPolicy::NonProductionOpen => router,
        SchemaAccessPolicy::ProductionAdminRequired => router.layer(middleware::from_fn(
            |request: Request<Body>, _next: Next| async move {
                let path = request.uri().path().to_owned();
                let (status, details) = ProblemDetails::unauthorized(path);
                ProblemResponse::new(status, details).into_response()
            },
        )),
    }
}

/// Composes the `/api/v1` router and the vendored `/api/docs` Swagger UI router.
pub fn build_api_and_docs_router(
    probe: Arc<dyn HealthProbe>,
    schema_policy: SchemaAccessPolicy,
) -> (Router, OpenApiSpec) {
    let (api_router, doc) = build_api_router(probe, schema_policy);
    let combined = api_router.merge(swagger_docs_router(schema_policy));
    (combined, doc)
}
