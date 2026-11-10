//! Pure CSRF, same-origin, and cookie-attribute transport validation.
//!
//! This module generates no randomness and invokes no infrastructure/application port.
//! Browser-bound expected token material is supplied via [`CsrfContext`] in request extensions.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use kernel::locale::Locale;
use thiserror::Error;

use crate::i18n::ConsoleCatalog;

/// Maximum form-body bytes inspected by the CSRF validator when no `X-CSRF-Token` header is present.
const MAX_CSRF_FORM_INSPECT_BYTES: usize = 256 * 1024;

/// Validation error returned when configuring CSRF or cookie security policies.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SecurityPolicyError {
    /// Expected origin is empty, contains a path/wildcard, or lacks `http(s)://host`.
    #[error("invalid configured origin")]
    InvalidOrigin,
    /// Cookie attributes violate host-only / `__Host-` / `Secure` transport invariants.
    #[error("insecure cookie attributes: {0}")]
    InsecureCookieAttributes(&'static str),
    /// Cookie value contains forbidden control or separator characters.
    #[error("invalid cookie value characters")]
    InvalidCookieValue,
}

/// Browser-bound expected CSRF token supplied by the upstream session/ceremony layer.
#[derive(Clone, PartialEq, Eq)]
pub struct CsrfContext {
    expected_token: Arc<str>,
}

impl std::fmt::Debug for CsrfContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CsrfContext")
            .field("expected_token", &"[REDACTED]")
            .finish()
    }
}

impl CsrfContext {
    /// Construct a browser-bound CSRF token context.
    #[must_use]
    pub fn new(expected_token: impl Into<Arc<str>>) -> Self {
        Self {
            expected_token: expected_token.into(),
        }
    }

    /// Expected token bytes for constant-time comparison.
    #[must_use]
    pub fn expected_bytes(&self) -> &[u8] {
        self.expected_token.as_bytes()
    }
}

/// Shared configuration for pure CSRF + same-origin middleware.
#[derive(Clone)]
pub struct CsrfProtectionState {
    expected_origin: Arc<str>,
    catalog: Arc<ConsoleCatalog>,
    default_locale: Locale,
}

impl CsrfProtectionState {
    /// Validate `expected_origin` (`http://host[:port]` or `https://host[:port]`, no wildcard or path)
    /// and construct middleware state.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityPolicyError::InvalidOrigin`] if `expected_origin` is malformed.
    pub fn new(
        expected_origin: &str,
        catalog: ConsoleCatalog,
        default_locale: Locale,
    ) -> Result<Self, SecurityPolicyError> {
        let normalized = normalize_exact_origin(expected_origin)?;
        Ok(Self {
            expected_origin: Arc::from(normalized),
            catalog: Arc::new(catalog),
            default_locale,
        })
    }

    /// Configured exact origin (`scheme://authority`).
    #[must_use]
    pub fn expected_origin(&self) -> &str {
        &self.expected_origin
    }
}

/// Enforce same-origin and CSRF token verification on every state-changing HTTP method.
///
/// Safe methods (`GET`, `HEAD`, `OPTIONS`, `TRACE`) pass through without mutating state.
pub async fn enforce_csrf_middleware(
    State(state): State<CsrfProtectionState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if is_safe_method(request.method()) {
        return next.run(request).await;
    }

    let Some(context) = request.extensions().get::<CsrfContext>().cloned() else {
        return forbidden_csrf_response(&state);
    };
    if context.expected_bytes().is_empty() {
        return forbidden_csrf_response(&state);
    }

    if !matches_expected_origin(request.headers(), &state.expected_origin) {
        return forbidden_csrf_response(&state);
    }

    if let Some(header_token) = extract_header_csrf_token(request.headers()) {
        if constant_time_eq(header_token.as_bytes(), context.expected_bytes()) {
            return next.run(request).await;
        }
        return forbidden_csrf_response(&state);
    }

    if !is_urlencoded_form(request.headers()) {
        return forbidden_csrf_response(&state);
    }

    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, MAX_CSRF_FORM_INSPECT_BYTES).await else {
        return forbidden_csrf_response(&state);
    };

    let Some(submitted_token) = extract_urlencoded_csrf_field(&bytes) else {
        return forbidden_csrf_response(&state);
    };

    if !constant_time_eq(submitted_token.as_bytes(), context.expected_bytes()) {
        return forbidden_csrf_response(&state);
    }

    let rebuilt = Request::from_parts(parts, Body::from(bytes));
    next.run(rebuilt).await
}

/// Distinct console cookie purposes (never interchangeable, never shared with end-user cookies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieKind {
    /// Short-lived primary-factor / enrollment / ceremony cookie.
    PrimaryCeremony,
    /// Full verified two-factor administrator session cookie.
    AdminSession,
}

impl CookieKind {
    /// Cookie name under the active [`CookieSecurityPolicy`].
    #[must_use]
    pub const fn cookie_name(self, policy: CookieSecurityPolicy) -> &'static str {
        match (self, policy.use_host_prefix) {
            (Self::PrimaryCeremony, true) => "__Host-tankhor_admin_ceremony",
            (Self::AdminSession, true) => "__Host-tankhor_admin_session",
            (Self::PrimaryCeremony, false) => "tankhor_dev_admin_ceremony",
            (Self::AdminSession, false) => "tankhor_dev_admin_session",
        }
    }
}

/// Cookie transport attribute policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CookieSecurityPolicy {
    use_host_prefix: bool,
    secure: bool,
}

impl CookieSecurityPolicy {
    /// Production HTTPS policy (`__Host-` prefix, `Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`, no `Domain`).
    #[must_use]
    pub const fn production() -> Self {
        Self {
            use_host_prefix: true,
            secure: true,
        }
    }

    /// Local HTTP loopback test policy (`HttpOnly`, `SameSite=Lax`, `Path=/`, no `Domain`).
    #[must_use]
    pub const fn loopback_insecure_dev() -> Self {
        Self {
            use_host_prefix: false,
            secure: false,
        }
    }

    /// Validate raw cookie transport attributes against Tankhor's host-only cookie rules.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityPolicyError::InsecureCookieAttributes`] if `Domain` is set, `Path` is not `/`,
    /// or `__Host-` is used without `Secure`.
    pub const fn validate_raw_attributes(
        use_host_prefix: bool,
        secure: bool,
        path: &str,
        domain: Option<&str>,
    ) -> Result<(), SecurityPolicyError> {
        if domain.is_some() {
            return Err(SecurityPolicyError::InsecureCookieAttributes(
                "host-only cookies must never set Domain",
            ));
        }
        if !const_str_eq(path, "/") {
            return Err(SecurityPolicyError::InsecureCookieAttributes(
                "console cookies must set Path=/",
            ));
        }
        if use_host_prefix && !secure {
            return Err(SecurityPolicyError::InsecureCookieAttributes(
                "__Host- cookies require Secure",
            ));
        }
        Ok(())
    }
}

/// Format a validated `Set-Cookie` header value for `kind`.
///
/// # Errors
///
/// Returns [`SecurityPolicyError`] if policy invariants or `opaque_value` characters are invalid.
pub fn format_cookie_header(
    kind: CookieKind,
    opaque_value: &str,
    policy: CookieSecurityPolicy,
) -> Result<String, SecurityPolicyError> {
    CookieSecurityPolicy::validate_raw_attributes(
        policy.use_host_prefix,
        policy.secure,
        "/",
        None,
    )?;
    if opaque_value.is_empty()
        || !opaque_value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(SecurityPolicyError::InvalidCookieValue);
    }

    let name = kind.cookie_name(policy);
    let mut header_value = format!("{name}={opaque_value}; Path=/; HttpOnly; SameSite=Lax");
    if policy.secure {
        header_value.push_str("; Secure");
    }
    Ok(header_value)
}

const fn const_str_eq(lhs: &str, rhs: &str) -> bool {
    let lb = lhs.as_bytes();
    let rb = rhs.as_bytes();
    if lb.len() != rb.len() {
        return false;
    }
    let mut idx = 0;
    while idx < lb.len() {
        if lb[idx] != rb[idx] {
            return false;
        }
        idx += 1;
    }
    true
}

fn is_safe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

fn normalize_exact_origin(raw: &str) -> Result<String, SecurityPolicyError> {
    let trimmed = raw.trim();
    if trimmed.contains('*')
        || trimmed.contains(' ')
        || trimmed.contains('?')
        || trimmed.contains('#')
    {
        return Err(SecurityPolicyError::InvalidOrigin);
    }
    let (scheme, rest) = trimmed
        .split_once("://")
        .ok_or(SecurityPolicyError::InvalidOrigin)?;
    let lower_scheme = scheme.to_ascii_lowercase();
    if lower_scheme != "https" && lower_scheme != "http" {
        return Err(SecurityPolicyError::InvalidOrigin);
    }
    if rest.is_empty() || rest.contains('/') || rest.contains('@') {
        return Err(SecurityPolicyError::InvalidOrigin);
    }
    Ok(format!("{lower_scheme}://{}", rest.to_ascii_lowercase()))
}

fn extract_origin_from_referer(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let (scheme, rest) = trimmed.split_once("://")?;
    let authority = rest.split('/').next()?;
    normalize_exact_origin(&format!("{scheme}://{authority}")).ok()
}

fn matches_expected_origin(headers: &HeaderMap, expected_origin: &str) -> bool {
    if let Some(origin_val) = headers.get(header::ORIGIN) {
        let Ok(origin_str) = origin_val.to_str() else {
            return false;
        };
        let Ok(normalized) = normalize_exact_origin(origin_str) else {
            return false;
        };
        return normalized == expected_origin;
    }

    if let Some(referer_val) = headers.get(header::REFERER) {
        let Ok(referer_str) = referer_val.to_str() else {
            return false;
        };
        let Some(referer_origin) = extract_origin_from_referer(referer_str) else {
            return false;
        };
        return referer_origin == expected_origin;
    }

    false
}

fn extract_header_csrf_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn is_urlencoded_form(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| {
            ct.split(';').next().is_some_and(|mime| {
                mime.trim()
                    .eq_ignore_ascii_case("application/x-www-form-urlencoded")
            })
        })
}

fn extract_urlencoded_csrf_field(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    for pair in text.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == "csrf_token" {
            return percent_decode_form(value);
        }
    }
    None
}

fn percent_decode_form(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return None;
                }
                let hi = hex_nibble(bytes[i + 1])?;
                let lo = hex_nibble(bytes[i + 2])?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (&l, &r) in lhs.iter().zip(rhs.iter()) {
        diff |= l ^ r;
    }
    diff == 0
}

fn forbidden_csrf_response(state: &CsrfProtectionState) -> Response {
    let locale = state.default_locale;
    let message = state
        .catalog
        .format(locale, "error-csrf-invalid", &[])
        .unwrap_or_else(|_| String::from("Form verification failed."));
    let lang = locale.as_tag();
    let dir = locale.direction().as_str();
    let html = format!(
        "<!doctype html><html lang=\"{lang}\" dir=\"{dir}\"><head><meta charset=\"utf-8\"><title>403</title></head><body><main role=\"alert\"><p>{message}</p></main></body></html>"
    );
    (
        StatusCode::FORBIDDEN,
        [(header::CACHE_CONTROL, "no-store")],
        Html(html),
    )
        .into_response()
}
