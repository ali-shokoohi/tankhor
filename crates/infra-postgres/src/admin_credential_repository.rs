//! Opaque administrator credentials with database-enforced optimistic CAS.

use admin::domain::admin_state::SecurityEpoch;
use admin::domain::credential_state::{CredentialKind, CredentialState};
use admin::domain::secrets::SecretBytes;
use admin::ports::admin_credential_repository::{
    AdminCredentialRepository, CredentialMaterial, CredentialRecord, PasskeyRevision,
};
use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{AdminId, CredentialId};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{map_sql, stale};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresAdminCredentialRepository {
    uow: PostgresUnitOfWork,
}

impl PostgresAdminCredentialRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

#[async_trait::async_trait]
impl AdminCredentialRepository for PostgresAdminCredentialRepository {
    async fn list_active(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
    ) -> Result<Vec<CredentialRecord>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        sqlx::query_as!(
            CredentialRow,
            "SELECT c.id, c.admin_id, c.kind, c.security_epoch::text AS \"epoch!\",
                    c.credential_revision::text AS \"revision!\", c.last_totp_step::text AS last_totp_step,
                    c.revoked_at, c.webauthn_credential_id,
                    c.webauthn_public_key AS webauthn_credential,
                    c.totp_ciphertext AS totp_envelope
             FROM admin_credentials c
             JOIN admins a ON a.id = c.admin_id
             WHERE c.admin_id = $1 AND c.revoked_at IS NULL
               AND c.security_epoch = a.security_epoch
             ORDER BY c.id",
            admin.as_uuid(),
        )
        .fetch_all(connection)
        .await
        .map_err(|error| map_sql(&error))?
        .into_iter()
        .map(CredentialRow::into_record)
        .collect()
    }

    async fn insert(
        &self,
        tx: &mut TxHandle,
        credential: CredentialRecord,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let state = credential.state();
        let (kind, credential_id, passkey, envelope) = material(credential.material());
        let inserted = sqlx::query!(
            "INSERT INTO admin_credentials (
                 id, admin_id, kind, security_epoch, credential_revision,
                 webauthn_credential_id, webauthn_public_key,
                 totp_key_id, totp_nonce, totp_ciphertext
             ) SELECT $1, $2, $3, $4::text::numeric, $5::text::numeric, $6, $7,
                      CASE WHEN $8::bytea IS NULL THEN NULL ELSE 'opaque-v1' END,
                      CASE WHEN $8::bytea IS NULL THEN NULL ELSE '\\x00'::bytea END,
                      $8
             FROM admins WHERE id = $2 AND security_epoch = $4::text::numeric
             ON CONFLICT DO NOTHING",
            state.id().as_uuid(),
            state.admin_id().as_uuid(),
            kind,
            state.epoch().value().to_string(),
            state.revision().to_string(),
            credential_id,
            passkey,
            envelope,
        )
        .execute(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if inserted.rows_affected() == 0 {
            let current_epoch = sqlx::query_scalar!(
                "SELECT security_epoch::text AS \"epoch!\" FROM admins WHERE id = $1",
                state.admin_id().as_uuid(),
            )
            .fetch_optional(connection)
            .await
            .map_err(|error| map_sql(&error))?;
            return match current_epoch {
                None => Err(DomainError::validation(ErrorCode::NotFound)),
                Some(epoch) if epoch != state.epoch().value().to_string() => Err(stale()),
                Some(_) => Err(DomainError::conflict(ErrorCode::IdempotencyConflict)),
            };
        }
        Ok(())
    }

    async fn lock(
        &self,
        tx: &mut TxHandle,
        id: CredentialId,
    ) -> Result<Option<CredentialRecord>, DomainError> {
        let read_only = tx.mode() == TransactionMode::ReadOnly;
        let connection = self.uow.readable_connection(tx)?;
        let row = if read_only {
            sqlx::query_as!(
                CredentialRow,
                "SELECT id, admin_id, kind, security_epoch::text AS \"epoch!\",
                        credential_revision::text AS \"revision!\",
                        last_totp_step::text AS last_totp_step, revoked_at,
                        webauthn_credential_id, webauthn_public_key AS webauthn_credential,
                        totp_ciphertext AS totp_envelope
                 FROM admin_credentials WHERE id = $1",
                id.as_uuid(),
            )
            .fetch_optional(connection)
            .await
        } else {
            sqlx::query_as!(
                CredentialRow,
                "SELECT id, admin_id, kind, security_epoch::text AS \"epoch!\",
                        credential_revision::text AS \"revision!\",
                        last_totp_step::text AS last_totp_step, revoked_at,
                        webauthn_credential_id, webauthn_public_key AS webauthn_credential,
                        totp_ciphertext AS totp_envelope
                 FROM admin_credentials WHERE id = $1 FOR UPDATE",
                id.as_uuid(),
            )
            .fetch_optional(connection)
            .await
        };
        row.map_err(|error| map_sql(&error))?
            .map(CredentialRow::into_record)
            .transpose()
    }

    async fn compare_revision(
        &self,
        tx: &mut TxHandle,
        update: PasskeyRevision,
    ) -> Result<(), DomainError> {
        if update.credential.expose().is_empty() {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        let connection = self.uow.writable_connection(tx)?;
        let changed = sqlx::query!(
            "UPDATE admin_credentials
             SET credential_revision = $4::text::numeric, webauthn_public_key = $5
             WHERE id = $1 AND kind = 'passkey' AND security_epoch = $2::text::numeric
               AND credential_revision = $3::text::numeric AND revoked_at IS NULL
               AND $4::text::numeric > $3::text::numeric",
            update.id.as_uuid(),
            update.epoch.value().to_string(),
            update.expected_revision.to_string(),
            update.next_revision.to_string(),
            update.credential.expose(),
        )
        .execute(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if changed.rows_affected() == 0 {
            let exists = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM admin_credentials WHERE id = $1) AS \"exists!\"",
                update.id.as_uuid(),
            )
            .fetch_one(connection)
            .await
            .map_err(|error| map_sql(&error))?;
            return if exists {
                Err(stale())
            } else {
                Err(DomainError::validation(ErrorCode::NotFound))
            };
        }
        Ok(())
    }

    async fn accept_totp_step(
        &self,
        tx: &mut TxHandle,
        id: CredentialId,
        epoch: SecurityEpoch,
        revision: u64,
        step: u64,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let changed = sqlx::query!(
            "UPDATE admin_credentials SET last_totp_step = $4::text::numeric
             WHERE id = $1 AND kind = 'totp' AND security_epoch = $2::text::numeric
               AND credential_revision = $3::text::numeric AND revoked_at IS NULL
               AND (last_totp_step IS NULL OR last_totp_step < $4::text::numeric)",
            id.as_uuid(),
            epoch.value().to_string(),
            revision.to_string(),
            step.to_string(),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if changed.rows_affected() == 0 {
            return Err(stale());
        }
        Ok(())
    }

    async fn revoke_all(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        sqlx::query!(
            "UPDATE admin_credentials SET revoked_at = COALESCE(revoked_at, $2)
             WHERE admin_id = $1",
            admin.as_uuid(),
            now,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }
}

struct CredentialRow {
    id: Uuid,
    admin_id: Uuid,
    kind: String,
    epoch: String,
    revision: String,
    last_totp_step: Option<String>,
    revoked_at: Option<OffsetDateTime>,
    webauthn_credential_id: Option<Vec<u8>>,
    webauthn_credential: Option<Vec<u8>>,
    totp_envelope: Option<Vec<u8>>,
}

impl CredentialRow {
    fn into_record(self) -> Result<CredentialRecord, DomainError> {
        let id = CredentialId::try_new(self.id)
            .map_err(|_| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))?;
        let kind = match self.kind.as_str() {
            "passkey" => CredentialKind::Passkey,
            "totp" => CredentialKind::Totp,
            _ => return Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)),
        };
        let mut state = CredentialState::new(
            id,
            AdminId::try_new(self.admin_id)
                .map_err(|_| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))?,
            kind,
            SecurityEpoch::new(number(&self.epoch)?),
            number(&self.revision)?,
        )?;
        if let Some(step) = self.last_totp_step {
            state.accept_totp_step(state.epoch(), state.revision(), number(&step)?)?;
        }
        if let Some(revoked_at) = self.revoked_at {
            state.revoke(revoked_at);
        }
        let material = match kind {
            CredentialKind::Passkey => CredentialMaterial::Passkey {
                credential_id: SecretBytes::new(required(
                    self.webauthn_credential_id,
                    "passkey credential id",
                )?),
                credential: SecretBytes::new(required(
                    self.webauthn_credential,
                    "passkey credential",
                )?),
            },
            CredentialKind::Totp => CredentialMaterial::Totp {
                envelope: SecretBytes::new(required(self.totp_envelope, "TOTP envelope")?),
            },
        };
        CredentialRecord::new(state, material)
    }
}

type MaterialParts<'a> = (
    &'static str,
    Option<&'a [u8]>,
    Option<&'a [u8]>,
    Option<&'a [u8]>,
);

fn material(material: &CredentialMaterial) -> MaterialParts<'_> {
    match material {
        CredentialMaterial::Passkey {
            credential_id,
            credential,
        } => (
            "passkey",
            Some(credential_id.expose()),
            Some(credential.expose()),
            None,
        ),
        CredentialMaterial::Totp { envelope } => ("totp", None, None, Some(envelope.expose())),
    }
}

fn required(value: Option<Vec<u8>>, _field: &'static str) -> Result<Vec<u8>, DomainError> {
    value.ok_or_else(|| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
}

fn number(value: &str) -> Result<u64, DomainError> {
    value
        .parse()
        .map_err(|_| DomainError::unavailable(ErrorCode::PrerequisiteUnavailable))
}
