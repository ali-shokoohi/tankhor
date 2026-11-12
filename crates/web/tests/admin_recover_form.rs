mod support;

use std::str::FromStr;

use admin::domain::{
    admin_state::{AdminState, Eligibility, EnrollmentPurpose, EnrollmentStatus, SecurityEpoch},
    audit::ReasonCode,
};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use kernel::ids::{AdminId, SessionId};
use support::{CSRF, Fixture, ORIGIN, seed_console_session};
use tower::ServiceExt;
use web::{
    console::admins::admin_recover_router,
    security::{CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
async fn admin_recover_form_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    let actor = AdminId::from_str("019a53a0-0000-7000-8000-000000000031").unwrap();
    let session = SessionId::from_str("019a53a0-0000-7000-8000-000000000032").unwrap();
    let token = seed_console_session(&fixture, actor, session).await;
    let target = AdminId::from_str("019a53a0-0000-7000-8000-000000000033").unwrap();
    fixture.store.seed(|state| {
        state.admins.insert(
            target,
            AdminState::new(target, Eligibility::Active, SecurityEpoch::new(0)),
        );
    });
    let app = admin_recover_router(
        fixture.services.clone(),
        fixture.csrf(),
        CookieSecurityPolicy::production(),
    );
    let body = format!("target={target}&csrf_token={CSRF}&lang=en");
    let response = app
        .clone()
        .oneshot(
            Request::post("/console/admins/recover")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let state = fixture.store.try_read_committed().unwrap();
    assert_eq!(
        state.admins.get(&actor).unwrap().epoch(),
        SecurityEpoch::new(0)
    );
    let recovered = state.admins.get(&target).unwrap();
    assert_eq!(recovered.epoch(), SecurityEpoch::new(1));
    assert_eq!(recovered.eligibility(), Eligibility::Recovering);
    let enrollment = recovered.enrollment().unwrap();
    assert_eq!(enrollment.purpose(), EnrollmentPurpose::Recovery);
    assert_eq!(enrollment.authorizer(), actor);
    assert_eq!(enrollment.status(), EnrollmentStatus::Pending);
    assert!(
        state
            .audit_entries
            .iter()
            .any(|(_, entry)| entry.reason() == ReasonCode::AdminRecovery)
    );
    drop(state);

    let denied = app
        .oneshot(
            Request::post("/console/admins/recover")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert!(denied.headers().get(header::SET_COOKIE).is_none());
}
