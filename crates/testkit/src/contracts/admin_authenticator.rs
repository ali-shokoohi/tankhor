//! FR-002/024, Q23: local valid/invalid proofs; no live crypto/SMS service.

use admin::{
    domain::{auth_state::BindingDigest, secrets::SecretBytes},
    ports::{
        admin_authenticator::{
            AdminAuthenticator, CeremonyContext, PasskeyChallenge, PasskeyFinish, PasskeyStart,
            ProtectionContext, TokenPurpose, TotpCheck, VerifiedPasskey,
        },
        admin_repository::IdentityDigest,
    },
};
use kernel::{
    error::ErrorCode,
    ids::{AdminId, ChallengeId, CredentialId, SessionId},
};
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

pub trait AdminAuthenticatorContractHarness: Sync {
    type Authenticator: AdminAuthenticator;
    fn authenticator(&self) -> &Self::Authenticator;
    /// Valid signed local fixture for the challenge returned by `begin_passkey`.
    fn passkey_response(&self, request: &PasskeyStart, challenge: &PasskeyChallenge)
    -> SecretBytes;
    fn passkey_credentials(&self) -> Vec<SecretBytes>;
    fn verified_passkey(&self) -> VerifiedPasskey;
    /// Local fixture code and matched step for the generated protected TOTP secret.
    fn totp_proof(&self, envelope: &SecretBytes, now: OffsetDateTime) -> (SecretBytes, u64);
}

#[must_use]
/// # Panics
/// Panics if a fixed test ID is malformed.
pub fn context() -> CeremonyContext {
    use admin::domain::{
        admin_state::SecurityEpoch,
        auth_state::{AuthBinding, AuthPurpose, AuthStage},
    };
    CeremonyContext {
        challenge: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
        binding: AuthBinding {
            target: AdminId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap(),
            browser: BindingDigest::new([3; 32]),
            session: SessionId::from_str("019a53a0-0000-7000-8000-000000000004").unwrap(),
            purpose: AuthPurpose::SignIn,
            stage: AuthStage::Passkey,
            epoch: SecurityEpoch::new(1),
            credential_revision: Some(1),
        },
        credential: Some(CredentialId::from_str("019a53a0-0000-7000-8000-000000000005").unwrap()),
        primary_verified_at: time::macros::datetime!(2026-01-01 00:00 UTC),
        created_at: time::macros::datetime!(2026-01-01 00:00 UTC),
        expires_at: time::macros::datetime!(2026-01-01 00:01 UTC),
    }
}

fn bytes(text: &str) -> SecretBytes {
    SecretBytes::new(text.as_bytes().to_vec())
}

/// # Panics
/// Panics on accepted invalid/rebound proof, missing matched step or secret leakage.
#[allow(clippy::too_many_lines)]
pub async fn admin_authenticator_contract<H: AdminAuthenticatorContractHarness>(h: &H) {
    use admin::domain::{
        admin_state::{EnrollmentPurpose, SecurityEpoch},
        auth_state::{AuthPurpose, AuthStage},
    };
    let port = h.authenticator();
    let context = context();
    let phone = bytes("+989120000001");
    let identity = port.identity_digest(phone.clone()).await.unwrap();
    assert_eq!(port.identity_digest(phone.clone()).await.unwrap(), identity);
    assert_ne!(
        identity,
        port.identity_digest(bytes("+989120000002")).await.unwrap()
    );
    let otp = port.issue_otp(context.challenge).await.unwrap();
    assert_eq!(otp.code.expose().len(), 6);
    assert!(otp.code.expose().iter().all(u8::is_ascii_digit));
    assert!(
        port.verify_otp(context.challenge, otp.code.clone(), otp.digest.clone())
            .await
            .unwrap()
    );
    assert!(
        !port
            .verify_otp(context.challenge, bytes("bad-otp"), otp.digest.clone())
            .await
            .unwrap()
    );
    let other = ChallengeId::from_str("019a53a0-0000-7000-8000-000000000099").unwrap();
    assert!(
        !port
            .verify_otp(other, otp.code.clone(), otp.digest.clone())
            .await
            .unwrap()
    );
    let session = port.issue_token(TokenPurpose::Session).await.unwrap();
    let csrf = port.issue_token(TokenPurpose::Csrf).await.unwrap();
    assert!(!session.expose().is_empty());
    assert_ne!(session, csrf);
    let digest = port
        .token_digest(TokenPurpose::Session, session.clone())
        .await
        .unwrap();
    assert_eq!(
        port.token_digest(TokenPurpose::Session, session.clone())
            .await
            .unwrap(),
        digest
    );
    assert_ne!(
        port.token_digest(TokenPurpose::Csrf, session.clone())
            .await
            .unwrap(),
        digest
    );

    let protection = ProtectionContext::Ceremony(context.clone());
    let plaintext = bytes("synthetic-envelope-plaintext");
    let envelope = port
        .protect(protection.clone(), plaintext.clone())
        .await
        .unwrap();
    assert_ne!(envelope, plaintext);
    assert_eq!(
        port.unprotect(protection.clone(), envelope.clone())
            .await
            .unwrap(),
        plaintext
    );
    let mut wrong = context.clone();
    wrong.binding.epoch = SecurityEpoch::new(2);
    assert_eq!(
        port.unprotect(ProtectionContext::Ceremony(wrong), envelope.clone())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::AuthenticationRequired
    );
    assert!(
        port.unprotect(protection, bytes("tampered-envelope"))
            .await
            .is_err()
    );

    let request = PasskeyStart {
        context: context.clone(),
        rp_id: "console.example.test".into(),
        origin: "https://console.example.test".into(),
        credentials: h.passkey_credentials(),
    };
    let challenge = port.begin_passkey(request.clone()).await.unwrap();
    assert!(!challenge.public_options.expose().is_empty());
    assert!(!challenge.protected_state.expose().is_empty());
    assert_ne!(challenge.public_options, challenge.protected_state);
    let proof = PasskeyFinish {
        request,
        protected_state: challenge.protected_state.clone(),
        response: h.passkey_response(
            &PasskeyStart {
                context: context.clone(),
                rp_id: "console.example.test".into(),
                origin: "https://console.example.test".into(),
                credentials: h.passkey_credentials(),
            },
            &challenge,
        ),
        now: context.created_at,
    };
    assert_eq!(
        port.finish_passkey(proof.clone()).await.unwrap(),
        h.verified_passkey()
    );
    let mut bad = proof.clone();
    bad.response = bytes("invalid-passkey-response");
    assert_eq!(
        port.finish_passkey(bad).await.unwrap_err().code(),
        ErrorCode::AuthenticationRequired
    );
    let mut invalid = Vec::new();
    let mut bad = proof.clone();
    bad.request.origin = "https://evil.example.test".into();
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.rp_id = "evil.example.test".into();
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.challenge = other;
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.binding.browser = BindingDigest::new([9; 32]);
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.binding.target =
        AdminId::from_str("019a53a0-0000-7000-8000-000000000009").unwrap();
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.binding.session =
        SessionId::from_str("019a53a0-0000-7000-8000-000000000009").unwrap();
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery);
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.binding.epoch = SecurityEpoch::new(2);
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.binding.credential_revision = Some(2);
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.primary_verified_at -= Duration::SECOND;
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.request.context.credential = None;
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.now = context.expires_at;
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.now = context.created_at - Duration::SECOND;
    invalid.push(bad);
    let mut bad = proof.clone();
    bad.protected_state = bytes("tampered-server-state");
    invalid.push(bad);
    for bad in invalid {
        assert_eq!(
            port.finish_passkey(bad).await.unwrap_err().code(),
            ErrorCode::AuthenticationRequired
        );
    }

    let mut totp_context = context.clone();
    totp_context.binding.stage = AuthStage::Totp;
    totp_context.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery);
    let enrollment = port.begin_totp(totp_context.clone()).await.unwrap();
    assert!(!enrollment.envelope.expose().is_empty());
    assert!(!enrollment.protected_provisioning.expose().is_empty());
    let provisioning = port
        .unprotect(
            ProtectionContext::Ceremony(totp_context.clone()),
            enrollment.protected_provisioning.clone(),
        )
        .await
        .unwrap();
    assert!(!provisioning.expose().is_empty());
    let mut foreign_presentation = totp_context.clone();
    foreign_presentation.binding.browser = BindingDigest::new([91; 32]);
    assert!(
        port.unprotect(
            ProtectionContext::Ceremony(foreign_presentation),
            enrollment.protected_provisioning.clone(),
        )
        .await
        .is_err()
    );
    let (code, step) = h.totp_proof(&enrollment.envelope, context.created_at);
    let check = TotpCheck {
        context: totp_context,
        envelope: enrollment.envelope.clone(),
        code,
        now: context.created_at,
    };
    assert_eq!(port.verify_totp(check.clone()).await.unwrap(), step);
    // Replay CAS belongs to the application/repository, not this stateless verifier.
    assert_eq!(port.verify_totp(check.clone()).await.unwrap(), step);
    // Persistent credentials survive a separate, independently authorized ceremony.
    let mut signin = check.clone();
    signin.context.binding.purpose = AuthPurpose::SignIn;
    signin.context.binding.browser = BindingDigest::new([71; 32]);
    signin.context.binding.session =
        SessionId::from_str("019a53a0-0000-7000-8000-000000000071").unwrap();
    signin.context.challenge =
        ChallengeId::from_str("019a53a0-0000-7000-8000-000000000072").unwrap();
    assert_eq!(port.verify_totp(signin.clone()).await.unwrap(), step);
    let mut foreign = signin.clone();
    foreign.context.binding.target =
        AdminId::from_str("019a53a0-0000-7000-8000-000000000073").unwrap();
    assert_eq!(
        port.verify_totp(foreign).await.unwrap_err().code(),
        ErrorCode::AuthenticationRequired
    );
    let mut foreign = signin;
    foreign.context.credential =
        Some(CredentialId::from_str("019a53a0-0000-7000-8000-000000000074").unwrap());
    assert_eq!(
        port.verify_totp(foreign).await.unwrap_err().code(),
        ErrorCode::AuthenticationRequired
    );
    let mut bad = check.clone();
    bad.code = bytes("bad-totp");
    assert_eq!(
        port.verify_totp(bad).await.unwrap_err().code(),
        ErrorCode::AuthenticationRequired
    );
    let mut bad = check.clone();
    bad.envelope = bytes("wrong-envelope");
    assert_eq!(
        port.verify_totp(bad).await.unwrap_err().code(),
        ErrorCode::AuthenticationRequired
    );
    let mut bad = check.clone();
    bad.context.binding.epoch = SecurityEpoch::new(2);
    assert_eq!(
        port.verify_totp(bad).await.unwrap_err().code(),
        ErrorCode::AuthenticationRequired
    );
    let mut bad = check.clone();
    bad.now = context.expires_at;
    assert_eq!(
        port.verify_totp(bad).await.unwrap_err().code(),
        ErrorCode::AuthenticationRequired
    );

    for debug in [
        format!("{phone:?}"),
        format!("{identity:?}"),
        format!("{otp:?}"),
        format!("{session:?}"),
        format!("{csrf:?}"),
        format!("{digest:?}"),
        format!("{envelope:?}"),
        format!("{proof:?}"),
        format!("{enrollment:?}"),
        format!("{check:?}"),
    ] {
        for secret in [&phone, &otp.code, &session, &csrf, &plaintext, &check.code] {
            assert!(!debug.contains(std::str::from_utf8(secret.expose()).unwrap()));
        }
    }
    // Errors are closed sanitized categories, not raw vendor/crypto messages.
    let error = port
        .finish_passkey(PasskeyFinish {
            response: bytes("private-invalid-proof"),
            ..proof
        })
        .await
        .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("private-invalid-proof"));
    let _: IdentityDigest = identity;
}
