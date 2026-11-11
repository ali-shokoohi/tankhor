//! Pure, bounded, digest-backed OTP transitions (FR-002, Q09).

use super::{
    admin_state::SecurityEpoch,
    auth_state::{AuthPurpose, BindingDigest},
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ChallengeId},
};
use std::fmt;
use time::OffsetDateTime;

/// Opaque keyed digest supplied by the authenticator; never a plaintext code.
#[derive(Clone, PartialEq, Eq)]
pub struct OtpDigest([u8; 32]);

impl OtpDigest {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for OtpDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OtpDigest([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpBinding {
    pub target: AdminId,
    pub browser: BindingDigest,
    pub purpose: AuthPurpose,
    pub epoch: SecurityEpoch,
}

#[derive(Debug, Clone)]
pub struct OtpInput {
    pub id: ChallengeId,
    pub binding: OtpBinding,
    pub digest: OtpDigest,
    pub issued_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub max_attempts: u32,
    /// Restored failures are carried forward, never reset by transport retry.
    pub failed_attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Pending,
    Consumed,
    Superseded,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpState {
    id: ChallengeId,
    binding: OtpBinding,
    digest: OtpDigest,
    issued_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    max_attempts: u32,
    failed_attempts: u32,
    status: Status,
}

impl TryFrom<OtpInput> for OtpState {
    type Error = DomainError;

    fn try_from(input: OtpInput) -> Result<Self, Self::Error> {
        if input.expires_at <= input.issued_at
            || input.max_attempts == 0
            || input.failed_attempts > input.max_attempts
        {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self {
            id: input.id,
            binding: input.binding,
            digest: input.digest,
            issued_at: input.issued_at,
            expires_at: input.expires_at,
            max_attempts: input.max_attempts,
            failed_attempts: input.failed_attempts,
            status: Status::Pending,
        })
    }
}

impl OtpState {
    #[must_use]
    pub const fn id(&self) -> ChallengeId {
        self.id
    }

    #[must_use]
    pub const fn binding(&self) -> &OtpBinding {
        &self.binding
    }

    #[must_use]
    pub const fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }

    #[must_use]
    pub const fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// # Errors
    /// Rejects terminal state, a changed binding or any reset/changed attempt budget.
    pub fn supersede_with(&mut self, next: &Self) -> Result<(), DomainError> {
        if self.status != Status::Pending
            || self.binding != next.binding
            || self.failed_attempts != next.failed_attempts
            || self.max_attempts != next.max_attempts
        {
            return Err(denied());
        }
        self.status = Status::Superseded;
        Ok(())
    }

    /// # Errors
    /// Rejects stale counts, invalid bindings, expiry, terminal or exhausted state.
    pub fn record_failure(
        &mut self,
        binding: &OtpBinding,
        expected: u32,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        if self.failed_attempts != expected
            || self.status != Status::Pending
            || &self.binding != binding
            || now < self.issued_at
            || now >= self.expires_at
            || self.failed_attempts >= self.max_attempts
        {
            return Err(denied());
        }
        self.failed_attempts += 1;
        Ok(())
    }

    /// # Errors
    /// Rejects terminal/expired/mismatched state or an exhausted budget. A wrong
    /// digest on otherwise live state consumes one attempt, never changes expiry.
    pub fn consume(
        &mut self,
        binding: &OtpBinding,
        candidate: &OtpDigest,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        if self.status != Status::Pending
            || &self.binding != binding
            || now < self.issued_at
            || now >= self.expires_at
            || self.failed_attempts >= self.max_attempts
        {
            return Err(denied());
        }
        if &self.digest != candidate {
            // Strictly below max_attempts, so this cannot overflow.
            self.failed_attempts += 1;
            return Err(denied());
        }
        self.status = Status::Consumed;
        Ok(())
    }

    pub fn revoke(&mut self) {
        if self.status == Status::Pending {
            self.status = Status::Revoked;
        }
    }

    pub fn supersede(&mut self) {
        if self.status == Status::Pending {
            self.status = Status::Superseded;
        }
    }
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}

#[cfg(test)]
mod tests {
    use super::{OtpBinding, OtpDigest, OtpInput, OtpState};
    use crate::domain::{
        admin_state::SecurityEpoch,
        auth_state::{AuthPurpose, BindingDigest},
    };
    use kernel::ids::{AdminId, ChallengeId};
    use proptest::prelude::*;
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn input() -> OtpInput {
        OtpInput {
            id: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000030").unwrap(),
            binding: OtpBinding {
                target: AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
                browser: BindingDigest::new([11; 32]),
                purpose: AuthPurpose::SignIn,
                epoch: SecurityEpoch::new(7),
            },
            digest: OtpDigest::new([42; 32]),
            issued_at: OffsetDateTime::UNIX_EPOCH,
            expires_at: OffsetDateTime::UNIX_EPOCH + Duration::minutes(1),
            max_attempts: 3,
            failed_attempts: 0,
        }
    }

    #[test]
    fn otp_state_expires_supersedes_and_consumes_once() {
        let source = input();
        let now = source.issued_at;
        let mut state = OtpState::try_from(input()).unwrap();
        assert_eq!(state.id(), source.id);
        assert_eq!(state.binding(), &source.binding);
        assert_eq!(state.expires_at(), source.expires_at);
        assert_eq!(state.failed_attempts(), 0);
        assert_eq!(source.digest.as_bytes(), &[42; 32]);
        assert_eq!(format!("{:?}", source.digest), "OtpDigest([REDACTED])");
        for at in [
            now - Duration::SECOND,
            source.expires_at,
            source.expires_at + Duration::SECOND,
        ] {
            let before = state.clone();
            assert!(state.consume(&source.binding, &source.digest, at).is_err());
            assert_eq!(state, before);
        }
        state.consume(&source.binding, &source.digest, now).unwrap();
        let consumed = state.clone();
        assert!(state.consume(&source.binding, &source.digest, now).is_err());
        assert_eq!(state, consumed);
        assert_attempt_budget();
        assert_binding_and_terminal_states();
        assert_invalid_inputs();
        assert_transition_sequences();
    }

    fn assert_attempt_budget() {
        let source = input();
        let mut state = OtpState::try_from(input()).unwrap();
        for expected in 1..=3 {
            assert!(
                state
                    .consume(&source.binding, &OtpDigest::new([99; 32]), source.issued_at)
                    .is_err()
            );
            assert_eq!(state.failed_attempts(), expected);
            assert_eq!(state.expires_at(), source.expires_at);
        }
        let exhausted = state.clone();
        for digest in [&source.digest, &OtpDigest::new([99; 32])] {
            assert!(
                state
                    .consume(&source.binding, digest, source.issued_at)
                    .is_err()
            );
            assert_eq!(state, exhausted);
        }
        let mut source = input();
        source.failed_attempts = 2;
        let mut restored = OtpState::try_from(source.clone()).unwrap();
        assert_eq!(restored.failed_attempts(), 2);
        assert!(
            restored
                .consume(&source.binding, &OtpDigest::new([99; 32]), source.issued_at)
                .is_err()
        );
        assert_eq!(
            restored.failed_attempts(),
            3,
            "restoration never resets the budget"
        );
    }

    fn assert_binding_and_terminal_states() {
        let source = input();
        for field in 0..4 {
            let mut wrong = source.binding.clone();
            match field {
                0 => {
                    wrong.target =
                        AdminId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap();
                }
                1 => wrong.browser = BindingDigest::new([12; 32]),
                2 => {
                    wrong.purpose = AuthPurpose::Enrollment(
                        crate::domain::admin_state::EnrollmentPurpose::Addition,
                    );
                }
                _ => wrong.epoch = SecurityEpoch::new(8),
            }
            let mut state = OtpState::try_from(input()).unwrap();
            let before = state.clone();
            assert!(
                state
                    .consume(&wrong, &source.digest, source.issued_at)
                    .is_err()
            );
            assert_eq!(state, before);
        }
        for revoke in [false, true] {
            let mut state = OtpState::try_from(input()).unwrap();
            if revoke {
                state.revoke();
                state.revoke();
            } else {
                state.supersede();
                state.supersede();
            }
            assert!(
                state
                    .consume(&source.binding, &source.digest, source.issued_at)
                    .is_err()
            );
        }
    }

    fn assert_invalid_inputs() {
        for (attempts, failures) in [(0, 0), (3, 4)] {
            let mut source = input();
            source.max_attempts = attempts;
            source.failed_attempts = failures;
            assert!(OtpState::try_from(source).is_err());
        }
        let mut source = input();
        source.expires_at = source.issued_at;
        assert!(OtpState::try_from(source).is_err());
    }

    fn assert_transition_sequences() {
        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(
                &prop::collection::vec((0_i64..30, 0_u8..5), 1..100),
                |ops| {
                    let source = input();
                    let mut state = OtpState::try_from(input()).unwrap();
                    let mut elapsed = 0;
                    let mut failures = 0;
                    let mut terminal = false;
                    for (advance, operation) in ops {
                        elapsed += advance;
                        let now = source.issued_at + Duration::seconds(elapsed);
                        match operation {
                            0 => {
                                state.supersede();
                                terminal = true;
                            }
                            1 => {
                                state.revoke();
                                terminal = true;
                            }
                            _ => {
                                let valid = operation == 2;
                                let live = !terminal && elapsed < 60 && failures < 3;
                                let digest = if valid {
                                    source.digest.clone()
                                } else {
                                    OtpDigest::new([99; 32])
                                };
                                prop_assert_eq!(
                                    state.consume(&source.binding, &digest, now).is_ok(),
                                    live && valid
                                );
                                if live && valid {
                                    terminal = true;
                                }
                                if live && !valid {
                                    failures += 1;
                                }
                            }
                        }
                        prop_assert_eq!(state.failed_attempts(), failures);
                        prop_assert_eq!(state.expires_at(), source.expires_at);
                    }
                    Ok(())
                },
            )
            .unwrap();
    }
}
