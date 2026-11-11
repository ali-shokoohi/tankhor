//! FR-002/Q09: digest-only OTP fake sharing the caller's transactional store.

use super::store::FakeStore;
use admin::{
    domain::otp_state::{OtpDigest, OtpState},
    ports::otp_challenge_repository::{OtpChallengeRepository, OtpCheck, SubmissionCategory},
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ChallengeId},
    ports::unit_of_work::TxHandle,
};

#[derive(Debug, Clone)]
pub struct FakeOtpChallengeRepository {
    store: FakeStore,
}

impl FakeOtpChallengeRepository {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}

#[async_trait::async_trait]
impl OtpChallengeRepository for FakeOtpChallengeRepository {
    async fn revoke_all(&self, tx: &mut TxHandle, target: AdminId) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            for otp in state
                .admin_otps
                .values_mut()
                .filter(|otp| otp.binding().target == target)
            {
                otp.revoke();
            }
            Ok(())
        })
    }
    async fn issue(
        &self,
        tx: &mut TxHandle,
        next: OtpState,
        predecessor: Option<ChallengeId>,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if state.admin_otps.contains_key(&next.id()) {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            if !state.admins.contains_key(&next.binding().target) {
                return Err(DomainError::validation(ErrorCode::NotFound));
            }
            let head = state
                .otp_heads
                .values()
                .filter_map(|id| state.admin_otps.get(id))
                .find(|otp| otp.binding() == next.binding())
                .map(OtpState::id);
            if head != predecessor {
                return Err(denied());
            }
            if let Some(id) = predecessor {
                let mut previous = state.admin_otps.get(&id).ok_or_else(denied)?.clone();
                previous.supersede_with(&next)?;
                state.admin_otps.insert(id, previous);
                state.otp_heads.remove(&id);
            } else if next.failed_attempts() != 0 {
                return Err(denied());
            }
            state.otp_heads.insert(next.id(), next.id());
            state.admin_otps.insert(next.id(), next);
            Ok(())
        })
    }
    async fn lock(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
    ) -> Result<Option<OtpState>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.admin_otps.get(&id).cloned()))
    }
    async fn record_failure(&self, tx: &mut TxHandle, check: OtpCheck) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            state
                .admin_otps
                .get_mut(&check.id)
                .ok_or_else(denied)?
                .record_failure(&check.binding, check.expected_failed_attempts, check.now)
        })
    }
    async fn consume_once(
        &self,
        tx: &mut TxHandle,
        check: OtpCheck,
        candidate: OtpDigest,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            let mut otp = state.admin_otps.get(&check.id).ok_or_else(denied)?.clone();
            if otp.failed_attempts() != check.expected_failed_attempts {
                return Err(denied());
            }
            otp.consume(&check.binding, &candidate, check.now)?;
            state.admin_otps.insert(check.id, otp);
            Ok(())
        })
    }
    async fn record_submission(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
        category: SubmissionCategory,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if !state.admin_otps.contains_key(&id) {
                return Err(DomainError::validation(ErrorCode::NotFound));
            }
            state.otp_submissions.insert(id, category);
            Ok(())
        })
    }
    async fn submission(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
    ) -> Result<Option<SubmissionCategory>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.otp_submissions.get(&id).copied()))
    }
}
