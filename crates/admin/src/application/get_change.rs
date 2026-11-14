//! Read-only exact change draft, current head, baseline and review presentation.
use crate::{
    domain::{
        auth_state::BindingDigest,
        change_approval::{Approval, ApprovalEligibility},
        change_preview::PreviewReport,
        change_revision::{ChangeSubject, ChangeValue},
        scheduled_time::ScheduledTime,
    },
    ports::{
        admin_repository::AdminRepository, change_repository::ChangeRepository,
        session_store::SessionStore,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{ChangeId, RevisionId, VersionId},
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct GetChangeInput {
    pub actor: BindingDigest,
    pub change: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRevisionView {
    pub id: RevisionId,
    pub number: u32,
    pub value: ChangeValue,
    pub scheduled_at: Option<ScheduledTime>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDetails {
    pub id: ChangeId,
    pub subject: ChangeSubject,
    pub baseline_version: VersionId,
    pub head: Option<RevisionId>,
    pub current: Option<ChangeRevisionView>,
    pub preview: Option<PreviewReport>,
    pub approval: Option<Approval>,
    pub eligibility: Option<ApprovalEligibility>,
}

pub struct GetChange<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub changes: &'a dyn ChangeRepository,
    pub clock: &'a dyn Clock,
}

impl GetChange<'_> {
    /// # Errors
    /// Rejects malformed or unknown change IDs and actors without current full access.
    pub async fn execute(&self, input: GetChangeInput) -> Result<ChangeDetails, DomainError> {
        let change = input
            .change
            .parse::<ChangeId>()
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
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
            let draft = self
                .changes
                .draft(&mut tx, change)
                .await?
                .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?;
            let head = draft.head();
            let (preview, approval) = if let Some(head) = head {
                (
                    self.changes.latest_preview(&mut tx, change, head).await?,
                    self.changes.latest_approval(&mut tx, change, head).await?,
                )
            } else {
                (None, None)
            };
            let eligibility = match (&approval, &preview) {
                (Some(approval), Some(preview)) => Some(approval.eligibility(preview)),
                _ => None,
            };
            let current = draft.current().map(|revision| ChangeRevisionView {
                id: revision.id(),
                number: revision.revision_number(),
                value: revision.value(),
                scheduled_at: revision.scheduled_at().cloned(),
                created_at: revision.created_at(),
            });
            Ok(ChangeDetails {
                id: draft.id(),
                subject: draft.subject(),
                baseline_version: draft.baseline_version(),
                head,
                current,
                preview,
                approval,
                eligibility,
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
