//! Shared fake/real change workflow persistence contract, including attribution.

use admin::{
    domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::{EffectRule, ImpactProvenance, PopulationEvidence, PreviewReport},
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        settings::SettingKey,
    },
    ports::change_repository::ChangeRepository,
};
use kernel::{
    error::{DomainError, ErrorCategory},
    ids::{AdminId, ChangeId},
    ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
};
use std::str::FromStr;
use time::{OffsetDateTime, macros::datetime};

#[async_trait::async_trait]
pub trait ChangeRepositoryContractHarness: Send + Sync {
    type Uow: UnitOfWork;
    type Repository: ChangeRepository;
    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}
fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-5000-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}
/// Administrators the contract attributes work to; real harnesses must seed them.
pub fn change_repository_contract_admins() -> Vec<AdminId> {
    [6, CREATOR, EDITOR, REVIEWER]
        .into_iter()
        .map(admin)
        .collect()
}
const CREATOR: u16 = 10;
const EDITOR: u16 = 11;
const REVIEWER: u16 = 12;
fn admin(n: u16) -> AdminId {
    id(n)
}
fn draft(creator: AdminId) -> ChangeDraft {
    ChangeDraft::new(
        id(1),
        ChangeSubject::Setting(SettingKey::UploadMaxMb),
        id(2),
        creator,
    )
}
fn revision(
    change: ChangeId,
    n: u16,
    number: u32,
    value: u32,
    by: AdminId,
    at: OffsetDateTime,
) -> ChangeRevision {
    ChangeRevision::new(
        id(n),
        change,
        number,
        ChangeValue::Number(value),
        None,
        by,
        at,
    )
}
fn report(n: u16, draft: &ChangeDraft, by: AdminId, at: OffsetDateTime) -> PreviewReport {
    PreviewReport::new(
        id(n),
        draft,
        by,
        ImpactProvenance::new("upload/v1", "upload-ledger", at).unwrap(),
        EffectRule::new("preserve-stored/validate-new-only").unwrap(),
        vec![PopulationEvidence::known("stored", 0)],
    )
    .unwrap()
}

/// # Panics
/// Panics if the repository or unit-of-work violates the shared contract.
#[allow(clippy::too_many_lines)]
pub async fn change_repository_contract<H: ChangeRepositoryContractHarness>(harness: &H) {
    let empty = draft(admin(CREATOR));
    let change = id::<ChangeId>(1);
    let one = empty
        .append_revision(
            revision(
                change,
                3,
                1,
                12,
                admin(CREATOR),
                datetime!(2026-01-01 00:00 UTC),
            ),
            None,
        )
        .unwrap();
    let preview_one = report(4, &one, admin(EDITOR), datetime!(2026-01-01 00:30 UTC));
    let approval = Approval::new(
        id(5),
        id::<AdminId>(6),
        &preview_one,
        datetime!(2026-01-01 01:00 UTC),
    )
    .unwrap();
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    harness
        .repository()
        .create_draft(&mut tx, one.clone())
        .await
        .unwrap();
    assert!(
        harness
            .repository()
            .create_draft(&mut tx, one.clone())
            .await
            .is_err()
    );
    harness
        .repository()
        .save_preview(&mut tx, preview_one.clone())
        .await
        .unwrap();
    harness
        .repository()
        .save_approval(&mut tx, approval.clone())
        .await
        .unwrap();
    harness.uow().commit(tx).await.unwrap();

    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    assert_eq!(
        harness.repository().draft(&mut tx, change).await.unwrap(),
        Some(one.clone())
    );
    assert_eq!(
        harness
            .repository()
            .preview(&mut tx, preview_one.id())
            .await
            .unwrap(),
        Some(preview_one.clone())
    );
    assert_eq!(
        harness
            .repository()
            .latest_preview(&mut tx, change, one.head().unwrap())
            .await
            .unwrap(),
        Some(preview_one.clone())
    );
    assert_eq!(
        harness
            .repository()
            .approval(&mut tx, approval.id())
            .await
            .unwrap(),
        Some(approval.clone())
    );
    let stored = harness
        .repository()
        .draft(&mut tx, change)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.creator(), admin(CREATOR));
    assert_eq!(stored.current().unwrap().created_by(), admin(CREATOR));
    assert_eq!(
        harness
            .repository()
            .preview(&mut tx, preview_one.id())
            .await
            .unwrap()
            .unwrap()
            .reviewed_by(),
        admin(EDITOR)
    );
    harness.uow().commit(tx).await.unwrap();

    let rewritten_creator =
        ChangeDraft::new(change, one.subject(), one.baseline_version(), admin(EDITOR))
            .append_revision(one.revisions()[0].clone(), None)
            .unwrap()
            .append_revision(
                revision(
                    change,
                    13,
                    2,
                    12,
                    admin(EDITOR),
                    datetime!(2026-01-02 00:00 UTC),
                ),
                one.head(),
            )
            .unwrap();
    let rewritten_author = draft(admin(CREATOR))
        .append_revision(
            revision(
                change,
                3,
                1,
                12,
                admin(EDITOR),
                datetime!(2026-01-01 00:00 UTC),
            ),
            None,
        )
        .unwrap()
        .append_revision(
            revision(
                change,
                13,
                2,
                12,
                admin(EDITOR),
                datetime!(2026-01-02 00:00 UTC),
            ),
            one.head(),
        )
        .unwrap();
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    for forged in [rewritten_creator, rewritten_author] {
        assert_eq!(
            harness
                .repository()
                .append_revision(&mut tx, forged, one.head())
                .await
                .unwrap_err()
                .category(),
            ErrorCategory::Conflict
        );
    }
    harness.uow().rollback(tx).await.unwrap();

    let aborted = one
        .append_revision(
            revision(
                change,
                14,
                2,
                12,
                admin(EDITOR),
                datetime!(2026-01-02 00:00 UTC),
            ),
            one.head(),
        )
        .unwrap();
    let aborted_preview = report(
        15,
        &aborted,
        admin(REVIEWER),
        datetime!(2026-01-02 00:30 UTC),
    );
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    harness
        .repository()
        .append_revision(&mut tx, aborted, one.head())
        .await
        .unwrap();
    harness
        .repository()
        .save_preview(&mut tx, aborted_preview.clone())
        .await
        .unwrap();
    harness.uow().rollback(tx).await.unwrap();
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    assert_eq!(
        harness.repository().draft(&mut tx, change).await.unwrap(),
        Some(one.clone())
    );
    assert_eq!(
        harness
            .repository()
            .preview(&mut tx, aborted_preview.id())
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        harness
            .repository()
            .latest_preview(&mut tx, change, one.head().unwrap())
            .await
            .unwrap(),
        Some(preview_one.clone())
    );
    harness.uow().commit(tx).await.unwrap();

    let two = one
        .append_revision(
            revision(
                change,
                7,
                2,
                12,
                admin(EDITOR),
                datetime!(2026-01-02 00:00 UTC),
            ),
            one.head(),
        )
        .unwrap();
    let preview_two = report(8, &two, admin(REVIEWER), datetime!(2027-01-01 00:30 UTC));
    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    harness
        .repository()
        .append_revision(&mut tx, two.clone(), one.head())
        .await
        .unwrap();
    assert_eq!(
        harness
            .repository()
            .append_revision(&mut tx, two.clone(), one.head())
            .await
            .unwrap_err()
            .category(),
        ErrorCategory::Conflict
    );
    harness
        .repository()
        .save_preview(&mut tx, preview_two.clone())
        .await
        .unwrap();
    harness.uow().commit(tx).await.unwrap();

    let mut tx = harness
        .uow()
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    let stored = harness
        .repository()
        .draft(&mut tx, change)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.subject(), one.subject());
    assert_eq!(stored.baseline_version(), one.baseline_version());
    assert_eq!(&stored.revisions()[..1], one.revisions());
    assert_eq!(stored, two);
    assert_eq!(stored.creator(), admin(CREATOR));
    assert_eq!(stored.revisions()[0].created_by(), admin(CREATOR));
    assert_eq!(stored.current().unwrap().created_by(), admin(EDITOR));
    assert_eq!(
        harness
            .repository()
            .preview(&mut tx, preview_one.id())
            .await
            .unwrap(),
        Some(preview_one)
    );
    assert_eq!(
        harness
            .repository()
            .approval(&mut tx, approval.id())
            .await
            .unwrap()
            .unwrap()
            .eligibility(&preview_two),
        ApprovalEligibility::RequiresReapproval
    );
    assert_eq!(
        harness
            .repository()
            .latest_preview(&mut tx, change, two.head().unwrap())
            .await
            .unwrap(),
        Some(preview_two.clone())
    );
    assert_eq!(preview_two.reviewed_by(), admin(REVIEWER));
    harness.uow().commit(tx).await.unwrap();
    let mut foreign = harness.begin_foreign().await.unwrap();
    assert!(
        harness
            .repository()
            .draft(&mut foreign, change)
            .await
            .is_err()
    );
}
