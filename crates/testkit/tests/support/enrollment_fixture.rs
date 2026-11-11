use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, EnrollmentPurpose, SecurityEpoch},
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        secrets::SecretBytes,
        security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput},
    },
    ports::session_store::{PendingCheck, PendingRecord},
};
use time::{Duration, OffsetDateTime};

pub fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}
pub fn secret(value: &str) -> SecretBytes {
    SecretBytes::new(value.as_bytes().to_vec())
}
pub fn target(authorized: bool) -> AdminState {
    target_until(authorized, now() + Duration::minutes(5))
}
pub fn target_until(authorized: bool, expires_at: OffsetDateTime) -> AdminState {
    let mut target = AdminState::new(
        "019a53a0-0000-7000-8000-000000000001".parse().unwrap(),
        Eligibility::Invited,
        SecurityEpoch::new(0),
    );
    if authorized {
        let actor = AdminState::new(
            "019a53a0-0000-7000-8000-000000000002".parse().unwrap(),
            Eligibility::Active,
            SecurityEpoch::new(0),
        );
        target
            .authorize_enrollment(
                &actor,
                EnrollmentPurpose::Addition,
                target.epoch(),
                now(),
                expires_at,
            )
            .unwrap();
    }
    target
}
pub fn check() -> PendingCheck {
    PendingCheck {
        reference: BindingDigest::new([5; 32]),
        binding: AuthBinding {
            target: target(false).id(),
            browser: BindingDigest::new([3; 32]),
            session: "019a53a0-0000-7000-8000-000000000004".parse().unwrap(),
            purpose: AuthPurpose::Enrollment(EnrollmentPurpose::Addition),
            stage: AuthStage::PrimaryVerified,
            epoch: SecurityEpoch::new(0),
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
pub fn policy() -> SecurityPolicy {
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
