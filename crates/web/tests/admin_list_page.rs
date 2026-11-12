mod support;

use std::str::FromStr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use kernel::ids::{AdminId, SessionId};
use support::{Fixture, seed_console_session};
use tower::ServiceExt;
use web::{console::admins::admin_list_router, security::CookieSecurityPolicy};

#[tokio::test]
async fn admin_list_page_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    let admin = AdminId::from_str("019a53a0-0000-7000-8000-000000000021").unwrap();
    let session = SessionId::from_str("019a53a0-0000-7000-8000-000000000022").unwrap();
    let token = seed_console_session(&fixture, admin, session).await;
    let cookies = CookieSecurityPolicy::production();
    let response = admin_list_router(fixture.services.clone(), cookies)
        .oneshot(
            Request::get("/console/admins?lang=en&limit=50")
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let html = String::from_utf8(
        to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains(&admin.to_string()));
    assert!(html.contains("lang=\"en\""));
    assert!(html.contains("dir=\"ltr\""));
    assert!(!html.contains("synthetic"));

    let denied = admin_list_router(fixture.services.clone(), cookies)
        .oneshot(Request::get("/console/admins").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(denied.headers()[header::CACHE_CONTROL], "no-store");
}
