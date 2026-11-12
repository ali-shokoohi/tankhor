//! Frozen `OpenAPI` 3.1 document bridge and `/api/v1/openapi.json` router builder.
//!
//! Starts from [`ApiDoc`], registers all `/api/v1` handlers once via [`OpenApiRouter`],
//! splits the router and generated specification once, freezes the JSON payload, and
//! serves it without recursive literal document embedding.

use std::sync::Arc;

use admin::ports::health_probe::HealthProbe;
use axum::Router;
use axum::extract::{FromRef, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::OpenApi;
use utoipa::ToSchema;
use utoipa::openapi::OpenApi as OpenApiSpec;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::api::ApiDoc;
use crate::api::health::{self, HealthApiState};
use crate::api::problem::{ProblemDetails, ProblemResponse};
use crate::console::ConsoleServices;
use crate::console::guard::{AdminGuard, require_admin};
use crate::security::CookieSecurityPolicy;

/// Canonical `/api/v1/openapi.json` URI path.
pub const OPENAPI_JSON_PATH: &str = "/api/v1/openapi.json";

/// Environment-driven access policy for `/api/v1/openapi.json` and `/api/docs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaAccessPolicy {
    /// Explicit non-production policy permitting local inspection in development and staging.
    NonProductionOpen,
    /// Production policy requiring a verified administrator session (fails closed until US1).
    ProductionAdminRequired,
}

/// Finite non-recursive schema bridge describing the top-level `OpenAPI` 3.1 document shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OpenApiInfoSummary {
    /// Human-readable API title.
    pub title: String,
    /// Semantic API specification version.
    pub version: String,
    /// Optional human-readable API description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Finite top-level schema representation for the `OpenAPI` 3.1 endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OpenApiDocument {
    /// `OpenAPI` specification version string (e.g. `"3.1.0"`).
    pub openapi: String,
    /// Top-level API metadata block.
    pub info: OpenApiInfoSummary,
    /// Map of documented versioned path items.
    #[schema(value_type = Object)]
    pub paths: Value,
    /// Shared component schemas and security schemes.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub components: Option<Value>,
    /// Tag metadata entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option< Vec<Object> >)]
    pub tags: Option<Value>,
}

/// Shared state attached to the split `/api/v1` router.
#[derive(Clone)]
pub struct ApiRouterState {
    health: HealthApiState,
    frozen_openapi_json: Arc<String>,
    schema_policy: SchemaAccessPolicy,
}

impl FromRef<ApiRouterState> for HealthApiState {
    fn from_ref(input: &ApiRouterState) -> Self {
        input.health.clone()
    }
}

/// Serves the frozen `OpenAPI` 3.1 JSON document generated from the `/api/v1` router.
#[utoipa::path(
    get,
    path = "/api/v1/openapi.json",
    tag = "system",
    responses(
        (status = 200, description = "Frozen OpenAPI 3.1 specification for /api/v1", body = OpenApiDocument, content_type = "application/json"),
        (status = 401, description = "Administrator authentication required in production", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub async fn get_openapi_json_handler(State(state): State<ApiRouterState>) -> Response {
    match state.schema_policy {
        SchemaAccessPolicy::NonProductionOpen => {
            let mut response = (
                StatusCode::OK,
                state.frozen_openapi_json.as_str().to_owned(),
            )
                .into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        SchemaAccessPolicy::ProductionAdminRequired => {
            let (status, details) = ProblemDetails::unauthorized(OPENAPI_JSON_PATH);
            ProblemResponse::new(status, details).into_response()
        }
    }
}

fn assemble_openapi_router() -> OpenApiRouter<ApiRouterState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::get_health_handler))
        .routes(routes!(get_openapi_json_handler))
}

/// Builds the exact `OpenAPI` 3.1 document without constructing any live runtime state,
/// database pool, or network socket.
#[must_use]
pub fn build_openapi_document() -> OpenApiSpec {
    let (_router, doc) = assemble_openapi_router().split_for_parts();
    doc
}

/// Serializes the canonical pretty-printed `OpenAPI` 3.1 JSON document with a trailing newline.
///
/// # Errors
///
/// Returns `serde_json::Error` only if `OpenAPI` serialization fails.
pub fn render_openapi_json() -> Result<String, serde_json::Error> {
    let mut rendered = serde_json::to_string_pretty(&build_openapi_document())?;
    rendered.push('\n');
    Ok(rendered)
}

/// Builds the `/api/v1` router and returns both the live router and the frozen [`OpenApiSpec`]
/// split from the exact same [`OpenApiRouter`] registration.
///
/// # Panics
///
/// Panics only if `serde_json` fails to serialize the static [`OpenApiSpec`] metadata.
pub fn build_api_router(
    probe: Arc<dyn HealthProbe>,
    schema_policy: SchemaAccessPolicy,
) -> (Router, OpenApiSpec) {
    let (router, doc) = assemble_openapi_router().split_for_parts();
    let frozen_json =
        serde_json::to_string_pretty(&doc).expect("OpenAPI document serialization is infallible");
    let state = ApiRouterState {
        health: HealthApiState::new(probe),
        frozen_openapi_json: Arc::new(frozen_json),
        schema_policy,
    };
    (router.with_state(state), doc)
}

/// Builds the production API with only the schema route behind current-admin middleware.
///
/// # Panics
///
/// Panics only if `serde_json` fails to serialize the static [`OpenApiSpec`] metadata.
pub fn build_api_router_admin(
    probe: Arc<dyn HealthProbe>,
    services: ConsoleServices,
    cookies: CookieSecurityPolicy,
) -> (Router, OpenApiSpec) {
    let (health_router, _) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::get_health_handler))
        .split_for_parts();
    let (schema_router, _) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_openapi_json_handler))
        .split_for_parts();
    let doc = build_openapi_document();
    let schema_router = schema_router.route_layer(axum::middleware::from_fn_with_state(
        AdminGuard::new(services, cookies),
        require_admin,
    ));
    let frozen_json =
        serde_json::to_string_pretty(&doc).expect("OpenAPI document serialization is infallible");
    let state = ApiRouterState {
        health: HealthApiState::new(probe),
        frozen_openapi_json: Arc::new(frozen_json),
        schema_policy: SchemaAccessPolicy::NonProductionOpen,
    };
    (health_router.merge(schema_router).with_state(state), doc)
}
