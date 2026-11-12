mod support;

use std::str::FromStr;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use kernel::ids::{AdminId, SessionId};
use support::{CSRF, Fixture, ORIGIN, seed_console_session};
use tower::ServiceExt;
use web::{
    console::admins::admin_invite_router,
    security::{CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
async fn admin_invite_form_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    let actor = AdminId::from_str("019a53a0-0000-7000-8000-000000000031").unwrap();
    let session = SessionId::from_str("019a53a0-0000-7000-8000-000000000032").unwrap();
    let token = seed_console_session(&fixture, actor, session).await;
    let cookies = CookieSecurityPolicy::production();
    let app = admin_invite_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app
        .oneshot(
            Request::post("/console/admins/invite")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "phone=%2B989120000002&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::LOCATION], "/console/admins");
    let state = fixture.store.try_read_committed().unwrap();
    assert_eq!(state.admins.len(), 2);
    assert_eq!(state.audit_entries.len(), 1);

    let denied = admin_invite_router(fixture.services.clone(), fixture.csrf(), cookies)
        .oneshot(
            Request::post("/console/admins/invite")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "phone=%2B989120000002&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(denied.headers()[header::CACHE_CONTROL], "no-store");
}
