mod support;

use admin::{
    domain::{
        admin_state::{Eligibility, EnrollmentPurpose},
        auth_state::AuthStage,
        credential_state::CredentialKind,
    },
    ports::admin_authenticator::{
        AdminAuthenticator, CeremonyContext, PasskeyChallenge, PasskeyStart, TokenPurpose,
    },
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use kernel::ids::CredentialId;
use support::{CSRF, Fixture, ORIGIN, seed_enrollment};
use tower::ServiceExt;
use web::{
    console::enrollment::{enroll_passkey_begin_router, enroll_passkey_finish_router},
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn enroll_passkey_finish_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    let enrollment = seed_enrollment(&fixture, EnrollmentPurpose::Addition).await;
    let cookies = CookieSecurityPolicy::production();
    let primary_cookie = format!(
        "{}={}; {}={}",
        CookieKind::PrimaryProof.cookie_name(cookies),
        enrollment.proof,
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        enrollment.browser,
    );
    let begin_form = format!(
        "target={}&session={}&epoch=0&purpose=addition&csrf_token={CSRF}&lang=en",
        enrollment.target, enrollment.session,
    );
    let begin = enroll_passkey_begin_router(fixture.services.clone(), fixture.csrf(), cookies)
        .oneshot(
            Request::post("/console/enrol/passkey/begin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, primary_cookie)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(begin_form))
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
    let html = to_bytes(begin.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(html.to_vec()).unwrap();
    let credential = hidden_value(&html, "credential")
        .parse::<CredentialId>()
        .unwrap();
    let state = fixture.store.try_read_committed().unwrap();
    let pending = state
        .admin_pending
        .values()
        .find(|pending| pending.state().binding().stage == AuthStage::Passkey)
        .unwrap()
        .clone();
    drop(state);
    let reference = fixture
        .authenticator
        .token_digest(
            TokenPurpose::Ceremony,
            admin::domain::secrets::SecretBytes::new(ceremony.as_bytes().to_vec()),
        )
        .await
        .unwrap();
    assert_eq!(&reference, pending.reference());
    let auth = pending.state();
    let request = PasskeyStart {
        context: CeremonyContext {
            challenge: auth.id(),
            binding: auth.binding().clone(),
            credential: Some(credential),
            primary_verified_at: auth.primary_verified_at(),
            created_at: auth.created_at(),
            expires_at: auth.expires_at(),
        },
        rp_id: "console.example.test".into(),
        origin: "https://console.example.test".into(),
        credentials: Vec::new(),
    };
    let public_options = html
        .split("<output id=\"passkey-options\" data-kind=\"public-options\">")
        .nth(1)
        .unwrap()
        .split("</output>")
        .next()
        .unwrap();
    let challenge = PasskeyChallenge {
        public_options: admin::domain::secrets::SecretBytes::new(
            public_options.as_bytes().to_vec(),
        ),
        protected_state: pending.verification_envelope().unwrap().clone(),
    };
    let response = fixture
        .authenticator
        .passkey_fixture(&request, &challenge)
        .unwrap();
    let response = String::from_utf8(response.expose().to_vec()).unwrap();
    let cookie = format!(
        "{}={ceremony}; {}={}",
        CookieKind::PasskeyCeremony.cookie_name(cookies),
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        enrollment.browser,
    );
    let form = format!(
        "credential={credential}&target={}&session={}&epoch=0&credential_revision=1&purpose=addition&response={response}&csrf_token={CSRF}&lang=en",
        enrollment.target, enrollment.session,
    );
    let app = enroll_passkey_finish_router(fixture.services.clone(), fixture.csrf(), cookies);
    let completed = app
        .clone()
        .oneshot(
            Request::post("/console/enrol/passkey/finish")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(form.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::SEE_OTHER);
    assert_eq!(completed.headers()[header::CACHE_CONTROL], "no-store");
    assert!(
        completed.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("__Host-tankhor_admin_session=")
    );
    let state = fixture.store.try_read_committed().unwrap();
    assert_eq!(
        state.admins.get(&enrollment.target).unwrap().eligibility(),
        Eligibility::Active
    );
    assert_eq!(
        state
            .admin_credentials
            .get(&credential)
            .unwrap()
            .state()
            .kind(),
        CredentialKind::Passkey
    );
    assert_eq!(state.admin_sessions.len(), 1);
    drop(state);

    let replay = app
        .oneshot(
            Request::post("/console/enrol/passkey/finish")
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
}

fn hidden_value(html: &str, name: &str) -> String {
    let marker = format!("name=\"{name}\" value=\"");
    let rest = html.split(&marker).nth(1).unwrap();
    rest.split('"').next().unwrap().to_owned()
}
