//! New-deletion-request-only recovery-window impact; existing D never moves.

use super::change_preview::{EffectRule, ImpactProvenance};
use kernel::error::{DomainError, ErrorCode};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingRecovery {
    EndDatePreserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRecoverySnapshot {
    shown_end_date: OffsetDateTime,
}

impl PendingRecoverySnapshot {
    #[must_use]
    pub const fn new(shown_end_date: OffsetDateTime) -> Self {
        Self { shown_end_date }
    }

    #[must_use]
    pub const fn shown_end_date(self) -> OffsetDateTime {
        self.shown_end_date
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryImpact {
    existing_recovery: ExistingRecovery,
    pending_end_date: OffsetDateTime,
    new_request_end_date: OffsetDateTime,
    provenance: ImpactProvenance,
    effect_rule: EffectRule,
}

impl RecoveryImpact {
    #[must_use]
    pub const fn existing_recovery(&self) -> ExistingRecovery {
        self.existing_recovery
    }

    #[must_use]
    pub const fn pending_end_date(&self) -> OffsetDateTime {
        self.pending_end_date
    }

    #[must_use]
    pub const fn new_request_end_date(&self) -> OffsetDateTime {
        self.new_request_end_date
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
/// Rejects recovery windows outside `14..=90` days or timestamp overflow.
pub fn assess_recovery_window(
    proposed_days: u32,
    pending: PendingRecoverySnapshot,
    new_request_at: OffsetDateTime,
    assessed_at: OffsetDateTime,
) -> Result<RecoveryImpact, DomainError> {
    if !(14..=90).contains(&proposed_days) {
        return Err(DomainError::validation(ErrorCode::InvalidValue));
    }
    let new_request_end_date = new_request_at
        .checked_add(Duration::days(i64::from(proposed_days)))
        .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))?;
    Ok(RecoveryImpact {
        existing_recovery: ExistingRecovery::EndDatePreserved,
        pending_end_date: pending.shown_end_date,
        new_request_end_date,
        provenance: ImpactProvenance::new(
            "account-recovery-window/v1",
            "identity-recovery-policy",
            assessed_at,
        )?,
        effect_rule: EffectRule::new("preserve-pending-D/new-deletion-requests-use-new-window")?,
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        change_approval::{Approval, ApprovalEligibility},
        change_preview::PreviewReport,
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        impact_recovery::{ExistingRecovery, PendingRecoverySnapshot, assess_recovery_window},
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
        impact: &crate::domain::impact_recovery::RecoveryImpact,
    ) -> PreviewReport {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000701");
        let draft = ChangeDraft::new(
            change_id,
            ChangeSubject::Setting(SettingKey::AccountRecoveryDays),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000702"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1"),
        )
        .append_revision(
            ChangeRevision::new(
                id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000703"),
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
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn recovery_window_decrease_keeps_pending_end_dates() {
        let pending = PendingRecoverySnapshot::new(datetime!(2026-01-31 00:00 UTC));
        let impact = assess_recovery_window(
            14,
            pending,
            datetime!(2026-01-10 00:00 UTC),
            datetime!(2026-01-10 00:00 UTC),
        )
        .unwrap();
        assert_eq!(
            impact.existing_recovery(),
            ExistingRecovery::EndDatePreserved
        );
        assert_eq!(impact.pending_end_date(), datetime!(2026-01-31 00:00 UTC));
        assert_eq!(
            impact.new_request_end_date(),
            datetime!(2026-01-24 00:00 UTC)
        );
        assert_eq!(impact.provenance().policy(), "account-recovery-window/v1");
        assert!(!impact.provenance().source().is_empty());

        let later = assess_recovery_window(
            14,
            pending,
            datetime!(2026-01-10 00:00 UTC),
            datetime!(2027-01-01 00:00 UTC),
        )
        .unwrap();
        let reviewed = preview("018f2f7c-8a3c-7000-9000-000000000710", &impact);
        let later = preview("018f2f7c-8a3c-7000-9000-000000000711", &later);
        let approval = Approval::new(
            id::<ApprovalId>("018f2f7c-8a3c-7000-9000-000000000712"),
            id::<AdminId>("018f2f7c-8a3c-7000-9000-000000000713"),
            &reviewed,
            datetime!(2026-01-01 01:00 UTC),
        )
        .unwrap();
        assert_eq!(approval.eligibility(&later), ApprovalEligibility::Eligible);
        assert_eq!(pending.shown_end_date(), datetime!(2026-01-31 00:00 UTC));
    }
}
