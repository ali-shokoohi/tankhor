//! Permanent-photo limit preservation; no media/account lifecycle effects.

use super::change_preview::{EffectRule, ImpactProvenance};
use kernel::error::DomainError;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingPhotos {
    Preserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhotoAccountSnapshot {
    photo_count: u32,
}

impl PhotoAccountSnapshot {
    #[must_use]
    pub const fn new(photo_count: u32) -> Self {
        Self { photo_count }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoLimitImpact {
    existing_photos: ExistingPhotos,
    may_add_photo: bool,
    provenance: ImpactProvenance,
    effect_rule: EffectRule,
}

impl PhotoLimitImpact {
    #[must_use]
    pub const fn existing_photos(&self) -> ExistingPhotos {
        self.existing_photos
    }

    #[must_use]
    pub const fn may_add_photo(&self) -> bool {
        self.may_add_photo
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
pub fn assess_photo_limit(
    proposed_limit: u32,
    account: PhotoAccountSnapshot,
    assessed_at: OffsetDateTime,
) -> Result<PhotoLimitImpact, DomainError> {
    Ok(PhotoLimitImpact {
        existing_photos: ExistingPhotos::Preserved,
        may_add_photo: account.photo_count < proposed_limit,
        provenance: ImpactProvenance::new(
            "buyer-photo-limit/v1",
            "buyer-media-policy",
            assessed_at,
        )?,
        effect_rule: EffectRule::new("preserve-all-existing/add-only-below-limit")?,
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::PreviewReport,
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        impact_photos::{ExistingPhotos, PhotoAccountSnapshot, assess_photo_limit},
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
        impact: &crate::domain::impact_photos::PhotoLimitImpact,
    ) -> PreviewReport {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000401");
        let draft = ChangeDraft::new(
            change_id,
            ChangeSubject::Setting(SettingKey::BuyerMaxPermanentPhotos),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000402"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
        )
        .append_revision(
            ChangeRevision::new(
                id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000403"),
                change_id,
                1,
                ChangeValue::Number(5),
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
    fn photo_limit_preview_preserves_existing_photos() {
        let at_limit = assess_photo_limit(
            5,
            PhotoAccountSnapshot::new(5),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert_eq!(at_limit.existing_photos(), ExistingPhotos::Preserved);
        assert!(!at_limit.may_add_photo());
        assert_eq!(at_limit.provenance().policy(), "buyer-photo-limit/v1");
        assert!(!at_limit.provenance().source().is_empty());

        let above = assess_photo_limit(
            5,
            PhotoAccountSnapshot::new(6),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert_eq!(above.existing_photos(), ExistingPhotos::Preserved);
        assert!(!above.may_add_photo());

        let below = assess_photo_limit(
            5,
            PhotoAccountSnapshot::new(4),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert!(below.may_add_photo());

        let later = assess_photo_limit(
            5,
            PhotoAccountSnapshot::new(5),
            datetime!(2027-01-01 00:00 UTC),
        )
        .unwrap();
        let reviewed = preview("018f2f7c-8a3c-7000-9000-000000000410", &at_limit);
        let later = preview("018f2f7c-8a3c-7000-9000-000000000411", &later);
        let approval = Approval::new(
            id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000412"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000413"),
            &reviewed,
            datetime!(2026-01-01 01:00 UTC),
        )
        .unwrap();
        assert_eq!(approval.eligibility(&later), ApprovalEligibility::Eligible);
    }
}
