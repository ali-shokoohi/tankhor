//! FR-002/004, AC-ADM-002/043: serialized, caller-owned credential transactions.

use admin::{
    domain::admin_state::SecurityEpoch,
    ports::admin_credential_repository::{
        AdminCredentialRepository, CredentialMaterial, CredentialRecord, PasskeyRevision,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, CredentialId},
    ports::unit_of_work::TxHandle,
};
use time::OffsetDateTime;

use super::store::FakeStore;

#[derive(Debug, Clone)]
pub struct FakeAdminCredentialRepository {
    store: FakeStore,
}

impl FakeAdminCredentialRepository {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }
}

fn missing() -> DomainError {
    DomainError::validation(ErrorCode::NotFound)
}

#[async_trait::async_trait]
impl AdminCredentialRepository for FakeAdminCredentialRepository {
    async fn list_active(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
    ) -> Result<Vec<CredentialRecord>, DomainError> {
        self.store.with_read(tx, |store| {
            Ok(store
                .admin_credentials
                .values()
                .filter(|record| {
                    let state = record.state();
                    state.admin_id() == admin
                        && state.check_current(state.epoch(), state.revision()).is_ok()
                })
                .cloned()
                .collect())
        })
    }

    async fn insert(
        &self,
        tx: &mut TxHandle,
        credential: CredentialRecord,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |store| {
            if !store.admins.contains_key(&credential.state().admin_id()) {
                return Err(missing());
            }
            if store
                .admin_credentials
                .contains_key(&credential.state().id())
                || store.admin_credentials.values().any(|existing| {
                    match (existing.material(), credential.material()) {
                        (
                            CredentialMaterial::Passkey {
                                credential_id: left,
                                ..
                            },
                            CredentialMaterial::Passkey {
                                credential_id: right,
                                ..
                            },
                        ) => left == right,
                        _ => false,
                    }
                })
            {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            store
                .admin_credentials
                .insert(credential.state().id(), credential);
            Ok(())
        })
    }

    async fn lock(
        &self,
        tx: &mut TxHandle,
        id: CredentialId,
    ) -> Result<Option<CredentialRecord>, DomainError> {
        self.store
            .with_read(tx, |store| Ok(store.admin_credentials.get(&id).cloned()))
    }

    async fn compare_revision(
        &self,
        tx: &mut TxHandle,
        update: PasskeyRevision,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |store| {
            let record = store
                .admin_credentials
                .get_mut(&update.id)
                .ok_or_else(missing)?;
            let CredentialMaterial::Passkey { credential_id, .. } = record.material() else {
                return Err(DomainError::conflict(ErrorCode::StaleRevision));
            };
            let mut state = record.state().clone();
            state.advance_revision(update.epoch, update.expected_revision, update.next_revision)?;
            let replacement = CredentialRecord::new(
                state,
                CredentialMaterial::Passkey {
                    credential_id: credential_id.clone(),
                    credential: update.credential,
                },
            )?;
            *record = replacement;
            Ok(())
        })
    }

    async fn accept_totp_step(
        &self,
        tx: &mut TxHandle,
        id: CredentialId,
        epoch: SecurityEpoch,
        revision: u64,
        step: u64,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |store| {
            let record = store.admin_credentials.get_mut(&id).ok_or_else(missing)?;
            let mut state = record.state().clone();
            state.accept_totp_step(epoch, revision, step)?;
            *record = CredentialRecord::new(state, record.material().clone())?;
            Ok(())
        })
    }

    async fn revoke_all(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |store| {
            for record in store
                .admin_credentials
                .values_mut()
                .filter(|record| record.state().admin_id() == admin)
            {
                let mut state = record.state().clone();
                state.revoke(now);
                *record = CredentialRecord::new(state, record.material().clone())?;
            }
            Ok(())
        })
    }
}
