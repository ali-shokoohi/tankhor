//! Read-only current-actor authorization and complete settings presentation metadata.
use crate::{
    domain::{
        auth_state::BindingDigest,
        settings::{
            NumericPolicy, SettingKey, SettingUnit, SettingValue, initial_numeric_settings,
            initial_upload_formats,
        },
    },
    ports::{
        admin_repository::AdminRepository,
        session_store::SessionStore,
        settings_repository::{SettingVersion, SettingsRepository},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, VersionId},
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};
use time::OffsetDateTime;

const HISTORY_LIMIT: u16 = 20;

#[derive(Debug, Clone)]
pub struct ListSettingsInput {
    pub actor: BindingDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingVersionView {
    pub version: VersionId,
    pub value: SettingValue,
    pub actor: AdminId,
    pub effective_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingSummary {
    pub key: SettingKey,
    pub value: SettingValue,
    pub live_version: VersionId,
    pub unit: SettingUnit,
    pub label_key: String,
    pub minimum: u32,
    pub maximum: u32,
    pub effective_at: OffsetDateTime,
    pub history: Vec<SettingVersionView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingList {
    pub entries: Vec<SettingSummary>,
}

pub struct ListSettings<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub settings: &'a dyn SettingsRepository,
    pub clock: &'a dyn Clock,
}

impl ListSettings<'_> {
    /// # Errors
    /// Refuses sessions without current full access and propagates bounded registry read errors.
    pub async fn execute(&self, input: ListSettingsInput) -> Result<SettingList, DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let session = self
                .sessions
                .lookup(&mut tx, input.actor)
                .await?
                .ok_or_else(denied)?;
            let admin = self
                .admins
                .get(&mut tx, session.admin_id())
                .await?
                .ok_or_else(denied)?;
            session.check_access(&admin, self.clock.now().await?)?;
            let records = self.settings.list(&mut tx).await?;
            let mut entries = Vec::with_capacity(records.len());
            for record in records {
                let current = record.version();
                let (unit, policy, label_key) = metadata(current.key())?;
                let history = self
                    .settings
                    .history(&mut tx, current.key(), HISTORY_LIMIT)
                    .await?
                    .into_iter()
                    .map(version_view)
                    .collect();
                entries.push(SettingSummary {
                    key: current.key(),
                    value: current.value().clone(),
                    live_version: current.version(),
                    unit,
                    label_key: label_key.to_owned(),
                    minimum: policy.minimum(),
                    maximum: policy.maximum(),
                    effective_at: current.effective_at(),
                    history,
                });
            }
            Ok(SettingList { entries })
        }
        .await;
        match result {
            Ok(output) => {
                self.uow.commit(tx).await?;
                Ok(output)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn version_view(version: SettingVersion) -> SettingVersionView {
    SettingVersionView {
        version: version.version(),
        value: version.value().clone(),
        actor: version.actor(),
        effective_at: version.effective_at(),
    }
}

pub(crate) fn metadata(
    key: SettingKey,
) -> Result<(SettingUnit, NumericPolicy, &'static str), DomainError> {
    if let Some(setting) = initial_numeric_settings()
        .into_iter()
        .find(|setting| setting.key() == key)
    {
        return Ok((setting.unit(), setting.policy(), setting.en_label_key()));
    }
    let formats = initial_upload_formats();
    if formats.key() == key {
        return Ok((formats.unit(), formats.policy(), formats.en_label_key()));
    }
    Err(DomainError::validation(ErrorCode::InvalidValue))
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
