mod support;

use admin::domain::auth_state::BindingDigest;
use admin::ports::admin_authenticator::{AdminAuthenticator, TokenPurpose};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use web::console::auth::{OriginBinding, request_code_router};
use web::security::{CookieKind, CookieSecurityPolicy, CsrfContext};

#[tokio::test]
async fn access_forms_never_grant_protected_access_from_primary_only() {
    let fixture = support::Fixture::new();
    let app = request_code_router(
        fixture.services.clone(),
        fixture.csrf(),
        CookieSecurityPolicy::loopback_insecure_dev(),
    );
    let browser = fixture
        .authenticator
        .issue_token(TokenPurpose::Browser)
        .await
        .unwrap();
    let browser = String::from_utf8(browser.expose().to_vec()).unwrap();
    let cookie = format!(
        "{}={browser}",
        CookieKind::PrimaryCeremony.cookie_name(CookieSecurityPolicy::loopback_insecure_dev())
    );
    let valid = || {
        let mut request = Request::builder()
            .method("POST")
            .uri("/console/sign-in/request-code")
            .header(header::ORIGIN, support::ORIGIN)
            .header(header::COOKIE, &cookie)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "csrf_token={}&phone=%2B989120000001&return=%2Fconsole%2Fadmins&lang=en",
                support::CSRF
            )))
            .unwrap();
        request
            .extensions_mut()
            .insert(CsrfContext::new(support::CSRF));
        request
            .extensions_mut()
            .insert(OriginBinding(BindingDigest::new([8; 32])));
        request
    };
    let response = app.clone().oneshot(valid()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(
        response.headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .starts_with("/console/sign-in/verify-code")
    );
    let state = fixture.store.try_read_committed().unwrap();
    assert!(
        state.admin_sessions.is_empty()
            && state.admin_pending.is_empty()
            && state.audit_entries.is_empty()
    );

    let mut bad_origin = valid();
    bad_origin
        .headers_mut()
        .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(bad_origin).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    let no_context = Request::builder()
        .method("POST")
        .uri("/console/sign-in/request-code")
        .header(header::ORIGIN, support::ORIGIN)
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!(
            "csrf_token={}&phone=%2B989120000001",
            support::CSRF
        )))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(no_context).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    let mut invalid = valid();
    *invalid.body_mut() = Body::from(format!("csrf_token={}&phone=&lang=en", support::CSRF));
    let response = app.oneshot(invalid).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = String::from_utf8(
        to_bytes(response.into_body(), 65536)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("valid") && !body.contains(support::CSRF));
}
