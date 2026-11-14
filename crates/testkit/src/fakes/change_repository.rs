use super::store::FakeStore;
use admin::{
    domain::{
        change_approval::Approval, change_preview::PreviewReport, change_revision::ChangeDraft,
    },
    ports::change_repository::ChangeRepository,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{ApprovalId, ChangeId, PreviewId, RevisionId},
    ports::unit_of_work::TxHandle,
};

#[derive(Debug, Clone)]
pub struct FakeChangeRepository {
    store: FakeStore,
}
impl FakeChangeRepository {
    #[must_use]
    pub const fn new(store: FakeStore) -> Self {
        Self { store }
    }
}
fn conflict() -> DomainError {
    DomainError::conflict(ErrorCode::StaleRevision)
}

#[async_trait::async_trait]
impl ChangeRepository for FakeChangeRepository {
    async fn draft(
        &self,
        tx: &mut TxHandle,
        id: ChangeId,
    ) -> Result<Option<ChangeDraft>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.changes.get(&id).cloned()))
    }
    async fn create_draft(&self, tx: &mut TxHandle, draft: ChangeDraft) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if state.changes.contains_key(&draft.id()) {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            state.changes.insert(draft.id(), draft);
            Ok(())
        })
    }
    async fn append_revision(
        &self,
        tx: &mut TxHandle,
        draft: ChangeDraft,
        expected_head: Option<RevisionId>,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            let current = state.changes.get(&draft.id()).ok_or_else(conflict)?;
            if current.head() != expected_head
                || current.subject() != draft.subject()
                || current.baseline_version() != draft.baseline_version()
                || current.creator() != draft.creator()
                || draft.revisions().len() != current.revisions().len() + 1
                || current.revisions() != &draft.revisions()[..current.revisions().len()]
            {
                return Err(conflict());
            }
            state.changes.insert(draft.id(), draft);
            Ok(())
        })
    }
    async fn save_preview(
        &self,
        tx: &mut TxHandle,
        report: PreviewReport,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if state.previews.contains_key(&report.id()) {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            let draft = state
                .changes
                .get(&report.change_id())
                .ok_or_else(conflict)?;
            if draft.head() != Some(report.revision_id())
                || draft.subject() != report.subject()
                || draft.baseline_version() != report.baseline_version()
            {
                return Err(conflict());
            }
            state.previews.insert(report.id(), report);
            Ok(())
        })
    }
    async fn preview(
        &self,
        tx: &mut TxHandle,
        id: PreviewId,
    ) -> Result<Option<PreviewReport>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.previews.get(&id).cloned()))
    }
    async fn latest_preview(
        &self,
        tx: &mut TxHandle,
        change: ChangeId,
        revision: RevisionId,
    ) -> Result<Option<PreviewReport>, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state
                .previews
                .values()
                .filter(|report| report.change_id() == change && report.revision_id() == revision)
                .max_by_key(|report| (report.generated_at(), report.id()))
                .cloned())
        })
    }
    async fn save_approval(
        &self,
        tx: &mut TxHandle,
        approval: Approval,
    ) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if state.approvals.contains_key(&approval.id()) {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            if !state
                .previews
                .contains_key(&approval.approved_report().id())
            {
                return Err(conflict());
            }
            state.approvals.insert(approval.id(), approval);
            Ok(())
        })
    }
    async fn approval(
        &self,
        tx: &mut TxHandle,
        id: ApprovalId,
    ) -> Result<Option<Approval>, DomainError> {
        self.store
            .with_read(tx, |state| Ok(state.approvals.get(&id).cloned()))
    }
    async fn latest_approval(
        &self,
        tx: &mut TxHandle,
        change: ChangeId,
        revision: RevisionId,
    ) -> Result<Option<Approval>, DomainError> {
        self.store.with_read(tx, |state| {
            Ok(state
                .approvals
                .values()
                .filter(|approval| {
                    approval.approved_report().change_id() == change
                        && approval.approved_report().revision_id() == revision
                })
                .max_by_key(|approval| (approval.approved_at(), approval.id()))
                .cloned())
        })
    }
}
