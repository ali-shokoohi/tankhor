//! FR-004/005, AC-ADM-004/007/043: caller-transaction admin persistence.

use admin::{
    domain::admin_state::{AdminState, EnrollmentAuthorization, Invalidation, SecurityEpoch},
    ports::admin_repository::{
        AdminListQuery, AdminPage, AdminRepository, AuthorizeEnrollment, IdentityDigest,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::AdminId,
    ports::unit_of_work::TxHandle,
};
use time::OffsetDateTime;

use super::store::{FakeState, FakeStore};

#[derive(Debug, Clone)]
pub struct FakeAdminRepository {
    store: FakeStore,
}

impl FakeAdminRepository {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }
}

fn stale() -> DomainError {
    DomainError::conflict(ErrorCode::StaleRevision)
}

fn target_mut(state: &mut FakeState, target: AdminId) -> Result<&mut AdminState, DomainError> {
    state
        .admins
        .get_mut(&target)
        .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))
}

fn check_authorization(
    target: &AdminState,
    expected: &EnrollmentAuthorization,
) -> Result<(), DomainError> {
    if target.enrollment() != Some(expected) {
        return Err(stale());
    }
    Ok(())
}

#[async_trait::async_trait]
impl AdminRepository for FakeAdminRepository {
    async fn find_by_identity(
        &self,
        tx: &mut TxHandle,
        identity: IdentityDigest,
    ) -> Result<Option<AdminState>, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state
                .admin_identities
                .get(identity.as_bytes())
                .and_then(|id| state.admins.get(id))
                .cloned())
        })
    }

    async fn list(
        &self,
        tx: &mut TxHandle,
        query: AdminListQuery,
    ) -> Result<AdminPage, DomainError> {
        self.store.with_read(tx, |state| {
            let mut entries: Vec<_> = state
                .admins
                .values()
                .filter(|value| query.after().is_none_or(|after| value.id() > after))
                .take(usize::from(query.limit()) + 1)
                .cloned()
                .collect();
            let more = entries.len() > usize::from(query.limit());
            entries.truncate(usize::from(query.limit()));
            let next = if more {
                entries.last().map(AdminState::id)
            } else {
                None
            };
            Ok(AdminPage { entries, next })
        })
    }

    async fn get(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
    ) -> Result<Option<AdminState>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.admins.get(&target).cloned()))
    }

    async fn lock(
        &self,
        tx: &mut TxHandle,
        mut targets: Vec<AdminId>,
    ) -> Result<Vec<AdminState>, DomainError> {
        // The shared fake UoW holds its serialization lock until commit/rollback/drop.
        self.store.with_read(tx, |state| {
            targets.sort_unstable();
            targets.dedup();
            Ok(targets
                .into_iter()
                .filter_map(|id| state.admins.get(&id).cloned())
                .collect())
        })
    }

    async fn invite(
        &self,
        tx: &mut TxHandle,
        value: AdminState,
        identity: IdentityDigest,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if state.admins.contains_key(&value.id())
                || state.admin_identities.contains_key(identity.as_bytes())
            {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            state
                .admin_identities
                .insert(*identity.as_bytes(), value.id());
            state.admins.insert(value.id(), value);
            Ok(())
        })
    }

    async fn invalidate_epoch(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: SecurityEpoch,
        next: SecurityEpoch,
        kind: Invalidation,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            target_mut(state, target)?.invalidate(expected, next, kind)
        })
    }

    async fn restore(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: SecurityEpoch,
    ) -> Result<(), DomainError> {
        self.store
            .with_write(tx, |state| target_mut(state, target)?.restore(expected))
    }

    async fn authorize_enrollment(
        &self,
        tx: &mut TxHandle,
        command: AuthorizeEnrollment,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            let actor = state
                .admins
                .get(&command.actor)
                .cloned()
                .ok_or_else(|| DomainError::forbidden(ErrorCode::PermissionDenied))?;
            let target = target_mut(state, command.target)?;
            if target.enrollment() != command.expected_authorization.as_ref() {
                return Err(stale());
            }
            target.authorize_enrollment(
                &actor,
                command.purpose,
                command.expected_epoch,
                command.authorized_at,
                command.expires_at,
            )
        })
    }

    async fn enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
    ) -> Result<Option<EnrollmentAuthorization>, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state
                .admins
                .get(&target)
                .and_then(AdminState::enrollment)
                .cloned())
        })
    }

    async fn consume_enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: EnrollmentAuthorization,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            let value = target_mut(state, target)?;
            check_authorization(value, &expected)?;
            value.consume_enrollment(target, expected.purpose(), expected.epoch(), now)
        })
    }

    async fn revoke_enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: EnrollmentAuthorization,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            let value = target_mut(state, target)?;
            check_authorization(value, &expected)?;
            value.revoke_enrollment(expected.epoch())
        })
    }
}
