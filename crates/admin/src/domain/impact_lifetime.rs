//! Pure shorter-lifetime impact from original references; no expiry execution.

use super::change_preview::{EffectRule, ImpactProvenance, PopulationEvidence};
use kernel::error::{DomainError, ErrorCode};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifetimeReference {
    OriginalUpload,
    LastActivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifetimePolicy {
    BuyerTempPhotos,
    StudioTempUploads,
    StudioAutoDrafts,
}

impl LifetimePolicy {
    const fn reference(self) -> LifetimeReference {
        match self {
            Self::BuyerTempPhotos | Self::StudioTempUploads => LifetimeReference::OriginalUpload,
            Self::StudioAutoDrafts => LifetimeReference::LastActivity,
        }
    }

    const fn identity(self) -> &'static str {
        match self {
            Self::BuyerTempPhotos => "buyer-temp-photo-lifetime/v1",
            Self::StudioTempUploads => "studio-temp-upload-lifetime/v1",
            Self::StudioAutoDrafts => "studio-auto-draft-lifetime/v1",
        }
    }

    const fn source(self) -> &'static str {
        match self {
            Self::BuyerTempPhotos => "buyer-media-ledger",
            Self::StudioTempUploads => "studio-upload-ledger",
            Self::StudioAutoDrafts => "studio-activity-ledger",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifetimeItem {
    reference_at: OffsetDateTime,
    current_expiry: OffsetDateTime,
}

impl LifetimeItem {
    #[must_use]
    pub const fn new(reference_at: OffsetDateTime, current_expiry: OffsetDateTime) -> Self {
        Self {
            reference_at,
            current_expiry,
        }
    }

    #[must_use]
    pub const fn current_expiry(&self) -> OffsetDateTime {
        self.current_expiry
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifetimePopulation {
    Snapshots(Vec<LifetimeItem>),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextSweep {
    OwningLifecycleAfterPublication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifetimeImpact {
    reference: LifetimeReference,
    next_sweep: NextSweep,
    provenance: ImpactProvenance,
    effect_rule: EffectRule,
    evidence: Vec<PopulationEvidence>,
}

impl LifetimeImpact {
    #[must_use]
    pub const fn reference(&self) -> LifetimeReference {
        self.reference
    }

    #[must_use]
    pub const fn next_sweep(&self) -> NextSweep {
        self.next_sweep
    }

    #[must_use]
    pub const fn provenance(&self) -> &ImpactProvenance {
        &self.provenance
    }

    #[must_use]
    pub const fn effect_rule(&self) -> &EffectRule {
        &self.effect_rule
    }

    #[must_use]
    pub fn evidence(&self) -> &[PopulationEvidence] {
        &self.evidence
    }
}

/// # Errors
/// Rejects non-decreasing lifetimes or invalid timestamps.
pub fn assess_lifetime(
    policy: LifetimePolicy,
    previous_days: u32,
    proposed_days: u32,
    population: LifetimePopulation,
    assessed_at: OffsetDateTime,
) -> Result<LifetimeImpact, DomainError> {
    if proposed_days >= previous_days {
        return Err(DomainError::validation(ErrorCode::InvalidValue));
    }
    let evidence = match population {
        LifetimePopulation::Unavailable => vec![
            PopulationEvidence::unavailable("changed_count"),
            PopulationEvidence::unavailable("already_due_count"),
        ],
        LifetimePopulation::Snapshots(items) => {
            let mut changed = 0_u64;
            let mut already_due = 0_u64;
            for item in items {
                let recalculated = item
                    .reference_at
                    .checked_add(Duration::days(i64::from(proposed_days)))
                    .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))?;
                if recalculated < item.current_expiry {
                    changed += 1;
                }
                if recalculated <= assessed_at {
                    already_due += 1;
                }
            }
            vec![
                PopulationEvidence::known("changed_count", changed),
                PopulationEvidence::known("already_due_count", already_due),
            ]
        }
    };
    let reference = policy.reference();
    let effect = match reference {
        LifetimeReference::OriginalUpload => {
            "recalculate-original-upload/next-owning-lifecycle-sweep"
        }
        LifetimeReference::LastActivity => "recalculate-last-activity/next-owning-lifecycle-sweep",
    };
    Ok(LifetimeImpact {
        reference,
        next_sweep: NextSweep::OwningLifecycleAfterPublication,
        provenance: ImpactProvenance::new(policy.identity(), policy.source(), assessed_at)?,
        effect_rule: EffectRule::new(effect)?,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::PreviewReport,
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        impact_lifetime::{
            LifetimeItem, LifetimePolicy, LifetimePopulation, LifetimeReference, NextSweep,
            assess_lifetime,
        },
        settings::SettingKey,
    };
    use kernel::ids::{AdminId, ApprovalId, ChangeId, PreviewId, RevisionId, VersionId};
    use std::str::FromStr;
    use time::{Duration, macros::datetime};

    fn id<T: FromStr>(value: &str) -> T {
        value.parse().ok().unwrap()
    }

    fn preview(
        id_value: &str,
        impact: &crate::domain::impact_lifetime::LifetimeImpact,
    ) -> PreviewReport {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000201");
        let draft = ChangeDraft::new(
            change_id,
            ChangeSubject::Setting(SettingKey::BuyerTempPhotoDays),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000202"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
        )
        .append_revision(
            ChangeRevision::new(
                id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000203"),
                change_id,
                1,
                ChangeValue::Number(14),
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
            impact.evidence().to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn shorter_lifetime_previews_existing_item_impact() {
        let assessed_at = datetime!(2026-01-10 00:00 UTC);
        let items = vec![
            LifetimeItem::new(
                datetime!(2026-01-01 00:00 UTC),
                datetime!(2026-01-31 00:00 UTC),
            ),
            LifetimeItem::new(
                datetime!(2025-12-01 00:00 UTC),
                datetime!(2025-12-31 00:00 UTC),
            ),
            LifetimeItem::new(
                assessed_at - Duration::days(14),
                assessed_at + Duration::days(16),
            ),
        ];
        let impact = assess_lifetime(
            LifetimePolicy::BuyerTempPhotos,
            30,
            14,
            LifetimePopulation::Snapshots(items.clone()),
            assessed_at,
        )
        .unwrap();

        assert_eq!(impact.reference(), LifetimeReference::OriginalUpload);
        assert_eq!(impact.evidence()[0].known_count(), Some(3));
        assert_eq!(impact.evidence()[1].known_count(), Some(2));
        assert_eq!(
            impact.next_sweep(),
            NextSweep::OwningLifecycleAfterPublication
        );
        assert_eq!(items[0].current_expiry(), datetime!(2026-01-31 00:00 UTC));

        let empty = assess_lifetime(
            LifetimePolicy::StudioAutoDrafts,
            30,
            14,
            LifetimePopulation::Snapshots(vec![]),
            assessed_at,
        )
        .unwrap();
        assert_eq!(empty.reference(), LifetimeReference::LastActivity);
        assert_eq!(empty.evidence()[0].known_count(), Some(0));
        assert_eq!(empty.evidence()[1].known_count(), Some(0));

        let unknown = assess_lifetime(
            LifetimePolicy::StudioTempUploads,
            30,
            14,
            LifetimePopulation::Unavailable,
            assessed_at,
        )
        .unwrap();
        assert!(
            unknown
                .evidence()
                .iter()
                .all(super::super::change_preview::PopulationEvidence::is_unavailable)
        );
        assert_ne!(unknown.evidence()[0].known_count(), Some(0));

        let stable_items = vec![items[0].clone()];
        let earlier = assess_lifetime(
            LifetimePolicy::BuyerTempPhotos,
            30,
            14,
            LifetimePopulation::Snapshots(stable_items.clone()),
            assessed_at - Duration::hours(1),
        )
        .unwrap();
        let stable = assess_lifetime(
            LifetimePolicy::BuyerTempPhotos,
            30,
            14,
            LifetimePopulation::Snapshots(stable_items),
            assessed_at,
        )
        .unwrap();
        let current = preview("018f2f7c-8a3c-7000-9000-000000000210", &stable);
        let earlier = preview("018f2f7c-8a3c-7000-9000-000000000211", &earlier);
        let approval = Approval::new(
            id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000212"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000213"),
            &earlier,
            assessed_at,
        )
        .unwrap();
        assert_eq!(
            approval.eligibility(&current),
            ApprovalEligibility::Eligible
        );
        assert_eq!(
            approval.eligibility(&preview("018f2f7c-8a3c-7000-9000-000000000214", &unknown)),
            ApprovalEligibility::EvidenceUnavailable
        );
    }
}
