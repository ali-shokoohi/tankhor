//! Sanitized `/api/v1/health` readiness handler.

use std::sync::Arc;

use admin::application::health::{HealthReport, get_health};
use admin::ports::health_probe::HealthProbe;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::problem::{ProblemDetails, ProblemResponse};

/// Canonical `/api/v1/health` URI path.
pub const HEALTH_PATH: &str = "/api/v1/health";

/// Shared application state required by `/api/v1/health`.
#[derive(Clone)]
pub struct HealthApiState {
    probe: Arc<dyn HealthProbe>,
}

impl HealthApiState {
    /// Wraps the application [`HealthProbe`] port.
    #[must_use]
    pub fn new(probe: Arc<dyn HealthProbe>) -> Self {
        Self { probe }
    }
}

/// Sanitized readiness payload returned when required storage and seed state are ready.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// Readiness status string (`"ok"` when ready).
    pub status: String,
}

impl HealthResponse {
    /// Constructs the ready response body.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            status: "ok".to_owned(),
        }
    }
}

/// Checks sanitized storage and seed readiness through the application [`get_health`] service.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "system",
    responses(
        (status = 200, description = "Service store and foundational seed are ready", body = HealthResponse),
        (status = 503, description = "Required store or foundational seed is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub async fn get_health_handler(State(state): State<HealthApiState>) -> Response {
    match get_health(state.probe.as_ref()).await {
        Ok(HealthReport::Ready) => {
            let mut response = (StatusCode::OK, Json(HealthResponse::ok())).into_response();
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Err(err) => ProblemResponse::from_domain_error(&err, HEALTH_PATH).into_response(),
    }
}
