mod support;

use admin::domain::admin_state::EnrollmentPurpose;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use support::{CSRF, Fixture, seed_enrollment};
use tower::ServiceExt;
use web::{
    console::{auth::second_factor_router, enrollment::enrollment_router},
    security::{CookieKind, CookieSecurityPolicy},
};

#[tokio::test]
async fn no_js_totp_and_no_htmx_access_have_equivalent_security() {
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
    let second = second_factor_router(fixture.services.clone(), fixture.csrf(), cookies)
        .oneshot(
            Request::get(format!(
                "/console/sign-in/second-factor?target={}&session={}&epoch=0&lang=en",
                enrollment.target, enrollment.session
            ))
            .header(header::COOKIE, &cookie)
            .extension(web::security::CsrfContext::new(CSRF))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(second.headers()[header::CACHE_CONTROL], "no-store");
    let html = String::from_utf8(
        to_bytes(second.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("action=\"/console/sign-in/totp\""));
    assert!(html.contains("name=\"code\""));
    assert!(html.contains("noscript"));
    assert!(html.contains("role=\"note\""));
    assert!(!html.contains("htmx"));
    assert!(!html.contains("data-auto-enrol"));

    let enrollment_page = enrollment_router(fixture.services.clone(), cookies)
        .oneshot(
            Request::get(format!(
                "/console/access/enrol?target={}&session={}&epoch=0&purpose=addition&lang=en",
                enrollment.target, enrollment.session
            ))
            .header(header::COOKIE, cookie)
            .extension(web::security::CsrfContext::new(CSRF))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enrollment_page.status(), StatusCode::OK);
    let html = String::from_utf8(
        to_bytes(enrollment_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("action=\"/console/enrol/totp/begin\""));
    assert!(html.contains("action=\"/console/enrol/passkey/begin\""));
    assert!(html.contains("noscript"));
    assert!(html.contains("role=\"note\""));
    assert!(!html.contains("hx-"));
}
