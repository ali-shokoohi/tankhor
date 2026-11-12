mod support;

use admin::domain::{
    admin_state::EnrollmentPurpose,
    audit::{AuditAction, Factor},
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use support::{CSRF, Fixture, ORIGIN, seed_enrollment};
use tower::ServiceExt;
use web::{
    console::enrollment::enroll_passkey_begin_router,
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
async fn enroll_passkey_begin_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    let enrollment = seed_enrollment(&fixture, EnrollmentPurpose::Addition).await;
    let cookies = CookieSecurityPolicy::production();
    let app = enroll_passkey_begin_router(fixture.services.clone(), fixture.csrf(), cookies);
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
    let response = app
        .clone()
        .oneshot(
            Request::post("/console/enrol/passkey/begin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, cookie)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(form.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let set_cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(set_cookie.starts_with("__Host-tankhor_admin_passkey="));
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("public-options"));
    assert!(!body.contains("protected_state"));
    assert!(!body.contains(&enrollment.proof));
    let state = fixture.store.try_read_committed().unwrap();
    let pending = state
        .admin_pending
        .values()
        .find(|pending| {
            pending.state().binding().stage == admin::domain::auth_state::AuthStage::Passkey
        })
        .unwrap();
    assert_eq!(pending.state().binding().credential_revision, Some(1));
    assert!(state.audit_entries.values().any(|entry| {
        entry.action() == AuditAction::EnrollmentStarted
            && matches!(
                entry.metadata(),
                admin::domain::audit::AuditMetadata::Factor(Factor::Passkey)
            )
    }));
    drop(state);

    let denied = app
        .oneshot(
            Request::post("/console/enrol/passkey/begin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
}
