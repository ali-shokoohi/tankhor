//! Restricted `SessionStore` proof values, never an enrollment grant or full session.

use super::admin_state::{AdminState, EnrollmentPurpose, SecurityEpoch};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ChallengeId, SessionId},
};
use std::fmt;
use time::OffsetDateTime;

/// A purpose-separated digest produced by the authenticator, not a raw capability.
#[derive(Clone, PartialEq, Eq)]
pub struct BindingDigest([u8; 32]);

impl BindingDigest {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for BindingDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BindingDigest([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPurpose {
    SignIn,
    Enrollment(EnrollmentPurpose),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStage {
    PrimaryVerified,
    Passkey,
    Totp,
}

/// All bindings must match the current trusted request/credential context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthBinding {
    pub target: AdminId,
    pub browser: BindingDigest,
    pub session: SessionId,
    pub purpose: AuthPurpose,
    pub stage: AuthStage,
    pub epoch: SecurityEpoch,
    pub credential_revision: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct AuthInput {
    pub id: ChallengeId,
    pub binding: AuthBinding,
    pub primary_verified_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Pending,
    Consumed,
    Superseded,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthState {
    id: ChallengeId,
    binding: AuthBinding,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    status: Status,
}

impl TryFrom<AuthInput> for AuthState {
    type Error = DomainError;

    fn try_from(input: AuthInput) -> Result<Self, Self::Error> {
        if input.expires_at <= input.created_at
            || input
                .primary_verified_at
                .is_none_or(|at| at > input.created_at)
            || input.binding.credential_revision == Some(0)
        {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self {
            id: input.id,
            binding: input.binding,
            created_at: input.created_at,
            expires_at: input.expires_at,
            status: Status::Pending,
        })
    }
}

impl AuthState {
    #[must_use]
    pub const fn id(&self) -> ChallengeId {
        self.id
    }

    #[must_use]
    pub const fn binding(&self) -> &AuthBinding {
        &self.binding
    }

    /// # Errors
    /// Refuses mismatched, terminal, expired or currently unauthorized proof state.
    /// The caller supplies authoritative current bindings, not client assertions.
    pub fn check(
        &self,
        binding: &AuthBinding,
        admin: &AdminState,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        if self.status != Status::Pending
            || &self.binding != binding
            || admin.id() != binding.target
            || admin.epoch() != binding.epoch
            || now < self.created_at
            || now >= self.expires_at
        {
            return Err(denied());
        }
        match binding.purpose {
            AuthPurpose::SignIn if admin.is_eligible() => Ok(()),
            AuthPurpose::Enrollment(purpose) => {
                admin.check_enrollment(binding.target, purpose, binding.epoch, now)
            }
            AuthPurpose::SignIn => Err(denied()),
        }
    }

    /// # Errors
    /// Same refusals as `check`; a refused attempt leaves the value unchanged.
    pub fn consume(
        &mut self,
        binding: &AuthBinding,
        admin: &AdminState,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.check(binding, admin, now)?;
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
    DomainError::forbidden(ErrorCode::PermissionDenied)
}

#[cfg(test)]
mod tests {
    use super::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest};
    use crate::domain::admin_state::{
        AdminState, Eligibility, EnrollmentPurpose, Invalidation, SecurityEpoch,
    };
    use kernel::ids::{AdminId, ChallengeId, SessionId};
    use proptest::prelude::*;
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn admin(n: u8, eligibility: Eligibility) -> AdminState {
        AdminState::new(
            AdminId::from_str(&format!("019a53a0-0000-7000-8000-{n:012}")).unwrap(),
            eligibility,
            SecurityEpoch::new(7),
        )
    }

    fn input() -> AuthInput {
        AuthInput {
            id: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000010").unwrap(),
            binding: AuthBinding {
                target: admin(1, Eligibility::Active).id(),
                browser: BindingDigest::new([11; 32]),
                session: SessionId::from_str("019a53a0-0000-7000-8000-000000000020").unwrap(),
                purpose: AuthPurpose::SignIn,
                stage: AuthStage::PrimaryVerified,
                epoch: SecurityEpoch::new(7),
                credential_revision: None,
            },
            primary_verified_at: Some(OffsetDateTime::UNIX_EPOCH),
            created_at: OffsetDateTime::UNIX_EPOCH,
            expires_at: OffsetDateTime::UNIX_EPOCH + Duration::minutes(2),
        }
    }

    #[test]
    fn auth_state_requires_browser_purpose_epoch_and_one_use() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let actor = admin(1, Eligibility::Active);
        for stage in [
            AuthStage::PrimaryVerified,
            AuthStage::Passkey,
            AuthStage::Totp,
        ] {
            let mut source = input();
            source.binding.stage = stage;
            source.binding.credential_revision = (stage == AuthStage::Passkey).then_some(3);
            let binding = source.binding.clone();
            let mut state = AuthState::try_from(source).unwrap();
            assert_eq!(state.id(), input().id);
            assert_eq!(state.binding(), &binding);
            assert!(state.check(&binding, &actor, now).is_ok());
            assert_wrong_bindings(&state, &binding, &actor);
            assert!(
                state
                    .check(&binding, &actor, now - Duration::SECOND)
                    .is_err()
            );
            assert!(
                state
                    .check(&binding, &actor, now + Duration::minutes(2))
                    .is_err()
            );
            state.consume(&binding, &actor, now).unwrap();
            let consumed = state.clone();
            assert!(state.consume(&binding, &actor, now).is_err());
            assert_eq!(state, consumed);
        }
        assert_invalid_inputs();
        assert_enrollment_authority();
        assert_transition_sequences();
    }

    fn assert_wrong_bindings(state: &AuthState, binding: &AuthBinding, actor: &AdminState) {
        let now = OffsetDateTime::UNIX_EPOCH;
        for field in 0..7 {
            let mut wrong = binding.clone();
            match field {
                0 => wrong.browser = BindingDigest::new([12; 32]),
                1 => {
                    wrong.session =
                        SessionId::from_str("019a53a0-0000-7000-8000-000000000021").unwrap();
                }
                2 => wrong.target = admin(2, Eligibility::Active).id(),
                3 => wrong.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery),
                4 => wrong.epoch = SecurityEpoch::new(8),
                5 => {
                    wrong.stage = if binding.stage == AuthStage::Totp {
                        AuthStage::Passkey
                    } else {
                        AuthStage::Totp
                    }
                }
                _ => wrong.credential_revision = Some(4),
            }
            let mut candidate = state.clone();
            assert!(candidate.consume(&wrong, actor, now).is_err());
            assert_eq!(&candidate, state, "a rejected attempt cannot consume proof");
        }
        assert!(
            state
                .check(binding, &admin(2, Eligibility::Active), now)
                .is_err()
        );
        let mut recovered = actor.clone();
        recovered
            .invalidate(
                actor.epoch(),
                actor.epoch().advance().unwrap(),
                Invalidation::Recover,
            )
            .unwrap();
        recovered.restore(recovered.epoch()).unwrap();
        assert!(state.check(binding, &recovered, now).is_err());
    }

    fn assert_invalid_inputs() {
        let mut source = input();
        source.primary_verified_at = None;
        assert!(AuthState::try_from(source).is_err());
        let mut source = input();
        source.primary_verified_at = Some(source.created_at + Duration::SECOND);
        assert!(AuthState::try_from(source).is_err());
        let mut source = input();
        source.expires_at = source.created_at;
        assert!(AuthState::try_from(source).is_err());
        let mut source = input();
        source.binding.credential_revision = Some(0);
        assert!(AuthState::try_from(source).is_err());
        let digest = BindingDigest::new([11; 32]);
        assert_eq!(digest.as_bytes(), &[11; 32]);
        assert_eq!(format!("{digest:?}"), "BindingDigest([REDACTED])");
    }

    fn assert_enrollment_authority() {
        let now = OffsetDateTime::UNIX_EPOCH;
        for (eligibility, purpose) in [
            (Eligibility::Invited, EnrollmentPurpose::Addition),
            (Eligibility::Recovering, EnrollmentPurpose::Recovery),
        ] {
            let mut target = admin(1, eligibility);
            let mut source = input();
            source.binding.purpose = AuthPurpose::Enrollment(purpose);
            let binding = source.binding.clone();
            let mut state = AuthState::try_from(source).unwrap();
            assert!(
                state.check(&binding, &target, now).is_err(),
                "OTP proof is not authorization"
            );
            target
                .authorize_enrollment(
                    &admin(2, Eligibility::Active),
                    purpose,
                    target.epoch(),
                    now,
                    now + Duration::minutes(1),
                )
                .unwrap();
            assert!(state.check(&binding, &target, now).is_ok());
            assert!(
                !target.is_eligible(),
                "pending proof is never console access"
            );
            assert!(
                state
                    .check(&binding, &target, now + Duration::minutes(1))
                    .is_err()
            );
            let before = state.clone();
            target
                .consume_enrollment(target.id(), purpose, target.epoch(), now)
                .unwrap();
            assert!(state.consume(&binding, &target, now).is_err());
            assert_eq!(state, before);
        }
    }

    fn assert_transition_sequences() {
        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(
                &prop::collection::vec((0_i64..60, 0_u8..5), 1..100),
                |ops| {
                    let mut state = AuthState::try_from(input()).unwrap();
                    let binding = input().binding;
                    let actor = admin(1, Eligibility::Active);
                    let mut terminal = false;
                    let mut elapsed = 0;
                    for (advance, operation) in ops {
                        elapsed += advance;
                        let now = OffsetDateTime::UNIX_EPOCH + Duration::seconds(elapsed);
                        match operation {
                            0 => {
                                state.revoke();
                                terminal = true;
                            }
                            1 => {
                                state.supersede();
                                terminal = true;
                            }
                            2 => {
                                let expected = !terminal && elapsed < 120;
                                prop_assert_eq!(
                                    state.consume(&binding, &actor, now).is_ok(),
                                    expected
                                );
                                terminal |= expected;
                            }
                            3 => {
                                let mut wrong = binding.clone();
                                wrong.browser = BindingDigest::new([99; 32]);
                                let before = state.clone();
                                prop_assert!(state.consume(&wrong, &actor, now).is_err());
                                prop_assert_eq!(&state, &before);
                            }
                            _ => {}
                        }
                        prop_assert_eq!(
                            state.check(&binding, &actor, now).is_ok(),
                            !terminal && elapsed < 120
                        );
                    }
                    Ok(())
                },
            )
            .unwrap();
    }
}
