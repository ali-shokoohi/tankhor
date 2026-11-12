mod support;

use std::sync::Arc;

use admin::{
    application::recover_admin::{RecoverAdmin, RecoverAdminInput},
    domain::{audit::ReasonCode, secrets::SecretBytes},
    ports::admin_authenticator::{AdminAuthenticator, TokenPurpose},
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::get,
};
use support::{Fixture, seed_console_session};
use testkit::fakes::health_probe::FakeHealthProbe;
use tower::ServiceExt;
use web::{
    api::docs::build_api_and_docs_router_admin,
    console::guard::{AdminGuard, require_admin},
    security::{CookieKind, CookieSecurityPolicy},
};

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn production_docs_and_assets_require_admin() {
    let fixture = Fixture::new();
    fixture.store.seed(|state| state.health_ready = true);
    let admin = fixture.services.ids.next_admin_id().await.unwrap();
    let session = fixture.services.ids.next_session_id().await.unwrap();
    let token = seed_console_session(&fixture, admin, session).await;
    let cookies = CookieSecurityPolicy::production();
    let cookie = format!("{}={token}", CookieKind::AdminSession.cookie_name(cookies));
    let protected = Router::new()
        .route(
            "/console/protected",
            get(|| async { StatusCode::OK.into_response() }),
        )
        .layer(middleware::from_fn_with_state(
            AdminGuard::new(fixture.services.clone(), cookies),
            require_admin,
        ));
    for token in [None, Some("end-user-session")] {
        let mut request = Request::get("/console/protected");
        if let Some(token) = token {
            request = request.header(
                header::COOKIE,
                format!("{}={token}", CookieKind::AdminSession.cookie_name(cookies)),
            );
        }
        let response = protected
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    let allowed = protected
        .clone()
        .oneshot(
            Request::get("/console/protected")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);

    let (docs, _) = build_api_and_docs_router_admin(
        Arc::new(FakeHealthProbe::new(fixture.store.clone())),
        fixture.services.clone(),
        cookies,
    );
    for path in [
        "/api/docs",
        "/api/docs/",
        "/api/docs/swagger-ui.css",
        "/api/v1/openapi.json",
    ] {
        let denied = docs
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(
            denied.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
        assert_eq!(denied.headers()[header::CACHE_CONTROL], "no-store");
        let allowed = docs
            .clone()
            .oneshot(
                Request::get(path)
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            allowed.status().is_success() || allowed.status().is_redirection(),
            "{path}: {}",
            allowed.status()
        );
    }

    let other = fixture.services.ids.next_admin_id().await.unwrap();
    let other_session = fixture.services.ids.next_session_id().await.unwrap();
    let other_token = seed_console_session(&fixture, other, other_session).await;
    let actor = fixture
        .authenticator
        .token_digest(TokenPurpose::Session, SecretBytes::new(token.into_bytes()))
        .await
        .unwrap();
    RecoverAdmin {
        uow: fixture.services.uow.as_ref(),
        admins: fixture.services.admins.as_ref(),
        credentials: fixture.services.credentials.as_ref(),
        sessions: fixture.services.sessions.as_ref(),
        otp: fixture.services.otps.as_ref(),
        audit: fixture.services.audit.as_ref(),
        clock: fixture.services.clock.as_ref(),
        ids: fixture.services.ids.as_ref(),
        policy: &fixture.services.policy,
    }
    .execute(RecoverAdminInput {
        actor,
        target: other,
        reason: Some(ReasonCode::AdminRecovery),
    })
    .await
    .unwrap();
    let stale_cookie = format!(
        "{}={other_token}",
        CookieKind::AdminSession.cookie_name(cookies)
    );
    let stale = protected
        .oneshot(
            Request::get("/console/protected")
                .header(header::COOKIE, stale_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::UNAUTHORIZED);
}
