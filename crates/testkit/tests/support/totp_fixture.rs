use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        credential_state::{CredentialKind, CredentialState},
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::CeremonyContext,
        admin_credential_repository::{CredentialMaterial, CredentialRecord},
        session_store::{PendingCheck, PendingRecord},
    },
};
use kernel::ids::CredentialId;
use time::{Duration, OffsetDateTime};

pub fn now() -> OffsetDateTime {
    time::macros::datetime!(2026-01-01 00:00 UTC)
}
pub fn secret(value: &str) -> SecretBytes {
    SecretBytes::new(value.as_bytes().to_vec())
}
pub fn admin() -> AdminState {
    AdminState::new(
        "019a53a0-0000-7000-8000-000000000001".parse().unwrap(),
        Eligibility::Active,
        SecurityEpoch::new(0),
    )
}
pub fn credential_id() -> CredentialId {
    "019a53a0-0000-7000-8000-000000000002".parse().unwrap()
}
pub fn credential(envelope: SecretBytes) -> CredentialRecord {
    CredentialRecord::new(
        CredentialState::new(
            credential_id(),
            admin().id(),
            CredentialKind::Totp,
            admin().epoch(),
            1,
        )
        .unwrap(),
        CredentialMaterial::Totp { envelope },
    )
    .unwrap()
}
pub fn check() -> PendingCheck {
    PendingCheck {
        reference: BindingDigest::new([5; 32]),
        binding: AuthBinding {
            target: admin().id(),
            browser: BindingDigest::new([3; 32]),
            session: "019a53a0-0000-7000-8000-000000000004".parse().unwrap(),
            purpose: AuthPurpose::SignIn,
            stage: AuthStage::PrimaryVerified,
            epoch: admin().epoch(),
            credential_revision: None,
        },
        now: now(),
    }
}
pub fn pending() -> PendingRecord {
    PendingRecord::new(
        check().reference,
        AuthState::try_from(AuthInput {
            id: "019a53a0-0000-7000-8000-000000000006".parse().unwrap(),
            binding: check().binding,
            primary_verified_at: Some(now()),
            created_at: now(),
            expires_at: now() + Duration::MINUTE,
        })
        .unwrap(),
        None,
        None,
    )
    .unwrap()
}
pub fn context() -> CeremonyContext {
    let mut binding = check().binding;
    binding.stage = AuthStage::Totp;
    binding.credential_revision = Some(1);
    CeremonyContext {
        challenge: pending().state().id(),
        binding,
        credential: Some(credential_id()),
        primary_verified_at: now(),
        created_at: now(),
        expires_at: now() + Duration::MINUTE,
    }
}
pub fn policy() -> admin::domain::security_policy::SecurityPolicy {
    use admin::domain::security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput};
    SecurityPolicy::try_from(SecurityPolicyInput {
        otp_ttl_seconds: Some(60),
        primary_proof_ttl_seconds: Some(60),
        ceremony_ttl_seconds: Some(60),
        enrollment_ttl_seconds: Some(300),
        session_idle_ttl_seconds: Some(60),
        session_absolute_ttl_seconds: Some(300),
        otp_max_attempts: Some(3),
        subject_rate_limit: Some(RateLimitInput {
            max_attempts: Some(3),
            window_seconds: Some(300),
        }),
        origin_rate_limit: Some(RateLimitInput {
            max_attempts: Some(5),
            window_seconds: Some(300),
        }),
        rp_id: Some("console.example.test".into()),
        origin: Some("https://console.example.test".into()),
    })
    .unwrap()
}
