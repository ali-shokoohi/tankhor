//! FR-002, FR-CON-02, Q09: digest-only OTP persistence under the caller's `UoW`.

use crate::domain::otp_state::{OtpBinding, OtpDigest, OtpState};
use kernel::{error::DomainError, ids::ChallengeId, ports::unit_of_work::TxHandle};
use time::OffsetDateTime;

/// Safe transport outcome only: neither delivery nor identity verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionCategory {
    Accepted,
    Rejected,
    Unavailable,
    Unknown,
}

/// Conditional snapshot and complete binding, without a raw OTP or identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpCheck {
    pub id: ChallengeId,
    pub binding: OtpBinding,
    pub expected_failed_attempts: u32,
    pub now: OffsetDateTime,
}

/// All operations use the supplied transaction, never independently commit.
/// Issue atomically supersedes the exact predecessor for the same target/browser/
/// purpose/epoch, carrying forward its failure count and attempt limit. Reject
/// missing/wrong/stale predecessors and duplicate IDs without partial mutation.
/// A predecessor is mandatory when that binding already has a challenge.
/// Transport retries update submission category only, never digest, expiry or budgets.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait OtpChallengeRepository: Send + Sync {
    async fn issue(
        &self,
        tx: &mut TxHandle,
        state: OtpState,
        predecessor: Option<ChallengeId>,
    ) -> Result<(), DomainError>;
    async fn lock(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
    ) -> Result<Option<OtpState>, DomainError>;
    /// Increment exactly once under expected-count CAS, only for a live binding.
    async fn record_failure(&self, tx: &mut TxHandle, check: OtpCheck) -> Result<(), DomainError>;
    /// Consume only a matching digest/live binding and expected count. Invalid
    /// candidates never consume; failure budgets use `record_failure` explicitly.
    async fn consume_once(
        &self,
        tx: &mut TxHandle,
        check: OtpCheck,
        candidate: OtpDigest,
    ) -> Result<(), DomainError>;
    async fn record_submission(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
        category: SubmissionCategory,
    ) -> Result<(), DomainError>;
    async fn submission(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
    ) -> Result<Option<SubmissionCategory>, DomainError>;
}

#[cfg(test)]
mod tests {
    use super::{MockOtpChallengeRepository, OtpChallengeRepository, OtpCheck, SubmissionCategory};
    use crate::domain::{
        admin_state::SecurityEpoch,
        auth_state::{AuthPurpose, BindingDigest},
        otp_state::{OtpBinding, OtpDigest, OtpInput, OtpState},
    };
    use kernel::{
        ids::{AdminId, ChallengeId},
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    #[tokio::test]
    async fn otp_challenge_repository_port_is_mockable() {
        let id = ChallengeId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap();
        let binding = OtpBinding {
            target: AdminId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap(),
            browser: BindingDigest::new([3; 32]),
            purpose: AuthPurpose::SignIn,
            epoch: SecurityEpoch::new(1),
        };
        let now = OffsetDateTime::UNIX_EPOCH;
        let state = OtpState::try_from(OtpInput {
            id,
            binding: binding.clone(),
            digest: OtpDigest::new([4; 32]),
            issued_at: now,
            expires_at: now + Duration::MINUTE,
            max_attempts: 3,
            failed_attempts: 0,
        })
        .unwrap();
        let check = OtpCheck {
            id,
            binding,
            expected_failed_attempts: 0,
            now,
        };
        let mut mock = MockOtpChallengeRepository::new();
        mock.expect_issue().times(1).returning(|_, _, _| Ok(()));
        let loaded = state.clone();
        mock.expect_lock()
            .times(1)
            .returning(move |_, _| Ok(Some(loaded.clone())));
        mock.expect_record_failure()
            .times(1)
            .returning(|_, _| Ok(()));
        mock.expect_consume_once()
            .times(1)
            .returning(|_, _, _| Ok(()));
        mock.expect_record_submission()
            .times(1)
            .returning(|_, _, _| Ok(()));
        mock.expect_submission()
            .times(1)
            .returning(|_, _| Ok(Some(SubmissionCategory::Unknown)));
        let port: &dyn OtpChallengeRepository = &mock;
        let mut tx = TxHandle::new(StoreId::new(), TransactionMode::ReadWrite, ());
        port.issue(&mut tx, state.clone(), None).await.unwrap();
        assert_eq!(port.lock(&mut tx, id).await.unwrap(), Some(state.clone()));
        port.record_failure(&mut tx, check.clone()).await.unwrap();
        port.consume_once(&mut tx, check.clone(), OtpDigest::new([4; 32]))
            .await
            .unwrap();
        port.record_submission(&mut tx, id, SubmissionCategory::Unknown)
            .await
            .unwrap();
        assert_eq!(
            port.submission(&mut tx, id).await.unwrap(),
            Some(SubmissionCategory::Unknown)
        );
        assert!(!format!("{state:?} {check:?}").contains("04040404"));
        assert_eq!(state.expires_at(), now + Duration::MINUTE);
        assert_eq!(
            [
                SubmissionCategory::Accepted,
                SubmissionCategory::Rejected,
                SubmissionCategory::Unavailable,
                SubmissionCategory::Unknown
            ]
            .len(),
            4
        );
    }
}
