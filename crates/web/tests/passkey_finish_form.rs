mod support;

use std::str::FromStr;

use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        credential_state::{CredentialKind, CredentialState},
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, CeremonyContext, PasskeyStart, TokenPurpose},
        admin_credential_repository::{CredentialMaterial, CredentialRecord},
        session_store::PendingRecord,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use kernel::ids::{AdminId, ChallengeId, CredentialId, SessionId};
use support::{CSRF, Fixture, ORIGIN};
use time::OffsetDateTime;
use tower::ServiceExt;
use web::{
    console::auth::{OriginBinding, passkey_finish_router},
    // Proof bindings are reconstructed from transport state and verified by the application service.
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
async fn passkey_finish_form_requires_current_bound_proof() {
    let fixture = Fixture::new();
    let seeded = seed_passkey(&fixture).await;
    let cookies = CookieSecurityPolicy::production();
    let cookie = format!(
        "{}={}; {}={}",
        CookieKind::PasskeyCeremony.cookie_name(cookies),
        seeded.ceremony,
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        seeded.browser
    );
    let body = format!(
        "credential={}&target={}&session={}&epoch=0&credential_revision=1&response={}&csrf_token={CSRF}&lang=en",
        seeded.credential, seeded.admin, seeded.session, seeded.response
    );
    let app = passkey_finish_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app
        .oneshot(
            Request::post("/console/sign-in/passkey/finish")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .extension(OriginBinding(BindingDigest::new([8; 32])))
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::LOCATION], "/console/admins");
    let session_cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(session_cookie.starts_with("__Host-tankhor_admin_session="));
    assert!(session_cookie.contains("HttpOnly"));
    assert!(session_cookie.contains("Secure"));
    let state = fixture.store.try_read_committed().unwrap();
    assert_eq!(state.admin_sessions.len(), 1);
    assert_eq!(state.audit_entries.len(), 1);

    let app = passkey_finish_router(fixture.services.clone(), fixture.csrf(), cookies);
    let replay = app
        .oneshot(
            Request::post("/console/sign-in/passkey/finish")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, cookie)
                .extension(OriginBinding(BindingDigest::new([8; 32])))
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(replay.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        fixture
            .store
            .try_read_committed()
            .unwrap()
            .admin_sessions
            .len(),
        1
    );
}

struct Seeded {
    admin: AdminId,
    credential: CredentialId,
    session: SessionId,
    ceremony: String,
    browser: String,
    response: String,
}

#[allow(clippy::too_many_lines)]
async fn seed_passkey(fixture: &Fixture) -> Seeded {
    let admin = AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap();
    let credential = CredentialId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap();
    let session = SessionId::from_str("019a53a0-0000-7000-8000-000000000003").unwrap();
    let record = CredentialRecord::new(
        CredentialState::new(
            credential,
            admin,
            CredentialKind::Passkey,
            SecurityEpoch::new(0),
            1,
        )
        .unwrap(),
        CredentialMaterial::Passkey {
            credential_id: SecretBytes::new(b"synthetic-credential-id".to_vec()),
            credential: SecretBytes::new(b"synthetic-public-credential".to_vec()),
        },
    )
    .unwrap();
    fixture.store.seed(|state| {
        state.admins.insert(
            admin,
            AdminState::new(admin, Eligibility::Active, SecurityEpoch::new(0)),
        );
        state.admin_credentials.insert(credential, record);
    });
    let browser_token = fixture
        .authenticator
        .issue_token(TokenPurpose::Browser)
        .await
        .unwrap();
    let browser = fixture
        .authenticator
        .token_digest(TokenPurpose::Browser, browser_token.clone())
        .await
        .unwrap();
    let now = OffsetDateTime::UNIX_EPOCH;
    let binding = AuthBinding {
        target: admin,
        browser,
        session,
        purpose: AuthPurpose::SignIn,
        stage: AuthStage::Passkey,
        epoch: SecurityEpoch::new(0),
        credential_revision: Some(1),
    };
    let auth = AuthState::try_from(AuthInput {
        id: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000004").unwrap(),
        binding,
        primary_verified_at: Some(now),
        created_at: now,
        expires_at: now + time::Duration::seconds(60),
    })
    .unwrap();
    let request = PasskeyStart {
        context: CeremonyContext {
            challenge: auth.id(),
            binding: auth.binding().clone(),
            credential: Some(credential),
            primary_verified_at: now,
            created_at: now,
            expires_at: auth.expires_at(),
        },
        rp_id: "console.example.test".into(),
        origin: "https://console.example.test".into(),
        credentials: vec![SecretBytes::new(b"synthetic-public-credential".to_vec())],
    };
    let challenge = fixture
        .authenticator
        .begin_passkey(request.clone())
        .await
        .unwrap();
    let response = fixture
        .authenticator
        .passkey_fixture(&request, &challenge)
        .unwrap();
    let token = fixture
        .authenticator
        .issue_token(TokenPurpose::Ceremony)
        .await
        .unwrap();
    let reference = fixture
        .authenticator
        .token_digest(TokenPurpose::Ceremony, token.clone())
        .await
        .unwrap();
    let pending =
        PendingRecord::new(reference, auth, Some(challenge.protected_state), None).unwrap();
    fixture.store.seed(|state| {
        state
            .admin_pending
            .insert(*pending.reference().as_bytes(), pending);
    });
    Seeded {
        admin,
        credential,
        session,
        ceremony: String::from_utf8(token.expose().to_vec()).unwrap(),
        browser: String::from_utf8(browser_token.expose().to_vec()).unwrap(),
        response: String::from_utf8(response.expose().to_vec()).unwrap(),
    }
}
