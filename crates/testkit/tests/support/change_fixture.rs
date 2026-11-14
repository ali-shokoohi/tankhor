use admin::domain::{
    change_approval::Approval,
    change_preview::{EffectRule, ImpactProvenance, PopulationEvidence, PreviewReport},
    change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
    settings::SettingKey,
};
use kernel::ids::{AdminId, ApprovalId, ChangeId, PreviewId, RevisionId, VersionId};
use std::str::FromStr;
use time::{OffsetDateTime, macros::datetime};

pub fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-6300-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}
pub fn now() -> OffsetDateTime {
    datetime!(2026-01-01 00:00 UTC)
}
#[allow(dead_code)]
/// Administrator who originally created the fixture draft and its first revision.
pub fn creator() -> AdminId {
    id(7)
}
#[allow(dead_code)]
/// Administrator who reviewed the fixture preview.
pub fn reviewer() -> AdminId {
    id(8)
}
#[allow(dead_code)]
pub fn draft() -> ChangeDraft {
    let change = id::<ChangeId>(1);
    ChangeDraft::new(
        change,
        ChangeSubject::Setting(SettingKey::UploadMaxMb),
        id::<VersionId>(2),
        creator(),
    )
    .append_revision(
        ChangeRevision::new(
            id::<RevisionId>(3),
            change,
            1,
            ChangeValue::Number(20),
            None,
            creator(),
            now(),
        ),
        None,
    )
    .unwrap()
}
#[allow(dead_code)]
pub fn report(draft: &ChangeDraft) -> PreviewReport {
    PreviewReport::new(
        id::<PreviewId>(4),
        draft,
        reviewer(),
        ImpactProvenance::new("upload-policy/v1", "upload-ledger", now()).unwrap(),
        EffectRule::new("preserve-stored/validate-new-only").unwrap(),
        vec![PopulationEvidence::known("stored-uploads", 0)],
    )
    .unwrap()
}
#[allow(dead_code)]
pub fn approval(report: &PreviewReport) -> Approval {
    Approval::new(id::<ApprovalId>(5), id::<AdminId>(6), report, now()).unwrap()
}
