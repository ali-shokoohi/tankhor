mod support;

use admin::domain::admin_state::EnrollmentPurpose;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use support::{Fixture, seed_enrollment};
use tower::ServiceExt;
use web::{
    console::enrollment::enrollment_router,
    security::{CookieKind, CookieSecurityPolicy},
};

#[tokio::test]
async fn enrollment_page_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    let enrollment = seed_enrollment(&fixture, EnrollmentPurpose::Addition).await;
    let cookies = CookieSecurityPolicy::production();
    let cookie = format!(
        "{}={}; {}={}",
        CookieKind::PrimaryProof.cookie_name(cookies),
        enrollment.proof,
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        enrollment.browser,
    );
    let query = format!(
        "target={}&session={}&epoch=0&purpose=addition&lang=en",
        enrollment.target, enrollment.session
    );
    let app = enrollment_router(fixture.services.clone(), cookies);
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/console/access/enrol?{query}"))
                .header(header::COOKIE, cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("Choose an authentication factor"));
    assert!(body.contains("/console/enrol/passkey/begin"));
    assert!(body.contains("/console/enrol/totp/begin"));
    assert!(!body.contains(&enrollment.proof));

    let denied = app
        .oneshot(
            Request::get(format!("/console/access/enrol?{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
}
