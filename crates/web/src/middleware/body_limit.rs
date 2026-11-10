//! Pre-handler bounded request body enforcement (`contracts/http-ui.md` §1).
//!
//! Rejects request bodies exceeding the configured finite positive byte limit before
//! downstream handlers or application services run, rendering localized HTML 413 for
//! console routes and RFC 9457 `application/problem+json` 413 for `/api/v1` routes.

use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use kernel::locale::Locale;

use crate::api::problem::{ProblemDetails, ProblemResponse};
use crate::i18n::ConsoleCatalog;

/// Shared state for pre-handler bounded request-body enforcement.
#[derive(Clone)]
pub struct BodyLimitState {
    max_body_bytes: NonZeroUsize,
    catalog: Arc<ConsoleCatalog>,
    default_locale: Locale,
}

impl BodyLimitState {
    /// Construct a finite positive body-byte limit policy.
    #[must_use]
    pub const fn new(
        max_body_bytes: NonZeroUsize,
        catalog: Arc<ConsoleCatalog>,
        default_locale: Locale,
    ) -> Self {
        Self {
            max_body_bytes,
            catalog,
            default_locale,
        }
    }

    /// Maximum permitted request body size in bytes.
    #[must_use]
    pub const fn max_body_bytes(&self) -> NonZeroUsize {
        self.max_body_bytes
    }
}

/// Rejects requests whose body exceeds [`BodyLimitState::max_body_bytes`] before invoking
/// downstream CSRF/auth layers or application handlers.
pub async fn enforce_body_limit_middleware(
    State(state): State<BodyLimitState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let limit = state.max_body_bytes.get();
    let path = request.uri().path().to_owned();
    let locale = resolve_request_locale(request.uri(), state.default_locale);

    if let Some(declared_len) = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        && declared_len > limit
    {
        return payload_too_large_response(&state, &path, locale);
    }

    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, limit).await else {
        return payload_too_large_response(&state, &path, locale);
    };

    let rebuilt = Request::from_parts(parts, Body::from(bytes));
    next.run(rebuilt).await
}

fn payload_too_large_response(state: &BodyLimitState, path: &str, locale: Locale) -> Response {
    if is_json_api_path(path) {
        let (status, details) = ProblemDetails::payload_too_large(path.to_owned());
        return ProblemResponse::new(status, details).into_response();
    }

    let message = state
        .catalog
        .format(locale, "error-body-too-large", &[])
        .unwrap_or_else(|_| String::from("Request payload exceeds the maximum allowed size."));
    let lang = locale.as_tag();
    let dir = locale.direction().as_str();
    let html = format!(
        "<!doctype html><html lang=\"{lang}\" dir=\"{dir}\"><head><meta charset=\"utf-8\"><title>413</title></head><body><main role=\"alert\"><p>{message}</p></main></body></html>"
    );
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        [(header::CACHE_CONTROL, "no-store")],
        Html(html),
    )
        .into_response()
}

fn is_json_api_path(path: &str) -> bool {
    path == "/api/v1" || path.starts_with("/api/v1/")
}

fn resolve_request_locale(uri: &Uri, default_locale: Locale) -> Locale {
    let Some(query) = uri.query() else {
        return default_locale;
    };
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == "lang"
            && let Ok(parsed) = Locale::from_str(value)
        {
            return parsed;
        }
    }
    default_locale
}
