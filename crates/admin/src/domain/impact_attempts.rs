//! Attempt-limit preservation for existing generation units; no job execution.

use super::change_preview::{EffectRule, ImpactProvenance};
use kernel::error::{DomainError, ErrorCode};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitQueueState {
    PreviouslyQueued,
    Processing,
    QueuedAfterPublication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationUnitSnapshot {
    state: UnitQueueState,
    assigned_limit: u32,
}

impl GenerationUnitSnapshot {
    #[must_use]
    pub const fn new(state: UnitQueueState, assigned_limit: u32) -> Self {
        Self {
            state,
            assigned_limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptLimitImpact {
    effective_attempt_limit: u32,
    provenance: ImpactProvenance,
    effect_rule: EffectRule,
}

impl AttemptLimitImpact {
    #[must_use]
    pub const fn effective_attempt_limit(&self) -> u32 {
        self.effective_attempt_limit
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
/// Rejects attempt limits outside `1..=3` or invalid provenance.
pub fn assess_attempt_limit(
    proposed_limit: u32,
    unit: GenerationUnitSnapshot,
    assessed_at: OffsetDateTime,
) -> Result<AttemptLimitImpact, DomainError> {
    if !(1..=3).contains(&proposed_limit) || !(1..=3).contains(&unit.assigned_limit) {
        return Err(DomainError::validation(ErrorCode::InvalidValue));
    }
    let effective_attempt_limit = match unit.state {
        UnitQueueState::PreviouslyQueued | UnitQueueState::Processing => unit.assigned_limit,
        UnitQueueState::QueuedAfterPublication => proposed_limit,
    };
    Ok(AttemptLimitImpact {
        effective_attempt_limit,
        provenance: ImpactProvenance::new(
            "generation-attempt-limit/v1",
            "generation-queue-policy",
            assessed_at,
        )?,
        effect_rule: EffectRule::new(
            "preserve-previously-queued-and-processing/future-queued-use-new-limit",
        )?,
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::PreviewReport,
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        impact_attempts::{GenerationUnitSnapshot, UnitQueueState, assess_attempt_limit},
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
        impact: &crate::domain::impact_attempts::AttemptLimitImpact,
    ) -> PreviewReport {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000601");
        let draft = ChangeDraft::new(
            change_id,
            ChangeSubject::Setting(SettingKey::GenerationMaxAttempts),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000602"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
        )
        .append_revision(
            ChangeRevision::new(
                id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000603"),
                change_id,
                1,
                ChangeValue::Number(1),
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
    fn attempt_limit_preview_preserves_existing_units() {
        let queued = assess_attempt_limit(
            1,
            GenerationUnitSnapshot::new(UnitQueueState::PreviouslyQueued, 3),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert_eq!(queued.effective_attempt_limit(), 3);
        assert_eq!(queued.provenance().policy(), "generation-attempt-limit/v1");
        assert!(!queued.provenance().source().is_empty());

        let processing = assess_attempt_limit(
            1,
            GenerationUnitSnapshot::new(UnitQueueState::Processing, 2),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert_eq!(processing.effective_attempt_limit(), 2);

        let future = assess_attempt_limit(
            1,
            GenerationUnitSnapshot::new(UnitQueueState::QueuedAfterPublication, 3),
            datetime!(2026-01-01 00:00 UTC),
        )
        .unwrap();
        assert_eq!(future.effective_attempt_limit(), 1);
        assert!(
            assess_attempt_limit(
                4,
                GenerationUnitSnapshot::new(UnitQueueState::QueuedAfterPublication, 3),
                datetime!(2026-01-01 00:00 UTC),
            )
            .is_err()
        );

        let later = assess_attempt_limit(
            1,
            GenerationUnitSnapshot::new(UnitQueueState::PreviouslyQueued, 3),
            datetime!(2027-01-01 00:00 UTC),
        )
        .unwrap();
        let reviewed = preview("018f2f7c-8a3c-7000-9000-000000000610", &queued);
        let later = preview("018f2f7c-8a3c-7000-9000-000000000611", &later);
        let approval = Approval::new(
            id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000612"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000613"),
            &reviewed,
            datetime!(2026-01-01 01:00 UTC),
        )
        .unwrap();
        assert_eq!(approval.eligibility(&later), ApprovalEligibility::Eligible);
    }
}
