//! Single-subject change draft, preview and approval persistence port.

use crate::domain::{
    change_approval::Approval, change_preview::PreviewReport, change_revision::ChangeDraft,
};
use kernel::{
    error::DomainError,
    ids::{ApprovalId, ChangeId, PreviewId, RevisionId},
    ports::unit_of_work::TxHandle,
};

#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait ChangeRepository: Send + Sync {
    async fn draft(
        &self,
        tx: &mut TxHandle,
        id: ChangeId,
    ) -> Result<Option<ChangeDraft>, DomainError>;
    async fn create_draft(&self, tx: &mut TxHandle, draft: ChangeDraft) -> Result<(), DomainError>;
    async fn append_revision(
        &self,
        tx: &mut TxHandle,
        draft: ChangeDraft,
        expected_head: Option<RevisionId>,
    ) -> Result<(), DomainError>;
    async fn save_preview(
        &self,
        tx: &mut TxHandle,
        report: PreviewReport,
    ) -> Result<(), DomainError>;
    async fn preview(
        &self,
        tx: &mut TxHandle,
        id: PreviewId,
    ) -> Result<Option<PreviewReport>, DomainError>;
    async fn latest_preview(
        &self,
        tx: &mut TxHandle,
        change: ChangeId,
        revision: RevisionId,
    ) -> Result<Option<PreviewReport>, DomainError>;
    async fn save_approval(&self, tx: &mut TxHandle, approval: Approval)
    -> Result<(), DomainError>;
    async fn approval(
        &self,
        tx: &mut TxHandle,
        id: ApprovalId,
    ) -> Result<Option<Approval>, DomainError>;
    async fn latest_approval(
        &self,
        tx: &mut TxHandle,
        change: ChangeId,
        revision: RevisionId,
    ) -> Result<Option<Approval>, DomainError>;
}

#[cfg(test)]
mod tests {
    use super::{ChangeRepository, MockChangeRepository};
    use kernel::{
        ids::{ApprovalId, ChangeId, PreviewId, RevisionId},
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    fn id<T: FromStr>(n: u16) -> T {
        format!("019a53a0-4000-7000-8000-{n:012x}")
            .parse()
            .ok()
            .unwrap()
    }

    #[tokio::test]
    async fn change_repository_port_is_mockable() {
        let mut mock = MockChangeRepository::new();
        let mut tx = TxHandle::new(StoreId::new(), TransactionMode::ReadWrite, ());
        let change = id::<ChangeId>(1);
        let revision = id::<RevisionId>(2);
        mock.expect_draft().return_once(|_, _| Ok(None));
        assert!(mock.draft(&mut tx, change).await.unwrap().is_none());
        mock.expect_create_draft().return_once(|_, _| Ok(()));
        let draft = crate::domain::change_revision::ChangeDraft::new(
            change,
            crate::domain::change_revision::ChangeSubject::Setting(
                crate::domain::settings::SettingKey::UploadMaxMb,
            ),
            id(3),
            id(9),
        );
        mock.create_draft(&mut tx, draft.clone()).await.unwrap();
        mock.expect_append_revision().return_once(|_, _, _| Ok(()));
        mock.append_revision(&mut tx, draft, None).await.unwrap();
        mock.expect_save_preview().return_once(|_, _| Ok(()));
        mock.expect_preview().return_once(|_, _| Ok(None));
        assert!(
            mock.preview(&mut tx, id::<PreviewId>(4))
                .await
                .unwrap()
                .is_none()
        );
        mock.expect_latest_preview().return_once(|_, _, _| Ok(None));
        assert!(
            mock.latest_preview(&mut tx, change, revision)
                .await
                .unwrap()
                .is_none()
        );
        mock.expect_save_approval().return_once(|_, _| Ok(()));
        mock.expect_approval().return_once(|_, _| Ok(None));
        assert!(
            mock.approval(&mut tx, id::<ApprovalId>(5))
                .await
                .unwrap()
                .is_none()
        );
        mock.expect_latest_approval()
            .return_once(|_, _, _| Ok(None));
        assert!(
            mock.latest_approval(&mut tx, change, revision)
                .await
                .unwrap()
                .is_none()
        );
    }
}
