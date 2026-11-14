//! Canonical launch-flag registry and immutable boolean history.

use std::str::FromStr;

use admin::domain::flag::{FlagKey, launch_flags};
use admin::ports::flag_repository::{FlagRecord, FlagRepository, FlagVersion};
use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{AdminId, VersionId};
use kernel::ports::unit_of_work::TxHandle;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{map_sql, stale};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresFlagRepository {
    uow: PostgresUnitOfWork,
}
impl PostgresFlagRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

struct FlagRow {
    id: Uuid,
    flag_key: String,
    enabled: bool,
    actor_ref: Uuid,
    applied_at: OffsetDateTime,
}
impl FlagRow {
    fn into_version(self) -> Result<FlagVersion, DomainError> {
        Ok(FlagVersion::new(
            VersionId::try_new(self.id).map_err(|_| corrupt())?,
            FlagKey::from_str(&self.flag_key)?,
            self.enabled,
            AdminId::try_new(self.actor_ref).map_err(|_| corrupt())?,
            self.applied_at,
        ))
    }
}
const fn corrupt() -> DomainError {
    DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)
}

async fn current(
    connection: &mut sqlx::PgConnection,
    key: Option<FlagKey>,
    lock: bool,
) -> Result<Vec<FlagRecord>, DomainError> {
    let rows = if lock {
        sqlx::query_as!(
            FlagRow,
            "SELECT v.id, v.flag_key, v.enabled, v.actor_ref, v.applied_at
             FROM flags f JOIN flag_versions v ON v.id = f.current_version_id
             WHERE ($1::text IS NULL OR f.key = $1) ORDER BY f.key FOR UPDATE OF f",
            key.map(FlagKey::as_str)
        )
        .fetch_all(connection)
        .await
    } else {
        sqlx::query_as!(
            FlagRow,
            "SELECT v.id, v.flag_key, v.enabled, v.actor_ref, v.applied_at
             FROM flags f JOIN flag_versions v ON v.id = f.current_version_id
             WHERE ($1::text IS NULL OR f.key = $1) ORDER BY f.key",
            key.map(FlagKey::as_str)
        )
        .fetch_all(connection)
        .await
    }
    .map_err(|error| map_sql(&error))?;
    rows.into_iter()
        .map(|row| row.into_version().map(FlagRecord::new))
        .collect()
}

#[async_trait::async_trait]
impl FlagRepository for PostgresFlagRepository {
    async fn list(&self, tx: &mut TxHandle) -> Result<Vec<FlagRecord>, DomainError> {
        current(self.uow.readable_connection(tx)?, None, false).await
    }
    async fn get(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
    ) -> Result<Option<FlagRecord>, DomainError> {
        Ok(current(self.uow.readable_connection(tx)?, Some(key), false)
            .await?
            .into_iter()
            .next())
    }
    async fn history(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        limit: u16,
    ) -> Result<Vec<FlagVersion>, DomainError> {
        if limit == 0 {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        sqlx::query_as!(
            FlagRow,
            "SELECT id, flag_key, enabled, actor_ref, applied_at FROM flag_versions
             WHERE flag_key = $1 ORDER BY applied_at DESC, id DESC LIMIT $2",
            key.as_str(),
            i64::from(limit)
        )
        .fetch_all(self.uow.readable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?
        .into_iter()
        .map(FlagRow::into_version)
        .collect()
    }
    async fn version(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        version: VersionId,
    ) -> Result<Option<FlagVersion>, DomainError> {
        sqlx::query_as!(
            FlagRow,
            "SELECT id, flag_key, enabled, actor_ref, applied_at FROM flag_versions
             WHERE flag_key = $1 AND id = $2",
            key.as_str(),
            version.as_uuid()
        )
        .fetch_optional(self.uow.readable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?
        .map(FlagRow::into_version)
        .transpose()
    }
    async fn lock(
        &self,
        tx: &mut TxHandle,
        mut keys: Vec<FlagKey>,
    ) -> Result<Vec<FlagRecord>, DomainError> {
        keys.sort_unstable();
        keys.dedup();
        let names: Vec<String> = keys.iter().map(|key| key.as_str().to_owned()).collect();
        sqlx::query_as!(
            FlagRow,
            "SELECT v.id, v.flag_key, v.enabled, v.actor_ref, v.applied_at
             FROM flags f JOIN flag_versions v ON v.id = f.current_version_id
             WHERE f.key = ANY($1::text[]) ORDER BY f.key FOR UPDATE OF f",
            &names
        )
        .fetch_all(self.uow.readable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?
        .into_iter()
        .map(|row| row.into_version().map(FlagRecord::new))
        .collect()
    }
    async fn append_version(
        &self,
        tx: &mut TxHandle,
        version: FlagVersion,
    ) -> Result<(), DomainError> {
        let exists = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM flag_versions WHERE id = $1) AS present",
            version.version().as_uuid()
        )
        .fetch_one(self.uow.readable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?
        .present
        .unwrap_or(false);
        if exists {
            return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
        }
        sqlx::query!(
            "INSERT INTO flag_versions (
                 id, flag_key, enabled, publication_id, actor_ref, reason_code, applied_at
             ) VALUES ($1, $2, $3, NULL, $4, 'draft_change', $5)",
            version.version().as_uuid(),
            version.key().as_str(),
            version.value(),
            version.actor().as_uuid(),
            version.effective_at()
        )
        .execute(self.uow.writable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }
    async fn set_live(
        &self,
        tx: &mut TxHandle,
        key: FlagKey,
        expected: Option<VersionId>,
        next: VersionId,
    ) -> Result<(), DomainError> {
        let result = sqlx::query!(
            "UPDATE flags SET current_version_id = $2
             WHERE key = $1 AND current_version_id IS NOT DISTINCT FROM $3",
            key.as_str(),
            next.as_uuid(),
            expected.map(VersionId::as_uuid)
        )
        .execute(self.uow.writable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?;
        if result.rows_affected() != 1 {
            return Err(stale());
        }
        Ok(())
    }
    async fn seed(&self, tx: &mut TxHandle, versions: Vec<FlagVersion>) -> Result<(), DomainError> {
        for version in versions {
            let definition = launch_flags()
                .into_iter()
                .find(|flag| flag.key() == version.key())
                .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))?;
            let connection = self.uow.writable_connection(tx)?;
            sqlx::query!(
                "INSERT INTO flags (key, label_key, description_key) VALUES ($1, $2, $3)
                 ON CONFLICT (key) DO NOTHING",
                version.key().as_str(),
                definition.fa_label_key(),
                definition.description_key()
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
            sqlx::query!(
                "INSERT INTO flag_versions (
                     id, flag_key, enabled, publication_id, actor_ref, reason_code, applied_at
                 ) VALUES ($1, $2, $3, NULL, $4, 'foundation_seed', $5)
                 ON CONFLICT (id) DO NOTHING",
                version.version().as_uuid(),
                version.key().as_str(),
                version.value(),
                version.actor().as_uuid(),
                version.effective_at()
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
            sqlx::query!(
                "UPDATE flags SET current_version_id = $2 WHERE key = $1 AND current_version_id IS NULL",
                version.key().as_str(), version.version().as_uuid()
            ).execute(connection).await.map_err(|error| map_sql(&error))?;
        }
        Ok(())
    }
}
