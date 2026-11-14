//! Fixed settings registry and immutable published versions in caller transactions.

use std::str::FromStr;

use admin::domain::formats::FormatSet;
use admin::domain::settings::{
    SettingKey, SettingUnit, SettingValue, initial_numeric_settings, initial_upload_formats,
};
use admin::ports::settings_repository::{
    SettingRecord, SettingVersion, SettingsRepository, validate_value,
};
use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{AdminId, VersionId};
use kernel::ports::unit_of_work::TxHandle;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{map_sql, stale};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresSettingsRepository {
    uow: PostgresUnitOfWork,
}

impl PostgresSettingsRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

struct SettingRow {
    id: Uuid,
    setting_key: String,
    numeric_value: Option<i64>,
    format_mask: Option<i32>,
    actor_ref: Uuid,
    applied_at: OffsetDateTime,
}

impl SettingRow {
    fn into_version(self) -> Result<SettingVersion, DomainError> {
        let key = SettingKey::from_str(&self.setting_key)?;
        let value = match (self.numeric_value, self.format_mask) {
            (Some(value), None) => SettingValue::Number(i128::from(value)),
            (None, Some(mask)) => SettingValue::Formats(FormatSet::from_mask(
                u8::try_from(mask).map_err(|_| corrupt())?,
            )?),
            _ => return Err(corrupt()),
        };
        SettingVersion::new(
            VersionId::try_new(self.id).map_err(|_| corrupt())?,
            key,
            value,
            AdminId::try_new(self.actor_ref).map_err(|_| corrupt())?,
            self.applied_at,
        )
    }
}

const fn corrupt() -> DomainError {
    DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)
}

fn split_value(value: &SettingValue) -> Result<(Option<i64>, Option<i32>), DomainError> {
    match value {
        SettingValue::Number(value) => Ok((
            Some(
                i64::try_from(*value)
                    .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?,
            ),
            None,
        )),
        SettingValue::Formats(value) => Ok((None, Some(i32::from(value.mask())))),
        SettingValue::Boolean(_) => Err(DomainError::validation(ErrorCode::InvalidValue)),
    }
}

fn metadata(key: SettingKey) -> (&'static str, SettingUnit, Option<i64>, Option<i64>) {
    if key == SettingKey::UploadFormats {
        let setting = initial_upload_formats();
        return (setting.fa_label_key(), setting.unit(), None, None);
    }
    let setting = initial_numeric_settings()
        .into_iter()
        .find(|setting| setting.key() == key)
        .expect("every numeric key is registered");
    (
        setting.fa_label_key(),
        setting.unit(),
        Some(i64::from(setting.policy().minimum())),
        Some(i64::from(setting.policy().maximum())),
    )
}

async fn select_current(
    connection: &mut sqlx::PgConnection,
    key: Option<SettingKey>,
    lock: bool,
) -> Result<Vec<SettingRecord>, DomainError> {
    let rows = if lock {
        sqlx::query_as!(
            SettingRow,
            "SELECT v.id, v.setting_key, v.numeric_value, v.format_mask, v.actor_ref, v.applied_at
             FROM settings s JOIN setting_versions v ON v.id = s.current_version_id
             WHERE ($1::text IS NULL OR s.key = $1)
             ORDER BY s.key FOR UPDATE OF s",
            key.map(SettingKey::as_str),
        )
        .fetch_all(connection)
        .await
    } else {
        sqlx::query_as!(
            SettingRow,
            "SELECT v.id, v.setting_key, v.numeric_value, v.format_mask, v.actor_ref, v.applied_at
             FROM settings s JOIN setting_versions v ON v.id = s.current_version_id
             WHERE ($1::text IS NULL OR s.key = $1) ORDER BY s.key",
            key.map(SettingKey::as_str),
        )
        .fetch_all(connection)
        .await
    }
    .map_err(|error| map_sql(&error))?;
    rows.into_iter()
        .map(|row| row.into_version().map(SettingRecord::new))
        .collect()
}

#[async_trait::async_trait]
impl SettingsRepository for PostgresSettingsRepository {
    async fn list(&self, tx: &mut TxHandle) -> Result<Vec<SettingRecord>, DomainError> {
        select_current(self.uow.readable_connection(tx)?, None, false).await
    }

    async fn get(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
    ) -> Result<Option<SettingRecord>, DomainError> {
        Ok(
            select_current(self.uow.readable_connection(tx)?, Some(key), false)
                .await?
                .into_iter()
                .next(),
        )
    }

    async fn history(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        limit: u16,
    ) -> Result<Vec<SettingVersion>, DomainError> {
        if limit == 0 {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        sqlx::query_as!(
            SettingRow,
            "SELECT id, setting_key, numeric_value, format_mask, actor_ref, applied_at
             FROM setting_versions WHERE setting_key = $1
             ORDER BY applied_at DESC, id DESC LIMIT $2",
            key.as_str(),
            i64::from(limit),
        )
        .fetch_all(self.uow.readable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?
        .into_iter()
        .map(SettingRow::into_version)
        .collect()
    }

    async fn version(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        version: VersionId,
    ) -> Result<Option<SettingVersion>, DomainError> {
        sqlx::query_as!(
            SettingRow,
            "SELECT id, setting_key, numeric_value, format_mask, actor_ref, applied_at
             FROM setting_versions WHERE setting_key = $1 AND id = $2",
            key.as_str(),
            version.as_uuid(),
        )
        .fetch_optional(self.uow.readable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?
        .map(SettingRow::into_version)
        .transpose()
    }

    async fn lock(
        &self,
        tx: &mut TxHandle,
        mut keys: Vec<SettingKey>,
    ) -> Result<Vec<SettingRecord>, DomainError> {
        keys.sort_unstable();
        keys.dedup();
        let names: Vec<String> = keys.iter().map(|key| key.as_str().to_owned()).collect();
        let rows = sqlx::query_as!(
            SettingRow,
            "SELECT v.id, v.setting_key, v.numeric_value, v.format_mask, v.actor_ref, v.applied_at
             FROM settings s JOIN setting_versions v ON v.id = s.current_version_id
             WHERE s.key = ANY($1::text[]) ORDER BY s.key FOR UPDATE OF s",
            &names,
        )
        .fetch_all(self.uow.readable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?;
        rows.into_iter()
            .map(|row| row.into_version().map(SettingRecord::new))
            .collect()
    }

    async fn append_version(
        &self,
        tx: &mut TxHandle,
        version: SettingVersion,
    ) -> Result<(), DomainError> {
        validate_value(version.key(), version.value())?;
        let (numeric, formats) = split_value(version.value())?;
        let exists = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM setting_versions WHERE id = $1) AS present",
            version.version().as_uuid(),
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
            "INSERT INTO setting_versions (
                 id, setting_key, numeric_value, format_mask, publication_id,
                 actor_ref, reason_code, applied_at
             ) VALUES ($1, $2, $3, $4, NULL, $5, 'draft_change', $6)",
            version.version().as_uuid(),
            version.key().as_str(),
            numeric,
            formats,
            version.actor().as_uuid(),
            version.effective_at(),
        )
        .execute(self.uow.writable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }

    async fn set_live(
        &self,
        tx: &mut TxHandle,
        key: SettingKey,
        expected: Option<VersionId>,
        next: VersionId,
    ) -> Result<(), DomainError> {
        let result = sqlx::query!(
            "UPDATE settings SET current_version_id = $2
             WHERE key = $1 AND current_version_id IS NOT DISTINCT FROM $3",
            key.as_str(),
            next.as_uuid(),
            expected.map(VersionId::as_uuid),
        )
        .execute(self.uow.writable_connection(tx)?)
        .await
        .map_err(|error| map_sql(&error))?;
        if result.rows_affected() != 1 {
            return Err(stale());
        }
        Ok(())
    }

    async fn seed(
        &self,
        tx: &mut TxHandle,
        versions: Vec<SettingVersion>,
    ) -> Result<(), DomainError> {
        for version in versions {
            validate_value(version.key(), version.value())?;
            let (label, unit, minimum, maximum) = metadata(version.key());
            let (numeric, formats) = split_value(version.value())?;
            let connection = self.uow.writable_connection(tx)?;
            sqlx::query!(
                "INSERT INTO settings (key, label_key, unit, min_value, max_value)
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT (key) DO NOTHING",
                version.key().as_str(),
                label,
                unit.as_str(),
                minimum,
                maximum,
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
            sqlx::query!(
                "INSERT INTO setting_versions (
                     id, setting_key, numeric_value, format_mask, publication_id,
                     actor_ref, reason_code, applied_at
                 ) VALUES ($1, $2, $3, $4, NULL, $5, 'foundation_seed', $6)
                 ON CONFLICT (id) DO NOTHING",
                version.version().as_uuid(),
                version.key().as_str(),
                numeric,
                formats,
                version.actor().as_uuid(),
                version.effective_at(),
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
            sqlx::query!(
                "UPDATE settings SET current_version_id = $2
                 WHERE key = $1 AND current_version_id IS NULL",
                version.key().as_str(),
                version.version().as_uuid(),
            )
            .execute(connection)
            .await
            .map_err(|error| map_sql(&error))?;
        }
        Ok(())
    }
}
