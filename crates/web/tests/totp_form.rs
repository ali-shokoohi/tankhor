mod support;

use std::str::FromStr;

use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        credential_state::{CredentialKind, CredentialState},
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, CeremonyContext, TokenPurpose},
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
use time::Duration;
use tower::ServiceExt;
use web::{
    console::auth::{OriginBinding, totp_router},
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn totp_form_requires_primary_and_second_factor() {
    let fixture = Fixture::new();
    let admin = AdminId::from_str("019a53a0-0000-7000-8000-000000000011").unwrap();
    let credential = CredentialId::from_str("019a53a0-0000-7000-8000-000000000012").unwrap();
    let session = SessionId::from_str("019a53a0-0000-7000-8000-000000000013").unwrap();
    fixture.store.seed(|state| {
        state.admins.insert(
            admin,
            AdminState::new(admin, Eligibility::Active, SecurityEpoch::new(0)),
        );
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
    let now = time::macros::datetime!(2026-01-01 00:00 UTC);
    fixture.clock.set(now);
    let auth = AuthState::try_from(AuthInput {
        id: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000014").unwrap(),
        binding: AuthBinding {
            target: admin,
            browser,
            session,
            purpose: AuthPurpose::SignIn,
            stage: AuthStage::PrimaryVerified,
            epoch: SecurityEpoch::new(0),
            credential_revision: Some(1),
        },
        primary_verified_at: Some(now),
        created_at: now,
        expires_at: now + Duration::seconds(60),
    })
    .unwrap();
    let mut totp_binding = auth.binding().clone();
    totp_binding.stage = AuthStage::Totp;
    let enrollment = fixture
        .authenticator
        .begin_totp(CeremonyContext {
            challenge: auth.id(),
            binding: totp_binding,
            credential: Some(credential),
            primary_verified_at: now,
            created_at: now,
            expires_at: auth.expires_at(),
        })
        .await
        .unwrap();
    let code = fixture
        .authenticator
        .totp_fixture(&enrollment.envelope, now)
        .unwrap()
        .0;
    let record = CredentialRecord::new(
        CredentialState::new(
            credential,
            admin,
            CredentialKind::Totp,
            SecurityEpoch::new(0),
            1,
        )
        .unwrap(),
        CredentialMaterial::Totp {
            envelope: enrollment.envelope,
        },
    )
    .unwrap();
    fixture.store.seed(|state| {
        state.admin_credentials.insert(credential, record);
    });
    let proof = fixture
        .authenticator
        .issue_token(TokenPurpose::PrimaryProof)
        .await
        .unwrap();
    let reference = fixture
        .authenticator
        .token_digest(TokenPurpose::PrimaryProof, proof.clone())
        .await
        .unwrap();
    let pending = PendingRecord::new(reference, auth, None, None).unwrap();
    fixture.store.seed(|state| {
        state
            .admin_pending
            .insert(*pending.reference().as_bytes(), pending);
    });
    let cookies = CookieSecurityPolicy::production();
    let cookie = format!(
        "{}={}; {}={}",
        CookieKind::PrimaryProof.cookie_name(cookies),
        String::from_utf8(proof.expose().to_vec()).unwrap(),
        CookieKind::PrimaryCeremony.cookie_name(cookies),
        String::from_utf8(browser_token.expose().to_vec()).unwrap()
    );
    let body = format!(
        "credential={credential}&target={admin}&session={session}&epoch=0&credential_revision=1&code={}&csrf_token={CSRF}&lang=en",
        String::from_utf8(code.expose().to_vec()).unwrap()
    );
    let app = totp_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app
        .oneshot(
            Request::post("/console/sign-in/totp")
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
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("__Host-tankhor_admin_session=")
    );
    let state = fixture.store.try_read_committed().unwrap();
    assert_eq!(state.admin_sessions.len(), 1);
    assert_eq!(state.audit_entries.len(), 1);

    let replay = totp_router(fixture.services.clone(), fixture.csrf(), cookies)
        .oneshot(
            Request::post("/console/sign-in/totp")
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
