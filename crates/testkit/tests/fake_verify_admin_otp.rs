//! T155: FR-002, FR-CON-02, AC-ADM-002.
use admin::{
    application::{
        request_admin_otp::{RequestAdminOtp, RequestOtp},
        verify_admin_otp::{VerifyAdminOtp, VerifyOtp},
    },
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthPurpose, AuthStage, BindingDigest},
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        otp_challenge_repository::SubmissionCategory,
        session_store::{PendingCheck, SessionStore},
    },
};
use kernel::{
    ids::AdminId,
    ports::unit_of_work::{TransactionMode, UnitOfWork},
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    clock::FixedClock, id_generator::SequenceIdGenerator,
    otp_challenge_repository::FakeOtpChallengeRepository,
    rate_limit_repository::FakeRateLimitRepository, session_store::FakeSessionStore,
    sms_sender::FakeSmsSender, store::FakeStore, unit_of_work::FakeUnitOfWork,
};
use time::{Duration, OffsetDateTime};
#[path = "support/otp_policy.rs"]
mod otp_policy;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "independent fixtures for security refusal matrix and one-use proof"
)]
async fn primary_proof_is_browser_bound_and_one_use() {
    for scenario in [
        "valid",
        "wrong_browser",
        "wrong_code",
        "expired",
        "epoch",
        "throttled",
        "unknown_commit",
        "expires_during_finalization",
    ] {
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
        let sessions = FakeSessionStore::new(store.clone());
        let crypto = FakeAdminAuthenticator::controlled_fixture();
        let clock = FixedClock::new(now);
        let ids = SequenceIdGenerator::new(100);
        let policy = otp_policy::policy();
        let primary = FakeSmsSender::new(vec![SubmissionCategory::Accepted]);
        let secondary = FakeSmsSender::new(vec![]);
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
            policy: &policy,
        }
        .execute(RequestOtp {
            phone: SecretBytes::new(b"09120000001".to_vec()),
            browser: BindingDigest::new([7; 32]),
            origin: BindingDigest::new([8; 32]),
            purpose: AuthPurpose::SignIn,
            predecessor: None,
        })
        .await
        .unwrap();
        let sms = primary.captured()[0].clone();
        if scenario == "expired" {
            clock.advance(Duration::MINUTE);
        }
        if scenario == "epoch" {
            store.seed(|state| {
                state.admins.insert(
                    target,
                    AdminState::new(target, Eligibility::Active, SecurityEpoch::new(1)),
                );
            });
        }
        let mut finalization_clock = kernel::ports::clock::MockClock::new();
        let samples = std::sync::atomic::AtomicUsize::new(0);
        finalization_clock.expect_now().returning(move || {
            let sample = samples.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(if sample < 2 {
                now
            } else {
                now + Duration::MINUTE
            })
        });
        let verification_clock: &dyn kernel::ports::clock::Clock =
            if scenario == "expires_during_finalization" {
                &finalization_clock
            } else {
                &clock
            };
        let service = VerifyAdminOtp {
            uow: &uow,
            admins: &admins,
            rates: &rates,
            otps: &otps,
            sessions: &sessions,
            authenticator: &crypto,
            clock: verification_clock,
            ids: &ids,
            policy: &policy,
        };
        let mut request = VerifyOtp {
            phone: SecretBytes::new(b"+989120000001".to_vec()),
            challenge: sms.challenge,
            code: sms.code.clone(),
            browser: BindingDigest::new([7; 32]),
            origin: BindingDigest::new([8; 32]),
            purpose: AuthPurpose::SignIn,
        };
        if scenario == "wrong_browser" {
            request.browser = BindingDigest::new([6; 32]);
        }
        if scenario == "wrong_code" || scenario == "throttled" {
            request.code = SecretBytes::new(b"000000".to_vec());
        }
        if scenario == "throttled" {
            for _ in 0..3 {
                assert!(service.execute(request.clone()).await.is_err());
            }
            request.code = sms.code;
        }
        let unknown_uow = FakeUnitOfWork::new(store.clone()).fail_next_commit_unknown();
        let result = if scenario == "unknown_commit" {
            VerifyAdminOtp {
                uow: &unknown_uow,
                ..service
            }
            .execute(request.clone())
            .await
        } else {
            service.execute(request.clone()).await
        };
        assert!(
            store
                .try_read_committed()
                .unwrap()
                .admin_sessions
                .is_empty()
        );
        if scenario != "valid" {
            assert!(result.is_err(), "{scenario}");
            let snapshot = store.try_read_committed().unwrap();
            assert!(snapshot.admin_pending.is_empty());
            if scenario == "wrong_code" {
                assert_eq!(snapshot.admin_otps[&sms.challenge].failed_attempts(), 1);
            }
            continue;
        }
        let proof = result.unwrap();
        assert_eq!(proof.binding.stage, AuthStage::PrimaryVerified);
        assert_eq!(proof.binding.browser, request.browser);
        assert_eq!(proof.binding.target, target);
        let reference = crypto
            .token_digest(TokenPurpose::PrimaryProof, proof.token)
            .await
            .unwrap();
        let check = PendingCheck {
            reference,
            binding: proof.binding,
            now,
        };
        let mut wrong = check.clone();
        wrong.binding.browser = BindingDigest::new([6; 32]);
        let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
        assert!(sessions.read_pending(&mut tx, wrong).await.is_err());
        let mut expired = check.clone();
        expired.now += Duration::MINUTE;
        assert!(sessions.read_pending(&mut tx, expired).await.is_err());
        assert!(sessions.read_pending(&mut tx, check.clone()).await.is_ok());
        sessions
            .consume_pending(&mut tx, check.clone())
            .await
            .unwrap();
        assert!(sessions.consume_pending(&mut tx, check).await.is_err());
        uow.commit(tx).await.unwrap();
        assert!(
            service.execute(request).await.is_err(),
            "OTP replay cannot create another proof"
        );
        assert_eq!(store.try_read_committed().unwrap().admin_pending.len(), 1);
    }
}
