//! T151: FR-002, FR-CON-02, AC-ADM-002, Q09.
use admin::{
    application::request_admin_otp::{RequestAdminOtp, RequestOtp},
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthPurpose, BindingDigest},
        otp_state::OtpDigest,
        secrets::SecretBytes,
        security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput},
    },
    ports::{
        admin_authenticator::{IssuedOtp, MockAdminAuthenticator},
        admin_repository::{IdentityDigest, MockAdminRepository},
        otp_challenge_repository::{MockOtpChallengeRepository, SubmissionCategory},
        rate_limit_repository::{MockRateLimitRepository, RateLimitStatus},
        sms_sender::MockSmsSender,
    },
};
use kernel::{
    ids::{AdminId, ChallengeId},
    ports::{
        clock::MockClock,
        id_generator::MockIdGenerator,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime};

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
        phone: SecretBytes::new(b"09120000001".to_vec()),
        browser: BindingDigest::new([7; 32]),
        origin: BindingDigest::new([8; 32]),
        purpose: AuthPurpose::SignIn,
        predecessor: None,
    }
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one complete cross-port ordering scenario with exact call counts"
)]
async fn sms_failover_reuses_challenge_and_expiry() {
    for outcome in [
        SubmissionCategory::Unknown,
        SubmissionCategory::Unavailable,
        SubmissionCategory::Rejected,
        SubmissionCategory::Accepted,
    ] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let store = StoreId::new();
        let now = OffsetDateTime::UNIX_EPOCH;
        let id: ChallengeId = "019a53a0-0000-7000-8000-000000000030".parse().unwrap();
        let target: AdminId = "019a53a0-0000-7000-8000-000000000001".parse().unwrap();
        let mut clock = MockClock::new();
        clock.expect_now().returning(move || Ok(now));
        let mut crypto = MockAdminAuthenticator::new();
        crypto
            .expect_identity_digest()
            .times(1)
            .withf(|phone| phone.expose() == b"+989120000001")
            .returning(|_| Ok(IdentityDigest::new([1; 32])));
        let log = events.clone();
        crypto
            .expect_issue_otp()
            .times(1)
            .returning(move |challenge| {
                assert_eq!(challenge, id);
                assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
                log.lock().unwrap().push("crypto");
                Ok(IssuedOtp {
                    code: SecretBytes::new(b"123456".to_vec()),
                    digest: OtpDigest::new([9; 32]),
                })
            });
        let mut ids = MockIdGenerator::new();
        ids.expect_next_challenge_id()
            .times(1)
            .returning(move || Ok(id));
        let mut uow = MockUnitOfWork::new();
        let log = events.clone();
        uow.expect_begin().returning(move |mode| {
            assert_eq!(mode, TransactionMode::ReadWrite);
            log.lock().unwrap().push("begin");
            Ok(TxHandle::new(store, mode, ()))
        });
        let log = events.clone();
        uow.expect_commit().returning(move |mut tx| {
            tx.take::<()>(store)?;
            log.lock().unwrap().push("commit");
            Ok(())
        });
        uow.expect_rollback().times(0);
        let mut rates = MockRateLimitRepository::new();
        let log = events.clone();
        rates
            .expect_increment()
            .times(2)
            .returning(move |_, _, _, _| {
                log.lock().unwrap().push("rate");
                Ok(RateLimitStatus {
                    count: 1,
                    started_at: now,
                    expires_at: now + Duration::minutes(5),
                    blocked_until: None,
                    allowed: true,
                })
            });
        let mut admins = MockAdminRepository::new();
        let log = events.clone();
        admins
            .expect_find_by_identity()
            .times(1)
            .returning(move |_, _| {
                assert_eq!(&log.lock().unwrap()[..], &["begin", "rate", "rate"]);
                log.lock().unwrap().push("eligible");
                Ok(Some(AdminState::new(
                    target,
                    Eligibility::Active,
                    SecurityEpoch::new(0),
                )))
            });
        admins.expect_lock().times(1).returning(move |_, targets| {
            assert_eq!(targets, vec![target]);
            Ok(vec![AdminState::new(
                target,
                Eligibility::Active,
                SecurityEpoch::new(0),
            )])
        });
        let mut otps = MockOtpChallengeRepository::new();
        let log = events.clone();
        otps.expect_issue()
            .times(1)
            .returning(move |_, state, predecessor| {
                assert_eq!(state.id(), id);
                assert_eq!(state.expires_at(), now + Duration::minutes(1));
                assert_eq!(state.failed_attempts(), 0);
                assert_eq!(predecessor, None);
                log.lock().unwrap().push("persist");
                Ok(())
            });
        otps.expect_record_submission()
            .times(1)
            .returning(|_, _, _| Ok(()));
        let mut primary = MockSmsSender::new();
        let log = events.clone();
        primary.expect_send_otp().times(1).returning(move |sms| {
            assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
            assert_eq!(sms.challenge, id);
            assert_eq!(sms.expires_at, now + Duration::minutes(1));
            assert_eq!(sms.code.expose(), b"123456");
            log.lock().unwrap().push("primary");
            outcome
        });
        let mut secondary = MockSmsSender::new();
        let eligible = matches!(
            outcome,
            SubmissionCategory::Unknown | SubmissionCategory::Unavailable
        );
        let log = events.clone();
        secondary
            .expect_send_otp()
            .times(usize::from(eligible))
            .returning(move |sms| {
                assert_eq!(log.lock().unwrap().last(), Some(&"primary"));
                assert_eq!(sms.challenge, id);
                assert_eq!(sms.expires_at, now + Duration::minutes(1));
                assert_eq!(sms.code.expose(), b"123456");
                assert_eq!(sms.recipient.expose(), b"+989120000001");
                SubmissionCategory::Unknown
            });
        RequestAdminOtp {
            uow: &uow,
            admins: &admins,
            rates: &rates,
            otps: &otps,
            authenticator: &crypto,
            clock: &clock,
            ids: &ids,
            primary: &primary,
            secondary: &secondary,
            policy: &policy(),
        }
        .execute(request())
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn invalid_identity_never_generates_or_sends_otp() {
    let mut invalid = request();
    invalid.phone = SecretBytes::new(b"invalid".to_vec());
    let uow = MockUnitOfWork::new();
    let admins = MockAdminRepository::new();
    let rates = MockRateLimitRepository::new();
    let otps = MockOtpChallengeRepository::new();
    let crypto = MockAdminAuthenticator::new();
    let clock = MockClock::new();
    let ids = MockIdGenerator::new();
    let primary = MockSmsSender::new();
    let secondary = MockSmsSender::new();
    assert!(
        RequestAdminOtp {
            uow: &uow,
            admins: &admins,
            rates: &rates,
            otps: &otps,
            authenticator: &crypto,
            clock: &clock,
            ids: &ids,
            primary: &primary,
            secondary: &secondary,
            policy: &policy()
        }
        .execute(invalid)
        .await
        .is_err()
    );
}
