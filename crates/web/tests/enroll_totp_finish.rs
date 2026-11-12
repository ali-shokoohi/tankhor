mod support;

use admin::domain::{
    admin_state::{Eligibility, EnrollmentPurpose},
    auth_state::AuthStage,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use kernel::{ids::CredentialId, ports::clock::Clock};
use support::{CSRF, Fixture, ORIGIN, seed_enrollment};
use tower::ServiceExt;
use web::{
    console::enrollment::{enroll_totp_begin_router, enroll_totp_finish_router},
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn enroll_totp_finish_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    fixture
        .clock
        .set(time::macros::datetime!(2026-01-01 00:00 UTC));
    let enrollment = seed_enrollment(&fixture, EnrollmentPurpose::Addition).await;
    let cookies = CookieSecurityPolicy::production();
    let primary_cookie = format!(
        "{}={}; {}={}",
        CookieKind::PrimaryProof.cookie_name(cookies),
        enrollment.proof,
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        enrollment.browser,
    );
    let binding = format!(
        "target={}&session={}&epoch=0&purpose=addition&csrf_token={CSRF}&lang=en",
        enrollment.target, enrollment.session,
    );
    let begin = enroll_totp_begin_router(fixture.services.clone(), fixture.csrf(), cookies)
        .oneshot(
            Request::post("/console/enrol/totp/begin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, primary_cookie)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(binding))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(begin.status(), StatusCode::OK);
    let ceremony = begin.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .split('=')
        .nth(1)
        .unwrap()
        .to_owned();
    let html = String::from_utf8(
        to_bytes(begin.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let credential = hidden_value(&html, "credential")
        .parse::<CredentialId>()
        .unwrap();
    let state = fixture.store.try_read_committed().unwrap();
    let pending = state
        .admin_pending
        .values()
        .find(|pending| pending.state().binding().stage == AuthStage::Totp)
        .unwrap()
        .clone();
    drop(state);
    let envelope = pending.verification_envelope().unwrap();
    let (code, _) = fixture
        .authenticator
        .totp_fixture(envelope, fixture.clock.now().await.unwrap())
        .unwrap();
    let code = String::from_utf8(code.expose().to_vec()).unwrap();
    let cookie = format!(
        "{}={ceremony}; {}={}",
        CookieKind::TotpCeremony.cookie_name(cookies),
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        enrollment.browser,
    );
    let form = format!(
        "credential={credential}&target={}&session={}&epoch=0&credential_revision=1&purpose=addition&code={code}&csrf_token={CSRF}&lang=en",
        enrollment.target, enrollment.session,
    );
    let app = enroll_totp_finish_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app
        .clone()
        .oneshot(
            Request::post("/console/enrol/totp/finish")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(form.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("__Host-tankhor_admin_session=")
    );
    let state = fixture.store.try_read_committed().unwrap();
    assert_eq!(
        state.admins.get(&enrollment.target).unwrap().eligibility(),
        Eligibility::Active
    );
    assert!(state.admin_credentials.contains_key(&credential));
    assert_eq!(state.admin_sessions.len(), 1);
    drop(state);
    let replay = app
        .oneshot(
            Request::post("/console/enrol/totp/finish")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, cookie)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(replay.headers()[header::CACHE_CONTROL], "no-store");
}

fn hidden_value(html: &str, name: &str) -> String {
    let marker = format!("name=\"{name}\" value=\"");
    html.split(&marker)
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned()
}
