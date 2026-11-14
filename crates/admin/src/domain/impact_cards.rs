//! Card-limit preservation rules; no Studio project mutation.

use super::change_preview::{EffectRule, ImpactProvenance};
use kernel::error::DomainError;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingProjects {
    Preserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmedProjects {
    Unaffected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardProjectSnapshot {
    card_count: u32,
    confirmed: bool,
}

impl CardProjectSnapshot {
    #[must_use]
    pub const fn new(card_count: u32, confirmed: bool) -> Self {
        Self {
            card_count,
            confirmed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardLimitImpact {
    existing_projects: ExistingProjects,
    confirmed_projects: ConfirmedProjects,
    may_add_card: bool,
    may_start_generation: bool,
    provenance: ImpactProvenance,
    effect_rule: EffectRule,
}

impl CardLimitImpact {
    #[must_use]
    pub const fn existing_projects(&self) -> ExistingProjects {
        self.existing_projects
    }

    #[must_use]
    pub const fn confirmed_projects(&self) -> ConfirmedProjects {
        self.confirmed_projects
    }

    #[must_use]
    pub const fn may_add_card(&self) -> bool {
        self.may_add_card
    }

    #[must_use]
    pub const fn may_start_generation(&self) -> bool {
        self.may_start_generation
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
pub fn assess_card_limit(
    proposed_limit: u32,
    project: CardProjectSnapshot,
    assessed_at: OffsetDateTime,
) -> Result<CardLimitImpact, DomainError> {
    Ok(CardLimitImpact {
        existing_projects: ExistingProjects::Preserved,
        confirmed_projects: ConfirmedProjects::Unaffected,
        may_add_card: project.confirmed || project.card_count < proposed_limit,
        may_start_generation: project.confirmed || project.card_count <= proposed_limit,
        provenance: ImpactProvenance::new(
            "studio-card-limit/v1",
            "studio-project-policy",
            assessed_at,
        )?,
        effect_rule: EffectRule::new(
            "preserve-existing/confirmed-unaffected/at-limit-generate-no-addition",
        )?,
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::PreviewReport,
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        impact_cards::{
            CardProjectSnapshot, ConfirmedProjects, ExistingProjects, assess_card_limit,
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
        impact: &crate::domain::impact_cards::CardLimitImpact,
    ) -> PreviewReport {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000301");
        let draft = ChangeDraft::new(
            change_id,
            ChangeSubject::Setting(SettingKey::StudioMaxCards),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000302"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
        )
        .append_revision(
            ChangeRevision::new(
                id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000303"),
                change_id,
                1,
                ChangeValue::Number(2),
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
    fn card_limit_preview_preserves_existing_projects() {
        let at_limit = assess_card_limit(
            2,
            CardProjectSnapshot::new(2, false),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert_eq!(at_limit.existing_projects(), ExistingProjects::Preserved);
        assert_eq!(at_limit.confirmed_projects(), ConfirmedProjects::Unaffected);
        assert!(at_limit.may_start_generation());
        assert!(!at_limit.may_add_card());
        assert_eq!(at_limit.provenance().policy(), "studio-card-limit/v1");
        assert!(!at_limit.provenance().source().is_empty());

        let above = assess_card_limit(
            2,
            CardProjectSnapshot::new(3, false),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert!(!above.may_start_generation());
        assert!(!above.may_add_card());

        let below = assess_card_limit(
            2,
            CardProjectSnapshot::new(1, false),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert!(below.may_start_generation());
        assert!(below.may_add_card());

        let confirmed = assess_card_limit(
            2,
            CardProjectSnapshot::new(3, true),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert!(confirmed.may_start_generation());

        let later = assess_card_limit(
            2,
            CardProjectSnapshot::new(2, false),
            datetime!(2027-01-01 00:00 UTC),
        )
        .unwrap();
        let reviewed = preview("018f2f7c-8a3c-7000-9000-000000000310", &at_limit);
        let later = preview("018f2f7c-8a3c-7000-9000-000000000311", &later);
        let approval = Approval::new(
            id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000312"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000313"),
            &reviewed,
            datetime!(2026-01-01 01:00 UTC),
        )
        .unwrap();
        assert_eq!(approval.eligibility(&later), ApprovalEligibility::Eligible);
    }
}
