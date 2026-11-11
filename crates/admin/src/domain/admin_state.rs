//! Pure eligibility, durable epoch and identity-bound enrollment rules.
//! FR-004/005, AC-ADM-007/043. Browser/session proofs are separate values.

use kernel::{
    error::{DomainError, ErrorCode},
    ids::AdminId,
};
use time::OffsetDateTime;

/// Monotonic per-admin invalidation version, never a browser capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SecurityEpoch(u64);

impl SecurityEpoch {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// # Errors
    /// Exhaustion fails closed instead of wrapping to an old epoch.
    pub fn advance(self) -> Result<Self, DomainError> {
        self.0.checked_add(1).map(Self).ok_or_else(stale)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    Invited,
    Active,
    Recovering,
    Deactivated,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalidation {
    Deactivate,
    Revoke,
    Recover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentPurpose {
    Addition,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentStatus {
    Pending,
    Consumed,
    Revoked,
}

/// Every eligible launch admin has exactly this same permission set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permissions {
    Full,
}

/// Protected target state: no transferable token and no browser binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentAuthorization {
    purpose: EnrollmentPurpose,
    authorizer: AdminId,
    epoch: SecurityEpoch,
    authorized_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    status: EnrollmentStatus,
}

impl EnrollmentAuthorization {
    #[must_use]
    pub const fn purpose(&self) -> EnrollmentPurpose {
        self.purpose
    }
    #[must_use]
    pub const fn authorizer(&self) -> AdminId {
        self.authorizer
    }
    #[must_use]
    pub const fn epoch(&self) -> SecurityEpoch {
        self.epoch
    }
    #[must_use]
    pub const fn authorized_at(&self) -> OffsetDateTime {
        self.authorized_at
    }
    #[must_use]
    pub const fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
    #[must_use]
    pub const fn status(&self) -> EnrollmentStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminState {
    id: AdminId,
    eligibility: Eligibility,
    epoch: SecurityEpoch,
    enrollment: Option<EnrollmentAuthorization>,
}

impl AdminState {
    #[must_use]
    pub const fn new(id: AdminId, eligibility: Eligibility, epoch: SecurityEpoch) -> Self {
        Self {
            id,
            eligibility,
            epoch,
            enrollment: None,
        }
    }
    #[must_use]
    pub const fn id(&self) -> AdminId {
        self.id
    }
    #[must_use]
    pub const fn epoch(&self) -> SecurityEpoch {
        self.epoch
    }
    #[must_use]
    pub const fn is_eligible(&self) -> bool {
        matches!(self.eligibility, Eligibility::Active)
    }
    #[must_use]
    pub fn matches_epoch(&self, epoch: SecurityEpoch) -> bool {
        self.is_eligible() && self.epoch == epoch
    }
    #[must_use]
    pub const fn permissions(&self) -> Option<Permissions> {
        if self.is_eligible() {
            Some(Permissions::Full)
        } else {
            None
        }
    }
    #[must_use]
    pub const fn enrollment(&self) -> Option<&EnrollmentAuthorization> {
        self.enrollment.as_ref()
    }

    /// # Errors
    /// Refuses stale/non-increasing invalidation before changing either state or epoch.
    pub fn invalidate(
        &mut self,
        expected: SecurityEpoch,
        next: SecurityEpoch,
        kind: Invalidation,
    ) -> Result<(), DomainError> {
        if self.epoch != expected || next <= self.epoch {
            return Err(stale());
        }
        self.epoch = next;
        self.eligibility = match kind {
            Invalidation::Deactivate => Eligibility::Deactivated,
            Invalidation::Revoke => Eligibility::Revoked,
            Invalidation::Recover => Eligibility::Recovering,
        };
        if let Some(authorization) = &mut self.enrollment {
            authorization.status = EnrollmentStatus::Revoked;
        }
        Ok(())
    }

    /// Restore eligibility without resetting invalidation history. Caller owns authority.
    /// # Errors
    /// Refuses a stale expected epoch.
    pub fn restore(&mut self, expected: SecurityEpoch) -> Result<(), DomainError> {
        if self.epoch != expected {
            return Err(stale());
        }
        self.eligibility = Eligibility::Active;
        Ok(())
    }

    /// Record another eligible admin's authorization. Services verify that actor's session.
    /// # Errors
    /// Refuses self/ineligible authorization, wrong target state/epoch or non-finite lifetime.
    pub fn authorize_enrollment(
        &mut self,
        actor: &Self,
        purpose: EnrollmentPurpose,
        expected: SecurityEpoch,
        now: OffsetDateTime,
        expires_at: OffsetDateTime,
    ) -> Result<(), DomainError> {
        if actor.id == self.id || !actor.is_eligible() {
            return Err(denied());
        }
        if self.epoch != expected {
            return Err(stale());
        }
        if expires_at <= now {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        if !matches!(
            (self.eligibility, purpose),
            (Eligibility::Invited, EnrollmentPurpose::Addition)
                | (Eligibility::Recovering, EnrollmentPurpose::Recovery)
        ) {
            return Err(denied());
        }
        self.enrollment = Some(EnrollmentAuthorization {
            purpose,
            authorizer: actor.id,
            epoch: expected,
            authorized_at: now,
            expires_at,
            status: EnrollmentStatus::Pending,
        });
        Ok(())
    }

    /// # Errors
    /// Only the matching identity/purpose/current epoch may use pending unexpired state.
    pub fn check_enrollment(
        &self,
        target: AdminId,
        purpose: EnrollmentPurpose,
        epoch: SecurityEpoch,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let authorization = self.enrollment.as_ref().ok_or_else(denied)?;
        if target != self.id
            || purpose != authorization.purpose
            || epoch != self.epoch
            || epoch != authorization.epoch
            || authorization.status != EnrollmentStatus::Pending
            || now < authorization.authorized_at
            || now >= authorization.expires_at
            || !matches!(
                (self.eligibility, purpose),
                (Eligibility::Invited, EnrollmentPurpose::Addition)
                    | (Eligibility::Recovering, EnrollmentPurpose::Recovery)
            )
        {
            return Err(denied());
        }
        Ok(())
    }

    /// # Errors
    /// Refuses stale, expired, wrong-identity or already consumed/revoked authorization.
    pub fn consume_enrollment(
        &mut self,
        target: AdminId,
        purpose: EnrollmentPurpose,
        epoch: SecurityEpoch,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.check_enrollment(target, purpose, epoch, now)?;
        if let Some(authorization) = &mut self.enrollment {
            authorization.status = EnrollmentStatus::Consumed;
        }
        Ok(())
    }

    /// # Errors
    /// Refuses stale expected epoch or absent authorization.
    pub fn revoke_enrollment(&mut self, expected: SecurityEpoch) -> Result<(), DomainError> {
        if expected != self.epoch {
            return Err(stale());
        }
        let authorization = self.enrollment.as_mut().ok_or_else(denied)?;
        authorization.status = EnrollmentStatus::Revoked;
        Ok(())
    }
}

fn stale() -> DomainError {
    DomainError::conflict(ErrorCode::StaleRevision)
}
fn denied() -> DomainError {
    DomainError::forbidden(ErrorCode::PermissionDenied)
}

#[cfg(test)]
mod tests {
    use super::{
        AdminState, Eligibility, EnrollmentPurpose, EnrollmentStatus, Invalidation, SecurityEpoch,
    };
    use kernel::ids::AdminId;
    use proptest::prelude::*;
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn id(n: u8) -> AdminId {
        AdminId::from_str(&format!("019a53a0-0000-7000-8000-{n:012}")).unwrap()
    }

    fn admin(n: u8, eligibility: Eligibility, epoch: u64) -> AdminState {
        AdminState::new(id(n), eligibility, SecurityEpoch::new(epoch))
    }

    #[test]
    fn admin_eligibility_epoch_and_equal_permissions_are_pure() {
        let first = admin(1, Eligibility::Active, 7);
        let other = admin(2, Eligibility::Active, 7);
        assert_eq!(first.permissions(), other.permissions());
        assert_ne!(first.id(), other.id());
        assert!(first.is_eligible());
        assert!(first.matches_epoch(SecurityEpoch::new(7)));
        assert!(!first.matches_epoch(SecurityEpoch::new(6)));
        assert!(!first.matches_epoch(SecurityEpoch::new(8)));
        for state in [
            Eligibility::Invited,
            Eligibility::Recovering,
            Eligibility::Deactivated,
            Eligibility::Revoked,
        ] {
            let inactive = admin(1, state, 7);
            assert!(!inactive.is_eligible());
            assert!(inactive.permissions().is_none());
            assert!(!inactive.matches_epoch(SecurityEpoch::new(7)));
        }
        assert_invalidations(&first);
        assert_enrollment(&first);

        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(
                &(0_u64..1_000_000, prop::collection::vec(0_u8..5, 1..100)),
                |(start, ops)| {
                    let mut state = admin(1, Eligibility::Active, start);
                    let mut expected = start;
                    for op in ops {
                        let epoch = SecurityEpoch::new(expected);
                        match op {
                            0..=2 => {
                                let kind = [
                                    Invalidation::Deactivate,
                                    Invalidation::Revoke,
                                    Invalidation::Recover,
                                ][usize::from(op)];
                                let unchanged = state.clone();
                                prop_assert!(state.invalidate(epoch, epoch, kind).is_err());
                                prop_assert_eq!(&state, &unchanged);
                                state
                                    .invalidate(epoch, epoch.advance().unwrap(), kind)
                                    .unwrap();
                                expected += 1;
                            }
                            3 => {
                                state.restore(epoch).unwrap();
                            }
                            _ => {
                                prop_assert_eq!(state.epoch(), epoch);
                            }
                        }
                        prop_assert_eq!(state.epoch(), SecurityEpoch::new(expected));
                        prop_assert!(state.epoch().value() >= start);
                        if expected > start {
                            prop_assert!(
                                state
                                    .invalidate(
                                        SecurityEpoch::new(start),
                                        SecurityEpoch::new(expected + 1),
                                        Invalidation::Revoke
                                    )
                                    .is_err()
                            );
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
    }

    fn assert_invalidations(first: &AdminState) {
        for kind in [
            Invalidation::Deactivate,
            Invalidation::Revoke,
            Invalidation::Recover,
        ] {
            let mut state = first.clone();
            let epoch = state.epoch();
            assert!(
                state
                    .invalidate(SecurityEpoch::new(6), SecurityEpoch::new(8), kind)
                    .is_err()
            );
            assert!(
                state
                    .invalidate(epoch, SecurityEpoch::new(6), kind)
                    .is_err()
            );
            assert!(state.invalidate(epoch, epoch, kind).is_err());
            assert_eq!(&state, first);
            state
                .invalidate(epoch, epoch.advance().unwrap(), kind)
                .unwrap();
            assert!(!state.is_eligible());
            assert_eq!(state.epoch().value(), 8);
            assert!(state.restore(epoch).is_err());
            state.restore(state.epoch()).unwrap();
            assert!(state.is_eligible());
            assert_eq!(state.epoch().value(), 8);
            assert!(!state.matches_epoch(epoch));
        }
        let mut exhausted = admin(1, Eligibility::Active, u64::MAX);
        let before = exhausted.clone();
        assert!(exhausted.epoch().advance().is_err());
        assert!(
            exhausted
                .invalidate(
                    exhausted.epoch(),
                    SecurityEpoch::new(0),
                    Invalidation::Revoke
                )
                .is_err()
        );
        assert_eq!(exhausted, before);
    }

    fn assert_enrollment(actor: &AdminState) {
        let now = OffsetDateTime::UNIX_EPOCH;
        let expires = now + Duration::minutes(5);
        for (eligibility, purpose) in [
            (Eligibility::Invited, EnrollmentPurpose::Addition),
            (Eligibility::Recovering, EnrollmentPurpose::Recovery),
        ] {
            let mut target = admin(2, eligibility, 7);
            let epoch = target.epoch();
            assert!(
                target
                    .authorize_enrollment(actor, purpose, epoch, now, now)
                    .is_err()
            );
            assert!(
                target
                    .authorize_enrollment(actor, purpose, SecurityEpoch::new(6), now, expires)
                    .is_err()
            );
            let self_actor = admin(2, Eligibility::Active, 7);
            assert!(
                target
                    .authorize_enrollment(&self_actor, purpose, epoch, now, expires)
                    .is_err()
            );
            let ineligible = admin(1, Eligibility::Revoked, 7);
            assert!(
                target
                    .authorize_enrollment(&ineligible, purpose, epoch, now, expires)
                    .is_err()
            );
            let wrong_purpose = if purpose == EnrollmentPurpose::Addition {
                EnrollmentPurpose::Recovery
            } else {
                EnrollmentPurpose::Addition
            };
            assert!(
                target
                    .authorize_enrollment(actor, wrong_purpose, epoch, now, expires)
                    .is_err()
            );
            target
                .authorize_enrollment(actor, purpose, epoch, now, expires)
                .unwrap();
            assert!(
                !target.is_eligible(),
                "authorization is not a completed factor"
            );
            let authorization = target.enrollment().unwrap();
            assert_eq!(authorization.authorizer(), actor.id());
            assert_eq!(authorization.epoch(), epoch);
            assert_eq!(authorization.purpose(), purpose);
            assert_eq!(authorization.authorized_at(), now);
            assert_eq!(authorization.expires_at(), expires);
            assert_eq!(authorization.status(), EnrollmentStatus::Pending);
            assert!(target.check_enrollment(id(3), purpose, epoch, now).is_err());
            assert!(
                target
                    .check_enrollment(target.id(), wrong_purpose, epoch, now)
                    .is_err()
            );
            assert!(
                target
                    .check_enrollment(target.id(), purpose, SecurityEpoch::new(6), now)
                    .is_err()
            );
            assert!(
                target
                    .check_enrollment(target.id(), purpose, epoch, now - Duration::SECOND)
                    .is_err()
            );
            assert!(
                target
                    .check_enrollment(target.id(), purpose, epoch, expires)
                    .is_err()
            );
            assert!(
                target
                    .check_enrollment(target.id(), purpose, epoch, now)
                    .is_ok()
            );
            assert_enrollment_terminal_states(target, purpose, now);
        }
    }

    fn assert_enrollment_terminal_states(
        mut target: AdminState,
        purpose: EnrollmentPurpose,
        now: OffsetDateTime,
    ) {
        let epoch = target.epoch();
        let mut revoked = target.clone();
        revoked.revoke_enrollment(epoch).unwrap();
        assert_eq!(
            revoked.enrollment().unwrap().status(),
            EnrollmentStatus::Revoked
        );
        assert!(
            revoked
                .consume_enrollment(id(2), purpose, epoch, now)
                .is_err()
        );
        let mut invalidated = target.clone();
        invalidated
            .invalidate(epoch, epoch.advance().unwrap(), Invalidation::Recover)
            .unwrap();
        assert!(
            invalidated
                .check_enrollment(id(2), purpose, epoch, now)
                .is_err()
        );
        target
            .consume_enrollment(id(2), purpose, epoch, now)
            .unwrap();
        assert_eq!(
            target.enrollment().unwrap().status(),
            EnrollmentStatus::Consumed
        );
        assert!(
            target
                .consume_enrollment(id(2), purpose, epoch, now)
                .is_err()
        );
        assert!(target.check_enrollment(id(2), purpose, epoch, now).is_err());
    }
}
