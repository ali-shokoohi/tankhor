//! Exact-revision approvals and canonical impact comparison, without a time-to-live.

use super::change_preview::PreviewReport;
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ApprovalId},
};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalEligibility {
    Eligible,
    RequiresReapproval,
    EvidenceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    id: ApprovalId,
    approver_id: AdminId,
    approved_at: OffsetDateTime,
    approved_report: PreviewReport,
}

impl Approval {
    /// # Errors
    /// Rejects reports with unavailable population evidence.
    pub fn new(
        id: ApprovalId,
        approver_id: AdminId,
        report: &PreviewReport,
        approved_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        if report
            .evidence()
            .iter()
            .any(super::change_preview::PopulationEvidence::is_unavailable)
        {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self {
            id,
            approver_id,
            approved_at,
            approved_report: report.clone(),
        })
    }

    #[must_use]
    pub const fn id(&self) -> ApprovalId {
        self.id
    }

    #[must_use]
    pub const fn approver_id(&self) -> AdminId {
        self.approver_id
    }

    #[must_use]
    pub const fn approved_at(&self) -> OffsetDateTime {
        self.approved_at
    }

    #[must_use]
    pub const fn approved_report(&self) -> &PreviewReport {
        &self.approved_report
    }

    #[must_use]
    pub fn eligibility(&self, current: &PreviewReport) -> ApprovalEligibility {
        if current
            .evidence()
            .iter()
            .any(super::change_preview::PopulationEvidence::is_unavailable)
        {
            return ApprovalEligibility::EvidenceUnavailable;
        }
        let reviewed = &self.approved_report;
        let same_identity = reviewed.change_id() == current.change_id()
            && reviewed.revision_id() == current.revision_id()
            && reviewed.subject() == current.subject()
            && reviewed.baseline_version() == current.baseline_version()
            && reviewed.digest() == current.digest();
        let same_impact = reviewed.provenance().policy() == current.provenance().policy()
            && reviewed.provenance().source() == current.provenance().source()
            && reviewed.effect_rule() == current.effect_rule()
            && reviewed.evidence() == current.evidence()
            && reviewed.readiness() == current.readiness();
        if same_identity && same_impact {
            ApprovalEligibility::Eligible
        } else {
            ApprovalEligibility::RequiresReapproval
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::{EffectRule, ImpactProvenance, PopulationEvidence, PreviewReport},
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        settings::SettingKey,
    };
    use kernel::ids::{AdminId, ApprovalId, ChangeId, PreviewId, RevisionId, VersionId};
    use std::str::FromStr;
    use time::{OffsetDateTime, macros::datetime};

    fn id<T: FromStr>(value: &str) -> T {
        value.parse().ok().unwrap()
    }

    fn draft(revision: u32, value: u32) -> ChangeDraft {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000101");
        let base = ChangeDraft::new(
            change_id,
            ChangeSubject::Setting(SettingKey::UploadMaxMb),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000102"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
        );
        if revision == 1 {
            base.append_revision(
                ChangeRevision::new(
                    id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000103"),
                    change_id,
                    1,
                    ChangeValue::Number(value),
                    None,
                    id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
                    datetime!(2026-01-01 00:00 UTC),
                ),
                None,
            )
            .unwrap()
        } else {
            draft(1, 12)
                .append_revision(
                    ChangeRevision::new(
                        id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000104"),
                        change_id,
                        2,
                        ChangeValue::Number(value),
                        None,
                        id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
                        datetime!(2026-01-01 00:01 UTC),
                    ),
                    Some(id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000103")),
                )
                .unwrap()
        }
    }

    fn report(
        report_id: &str,
        draft: &ChangeDraft,
        assessed_at: OffsetDateTime,
        evidence: Vec<PopulationEvidence>,
    ) -> PreviewReport {
        PreviewReport::new(
            id::<PreviewId>(report_id),
            draft,
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
            ImpactProvenance::new("upload-policy/v1", "upload-ledger", assessed_at).unwrap(),
            EffectRule::new("preserve-stored/validate-new-only").unwrap(),
            evidence,
        )
        .unwrap()
    }

    fn approval(report: &PreviewReport) -> Approval {
        Approval::new(
            id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000110"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000111"),
            report,
            datetime!(2026-01-01 01:00 UTC),
        )
        .expect("known evidence can be approved")
    }

    #[test]
    fn editing_approved_draft_invalidates_approval() {
        let original = report(
            "018f2f7c-8a3c-7000-9000-000000000120",
            &draft(1, 12),
            datetime!(2026-01-01 00:30 UTC),
            vec![PopulationEvidence::known("oversize", 0)],
        );
        let approval = approval(&original);
        let edited = report(
            "018f2f7c-8a3c-7000-9000-000000000121",
            &draft(2, 10),
            datetime!(2026-01-01 00:31 UTC),
            vec![PopulationEvidence::known("oversize", 0)],
        );
        let reverted = report(
            "018f2f7c-8a3c-7000-9000-000000000122",
            &draft(2, 12),
            datetime!(2026-01-01 00:32 UTC),
            vec![PopulationEvidence::known("oversize", 0)],
        );

        assert_eq!(
            approval.eligibility(&edited),
            ApprovalEligibility::RequiresReapproval
        );
        assert_eq!(
            approval.eligibility(&reverted),
            ApprovalEligibility::RequiresReapproval
        );
        assert_eq!(approval.approved_report(), &original);
    }

    #[test]
    fn impact_comparison_ignores_assessment_time() {
        let reviewed = report(
            "018f2f7c-8a3c-7000-9000-000000000130",
            &draft(1, 12),
            datetime!(2026-01-01 00:30 UTC),
            vec![PopulationEvidence::known("oversize", 0)],
        );
        let later = report(
            "018f2f7c-8a3c-7000-9000-000000000131",
            &draft(1, 12),
            datetime!(2027-01-01 00:30 UTC),
            vec![PopulationEvidence::known("oversize", 0)],
        );
        assert_eq!(
            approval(&reviewed).eligibility(&later),
            ApprovalEligibility::Eligible
        );

        let recovered = report(
            "018f2f7c-8a3c-7000-9000-000000000132",
            &draft(1, 12),
            datetime!(2027-01-02 00:30 UTC),
            vec![PopulationEvidence::known("oversize", 0)],
        );
        assert_eq!(
            approval(&reviewed).eligibility(&recovered),
            ApprovalEligibility::Eligible
        );
    }

    #[test]
    fn changed_destructive_impact_requires_reapproval() {
        let reviewed = report(
            "018f2f7c-8a3c-7000-9000-000000000140",
            &draft(1, 12),
            datetime!(2026-01-01 00:30 UTC),
            vec![PopulationEvidence::known("oversize", 2)],
        );
        let changed_count = report(
            "018f2f7c-8a3c-7000-9000-000000000141",
            &draft(1, 12),
            datetime!(2026-01-01 00:31 UTC),
            vec![PopulationEvidence::known("oversize", 3)],
        );
        assert_eq!(
            approval(&reviewed).eligibility(&changed_count),
            ApprovalEligibility::RequiresReapproval
        );

        let mut changed_source = report(
            "018f2f7c-8a3c-7000-9000-000000000142",
            &draft(1, 12),
            datetime!(2026-01-01 00:31 UTC),
            vec![PopulationEvidence::known("oversize", 2)],
        );
        changed_source = PreviewReport::new(
            changed_source.id(),
            &draft(1, 12),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
            ImpactProvenance::new(
                "upload-policy/v1",
                "other-ledger",
                changed_source.generated_at(),
            )
            .unwrap(),
            changed_source.effect_rule().clone(),
            changed_source.evidence().to_vec(),
        )
        .unwrap();
        assert_eq!(
            approval(&reviewed).eligibility(&changed_source),
            ApprovalEligibility::RequiresReapproval
        );

        let changed_effect = PreviewReport::new(
            reviewed.id(),
            &draft(1, 12),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
            reviewed.provenance().clone(),
            EffectRule::new("delete-existing/immediately").unwrap(),
            reviewed.evidence().to_vec(),
        )
        .unwrap();
        assert_eq!(
            approval(&reviewed).eligibility(&changed_effect),
            ApprovalEligibility::RequiresReapproval
        );
    }

    #[test]
    fn unavailable_impact_blocks_review() {
        let known_zero = report(
            "018f2f7c-8a3c-7000-9000-000000000150",
            &draft(1, 12),
            datetime!(2026-01-01 00:30 UTC),
            vec![PopulationEvidence::known("oversize", 0)],
        );
        let unknown = report(
            "018f2f7c-8a3c-7000-9000-000000000151",
            &draft(1, 12),
            datetime!(2026-01-01 00:31 UTC),
            vec![PopulationEvidence::unavailable("oversize")],
        );
        assert_eq!(
            approval(&known_zero).eligibility(&unknown),
            ApprovalEligibility::EvidenceUnavailable
        );
        assert!(
            Approval::new(
                id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000152"),
                id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000111"),
                &unknown,
                datetime!(2026-01-01 01:00 UTC),
            )
            .is_err()
        );
        assert_ne!(
            PopulationEvidence::unavailable("oversize"),
            PopulationEvidence::known("oversize", 0)
        );
    }
}
