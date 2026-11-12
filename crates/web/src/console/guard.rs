//! Reusable current-authority middleware for HTML and local API documentation.

use admin::{application::console_access::VerifyConsoleAccess, domain::secrets::SecretBytes};
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::ConsoleServices;
use crate::{
    api::problem::{ProblemDetails, ProblemResponse},
    security::CookieKind,
};

/// Ports and cookie policy needed to recheck current administrator authority.
#[derive(Clone)]
pub struct AdminGuard {
    services: ConsoleServices,
    cookies: crate::security::CookieSecurityPolicy,
}

impl AdminGuard {
    #[must_use]
    pub fn new(services: ConsoleServices, cookies: crate::security::CookieSecurityPolicy) -> Self {
        Self { services, cookies }
    }
}

/// Verify the presented session through the application service on every request.
pub async fn require_admin(
    State(guard): State<AdminGuard>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    let is_api = path.starts_with("/api/");
    let token = super::auth::cookie(
        request.headers(),
        CookieKind::AdminSession.cookie_name(guard.cookies),
    );
    let actor = match token {
        Some(token) => (VerifyConsoleAccess {
            uow: guard.services.uow.as_ref(),
            admins: guard.services.admins.as_ref(),
            sessions: guard.services.sessions.as_ref(),
            authenticator: guard.services.authenticator.as_ref(),
            clock: guard.services.clock.as_ref(),
        })
        .execute(SecretBytes::new(token.as_bytes().to_vec()))
        .await
        .ok(),
        None => None,
    };
    if let Some(actor) = actor {
        request.extensions_mut().insert(actor);
        return next.run(request).await;
    }
    if is_api {
        let (status, details) = ProblemDetails::unauthorized(path);
        let mut response = ProblemResponse::new(status, details).into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );
        response
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::CACHE_CONTROL, "no-store")],
        )
            .into_response()
    }
}
