//! Strict Content-Security-Policy and companion browser hardening headers.

use axum::body::Body;
use axum::http::{HeaderValue, Request, header};
use axum::middleware::Next;
use axum::response::Response;

/// Authoritative strict Content-Security-Policy header value (`contracts/http-ui.md` §1).
///
/// Permits only same-origin assets and scripts (`'self'`), locally rendered `data:` images
/// (for one-time TOTP enrollment QR codes), and zero `unsafe-inline`, `unsafe-eval`,
/// wildcard, or remote origins.
pub const CSP_POLICY_VALUE: &str = concat!(
    "default-src 'self'; ",
    "object-src 'none'; ",
    "base-uri 'none'; ",
    "frame-ancestors 'none'; ",
    "form-action 'self'; ",
    "connect-src 'self'; ",
    "font-src 'self'; ",
    "img-src 'self' data:; ",
    "script-src 'self'; ",
    "style-src 'self'"
);

/// Applies the strict Content-Security-Policy, `X-Content-Type-Options: nosniff`, and
/// `Referrer-Policy: same-origin` headers to every response.
pub async fn apply_csp_middleware(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP_POLICY_VALUE),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    response
}
