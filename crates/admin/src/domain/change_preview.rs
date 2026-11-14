//! Immutable preview identity and population evidence; no lifecycle effects.

use super::{
    change_hash::{RevisionDigest, draft_hash},
    change_revision::{ChangeDraft, ChangeSubject},
    flag_readiness::ReadinessStatus,
};
use kernel::{
    error::DomainError,
    ids::{AdminId, ChangeId, PreviewId, RevisionId, VersionId},
};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImpactProvenance {
    policy: String,
    source: String,
    assessed_at: OffsetDateTime,
}

impl ImpactProvenance {
    /// # Errors
    /// Rejects blank policy or source provenance.
    pub fn new(
        policy: impl Into<String>,
        source: impl Into<String>,
        assessed_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let policy = policy.into();
        let source = source.into();
        if policy.trim().is_empty() || source.trim().is_empty() {
            return Err(kernel::error::DomainError::validation(
                kernel::error::ErrorCode::InvalidValue,
            ));
        }
        Ok(Self {
            policy,
            source,
            assessed_at,
        })
    }

    #[must_use]
    pub fn policy(&self) -> &str {
        &self.policy
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn assessed_at(&self) -> OffsetDateTime {
        self.assessed_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EffectRule(String);

impl EffectRule {
    /// # Errors
    /// Rejects blank effect rules.
    pub fn new(rule: impl Into<String>) -> Result<Self, DomainError> {
        let rule = rule.into();
        if rule.trim().is_empty() {
            return Err(kernel::error::DomainError::validation(
                kernel::error::ErrorCode::InvalidValue,
            ));
        }
        Ok(Self(rule))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PopulationEvidence {
    Known { population: String, count: u64 },
    Unavailable { population: String },
}

impl PopulationEvidence {
    #[must_use]
    pub fn known(population: impl Into<String>, count: u64) -> Self {
        Self::Known {
            population: population.into(),
            count,
        }
    }

    #[must_use]
    pub fn unavailable(population: impl Into<String>) -> Self {
        Self::Unavailable {
            population: population.into(),
        }
    }

    #[must_use]
    pub const fn known_count(&self) -> Option<u64> {
        match self {
            Self::Known { count, .. } => Some(*count),
            Self::Unavailable { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewReport {
    id: PreviewId,
    change_id: ChangeId,
    revision_id: RevisionId,
    subject: ChangeSubject,
    baseline_version: VersionId,
    digest: RevisionDigest,
    reviewed_by: AdminId,
    provenance: ImpactProvenance,
    effect_rule: EffectRule,
    evidence: Vec<PopulationEvidence>,
    readiness: Option<ReadinessStatus>,
}

impl PreviewReport {
    /// # Errors
    /// Rejects drafts without a current revision.
    pub fn new(
        id: PreviewId,
        draft: &ChangeDraft,
        reviewed_by: AdminId,
        provenance: ImpactProvenance,
        effect_rule: EffectRule,
        evidence: Vec<PopulationEvidence>,
    ) -> Result<Self, DomainError> {
        Self::new_with_readiness(
            id,
            draft,
            reviewed_by,
            provenance,
            effect_rule,
            evidence,
            None,
        )
    }

    /// # Errors
    /// Rejects drafts without a current revision.
    pub fn new_with_readiness(
        id: PreviewId,
        draft: &ChangeDraft,
        reviewed_by: AdminId,
        provenance: ImpactProvenance,
        effect_rule: EffectRule,
        evidence: Vec<PopulationEvidence>,
        readiness: Option<ReadinessStatus>,
    ) -> Result<Self, DomainError> {
        let revision = draft.current().ok_or_else(|| {
            kernel::error::DomainError::validation(kernel::error::ErrorCode::InvalidValue)
        })?;
        Ok(Self {
            id,
            change_id: revision.change_id(),
            revision_id: revision.id(),
            subject: draft.subject(),
            baseline_version: draft.baseline_version(),
            digest: draft_hash(draft)?,
            reviewed_by,
            provenance,
            effect_rule,
            evidence,
            readiness,
        })
    }

    #[must_use]
    pub const fn id(&self) -> PreviewId {
        self.id
    }

    #[must_use]
    pub const fn change_id(&self) -> ChangeId {
        self.change_id
    }

    #[must_use]
    pub const fn revision_id(&self) -> RevisionId {
        self.revision_id
    }

    #[must_use]
    pub const fn subject(&self) -> ChangeSubject {
        self.subject
    }

    #[must_use]
    pub const fn baseline_version(&self) -> VersionId {
        self.baseline_version
    }

    #[must_use]
    pub const fn digest(&self) -> RevisionDigest {
        self.digest
    }

    #[must_use]
    pub const fn reviewed_by(&self) -> AdminId {
        self.reviewed_by
    }

    #[must_use]
    pub const fn generated_at(&self) -> OffsetDateTime {
        self.provenance.assessed_at()
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

    #[must_use]
    pub const fn readiness(&self) -> Option<ReadinessStatus> {
        self.readiness
    }
}

#[cfg(test)]
mod tests {
    use time::{Duration, OffsetDateTime};

    use crate::domain::{
        change_preview::{EffectRule, ImpactProvenance, PopulationEvidence, PreviewReport},
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        flag::FlagKey,
        settings::SettingKey,
    };
    use kernel::ids::{AdminId, ChangeId, PreviewId, RevisionId, VersionId};
    use std::str::FromStr;

    fn id<T: FromStr>(value: &str) -> T {
        value.parse().ok().unwrap()
    }

    #[test]
    fn preview_identity_binds_revision_subject_and_baseline() {
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000001");
        let revision_id = id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000002");
        let baseline = id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000003");
        let subject = ChangeSubject::Setting(SettingKey::UploadMaxMb);
        let creator = id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1");
        let reviewer = id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a3");
        let draft = ChangeDraft::new(change_id, subject, baseline, creator);
        let revision = ChangeRevision::new(
            revision_id,
            change_id,
            1,
            ChangeValue::Number(12),
            None,
            creator,
            OffsetDateTime::UNIX_EPOCH,
        );
        let draft = draft
            .append_revision(revision.clone(), None)
            .expect("revision");
        let report_id = id::<PreviewId>("018f2f7c-8a3c-7000-9000-000000000004");
        let assessed_at = OffsetDateTime::UNIX_EPOCH + Duration::minutes(5);
        let provenance = ImpactProvenance::new(
            "upload-policy/v1",
            "authoritative-upload-ledger",
            assessed_at,
        )
        .expect("provenance");
        let report = PreviewReport::new(
            report_id,
            &draft,
            reviewer,
            provenance,
            EffectRule::new("preserve-stored/validate-new-only").expect("effect"),
            vec![
                PopulationEvidence::known("uploads-over-limit", 7),
                PopulationEvidence::known("no-affected-accounts", 0),
                PopulationEvidence::unavailable("account-recovery"),
            ],
        )
        .expect("preview");

        assert_eq!(report.id(), report_id);
        assert_eq!(report.change_id(), change_id);
        assert_eq!(report.revision_id(), revision_id);
        assert_eq!(report.subject(), subject);
        assert_eq!(report.baseline_version(), baseline);
        assert_eq!(report.generated_at(), assessed_at);
        assert_eq!(report.reviewed_by(), reviewer);
        assert_ne!(report.reviewed_by(), draft.creator());
        assert_eq!(report.provenance().policy(), "upload-policy/v1");
        assert_eq!(report.provenance().source(), "authoritative-upload-ledger");
        assert_eq!(
            report.effect_rule().as_str(),
            "preserve-stored/validate-new-only"
        );
        assert_eq!(report.evidence()[0].known_count(), Some(7));
        assert_eq!(report.evidence()[1].known_count(), Some(0));
        assert_eq!(report.evidence()[2].known_count(), None);

        let other = ChangeDraft::new(
            id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000005"),
            ChangeSubject::Flag(FlagKey::Ticketing),
            id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000006"),
            creator,
        )
        .append_revision(
            ChangeRevision::new(
                id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000007"),
                id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000005"),
                1,
                ChangeValue::Boolean(false),
                None,
                creator,
                OffsetDateTime::UNIX_EPOCH,
            ),
            None,
        )
        .expect("other revision");
        assert_ne!(other.current().unwrap().change_id(), report.change_id());
        assert_ne!(other.current().unwrap().id(), report.revision_id());
        assert_ne!(other.subject(), report.subject());
        assert_ne!(other.baseline_version(), report.baseline_version());
    }

    #[test]
    fn unavailable_population_is_not_known_empty() {
        let unavailable = PopulationEvidence::unavailable("history-scan");
        assert!(unavailable.is_unavailable());
        assert_ne!(unavailable.known_count(), Some(0));
    }
}
