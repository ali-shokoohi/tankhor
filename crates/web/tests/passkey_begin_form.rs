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
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        admin_credential_repository::{CredentialMaterial, CredentialRecord},
        session_store::PendingRecord,
    },
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use kernel::ids::{AdminId, CredentialId, SessionId};
use support::{CSRF, Fixture, ORIGIN};
use time::OffsetDateTime;
use tower::ServiceExt;
use web::{
    console::auth::{OriginBinding, passkey_begin_router},
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
async fn passkey_begin_form_escapes_public_options_and_never_exposes_server_state() {
    let fixture = Fixture::new();
    let (proof, browser, credential) = seed_primary(&fixture).await;
    let cookies = CookieSecurityPolicy::production();
    let app = passkey_begin_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app
        .oneshot(
            Request::post("/console/sign-in/passkey/begin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!(
                        "{}={proof}; {}={browser}",
                        CookieKind::PrimaryProof.cookie_name(cookies),
                        CookieKind::PrimaryCeremony.cookie_name(cookies)
                    ),
                )
                .extension(OriginBinding(BindingDigest::new([8; 32])))
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "credential={credential}&target=019a53a0-0000-7000-8000-000000000001&session=019a53a0-0000-7000-8000-000000000003&epoch=0&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(response.headers().contains_key(header::SET_COOKIE));
    let body = String::from_utf8(
        to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("public-options"));
    assert!(!body.contains("server-state"));
    assert!(!body.contains("protected_state"));
    assert!(!body.contains("<script>"));
    let state = fixture.store.try_read_committed().unwrap();
    assert!(state.admin_sessions.is_empty());
    assert!(
        state
            .admin_pending
            .values()
            .any(|pending| pending.state().binding().stage == AuthStage::Passkey)
    );
}

async fn seed_primary(fixture: &Fixture) -> (String, String, CredentialId) {
    let admin = AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap();
    let credential = CredentialId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap();
    let identity = fixture
        .authenticator
        .identity_digest(SecretBytes::new(b"+989120000001".to_vec()))
        .await
        .unwrap();
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
        state.admin_identities.insert(*identity.as_bytes(), admin);
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
    let proof_token = fixture
        .authenticator
        .issue_token(TokenPurpose::PrimaryProof)
        .await
        .unwrap();
    let reference = fixture
        .authenticator
        .token_digest(TokenPurpose::PrimaryProof, proof_token.clone())
        .await
        .unwrap();
    let binding = AuthBinding {
        target: admin,
        browser,
        session: SessionId::from_str("019a53a0-0000-7000-8000-000000000003").unwrap(),
        purpose: AuthPurpose::SignIn,
        stage: AuthStage::PrimaryVerified,
        epoch: SecurityEpoch::new(0),
        credential_revision: None,
    };
    let now = OffsetDateTime::UNIX_EPOCH;
    let auth = AuthState::try_from(AuthInput {
        id: kernel::ids::ChallengeId::from_str("019a53a0-0000-7000-8000-000000000004").unwrap(),
        binding,
        primary_verified_at: Some(now),
        created_at: now,
        expires_at: now + time::Duration::seconds(60),
    })
    .unwrap();
    let pending = PendingRecord::new(reference, auth, None, None).unwrap();
    fixture.store.seed(|state| {
        state
            .admin_pending
            .insert(*pending.reference().as_bytes(), pending);
    });
    (
        String::from_utf8(proof_token.expose().to_vec()).unwrap(),
        String::from_utf8(browser_token.expose().to_vec()).unwrap(),
        credential,
    )
}
