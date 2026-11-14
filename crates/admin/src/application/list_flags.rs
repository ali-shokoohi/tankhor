//! Read-only current-actor authorization and canonical launch-flag metadata.
use crate::{
    domain::{
        auth_state::BindingDigest,
        flag::{FlagKey, launch_flags},
    },
    ports::{
        admin_repository::AdminRepository, flag_repository::FlagRepository,
        session_store::SessionStore,
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
pub struct ListFlagsInput {
    pub actor: BindingDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagVersionView {
    pub version: VersionId,
    pub value: bool,
    pub actor: AdminId,
    pub effective_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagSummary {
    pub key: FlagKey,
    pub value: bool,
    pub live_version: VersionId,
    pub label_key: String,
    pub description_key: String,
    pub effective_at: OffsetDateTime,
    pub history: Vec<FlagVersionView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagList {
    pub entries: Vec<FlagSummary>,
}

pub struct ListFlags<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub flags: &'a dyn FlagRepository,
    pub clock: &'a dyn Clock,
}

impl ListFlags<'_> {
    /// # Errors
    /// Refuses sessions without current full access and propagates bounded registry read errors.
    pub async fn execute(&self, input: ListFlagsInput) -> Result<FlagList, DomainError> {
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
            let records = self.flags.list(&mut tx).await?;
            let mut entries = Vec::with_capacity(records.len());
            for record in records {
                let current = record.version();
                let definition = launch_flags()
                    .into_iter()
                    .find(|definition| definition.key() == current.key())
                    .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))?;
                let history = self
                    .flags
                    .history(&mut tx, current.key(), HISTORY_LIMIT)
                    .await?
                    .into_iter()
                    .map(|version| FlagVersionView {
                        version: version.version(),
                        value: version.value(),
                        actor: version.actor(),
                        effective_at: version.effective_at(),
                    })
                    .collect();
                entries.push(FlagSummary {
                    key: current.key(),
                    value: current.value(),
                    live_version: current.version(),
                    label_key: definition.en_label_key().to_owned(),
                    description_key: definition.description_key().to_owned(),
                    effective_at: current.effective_at(),
                    history,
                });
            }
            Ok(FlagList { entries })
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
