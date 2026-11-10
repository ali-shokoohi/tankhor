//! RFC 9457 `application/problem+json` error representation for `/api/v1` endpoints.
//!
//! Translates sanitized [`DomainError`] and transport rejections into structured
//! problem documents without exposing infrastructure, database, or secret payloads.

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kernel::error::{DomainError, ErrorCategory, ErrorCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Content-Type header value mandated by RFC 9457.
pub const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// Sanitized RFC 9457 problem document returned by JSON API endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProblemDetails {
    /// Stable URI reference identifying the problem type (`urn:tankhor:problem:<slug>`).
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Short, human-readable summary of the problem type.
    pub title: String,
    /// HTTP status code generated for this occurrence of the problem.
    pub status: u16,
    /// Sanitized human-readable explanation specific to this occurrence.
    pub detail: String,
    /// Opaque URI reference identifying the specific occurrence or request path.
    pub instance: String,
}

impl ProblemDetails {
    /// Builds an explicit sanitized problem document.
    #[must_use]
    pub fn new(
        status: StatusCode,
        slug: &str,
        title: impl Into<String>,
        detail: impl Into<String>,
        instance: impl Into<String>,
    ) -> Self {
        Self {
            problem_type: format!("urn:tankhor:problem:{slug}"),
            title: title.into(),
            status: status.as_u16(),
            detail: detail.into(),
            instance: instance.into(),
        }
    }

    /// Maps a sanitized [`DomainError`] to an HTTP status and [`ProblemDetails`] payload.
    #[must_use]
    pub fn from_domain_error(
        error: &DomainError,
        instance: impl Into<String>,
    ) -> (StatusCode, Self) {
        let status = match (error.category(), error.code()) {
            (ErrorCategory::Validation, _) => StatusCode::UNPROCESSABLE_ENTITY,
            (ErrorCategory::Unauthorised, _) => StatusCode::UNAUTHORIZED,
            (ErrorCategory::Forbidden, _) => StatusCode::FORBIDDEN,
            (_, ErrorCode::NotFound) => StatusCode::NOT_FOUND,
            (ErrorCategory::Conflict, _) => StatusCode::CONFLICT,
            (ErrorCategory::Unavailable | ErrorCategory::Retryable, _) => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            (ErrorCategory::InvalidHandle | ErrorCategory::UnknownCommit, _) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        let (slug, title, detail) = match error.code() {
            ErrorCode::InvalidValue => (
                "invalid-value",
                "Invalid Request Value",
                "One or more request parameters failed validation.",
            ),
            ErrorCode::AuthenticationRequired => (
                "authentication-required",
                "Authentication Required",
                "Valid administrator authentication is required for this endpoint.",
            ),
            ErrorCode::PermissionDenied => (
                "permission-denied",
                "Permission Denied",
                "The authenticated caller is not permitted to perform this operation.",
            ),
            ErrorCode::StaleRevision => (
                "stale-revision",
                "Resource Conflict",
                "The target resource changed before the request completed.",
            ),
            ErrorCode::NotFound => (
                "not-found",
                "Resource Not Found",
                "The requested resource does not exist.",
            ),
            ErrorCode::IdempotencyConflict => (
                "idempotency-conflict",
                "Idempotency Conflict",
                "The operation key was already used with different parameters.",
            ),
            ErrorCode::PrerequisiteUnavailable | ErrorCode::SourceExhausted => (
                "service-unavailable",
                "Service Unavailable",
                "Required service prerequisites are not currently ready.",
            ),
            ErrorCode::TransientConflict => (
                "retryable-conflict",
                "Temporary Conflict",
                "The request encountered temporary contention and may be retried.",
            ),
            ErrorCode::FinishedTransactionHandle
            | ErrorCode::ForeignTransactionHandle
            | ErrorCode::ReadOnlyTransaction
            | ErrorCode::CommitOutcomeUnknown => (
                "internal-error",
                "Internal Error",
                "The request could not be completed safely.",
            ),
        };

        (status, Self::new(status, slug, title, detail, instance))
    }

    /// Standard 413 Content Too Large problem response.
    #[must_use]
    pub fn payload_too_large(instance: impl Into<String>) -> (StatusCode, Self) {
        let status = StatusCode::PAYLOAD_TOO_LARGE;
        (
            status,
            Self::new(
                status,
                "payload-too-large",
                "Payload Too Large",
                "The request body exceeds the configured maximum byte limit.",
                instance,
            ),
        )
    }

    /// Standard 503 Service Unavailable overload problem response.
    #[must_use]
    pub fn service_overloaded(instance: impl Into<String>) -> (StatusCode, Self) {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        (
            status,
            Self::new(
                status,
                "service-overloaded",
                "Service Busy",
                "The server is at its configured concurrent request limit.",
                instance,
            ),
        )
    }

    /// Standard 401 Unauthorized problem response.
    #[must_use]
    pub fn unauthorized(instance: impl Into<String>) -> (StatusCode, Self) {
        let status = StatusCode::UNAUTHORIZED;
        (
            status,
            Self::new(
                status,
                "authentication-required",
                "Authentication Required",
                "Valid administrator authentication is required for this endpoint.",
                instance,
            ),
        )
    }
}

/// Axum response wrapper serializing [`ProblemDetails`] with `application/problem+json`.
#[derive(Debug, Clone)]
pub struct ProblemResponse {
    status: StatusCode,
    details: ProblemDetails,
}

impl ProblemResponse {
    /// Creates a new RFC 9457 HTTP response.
    #[must_use]
    pub const fn new(status: StatusCode, details: ProblemDetails) -> Self {
        Self { status, details }
    }

    /// Converts a sanitized [`DomainError`] into a [`ProblemResponse`].
    #[must_use]
    pub fn from_domain_error(error: &DomainError, instance: impl Into<String>) -> Self {
        let (status, details) = ProblemDetails::from_domain_error(error, instance);
        Self { status, details }
    }
}

impl IntoResponse for ProblemResponse {
    fn into_response(self) -> Response {
        let body = serde_json::to_vec(&self.details).unwrap_or_else(|_| {
            br#"{"type":"urn:tankhor:problem:internal-error","title":"Internal Error","status":500,"detail":"Problem serialization failed.","instance":"urn:tankhor:occurrence:serialization"}"#.to_vec()
        });
        let mut response = (self.status, body).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(PROBLEM_CONTENT_TYPE),
        );
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}
