//! Pure credential epoch, optimistic revision and TOTP replay rules (FR-002/004).

use super::admin_state::SecurityEpoch;
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, CredentialId},
};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Passkey,
    Totp,
}

/// No vendor credential material or secret bytes are included in this policy value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialState {
    id: CredentialId,
    admin_id: AdminId,
    kind: CredentialKind,
    epoch: SecurityEpoch,
    revision: u64,
    last_totp_step: Option<u64>,
    revoked_at: Option<OffsetDateTime>,
}

impl CredentialState {
    /// # Errors
    /// Revision zero is not an initialized credential revision.
    pub fn new(
        id: CredentialId,
        admin_id: AdminId,
        kind: CredentialKind,
        epoch: SecurityEpoch,
        revision: u64,
    ) -> Result<Self, DomainError> {
        if revision == 0 {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self {
            id,
            admin_id,
            kind,
            epoch,
            revision,
            last_totp_step: None,
            revoked_at: None,
        })
    }
    #[must_use]
    pub const fn id(&self) -> CredentialId {
        self.id
    }
    #[must_use]
    pub const fn admin_id(&self) -> AdminId {
        self.admin_id
    }
    #[must_use]
    pub const fn kind(&self) -> CredentialKind {
        self.kind
    }
    #[must_use]
    pub const fn epoch(&self) -> SecurityEpoch {
        self.epoch
    }
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    #[must_use]
    pub const fn last_totp_step(&self) -> Option<u64> {
        self.last_totp_step
    }

    /// # Errors
    /// A revoked credential or stale epoch/revision is unusable.
    pub fn check_current(&self, epoch: SecurityEpoch, revision: u64) -> Result<(), DomainError> {
        if self.revoked_at.is_some() || self.epoch != epoch || self.revision != revision {
            return Err(stale());
        }
        Ok(())
    }

    /// # Errors
    /// Refuses stale snapshots and non-increasing revisions, including wraparound.
    pub fn advance_revision(
        &mut self,
        epoch: SecurityEpoch,
        expected: u64,
        next: u64,
    ) -> Result<(), DomainError> {
        self.check_current(epoch, expected)?;
        if next <= expected {
            return Err(stale());
        }
        self.revision = next;
        Ok(())
    }

    /// Used for enrollment confirmation as well as sign-in.
    /// # Errors
    /// Refuses wrong kind, stale/revoked state and a previously accepted or older step.
    pub fn accept_totp_step(
        &mut self,
        epoch: SecurityEpoch,
        revision: u64,
        step: u64,
    ) -> Result<(), DomainError> {
        self.check_current(epoch, revision)?;
        if self.kind != CredentialKind::Totp || self.last_totp_step.is_some_and(|last| step <= last)
        {
            return Err(stale());
        }
        self.last_totp_step = Some(step);
        Ok(())
    }

    pub fn revoke(&mut self, now: OffsetDateTime) {
        self.revoked_at.get_or_insert(now);
    }
}

fn stale() -> DomainError {
    DomainError::conflict(ErrorCode::StaleRevision)
}

#[cfg(test)]
mod tests {
    use super::{CredentialKind, CredentialState};
    use crate::domain::admin_state::SecurityEpoch;
    use kernel::ids::{AdminId, CredentialId};
    use proptest::prelude::*;
    use std::str::FromStr;
    use time::OffsetDateTime;

    fn fixture(kind: CredentialKind) -> CredentialState {
        CredentialState::new(
            CredentialId::from_str("019a53a0-0000-7000-8000-000000000011").unwrap(),
            AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
            kind,
            SecurityEpoch::new(7),
            1,
        )
        .unwrap()
    }

    #[test]
    fn credential_revision_and_revocation_are_epoch_bound() {
        for kind in [CredentialKind::Passkey, CredentialKind::Totp] {
            let mut state = fixture(kind);
            let epoch = SecurityEpoch::new(7);
            assert_eq!(state.kind(), kind);
            assert_eq!(state.epoch(), epoch);
            assert_eq!(state.id(), fixture(kind).id());
            assert_eq!(state.admin_id(), fixture(kind).admin_id());
            assert!(state.check_current(epoch, 1).is_ok());
            assert!(state.check_current(SecurityEpoch::new(6), 1).is_err());
            assert!(state.check_current(SecurityEpoch::new(8), 1).is_err());
            assert!(state.check_current(epoch, 0).is_err());
            let before = state.clone();
            assert!(state.advance_revision(epoch, 1, 1).is_err());
            assert!(state.advance_revision(epoch, 0, 2).is_err());
            assert_eq!(state, before);
            state.advance_revision(epoch, 1, 2).unwrap();
            assert_eq!(state.revision(), 2);
            assert!(state.check_current(epoch, 1).is_err());
            state.revoke(OffsetDateTime::UNIX_EPOCH);
            state.revoke(OffsetDateTime::UNIX_EPOCH);
            assert!(state.check_current(epoch, 2).is_err());
            assert!(state.advance_revision(epoch, 2, 3).is_err());
            assert!(state.accept_totp_step(epoch, 2, 100).is_err());
        }
        let mut passkey = fixture(CredentialKind::Passkey);
        assert!(
            passkey
                .accept_totp_step(SecurityEpoch::new(7), 1, 100)
                .is_err()
        );
        let mut totp = fixture(CredentialKind::Totp);
        let epoch = SecurityEpoch::new(7);
        assert_eq!(totp.last_totp_step(), None);
        totp.accept_totp_step(epoch, 1, 0).unwrap();
        assert_eq!(totp.last_totp_step(), Some(0));
        assert!(totp.accept_totp_step(epoch, 1, 0).is_err());
        assert!(totp.accept_totp_step(SecurityEpoch::new(8), 1, 1).is_err());
        totp.accept_totp_step(epoch, 1, u64::MAX).unwrap();
        assert!(totp.accept_totp_step(epoch, 1, 0).is_err());
        assert!(totp.accept_totp_step(epoch, 1, u64::MAX).is_err());

        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(
                &prop::collection::vec((any::<u16>(), any::<bool>()), 1..100),
                |ops| {
                    let mut credential = fixture(CredentialKind::Totp);
                    let mut last = None;
                    let mut revoked = false;
                    for (step, revoke) in ops {
                        if revoke {
                            credential.revoke(OffsetDateTime::UNIX_EPOCH);
                            revoked = true;
                        }
                        let step = u64::from(step);
                        let expected = !revoked && last.is_none_or(|previous| step > previous);
                        prop_assert_eq!(
                            credential.accept_totp_step(epoch, 1, step).is_ok(),
                            expected
                        );
                        if expected {
                            last = Some(step);
                        }
                        prop_assert_eq!(credential.last_totp_step(), last);
                        prop_assert!(credential.check_current(SecurityEpoch::new(8), 1).is_err());
                    }
                    Ok(())
                },
            )
            .unwrap();
    }
}
