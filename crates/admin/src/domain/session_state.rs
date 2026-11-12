//! Admin-only session access with explicit time and authoritative admin state.

use super::{
    admin_state::{AdminState, SecurityEpoch},
    audit::Factor,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, SessionId},
};
use time::{Duration, OffsetDateTime};

/// Trusted authentication result, not an end-user cookie or caller-supplied claim.
#[derive(Debug, Clone)]
pub struct SessionInput {
    pub id: SessionId,
    pub admin_id: AdminId,
    pub epoch: SecurityEpoch,
    pub primary_verified: bool,
    pub factor: Option<Factor>,
    pub issued_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub idle_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionState {
    id: SessionId,
    admin_id: AdminId,
    epoch: SecurityEpoch,
    primary_verified: bool,
    factor: Option<Factor>,
    issued_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    idle_timeout: Duration,
    last_activity_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
}

impl TryFrom<SessionInput> for SessionState {
    type Error = DomainError;

    fn try_from(input: SessionInput) -> Result<Self, Self::Error> {
        if input.expires_at <= input.issued_at || input.idle_timeout <= Duration::ZERO {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self {
            id: input.id,
            admin_id: input.admin_id,
            epoch: input.epoch,
            primary_verified: input.primary_verified,
            factor: input.factor,
            issued_at: input.issued_at,
            expires_at: input.expires_at,
            idle_timeout: input.idle_timeout,
            last_activity_at: input.issued_at,
            revoked_at: None,
        })
    }
}

impl SessionState {
    #[must_use]
    pub const fn id(&self) -> SessionId {
        self.id
    }
    #[must_use]
    pub const fn admin_id(&self) -> AdminId {
        self.admin_id
    }
    #[must_use]
    pub const fn epoch(&self) -> SecurityEpoch {
        self.epoch
    }
    #[must_use]
    pub const fn factor(&self) -> Option<Factor> {
        self.factor
    }
    #[must_use]
    pub const fn last_activity_at(&self) -> OffsetDateTime {
        self.last_activity_at
    }
    #[must_use]
    pub const fn issued_at(&self) -> OffsetDateTime {
        self.issued_at
    }
    #[must_use]
    pub const fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
    #[must_use]
    pub const fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// # Errors
    /// Refuses incomplete authentication, wrong/ineligible admin, stale epoch,
    /// revocation, clock regression, idle expiry or absolute expiry.
    pub fn check_access(&self, admin: &AdminState, now: OffsetDateTime) -> Result<(), DomainError> {
        if !self.primary_verified
            || self.factor.is_none()
            || self.admin_id != admin.id()
            || !admin.matches_epoch(self.epoch)
            || self.revoked_at.is_some()
            || now < self.issued_at
            || now < self.last_activity_at
            || now >= self.expires_at
            || now - self.last_activity_at >= self.idle_timeout
        {
            return Err(DomainError::unauthorised(ErrorCode::AuthenticationRequired));
        }
        Ok(())
    }

    /// Activity updates only a still-authorized session and never its absolute expiry.
    /// # Errors
    /// Same refusals as `check_access`, without changing the previous activity time.
    pub fn record_activity(
        &mut self,
        admin: &AdminState,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.check_access(admin, now)?;
        self.last_activity_at = now;
        Ok(())
    }

    pub fn revoke(&mut self, now: OffsetDateTime) {
        self.revoked_at.get_or_insert(now);
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionInput, SessionState};
    use crate::domain::{
        admin_state::{AdminState, Eligibility, Invalidation, SecurityEpoch},
        audit::Factor,
    };
    use kernel::ids::{AdminId, SessionId};
    use proptest::prelude::*;
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn admin(n: u8) -> AdminState {
        AdminState::new(
            AdminId::from_str(&format!("019a53a0-0000-7000-8000-{n:012}")).unwrap(),
            Eligibility::Active,
            SecurityEpoch::new(7),
        )
    }

    fn input() -> SessionInput {
        SessionInput {
            id: SessionId::from_str("019a53a0-0000-7000-8000-000000000011").unwrap(),
            admin_id: admin(1).id(),
            epoch: SecurityEpoch::new(7),
            primary_verified: true,
            factor: Some(Factor::Passkey),
            issued_at: OffsetDateTime::UNIX_EPOCH,
            expires_at: OffsetDateTime::UNIX_EPOCH + Duration::minutes(10),
            idle_timeout: Duration::minutes(2),
        }
    }

    #[test]
    fn access_requires_primary_factor_and_current_epoch() {
        let actor = admin(1);
        let now = OffsetDateTime::UNIX_EPOCH;
        for factor in [Factor::Passkey, Factor::Totp] {
            let mut source = input();
            source.factor = Some(factor);
            let session = SessionState::try_from(source).unwrap();
            assert_eq!(session.id(), input().id);
            assert_eq!(session.admin_id(), actor.id());
            assert_eq!(session.epoch(), actor.epoch());
            assert_eq!(session.factor(), Some(factor));
            assert!(session.check_access(&actor, now).is_ok());
            assert!(session.check_access(&admin(2), now).is_err());
            assert!(
                session
                    .check_access(&actor, now - Duration::SECOND)
                    .is_err()
            );
            assert!(
                session
                    .check_access(&actor, now + Duration::minutes(2))
                    .is_err()
            );
            assert!(
                session
                    .check_access(&actor, now + Duration::minutes(10))
                    .is_err()
            );
            let mut invalidated = actor.clone();
            invalidated
                .invalidate(
                    actor.epoch(),
                    actor.epoch().advance().unwrap(),
                    Invalidation::Recover,
                )
                .unwrap();
            assert!(session.check_access(&invalidated, now).is_err());
            invalidated.restore(invalidated.epoch()).unwrap();
            assert!(session.check_access(&invalidated, now).is_err());
        }
        for (primary, factor) in [(false, None), (true, None), (false, Some(Factor::Totp))] {
            let mut source = input();
            source.primary_verified = primary;
            source.factor = factor;
            assert!(
                SessionState::try_from(source)
                    .unwrap()
                    .check_access(&actor, now)
                    .is_err()
            );
        }
        assert_expiry_and_revocation(&actor);
        assert_transition_sequences(&actor);
    }

    fn assert_expiry_and_revocation(actor: &AdminState) {
        let now = OffsetDateTime::UNIX_EPOCH;
        for idle in [Duration::ZERO, -Duration::SECOND] {
            let mut source = input();
            source.idle_timeout = idle;
            assert!(SessionState::try_from(source).is_err());
        }
        let mut source = input();
        source.expires_at = now;
        assert!(SessionState::try_from(source).is_err());
        let mut session = SessionState::try_from(input()).unwrap();
        let before = session.clone();
        assert!(
            session
                .record_activity(actor, now - Duration::SECOND)
                .is_err()
        );
        assert_eq!(session, before);
        for minute in 1..10 {
            session
                .record_activity(actor, now + Duration::minutes(minute))
                .unwrap();
        }
        assert_eq!(session.last_activity_at(), now + Duration::minutes(9));
        let before = session.clone();
        assert!(
            session
                .record_activity(actor, now + Duration::minutes(10))
                .is_err()
        );
        assert_eq!(session, before, "activity cannot extend absolute expiry");
        let mut session = SessionState::try_from(input()).unwrap();
        session.revoke(now);
        session.revoke(now + Duration::SECOND);
        assert!(session.check_access(actor, now).is_err());
        assert!(session.record_activity(actor, now).is_err());
        let mut expired = SessionState::try_from(input()).unwrap();
        assert!(
            expired
                .record_activity(actor, now + Duration::minutes(2))
                .is_err()
        );
        assert_eq!(expired.last_activity_at(), now);
    }

    fn assert_transition_sequences(actor: &AdminState) {
        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(
                &prop::collection::vec((0_i64..180, any::<bool>()), 1..100),
                |ops| {
                    let mut session = SessionState::try_from(input()).unwrap();
                    let mut elapsed = 0;
                    let mut last = 0;
                    let mut revoked = false;
                    for (advance, revoke) in ops {
                        elapsed += advance;
                        let now = OffsetDateTime::UNIX_EPOCH + Duration::seconds(elapsed);
                        if revoke {
                            session.revoke(now);
                            revoked = true;
                        }
                        let expected = !revoked && elapsed < 600 && elapsed - last < 120;
                        prop_assert_eq!(session.check_access(actor, now).is_ok(), expected);
                        prop_assert_eq!(session.record_activity(actor, now).is_ok(), expected);
                        if expected {
                            last = elapsed;
                        }
                        prop_assert_eq!(
                            session.last_activity_at(),
                            OffsetDateTime::UNIX_EPOCH + Duration::seconds(last)
                        );
                    }
                    Ok(())
                },
            )
            .unwrap();
    }
}
