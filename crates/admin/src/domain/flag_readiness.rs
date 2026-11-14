//! Pure flag capability readiness; absence of evidence always blocks enablement.

use crate::domain::{change_preview::ImpactProvenance, flag::FlagKey};
use kernel::ids::{RevisionId, VersionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessStatus {
    NotDelivered,
    Unknown,
    Failed,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enablement {
    Blocked,
    Permitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityEvidence {
    delivered: bool,
    conformance_passed: Option<bool>,
}

impl CapabilityEvidence {
    #[must_use]
    pub const fn new(delivered: bool, conformance_passed: Option<bool>) -> Self {
        Self {
            delivered,
            conformance_passed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReadiness {
    flag: FlagKey,
    revision: RevisionId,
    baseline: VersionId,
    provenance: ImpactProvenance,
    status: ReadinessStatus,
}

impl CapabilityReadiness {
    #[must_use]
    pub fn not_delivered(
        flag: FlagKey,
        revision: RevisionId,
        baseline: VersionId,
        provenance: ImpactProvenance,
    ) -> Self {
        Self::evaluate(
            flag,
            revision,
            baseline,
            provenance,
            CapabilityEvidence::new(false, None),
        )
    }

    #[must_use]
    pub fn evaluate(
        flag: FlagKey,
        revision: RevisionId,
        baseline: VersionId,
        provenance: ImpactProvenance,
        evidence: CapabilityEvidence,
    ) -> Self {
        let status = if evidence.delivered {
            match evidence.conformance_passed {
                None => ReadinessStatus::Unknown,
                Some(false) => ReadinessStatus::Failed,
                Some(true) => ReadinessStatus::Ready,
            }
        } else {
            ReadinessStatus::NotDelivered
        };
        Self {
            flag,
            revision,
            baseline,
            provenance,
            status,
        }
    }

    #[must_use]
    pub const fn status(&self) -> ReadinessStatus {
        self.status
    }

    #[must_use]
    pub const fn enablement(&self) -> Enablement {
        match self.status {
            ReadinessStatus::Ready => Enablement::Permitted,
            _ => Enablement::Blocked,
        }
    }

    #[must_use]
    pub const fn flag(&self) -> FlagKey {
        self.flag
    }

    #[must_use]
    pub const fn revision(&self) -> RevisionId {
        self.revision
    }

    #[must_use]
    pub const fn baseline(&self) -> VersionId {
        self.baseline
    }

    #[must_use]
    pub const fn provenance(&self) -> &ImpactProvenance {
        &self.provenance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{change_preview::ImpactProvenance, flag::launch_flags};
    use kernel::ids::{RevisionId, VersionId};
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn id<T: FromStr>(value: &str) -> T {
        value.parse().ok().unwrap()
    }

    #[test]
    fn flag_enablement_requires_delivered_passing_evidence() {
        let revision = id::<RevisionId>("018f2f7c-8a3c-7000-9000-0000000000a1");
        let baseline = id::<VersionId>("018f2f7c-8a3c-7000-9000-0000000000a2");
        let provenance = ImpactProvenance::new(
            "flag-capability-readiness/v1",
            "registered-capability-evidence",
            OffsetDateTime::UNIX_EPOCH + Duration::days(20_000),
        )
        .unwrap();
        for flag in launch_flags() {
            let readiness = CapabilityReadiness::not_delivered(
                flag.key(),
                revision,
                baseline,
                provenance.clone(),
            );
            assert_eq!(readiness.status(), ReadinessStatus::NotDelivered);
            assert_eq!(readiness.enablement(), Enablement::Blocked);
            assert_eq!(readiness.provenance(), &provenance);
        }

        let cases = [
            (
                CapabilityEvidence::new(false, None),
                ReadinessStatus::NotDelivered,
            ),
            (
                CapabilityEvidence::new(true, None),
                ReadinessStatus::Unknown,
            ),
            (
                CapabilityEvidence::new(true, Some(false)),
                ReadinessStatus::Failed,
            ),
            (
                CapabilityEvidence::new(true, Some(true)),
                ReadinessStatus::Ready,
            ),
        ];
        for (evidence, expected) in cases {
            let readiness = CapabilityReadiness::evaluate(
                launch_flags()[0].key(),
                revision,
                baseline,
                provenance.clone(),
                evidence,
            );
            assert_eq!(readiness.status(), expected);
            assert_eq!(
                readiness.enablement() == Enablement::Permitted,
                expected == ReadinessStatus::Ready
            );
            assert_eq!(readiness.revision(), revision);
            assert_eq!(readiness.baseline(), baseline);
        }
    }
}
