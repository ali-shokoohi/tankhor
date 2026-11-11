//! T154: FR-002, FR-CON-02, AC-ADM-002.
use admin::{
    application::verify_admin_otp::{VerifyAdminOtp, VerifyOtp},
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        auth_state::{AuthPurpose, BindingDigest},
        otp_state::{OtpBinding, OtpDigest, OtpInput, OtpState},
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::MockAdminAuthenticator,
        admin_repository::{IdentityDigest, MockAdminRepository},
        otp_challenge_repository::MockOtpChallengeRepository,
        rate_limit_repository::{MockRateLimitRepository, RateLimitStatus},
        session_store::MockSessionStore,
    },
};
use kernel::ports::{
    clock::MockClock,
    id_generator::MockIdGenerator,
    unit_of_work::{MockUnitOfWork, StoreId, TxHandle},
};
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime};
#[path = "support/otp_policy.rs"]
mod otp_policy;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "exact negative-path port order and zero access-state writes"
)]
async fn invalid_primary_proof_never_issues_session() {
    for expired in [false, true] {
        let now = OffsetDateTime::UNIX_EPOCH;
        let target = "019a53a0-0000-7000-8000-000000000001".parse().unwrap();
        let challenge = "019a53a0-0000-7000-8000-000000000002".parse().unwrap();
        let binding = OtpBinding {
            target,
            browser: BindingDigest::new([7; 32]),
            purpose: AuthPurpose::SignIn,
            epoch: SecurityEpoch::new(0),
        };
        let state = OtpState::try_from(OtpInput {
            id: challenge,
            binding,
            digest: OtpDigest::new([9; 32]),
            issued_at: now - Duration::MINUTE,
            expires_at: if expired { now } else { now + Duration::MINUTE },
            max_attempts: 3,
            failed_attempts: 0,
        })
        .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let store = StoreId::new();
        let mut uow = MockUnitOfWork::new();
        let log = events.clone();
        uow.expect_begin().returning(move |mode| {
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
        let mut crypto = MockAdminAuthenticator::new();
        crypto
            .expect_identity_digest()
            .times(1)
            .returning(|_| Ok(IdentityDigest::new([1; 32])));
        let log = events.clone();
        crypto
            .expect_verify_otp()
            .times(usize::from(!expired))
            .returning(move |id, _, digest| {
                assert_eq!(id, challenge);
                assert_eq!(digest, OtpDigest::new([9; 32]));
                assert_eq!(log.lock().unwrap().last(), Some(&"commit"));
                Ok(false)
            });
        crypto.expect_issue_token().times(0);
        let mut clock = MockClock::new();
        clock.expect_now().returning(move || Ok(now));
        let ids = MockIdGenerator::new();
        let mut admins = MockAdminRepository::new();
        admins
            .expect_find_by_identity()
            .times(1)
            .returning(move |_, _| {
                Ok(Some(AdminState::new(
                    target,
                    Eligibility::Active,
                    SecurityEpoch::new(0),
                )))
            });
        admins
            .expect_lock()
            .times(usize::from(!expired))
            .returning(move |_, _| {
                Ok(vec![AdminState::new(
                    target,
                    Eligibility::Active,
                    SecurityEpoch::new(0),
                )])
            });
        let mut rates = MockRateLimitRepository::new();
        rates
            .expect_increment()
            .times(2)
            .returning(move |_, _, _, _| {
                Ok(RateLimitStatus {
                    count: 1,
                    started_at: now,
                    expires_at: now + Duration::minutes(5),
                    blocked_until: None,
                    allowed: true,
                })
            });
        let mut otps = MockOtpChallengeRepository::new();
        otps.expect_lock()
            .times(if expired { 1 } else { 2 })
            .returning(move |_, _| Ok(Some(state.clone())));
        let log = events.clone();
        otps.expect_record_failure()
            .times(usize::from(!expired))
            .returning(move |_, check| {
                assert_eq!(check.id, challenge);
                assert_eq!(check.expected_failed_attempts, 0);
                log.lock().unwrap().push("failure");
                Ok(())
            });
        otps.expect_consume_once().times(0);
        let mut sessions = MockSessionStore::new();
        sessions.expect_insert().times(0);
        sessions.expect_create_pending().times(0);
        let service = VerifyAdminOtp {
            uow: &uow,
            admins: &admins,
            rates: &rates,
            otps: &otps,
            sessions: &sessions,
            authenticator: &crypto,
            clock: &clock,
            ids: &ids,
            policy: &otp_policy::policy(),
        };
        assert!(
            service
                .execute(VerifyOtp {
                    phone: SecretBytes::new(b"09120000001".to_vec()),
                    challenge,
                    code: SecretBytes::new(b"000000".to_vec()),
                    browser: BindingDigest::new([7; 32]),
                    origin: BindingDigest::new([8; 32]),
                    purpose: AuthPurpose::SignIn
                })
                .await
                .is_err()
        );
        if !expired {
            assert!(events.lock().unwrap().ends_with(&["failure", "commit"]));
        }
    }
}
