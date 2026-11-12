mod support;

use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::BindingDigest,
    },
    ports::admin_authenticator::AdminAuthenticator,
};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
// IDs are obtained through the fixture's application ports.
use support::{CSRF, Fixture, ORIGIN};
use tower::ServiceExt;
use web::{
    console::auth::{OriginBinding, request_code_router, verify_code_router},
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
async fn verify_code_form_sets_restricted_proof_only() {
    let fixture = Fixture::new();
    invite_active_admin(&fixture).await;
    let cookies = CookieSecurityPolicy::production();
    let browser = fixture
        .authenticator
        .issue_token(admin::ports::admin_authenticator::TokenPurpose::Browser)
        .await
        .unwrap();
    let browser = String::from_utf8(browser.expose().to_vec()).unwrap();
    // The issued browser capability is retained separately from the restricted proof cookie.
    let request_app = request_code_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = request_app
        .oneshot(
            Request::post("/console/sign-in/request-code")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!(
                        "{}={browser}",
                        CookieKind::PrimaryCeremony.cookie_name(cookies)
                    ),
                )
                .extension(OriginBinding(BindingDigest::new([8; 32])))
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "phone=%2B989120000001&csrf_token={CSRF}&purpose=sign-in"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let challenge = fixture
        .store
        .try_read_committed()
        .unwrap()
        .admin_otps
        .keys()
        .next()
        .copied()
        .unwrap()
        .to_string();
    let app = verify_code_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app
        .oneshot(
            Request::post("/console/sign-in/verify-code")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, format!("{}={browser}", CookieKind::PrimaryCeremony.cookie_name(cookies)))
                .extension(OriginBinding(BindingDigest::new([8; 32])))
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "phone=%2B989120000001&challenge={challenge}&code=987001&csrf_token={CSRF}&purpose=sign-in"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let proof_cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(proof_cookie.starts_with("__Host-tankhor_admin_proof="));
    assert!(proof_cookie.contains("HttpOnly"));
    assert!(proof_cookie.contains("Secure"));
    assert!(proof_cookie.contains("SameSite=Lax"));
    let state = fixture.store.try_read_committed().unwrap();
    assert!(state.admin_sessions.is_empty());
    assert!(state.audit_entries.is_empty());
    assert_eq!(state.admin_pending.len(), 1);
    let pending = state.admin_pending.values().next().unwrap();
    assert_eq!(
        pending.state().binding().stage,
        admin::domain::auth_state::AuthStage::PrimaryVerified
    );

    let app = verify_code_router(fixture.services.clone(), fixture.csrf(), cookies);
    let rejected = app
        .oneshot(
            Request::post("/console/sign-in/verify-code")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, "https://attacker.invalid")
                .body(Body::from("csrf_token=wrong"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
}

async fn invite_active_admin(fixture: &Fixture) {
    let id = fixture.services.ids.next_admin_id().await.unwrap();
    let identity = fixture
        .authenticator
        .identity_digest(admin::domain::secrets::SecretBytes::new(
            b"+989120000001".to_vec(),
        ))
        .await
        .unwrap();
    fixture.store.seed(|state| {
        state.admins.insert(
            id,
            AdminState::new(id, Eligibility::Active, SecurityEpoch::new(0)),
        );
        state.admin_identities.insert(*identity.as_bytes(), id);
    });
}
