//! FR-002/004, FR-CON-02: opaque factors persisted in the caller's transaction.

use crate::domain::{
    admin_state::SecurityEpoch,
    credential_state::{CredentialKind, CredentialState},
    secrets::SecretBytes,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, CredentialId},
    ports::unit_of_work::TxHandle,
};
use time::OffsetDateTime;

/// Adapter-owned serialized public credential state or an authenticated encrypted
/// TOTP envelope. Never plaintext TOTP secrets or vendor-library types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialMaterial {
    Passkey {
        credential_id: SecretBytes,
        credential: SecretBytes,
    },
    Totp {
        envelope: SecretBytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRecord {
    state: CredentialState,
    material: CredentialMaterial,
}

impl CredentialRecord {
    /// # Errors
    /// Rejects mismatched kinds and empty opaque material.
    pub fn new(state: CredentialState, material: CredentialMaterial) -> Result<Self, DomainError> {
        let valid = match &material {
            CredentialMaterial::Passkey {
                credential_id,
                credential,
            } => {
                state.kind() == CredentialKind::Passkey
                    && !credential_id.expose().is_empty()
                    && !credential.expose().is_empty()
            }
            CredentialMaterial::Totp { envelope } => {
                state.kind() == CredentialKind::Totp && !envelope.expose().is_empty()
            }
        };
        if !valid {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self { state, material })
    }

    #[must_use]
    pub const fn state(&self) -> &CredentialState {
        &self.state
    }

    #[must_use]
    pub const fn material(&self) -> &CredentialMaterial {
        &self.material
    }
}

/// Persist the complete verified library counter/backup state with revision CAS;
/// the globally unique credential identity cannot change through this operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyRevision {
    pub id: CredentialId,
    pub epoch: SecurityEpoch,
    pub expected_revision: u64,
    pub next_revision: u64,
    pub credential: SecretBytes,
}

#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait AdminCredentialRepository: Send + Sync {
    /// Active, unrevoked credentials in ascending credential-ID order.
    async fn list_active(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
    ) -> Result<Vec<CredentialRecord>, DomainError>;

    /// Reject duplicate record IDs and globally duplicate passkey identities,
    /// including identities belonging to revoked credentials. Caller owns commit.
    async fn insert(
        &self,
        tx: &mut TxHandle,
        credential: CredentialRecord,
    ) -> Result<(), DomainError>;

    /// Read/lock even revoked state for final CAS checks and recovery.
    async fn lock(
        &self,
        tx: &mut TxHandle,
        id: CredentialId,
    ) -> Result<Option<CredentialRecord>, DomainError>;

    async fn compare_revision(
        &self,
        tx: &mut TxHandle,
        update: PasskeyRevision,
    ) -> Result<(), DomainError>;

    /// Strictly increasing matched TOTP step under epoch/revision CAS, for both
    /// enrollment confirmation and sign-in. Refusal leaves all state unchanged.
    async fn accept_totp_step(
        &self,
        tx: &mut TxHandle,
        id: CredentialId,
        epoch: SecurityEpoch,
        revision: u64,
        step: u64,
    ) -> Result<(), DomainError>;

    /// Idempotent target-only revocation; never affects another administrator.
    async fn revoke_all(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;
}

#[cfg(test)]
mod tests {
    use super::{
        AdminCredentialRepository, CredentialMaterial, CredentialRecord,
        MockAdminCredentialRepository, PasskeyRevision,
    };
    use crate::domain::{
        admin_state::SecurityEpoch,
        credential_state::{CredentialKind, CredentialState},
        secrets::SecretBytes,
    };
    use kernel::{
        ids::{AdminId, CredentialId},
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    use time::OffsetDateTime;

    fn fixture() -> CredentialRecord {
        let admin = AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap();
        let id = CredentialId::from_str("019a53a0-0000-7000-8000-000000000011").unwrap();
        let epoch = SecurityEpoch::new(4);
        let state = CredentialState::new(id, admin, CredentialKind::Passkey, epoch, 1).unwrap();
        let material = CredentialMaterial::Passkey {
            credential_id: SecretBytes::new(b"synthetic-id".to_vec()),
            credential: SecretBytes::new(b"synthetic-library-state".to_vec()),
        };
        let record = CredentialRecord::new(state.clone(), material.clone()).unwrap();
        assert_eq!(record.state(), &state);
        assert_eq!(record.material(), &material);
        assert!(!format!("{record:?}").contains("synthetic"));
        assert!(
            CredentialRecord::new(
                state.clone(),
                CredentialMaterial::Totp {
                    envelope: SecretBytes::new(vec![1])
                }
            )
            .is_err()
        );
        assert!(
            CredentialRecord::new(
                state.clone(),
                CredentialMaterial::Passkey {
                    credential_id: SecretBytes::new(vec![]),
                    credential: SecretBytes::new(vec![1])
                }
            )
            .is_err()
        );
        record
    }

    #[tokio::test]
    async fn admin_credential_repository_port_is_mockable() {
        let record = fixture();
        let admin = record.state().admin_id();
        let id = record.state().id();
        let epoch = record.state().epoch();
        let mut mock = MockAdminCredentialRepository::new();
        let mut sequence = mockall::Sequence::new();
        let store = StoreId::new();
        let mut tx = TxHandle::new(store, TransactionMode::ReadWrite, ());
        let expected = record.clone();
        mock.expect_insert()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |tx, value| tx.store_id() == store && value == &expected)
            .return_once(|_, _| Ok(()));
        let found = record.clone();
        mock.expect_list_active()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |_, target| *target == admin)
            .return_once(move |_, _| Ok(vec![found]));
        let found = record.clone();
        mock.expect_lock()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |_, target| *target == id)
            .return_once(move |_, _| Ok(Some(found)));
        mock.expect_compare_revision()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |_, update| {
                update.id == id
                    && update.epoch == epoch
                    && update.expected_revision == 1
                    && update.next_revision == 2
                    && update.credential.expose() == b"updated-library-state"
            })
            .return_once(|_, _| Ok(()));
        mock.expect_accept_totp_step()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |_, target, current, revision, step| {
                *target == id && *current == epoch && *revision == 2 && *step == 100
            })
            .return_once(|_, _, _, _, _| Ok(()));
        mock.expect_revoke_all()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |_, target, now| *target == admin && *now == OffsetDateTime::UNIX_EPOCH)
            .return_once(|_, _, _| Ok(()));
        let repository: &dyn AdminCredentialRepository = &mock;
        repository.insert(&mut tx, record.clone()).await.unwrap();
        assert_eq!(
            repository.list_active(&mut tx, admin).await.unwrap(),
            vec![record.clone()]
        );
        assert_eq!(repository.lock(&mut tx, id).await.unwrap(), Some(record));
        repository
            .compare_revision(
                &mut tx,
                PasskeyRevision {
                    id,
                    epoch,
                    expected_revision: 1,
                    next_revision: 2,
                    credential: SecretBytes::new(b"updated-library-state".to_vec()),
                },
            )
            .await
            .unwrap();
        repository
            .accept_totp_step(&mut tx, id, epoch, 2, 100)
            .await
            .unwrap();
        repository
            .revoke_all(&mut tx, admin, OffsetDateTime::UNIX_EPOCH)
            .await
            .unwrap();
    }
}
