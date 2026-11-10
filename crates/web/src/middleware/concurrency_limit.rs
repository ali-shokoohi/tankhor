//! Bounded concurrent request enforcement with immediate non-queueing rejection (`contracts/http-ui.md` §1).
//!
//! Acquires an owned RAII permit via [`Semaphore::try_acquire_owned`] before delegating to
//! downstream middleware/handlers. Excess concurrent requests are immediately rejected
//! without unbounded queueing (`503 Service Unavailable` localized HTML or RFC 9457
//! `application/problem+json`), and permits are automatically released on completion,
//! error, or future cancellation/drop.

use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use kernel::locale::Locale;
use tokio::sync::Semaphore;

use crate::api::problem::{ProblemDetails, ProblemResponse};
use crate::i18n::ConsoleCatalog;

/// Shared state for bounded in-flight request concurrency enforcement.
#[derive(Clone)]
pub struct ConcurrencyLimitState {
    semaphore: Arc<Semaphore>,
    max_inflight: NonZeroUsize,
    catalog: Arc<ConsoleCatalog>,
    default_locale: Locale,
}

impl ConcurrencyLimitState {
    /// Construct a finite positive concurrent in-flight limit policy.
    #[must_use]
    pub fn new(
        max_inflight: NonZeroUsize,
        catalog: Arc<ConsoleCatalog>,
        default_locale: Locale,
    ) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_inflight.get())),
            max_inflight,
            catalog,
            default_locale,
        }
    }

    /// Configured maximum concurrent in-flight requests.
    #[must_use]
    pub const fn max_inflight(&self) -> NonZeroUsize {
        self.max_inflight
    }

    /// Currently available unused in-flight permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

/// Acquires one in-flight permit immediately without queueing, holding it across `next.run(...)`
/// so completion, error, or cancellation releases the permit automatically.
pub async fn enforce_concurrency_limit_middleware(
    State(state): State<ConcurrencyLimitState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Ok(_permit) = Arc::clone(&state.semaphore).try_acquire_owned() else {
        let path = request.uri().path().to_owned();
        let locale = resolve_request_locale(request.uri(), state.default_locale);
        return service_overloaded_response(&state, &path, locale);
    };

    next.run(request).await
}

fn service_overloaded_response(
    state: &ConcurrencyLimitState,
    path: &str,
    locale: Locale,
) -> Response {
    if is_json_api_path(path) {
        let (status, details) = ProblemDetails::service_overloaded(path.to_owned());
        return ProblemResponse::new(status, details).into_response();
    }

    let message = state
        .catalog
        .format(locale, "error-service-busy", &[])
        .unwrap_or_else(|_| String::from("Service is temporarily busy. Please retry shortly."));
    let lang = locale.as_tag();
    let dir = locale.direction().as_str();
    let html = format!(
        "<!doctype html><html lang=\"{lang}\" dir=\"{dir}\"><head><meta charset=\"utf-8\"><title>503</title></head><body><main role=\"alert\"><p>{message}</p></main></body></html>"
    );
    (
        StatusCode::SERVICE_UNAVAILABLE,
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
