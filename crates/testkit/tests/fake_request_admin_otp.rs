//! T152: FR-002, FR-CON-02, AC-ADM-002, Q09.
use admin::{
    application::request_admin_otp::{RequestAdminOtp, RequestOtp},
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthPurpose, BindingDigest},
        secrets::SecretBytes,
        security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput},
    },
    ports::otp_challenge_repository::{OtpChallengeRepository, OtpCheck, SubmissionCategory},
};
use kernel::{
    ids::AdminId,
    ports::unit_of_work::{TransactionMode, UnitOfWork},
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    clock::FixedClock, id_generator::SequenceIdGenerator,
    otp_challenge_repository::FakeOtpChallengeRepository,
    rate_limit_repository::FakeRateLimitRepository, sms_sender::FakeSmsSender, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
use time::OffsetDateTime;

fn policy() -> SecurityPolicy {
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
        rp_id: Some("tankhor.test".into()),
        origin: Some("https://tankhor.test".into()),
    })
    .unwrap()
}

fn request() -> RequestOtp {
    RequestOtp {
        phone: SecretBytes::new("۰۹۱۲۰۰۰۰۰۰۱".as_bytes().to_vec()),
        browser: BindingDigest::new([7; 32]),
        origin: BindingDigest::new([8; 32]),
        purpose: AuthPurpose::SignIn,
        predecessor: None,
    }
}

#[tokio::test]
async fn sms_acceptance_is_not_primary_proof() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let store = FakeStore::new();
    let target: AdminId = "019a53a0-0000-7000-8000-000000000001".parse().unwrap();
    store.seed(|state| {
        state.admins.insert(
            target,
            AdminState::new(target, Eligibility::Active, SecurityEpoch::new(0)),
        );
        state.admin_identities.insert([1; 32], target);
    });
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let rates = FakeRateLimitRepository::new(store.clone());
    let otps = FakeOtpChallengeRepository::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = FixedClock::new(now);
    let ids = SequenceIdGenerator::new(100);
    let primary = FakeSmsSender::new(vec![
        SubmissionCategory::Accepted,
        SubmissionCategory::Unknown,
        SubmissionCategory::Unavailable,
    ]);
    let secondary = FakeSmsSender::new(vec![
        SubmissionCategory::Unknown,
        SubmissionCategory::Unknown,
    ]);
    let policy = policy();
    let service = RequestAdminOtp {
        uow: &uow,
        admins: &admins,
        rates: &rates,
        otps: &otps,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        primary: &primary,
        secondary: &secondary,
        policy: &policy,
    };
    assert_eq!(service.execute(request()).await.unwrap(), ());
    assert!(secondary.captured().is_empty());
    let first = primary.captured()[0].clone();
    let snapshot = store.try_read_committed().unwrap();
    assert!(snapshot.admin_pending.is_empty());
    assert!(snapshot.admin_sessions.is_empty());
    assert_eq!(
        snapshot.otp_submissions[&first.challenge],
        SubmissionCategory::Accepted
    );
    let state = snapshot.admin_otps[&first.challenge].clone();
    let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
    otps.record_failure(
        &mut tx,
        OtpCheck {
            id: state.id(),
            binding: state.binding().clone(),
            expected_failed_attempts: 0,
            now,
        },
    )
    .await
    .unwrap();
    uow.commit(tx).await.unwrap();
    let mut resend = request();
    resend.predecessor = Some(first.challenge);
    service.execute(resend).await.unwrap();
    let second = primary.captured()[1].clone();
    assert_ne!(first.challenge, second.challenge);
    assert_ne!(first.code, second.code);
    assert_eq!(secondary.captured(), vec![second.clone()]);
    let snapshot = store.try_read_committed().unwrap();
    assert_eq!(snapshot.admin_otps[&second.challenge].failed_attempts(), 1);
    assert_eq!(
        snapshot.otp_submissions[&second.challenge],
        SubmissionCategory::Unknown
    );
    assert!(snapshot.admin_pending.is_empty());
    assert!(snapshot.admin_sessions.is_empty());
    let mut old = snapshot.admin_otps[&first.challenge].clone();
    assert!(
        old.record_failure(state.binding(), 1, now).is_err(),
        "resend supersedes old code"
    );
    let mut third = request();
    third.predecessor = Some(second.challenge);
    service.execute(third).await.unwrap();
    let third_id = primary.captured()[2].challenge;
    let mut blocked = request();
    blocked.predecessor = Some(third_id);
    assert_eq!(service.execute(blocked).await.unwrap(), ());
    assert_eq!(primary.captured().len(), 3);
    assert_eq!(secondary.captured().len(), 2);
    let mut unknown = request();
    unknown.phone = SecretBytes::new(b"+989120000002".to_vec());
    assert_eq!(service.execute(unknown).await.unwrap(), ());
    assert_eq!(primary.captured().len(), 3);
    assert_eq!(store.try_read_committed().unwrap().admins.len(), 1);
}
