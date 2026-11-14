//! Read-only exact typed registry-version lookup with historical attribution.
use crate::{
    domain::{
        auth_state::BindingDigest,
        flag::FlagKey,
        settings::{SettingKey, SettingValue},
    },
    ports::{
        admin_repository::AdminRepository, flag_repository::FlagRepository,
        session_store::SessionStore, settings_repository::SettingsRepository,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrySubject {
    Setting(SettingKey),
    Flag(FlagKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionValue {
    Setting(SettingValue),
    Flag(bool),
}

#[derive(Debug, Clone)]
pub struct GetVersionInput {
    pub actor: BindingDigest,
    pub subject: RegistrySubject,
    pub version: VersionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryVersion {
    pub subject: RegistrySubject,
    pub version: VersionId,
    pub value: VersionValue,
    pub actor: AdminId,
    pub effective_at: OffsetDateTime,
}

pub struct GetVersion<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub settings: &'a dyn SettingsRepository,
    pub flags: &'a dyn FlagRepository,
    pub clock: &'a dyn Clock,
}

impl GetVersion<'_> {
    /// # Errors
    /// Refuses unauthorized actors and subject/version combinations that do not exist exactly.
    pub async fn execute(&self, input: GetVersionInput) -> Result<RegistryVersion, DomainError> {
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
            let missing = || DomainError::validation(ErrorCode::NotFound);
            let (value, actor, effective_at) = match input.subject {
                RegistrySubject::Setting(key) => {
                    let row = self
                        .settings
                        .version(&mut tx, key, input.version)
                        .await?
                        .ok_or_else(missing)?;
                    (
                        VersionValue::Setting(row.value().clone()),
                        row.actor(),
                        row.effective_at(),
                    )
                }
                RegistrySubject::Flag(key) => {
                    let row = self
                        .flags
                        .version(&mut tx, key, input.version)
                        .await?
                        .ok_or_else(missing)?;
                    (
                        VersionValue::Flag(row.value()),
                        row.actor(),
                        row.effective_at(),
                    )
                }
            };
            Ok(RegistryVersion {
                subject: input.subject,
                version: input.version,
                value,
                actor,
                effective_at,
            })
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

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
