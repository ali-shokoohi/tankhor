//! Idempotent exact registry seed with one stable system-operation audit event.

use crate::{
    domain::{
        audit::{Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, ReasonCode, Subject},
        flag::launch_flags,
        settings::{SettingValue, initial_numeric_settings, initial_upload_formats},
    },
    ports::{
        audit_repository::AuditRepository,
        flag_repository::{FlagRepository, FlagVersion},
        settings_repository::{SettingVersion, SettingsRepository},
    },
};
use kernel::{
    error::DomainError,
    ids::{AdminId, AuditId},
    ports::{
        clock::Clock,
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedFoundationInput {
    pub operation: AuditId,
    pub system: AdminId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedOutcome {
    Seeded,
    AlreadyApplied,
}

pub struct SeedFoundation<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub settings: &'a dyn SettingsRepository,
    pub flags: &'a dyn FlagRepository,
    pub audit: &'a dyn AuditRepository,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
}

impl SeedFoundation<'_> {
    /// Seeds only absent registry rows and never creates an administrator or credential.
    ///
    /// # Errors
    /// Returns validation or persistence errors.
    pub async fn execute(&self, input: SeedFoundationInput) -> Result<SeedOutcome, DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            let setting_count = self.settings.list(&mut tx).await?.len();
            let flag_count = self.flags.list(&mut tx).await?.len();
            if setting_count == 10 && flag_count == 6 {
                return Ok(SeedOutcome::AlreadyApplied);
            }
            let now = self.clock.now().await?;
            let mut setting_versions = Vec::with_capacity(10);
            for setting in initial_numeric_settings() {
                setting_versions.push(SettingVersion::new(
                    self.ids.next_version_id().await?,
                    setting.key(),
                    SettingValue::Number(i128::from(setting.value())),
                    input.system,
                    now,
                )?);
            }
            let formats = initial_upload_formats();
            setting_versions.push(SettingVersion::new(
                self.ids.next_version_id().await?,
                formats.key(),
                SettingValue::Formats(formats.value()),
                input.system,
                now,
            )?);
            let mut flag_versions = Vec::with_capacity(6);
            for flag in launch_flags() {
                flag_versions.push(FlagVersion::new(
                    self.ids.next_version_id().await?,
                    flag.key(),
                    flag.initial_value(),
                    input.system,
                    now,
                ));
            }
            self.settings.seed(&mut tx, setting_versions).await?;
            self.flags.seed(&mut tx, flag_versions).await?;
            self.audit
                .append(
                    &mut tx,
                    AuditEntry::try_from(AuditInput {
                        id: input.operation,
                        event_key: input.operation,
                        actor: Some(Actor::Admin(input.system)),
                        action: Some(AuditAction::FoundationSeeded),
                        subject: Some(Subject::Admin(input.system)),
                        occurred_at: Some(now),
                        reason: Some(ReasonCode::FoundationSeed),
                        metadata: AuditMetadata::None,
                    })?,
                )
                .await?;
            Ok(SeedOutcome::Seeded)
        }
        .await;
        match result {
            Ok(outcome) => {
                self.uow.commit(tx).await?;
                Ok(outcome)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}
