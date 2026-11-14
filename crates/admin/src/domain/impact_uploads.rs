//! New-upload-only validation impact; stored files are never revalidated here.

use super::{
    change_preview::{EffectRule, ImpactProvenance},
    formats::FormatSet,
};
use kernel::error::DomainError;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredFiles {
    Preserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationScope {
    NewUploadsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadPolicySnapshot {
    max_mb: u32,
    min_shortest_edge_px: u32,
    formats: FormatSet,
}

impl UploadPolicySnapshot {
    #[must_use]
    pub const fn new(max_mb: u32, min_shortest_edge_px: u32, formats: FormatSet) -> Self {
        Self {
            max_mb,
            min_shortest_edge_px,
            formats,
        }
    }

    #[must_use]
    pub const fn max_mb(self) -> u32 {
        self.max_mb
    }

    #[must_use]
    pub const fn min_shortest_edge_px(self) -> u32 {
        self.min_shortest_edge_px
    }

    #[must_use]
    pub const fn formats(self) -> FormatSet {
        self.formats
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadPolicyImpact {
    stored_files: StoredFiles,
    validation_scope: ValidationScope,
    provenance: ImpactProvenance,
    effect_rule: EffectRule,
}

impl UploadPolicyImpact {
    #[must_use]
    pub const fn stored_files(&self) -> StoredFiles {
        self.stored_files
    }

    #[must_use]
    pub const fn validation_scope(&self) -> ValidationScope {
        self.validation_scope
    }

    #[must_use]
    pub const fn provenance(&self) -> &ImpactProvenance {
        &self.provenance
    }

    #[must_use]
    pub const fn effect_rule(&self) -> &EffectRule {
        &self.effect_rule
    }
}

/// # Errors
/// Returns validation errors if provenance or effect rules are invalid.
pub fn assess_upload_policy(
    policy: UploadPolicySnapshot,
    assessed_at: OffsetDateTime,
) -> Result<UploadPolicyImpact, DomainError> {
    let _new_upload_policy = (
        policy.max_mb(),
        policy.min_shortest_edge_px(),
        policy.formats(),
    );
    Ok(UploadPolicyImpact {
        stored_files: StoredFiles::Preserved,
        validation_scope: ValidationScope::NewUploadsOnly,
        provenance: ImpactProvenance::new(
            "upload-validation/v1",
            "upload-validation-policy",
            assessed_at,
        )?,
        effect_rule: EffectRule::new("preserve-stored/validate-new-only")?,
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::PreviewReport,
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        formats::FormatSet,
        impact_uploads::{
            StoredFiles, UploadPolicySnapshot, ValidationScope, assess_upload_policy,
        },
        settings::SettingKey,
    };
    use kernel::ids::{AdminId, ApprovalId, ChangeId, PreviewId, RevisionId, VersionId};
    use std::str::FromStr;
    use time::macros::datetime;

    fn id<T: FromStr>(value: &str) -> T {
        value.parse().ok().unwrap()
    }

    fn preview(
        id_value: &str,
        impact: &crate::domain::impact_uploads::UploadPolicyImpact,
    ) -> PreviewReport {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000501");
        let draft = ChangeDraft::new(
            change_id,
            ChangeSubject::Setting(SettingKey::UploadMaxMb),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000502"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
        )
        .append_revision(
            ChangeRevision::new(
                id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000503"),
                change_id,
                1,
                ChangeValue::Number(10),
                None,
                id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
                datetime!(2026-01-01 00:00 UTC),
            ),
            None,
        )
        .unwrap();
        PreviewReport::new(
            id::<PreviewId>(id_value),
            &draft,
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
            impact.provenance().clone(),
            impact.effect_rule().clone(),
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn upload_policy_preview_applies_only_to_new_uploads() {
        let impact = assess_upload_policy(
            UploadPolicySnapshot::new(10, 768, FormatSet::from_mask(7).unwrap()),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert_eq!(impact.stored_files(), StoredFiles::Preserved);
        assert_eq!(impact.validation_scope(), ValidationScope::NewUploadsOnly);
        assert_eq!(impact.provenance().policy(), "upload-validation/v1");
        assert!(!impact.provenance().source().is_empty());
        assert_eq!(
            impact.effect_rule().as_str(),
            "preserve-stored/validate-new-only"
        );

        let later = assess_upload_policy(
            UploadPolicySnapshot::new(10, 768, FormatSet::from_mask(7).unwrap()),
            datetime!(2027-01-01 00:00 UTC),
        )
        .unwrap();
        let reviewed = preview("018f2f7c-8a3c-7000-9000-000000000510", &impact);
        let later = preview("018f2f7c-8a3c-7000-9000-000000000511", &later);
        let approval = Approval::new(
            id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000512"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000513"),
            &reviewed,
            datetime!(2026-01-01 01:00 UTC),
        )
        .unwrap();
        assert_eq!(approval.eligibility(&later), ApprovalEligibility::Eligible);
    }
}
