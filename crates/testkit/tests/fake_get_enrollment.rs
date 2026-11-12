//! T197: safe enrollment choices require current authorization and an exact primary proof.
use admin::{
    application::{
        get_enrollment::{EnrollmentFactor, GetEnrollment, GetEnrollmentInput},
        verify_admin_otp::PrimaryProof,
    },
    domain::{
        admin_state::EnrollmentPurpose,
        auth_state::{AuthInput, AuthPurpose, AuthStage, AuthState},
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        session_store::PendingRecord,
    },
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    clock::FixedClock, session_store::FakeSessionStore, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
#[path = "support/enrollment_fixture.rs"]
#[allow(dead_code)]
mod fixture;

#[tokio::test]
async fn get_enrollment_refuses_invalid_state_and_returns_safe_choices() {
    for scenario in [
        "valid",
        "wrong_stage",
        "wrong_session",
        "wrong_browser",
        "wrong_purpose",
        "wrong_target",
        "stale_epoch",
        "expired",
        "revoked",
        "consumed",
        "missing",
    ] {
        drive(scenario).await;
    }
}

#[expect(clippy::too_many_lines)]
async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let now = if scenario == "expired" {
        fixture::now() + time::Duration::MINUTE
    } else {
        fixture::now()
    };
    let clock = FixedClock::new(now);
    let proof = crypto
        .issue_token(TokenPurpose::PrimaryProof)
        .await
        .unwrap();
    let browser = crypto.issue_token(TokenPurpose::Browser).await.unwrap();
    let reference = crypto
        .token_digest(TokenPurpose::PrimaryProof, proof.clone())
        .await
        .unwrap();
    let browser_digest = crypto
        .token_digest(TokenPurpose::Browser, browser.clone())
        .await
        .unwrap();
    let mut check = fixture::check();
    check.reference = reference.clone();
    check.binding.browser = browser_digest;
    let mut target = fixture::target(true);
    if scenario == "consumed" {
        target
            .consume_enrollment(
                target.id(),
                EnrollmentPurpose::Addition,
                target.epoch(),
                fixture::now(),
            )
            .unwrap();
    }
    let expires_at = fixture::now() + time::Duration::MINUTE;
    let mut pending = PendingRecord::new(
        reference.clone(),
        AuthState::try_from(AuthInput {
            id: "019a53a0-0000-7000-8000-000000000006".parse().unwrap(),
            binding: check.binding.clone(),
            primary_verified_at: Some(fixture::now()),
            created_at: fixture::now(),
            expires_at,
        })
        .unwrap(),
        None,
        None,
    )
    .unwrap();
    if scenario == "revoked" {
        pending.revoke();
    }
    if scenario != "missing" {
        store.seed(|state| {
            state.admins.insert(target.id(), target.clone());
            state
                .admin_pending
                .insert(*reference.as_bytes(), pending.clone());
        });
    }
    let before = store.try_read_committed().unwrap();
    let mut binding = check.binding.clone();
    match scenario {
        "wrong_stage" => binding.stage = AuthStage::Passkey,
        "wrong_session" => {
            binding.session = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
        }
        "wrong_browser" => binding.browser = fixture::check().binding.browser,
        "wrong_purpose" => binding.purpose = AuthPurpose::SignIn,
        "wrong_target" => {
            binding.target = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
        }
        "stale_epoch" => binding.epoch = admin::domain::admin_state::SecurityEpoch::new(1),
        _ => {}
    }
    let input = GetEnrollmentInput {
        proof: PrimaryProof {
            token: proof.clone(),
            binding,
        },
        browser: browser.clone(),
    };
    let result = GetEnrollment {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        authenticator: &crypto,
        clock: &clock,
    }
    .execute(input)
    .await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    if scenario == "valid" {
        let view = result.unwrap();
        assert_eq!(view.target, target.id());
        assert_eq!(
            view.factors,
            [EnrollmentFactor::Passkey, EnrollmentFactor::Totp]
        );
        let debug = format!("{view:?}");
        for secret in [proof, browser, fixture::secret("synthetic-form-secret")] {
            assert!(!debug.contains(&format!("{secret:?}")));
        }
    }
    let after = store.try_read_committed().unwrap();
    assert_eq!(after.admins, before.admins);
    assert_eq!(after.admin_pending, before.admin_pending);
    assert_eq!(after.admin_sessions, before.admin_sessions);
    assert_eq!(after.admin_credentials, before.admin_credentials);
    assert_eq!(after.audit_entries, before.audit_entries);
}
