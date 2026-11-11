use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        credential_state::{CredentialKind, CredentialState},
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::{CeremonyContext, PasskeyStart},
        admin_credential_repository::{CredentialMaterial, CredentialRecord},
        session_store::{PendingCheck, PendingRecord},
    },
};
use kernel::ids::CredentialId;
use time::{Duration, OffsetDateTime};

pub fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
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
pub fn secret(value: &str) -> SecretBytes {
    SecretBytes::new(value.as_bytes().to_vec())
}
pub fn credential() -> CredentialRecord {
    CredentialRecord::new(
        CredentialState::new(
            credential_id(),
            admin().id(),
            CredentialKind::Passkey,
            admin().epoch(),
            1,
        )
        .unwrap(),
        CredentialMaterial::Passkey {
            credential_id: secret("synthetic-credential-id"),
            credential: secret("synthetic-public-credential"),
        },
    )
    .unwrap()
}
pub fn binding(stage: AuthStage) -> AuthBinding {
    AuthBinding {
        target: admin().id(),
        browser: BindingDigest::new([3; 32]),
        session: "019a53a0-0000-7000-8000-000000000004".parse().unwrap(),
        purpose: AuthPurpose::SignIn,
        stage,
        epoch: admin().epoch(),
        credential_revision: (stage == AuthStage::Passkey).then_some(1),
    }
}
pub fn pending(stage: AuthStage) -> PendingRecord {
    PendingRecord::new(
        BindingDigest::new([5; 32]),
        AuthState::try_from(AuthInput {
            id: "019a53a0-0000-7000-8000-000000000006".parse().unwrap(),
            binding: binding(stage),
            primary_verified_at: Some(now()),
            created_at: now(),
            expires_at: now() + Duration::MINUTE,
        })
        .unwrap(),
        (stage == AuthStage::Passkey).then(|| secret("synthetic-server-state")),
        None,
    )
    .unwrap()
}
pub fn check(stage: AuthStage) -> PendingCheck {
    PendingCheck {
        reference: BindingDigest::new([5; 32]),
        binding: binding(stage),
        now: now(),
    }
}
pub fn start() -> PasskeyStart {
    PasskeyStart {
        context: CeremonyContext {
            challenge: pending(AuthStage::Passkey).state().id(),
            binding: binding(AuthStage::Passkey),
            credential: Some(credential_id()),
            primary_verified_at: now(),
            created_at: now(),
            expires_at: now() + Duration::MINUTE,
        },
        rp_id: "console.example.test".into(),
        origin: "https://console.example.test".into(),
        credentials: vec![secret("synthetic-public-credential")],
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
