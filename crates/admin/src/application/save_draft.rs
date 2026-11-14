//! Validated single-subject immutable draft revision with atomic safe audit.
use crate::{
    domain::{
        audit::{
            Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, DraftMetadata, ReasonCode,
            Subject,
        },
        auth_state::BindingDigest,
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        scheduled_time::ScheduledTime,
        settings::SettingValue,
    },
    ports::{
        admin_repository::AdminRepository,
        audit_repository::AuditRepository,
        change_repository::ChangeRepository,
        flag_repository::FlagRepository,
        session_store::SessionStore,
        settings_repository::{SettingsRepository, validate_value},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::ChangeId,
    ports::{
        clock::Clock,
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleInput {
    pub local: String,
    pub zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SaveDraftInput {
    pub actor: BindingDigest,
    pub change: Option<ChangeId>,
    pub subject: ChangeSubject,
    pub value: ChangeValue,
    pub reason: ReasonCode,
    pub expected_head: Option<kernel::ids::RevisionId>,
    pub schedule: Option<ScheduleInput>,
}

pub struct SaveDraft<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub sessions: &'a dyn SessionStore,
    pub admins: &'a dyn AdminRepository,
    pub settings: &'a dyn SettingsRepository,
    pub flags: &'a dyn FlagRepository,
    pub changes: &'a dyn ChangeRepository,
    pub audit: &'a dyn AuditRepository,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
}

impl SaveDraft<'_> {
    /// Saves exactly one draft revision and its audit; never moves a live pointer.
    ///
    /// # Errors
    /// Returns validation, authorisation, conflict, or persistence errors.
    #[allow(clippy::too_many_lines)]
    pub async fn execute(&self, input: SaveDraftInput) -> Result<ChangeDraft, DomainError> {
        validate(input.subject, input.value, input.reason)?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
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
            let now = self.clock.now().await?;
            session.check_access(&admin, now)?;
            let scheduled_at = input
                .schedule
                .as_ref()
                .map(|schedule| {
                    ScheduledTime::from_console(&schedule.local, schedule.zone.as_deref(), now)
                })
                .transpose()?;

            let (change, draft) = if let Some(id) = input.change {
                let draft = self
                    .changes
                    .draft(&mut tx, id)
                    .await?
                    .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?;
                if draft.subject() != input.subject || draft.head() != input.expected_head {
                    return Err(DomainError::conflict(ErrorCode::StaleRevision));
                }
                (id, draft)
            } else {
                if input.expected_head.is_some() {
                    return Err(DomainError::conflict(ErrorCode::StaleRevision));
                }
                let baseline = match input.subject {
                    ChangeSubject::Setting(key) => self
                        .settings
                        .get(&mut tx, key)
                        .await?
                        .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?
                        .version()
                        .version(),
                    ChangeSubject::Flag(key) => self
                        .flags
                        .get(&mut tx, key)
                        .await?
                        .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?
                        .version()
                        .version(),
                };
                let id = self.ids.next_change_id().await?;
                (
                    id,
                    ChangeDraft::new(id, input.subject, baseline, admin.id()),
                )
            };
            let before = draft.current().map(ChangeRevision::value);
            let revision_id = self.ids.next_revision_id().await?;
            let next_number = u32::try_from(draft.revisions().len())
                .ok()
                .and_then(|len| len.checked_add(1))
                .ok_or_else(|| DomainError::conflict(ErrorCode::StaleRevision))?;
            let next = draft.append_revision(
                ChangeRevision::new(
                    revision_id,
                    change,
                    next_number,
                    input.value,
                    scheduled_at.clone(),
                    admin.id(),
                    now,
                ),
                input.expected_head,
            )?;
            if input.change.is_some() {
                self.changes
                    .append_revision(&mut tx, next.clone(), input.expected_head)
                    .await?;
            } else {
                self.changes.create_draft(&mut tx, next.clone()).await?;
            }
            let audit_id = self.ids.next_audit_id().await?;
            self.audit
                .append(
                    &mut tx,
                    AuditEntry::try_from(AuditInput {
                        id: audit_id,
                        event_key: audit_id,
                        actor: Some(Actor::Admin(admin.id())),
                        action: Some(AuditAction::DraftSaved),
                        subject: Some(Subject::Change(change)),
                        occurred_at: Some(now),
                        reason: Some(input.reason),
                        metadata: AuditMetadata::Draft(DraftMetadata::new(
                            before,
                            input.value,
                            next.baseline_version(),
                            revision_id,
                            scheduled_at,
                        )),
                    })?,
                )
                .await?;
            Ok(next)
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

fn validate(
    subject: ChangeSubject,
    value: ChangeValue,
    reason: ReasonCode,
) -> Result<(), DomainError> {
    if reason != ReasonCode::DraftChange {
        return Err(DomainError::validation(ErrorCode::InvalidValue));
    }
    match subject {
        ChangeSubject::Setting(key) => {
            let setting = match value {
                ChangeValue::Number(value) => SettingValue::Number(i128::from(value)),
                ChangeValue::Formats(value) => SettingValue::Formats(value),
                ChangeValue::Boolean(_) => {
                    return Err(DomainError::validation(ErrorCode::InvalidValue));
                }
            };
            validate_value(key, &setting)
        }
        ChangeSubject::Flag(_) if matches!(value, ChangeValue::Boolean(_)) => Ok(()),
        ChangeSubject::Flag(_) => Err(DomainError::validation(ErrorCode::InvalidValue)),
    }
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
