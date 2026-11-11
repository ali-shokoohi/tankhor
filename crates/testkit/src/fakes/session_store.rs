//! Transactional `SessionStore` fake; all state shares the caller's serialized `UoW`.

use super::store::{FakeState, FakeStore};
use admin::{
    domain::{auth_state::BindingDigest, secrets::SecretBytes, session_state::SessionState},
    ports::session_store::{PendingCheck, PendingRecord, PendingView, SessionRecord, SessionStore},
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, SessionId},
    ports::unit_of_work::TxHandle,
};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct FakeSessionStore {
    store: FakeStore,
}

fn denied() -> DomainError {
    DomainError::forbidden(ErrorCode::PermissionDenied)
}
fn unauthenticated() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
fn conflict() -> DomainError {
    DomainError::conflict(ErrorCode::IdempotencyConflict)
}

impl FakeSessionStore {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }

    fn mutate_pending<R>(
        &self,
        tx: &mut TxHandle,
        check: &PendingCheck,
        f: impl FnOnce(
            &mut PendingRecord,
            &admin::domain::admin_state::AdminState,
        ) -> Result<R, DomainError>,
    ) -> Result<R, DomainError> {
        self.store.with_write(tx, |state| {
            let admin = state.admins.get(&check.binding.target).ok_or_else(denied)?;
            let key = *check.reference.as_bytes();
            let mut pending = state.admin_pending.get(&key).ok_or_else(denied)?.clone();
            let result = f(&mut pending, admin)?;
            state.admin_pending.insert(key, pending);
            Ok(result)
        })
    }
}

fn read_pending<R>(
    state: &FakeState,
    check: &PendingCheck,
    f: impl FnOnce(&PendingRecord, &admin::domain::admin_state::AdminState) -> Result<R, DomainError>,
) -> Result<R, DomainError> {
    let admin = state.admins.get(&check.binding.target).ok_or_else(denied)?;
    let pending = state
        .admin_pending
        .get(check.reference.as_bytes())
        .ok_or_else(denied)?;
    f(pending, admin)
}

#[async_trait::async_trait]
impl SessionStore for FakeSessionStore {
    async fn insert(&self, tx: &mut TxHandle, session: SessionRecord) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if !state.admins.contains_key(&session.state.admin_id()) {
                return Err(DomainError::validation(ErrorCode::NotFound));
            }
            let key = *session.digest.as_bytes();
            if state.admin_sessions.contains_key(&key)
                || state
                    .admin_sessions
                    .values()
                    .any(|s| s.state.id() == session.state.id())
            {
                return Err(conflict());
            }
            state.admin_sessions.insert(key, session);
            Ok(())
        })
    }

    async fn lookup(
        &self,
        tx: &mut TxHandle,
        digest: BindingDigest,
    ) -> Result<Option<SessionState>, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state
                .admin_sessions
                .get(digest.as_bytes())
                .map(|s| s.state.clone()))
        })
    }

    async fn check_current(
        &self,
        tx: &mut TxHandle,
        digest: BindingDigest,
        now: OffsetDateTime,
    ) -> Result<SessionState, DomainError> {
        self.store.with_write(tx, |state| {
            let mut record = state
                .admin_sessions
                .get(digest.as_bytes())
                .ok_or_else(unauthenticated)?
                .clone();
            let admin = state
                .admins
                .get(&record.state.admin_id())
                .ok_or_else(unauthenticated)?;
            record.state.record_activity(admin, now)?;
            let session = record.state.clone();
            state.admin_sessions.insert(*digest.as_bytes(), record);
            Ok(session)
        })
    }

    async fn revoke_one(
        &self,
        tx: &mut TxHandle,
        id: SessionId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if let Some(record) = state
                .admin_sessions
                .values_mut()
                .find(|s| s.state.id() == id)
            {
                record.state.revoke(now);
            }
            Ok(())
        })
    }

    async fn revoke_all(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            for record in state
                .admin_sessions
                .values_mut()
                .filter(|s| s.state.admin_id() == admin)
            {
                record.state.revoke(now);
            }
            Ok(())
        })
    }

    async fn create_pending(
        &self,
        tx: &mut TxHandle,
        pending: PendingRecord,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if !state.admins.contains_key(&pending.state().binding().target) {
                return Err(DomainError::validation(ErrorCode::NotFound));
            }
            let key = *pending.reference().as_bytes();
            if state.admin_pending.contains_key(&key)
                || state
                    .admin_pending
                    .values()
                    .any(|p| p.state().id() == pending.state().id())
            {
                return Err(conflict());
            }
            state.admin_pending.insert(key, pending);
            Ok(())
        })
    }

    async fn read_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<PendingView, DomainError> {
        self.store.with_read(tx, |state| {
            read_pending(state, &check, |p, a| p.view(&check, a))
        })
    }

    async fn verification_state(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<SecretBytes, DomainError> {
        self.store.with_read(tx, |state| {
            read_pending(state, &check, |p, a| p.verification_state(&check, a))
        })
    }

    async fn consume_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<(), DomainError> {
        self.mutate_pending(tx, &check, |p, a| p.consume(&check, a))
    }

    async fn supersede_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<(), DomainError> {
        self.mutate_pending(tx, &check, |p, a| p.supersede(&check, a))
    }

    async fn revoke_pending(&self, tx: &mut TxHandle, admin: AdminId) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            for pending in state
                .admin_pending
                .values_mut()
                .filter(|p| p.state().binding().target == admin)
            {
                pending.revoke();
            }
            Ok(())
        })
    }

    async fn present_provisioning(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<SecretBytes, DomainError> {
        self.mutate_pending(tx, &check, |p, a| p.present_provisioning(&check, a))
    }
}
