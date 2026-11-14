//! Read-only exact-key setting detail and bounded history.
use crate::{
    application::list_settings::{SettingSummary, metadata, version_view},
    domain::{auth_state::BindingDigest, settings::SettingKey},
    ports::{
        admin_repository::AdminRepository, session_store::SessionStore,
        settings_repository::SettingsRepository,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

const HISTORY_LIMIT: u16 = 20;

#[derive(Debug, Clone)]
pub struct GetSettingInput {
    pub actor: BindingDigest,
    pub key: String,
}

pub struct GetSetting<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub settings: &'a dyn SettingsRepository,
    pub clock: &'a dyn Clock,
}

impl GetSetting<'_> {
    /// # Errors
    /// Rejects unknown keys, missing live records and actors without current full access.
    pub async fn execute(&self, input: GetSettingInput) -> Result<SettingSummary, DomainError> {
        let key = input.key.parse::<SettingKey>()?;
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
            let record = self
                .settings
                .get(&mut tx, key)
                .await?
                .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?;
            let current = record.version();
            let (unit, policy, label_key) = metadata(key)?;
            let history = self
                .settings
                .history(&mut tx, key, HISTORY_LIMIT)
                .await?
                .into_iter()
                .map(version_view)
                .collect();
            Ok(SettingSummary {
                key,
                value: current.value().clone(),
                live_version: current.version(),
                unit,
                label_key: label_key.to_owned(),
                minimum: policy.minimum(),
                maximum: policy.maximum(),
                effective_at: current.effective_at(),
                history,
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
