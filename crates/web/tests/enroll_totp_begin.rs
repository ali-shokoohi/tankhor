mod support;

use admin::domain::admin_state::EnrollmentPurpose;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use support::{CSRF, Fixture, ORIGIN, seed_enrollment};
use tower::ServiceExt;
use web::{
    console::enrollment::enroll_totp_begin_router,
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

fn request(form: &str, cookie: &str) -> Request<Body> {
    Request::post("/console/enrol/totp/begin")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, cookie)
        .extension(CsrfContext::new(CSRF))
        .body(Body::from(form.to_owned()))
        .unwrap()
}

async fn body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn enroll_totp_begin_has_required_authorization_and_bound_state() {
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
    let form = format!(
        "target={}&session={}&epoch=0&purpose=addition&csrf_token={CSRF}&lang=en",
        enrollment.target, enrollment.session,
    );
    let response = enroll_totp_begin_router(fixture.services.clone(), fixture.csrf(), cookies)
        .oneshot(request(&form, &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(response.headers().get(header::LOCATION).is_none());
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("__Host-tankhor_admin_totp=")
    );
    let html = body(response).await;
    assert!(html.contains("id=\"provisioning\""));
    assert!(html.split("id=\"provisioning\">").nth(1).is_some());
    assert!(!html.contains("https://chart.googleapis.com"));
    assert!(!html.contains(&enrollment.proof));
}

#[tokio::test]
async fn totp_provisioning_is_once_authenticated_and_no_store() {
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
    let form = format!(
        "target={}&session={}&epoch=0&purpose=addition&csrf_token={CSRF}&lang=en",
        enrollment.target, enrollment.session,
    );
    let app = enroll_totp_begin_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app.clone().oneshot(request(&form, &cookie)).await.unwrap();
    let html = body(response).await;
    let provisioning = html
        .split("id=\"provisioning\">")
        .nth(1)
        .unwrap()
        .split('<')
        .next()
        .unwrap()
        .to_owned();
    assert!(!provisioning.is_empty());

    let replay = app.clone().oneshot(request(&form, &cookie)).await.unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(replay.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!body(replay).await.contains(&provisioning));
    let get = app
        .clone()
        .oneshot(
            Request::get("/console/enrol/totp/begin")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(get.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!body(get).await.contains(&provisioning));

    let concurrent = Fixture::new();
    let enrollment = seed_enrollment(&concurrent, EnrollmentPurpose::Addition).await;
    let cookie = format!(
        "{}={}; {}={}",
        CookieKind::PrimaryProof.cookie_name(cookies),
        enrollment.proof,
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        enrollment.browser,
    );
    let form = format!(
        "target={}&session={}&epoch=0&purpose=addition&csrf_token={CSRF}&lang=en",
        enrollment.target, enrollment.session,
    );
    let app = enroll_totp_begin_router(concurrent.services.clone(), concurrent.csrf(), cookies);
    let (first, second) = tokio::join!(
        app.clone().oneshot(request(&form, &cookie)),
        app.oneshot(request(&form, &cookie)),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(
        [first.status(), second.status()]
            .into_iter()
            .filter(|status| *status == StatusCode::OK)
            .count(),
        1
    );
    for response in [first, second] {
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        if response.status() != StatusCode::OK {
            assert!(!body(response).await.contains("otpauth://"));
        }
    }
}
