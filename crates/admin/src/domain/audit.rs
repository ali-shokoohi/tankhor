//! Typed, append-only audit values; no free-text or secret-bearing snapshots.
//! FR-021/024, AC-ADM-024; persistence and workflow authority are separate slices.

use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, AuditId, ChangeId, PublicationId, RevisionId, VersionId},
};
use std::str::FromStr;
use time::OffsetDateTime;

/// Internal attribution, never a phone number or client-provided display name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Admin(AdminId),
}

/// Closed event identifiers, extended by their driving workflow tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    SignedIn,
    EnrollmentStarted,
    EnrollmentConfirmed,
    Invited,
    Recovered,
    Published,
    ScheduledCancelled,
    FoundationSeeded,
    DraftSaved,
    Previewed,
}

/// Typed internal subject references only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Admin(AdminId),
    Publication(PublicationId),
    Change(ChangeId),
}

/// Safe authentication-factor name, never its proof/material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Factor {
    Passkey,
    Totp,
}

/// Trusted scheduled executor; not parsed from a client string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Executor {
    SystemWorker,
}

/// Managed reason identifiers shared by FA/EN presentation resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonCode {
    Authentication,
    AdminAddition,
    AdminRecovery,
    AdminRevoked,
    ScheduledPublication,
    FoundationSeed,
    DraftChange,
    PreviewAssessment,
}

impl ReasonCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::AdminAddition => "admin_addition",
            Self::AdminRecovery => "admin_recovery",
            Self::AdminRevoked => "admin_revoked",
            Self::ScheduledPublication => "scheduled_publication",
            Self::FoundationSeed => "foundation_seed",
            Self::DraftChange => "draft_change",
            Self::PreviewAssessment => "preview_assessment",
        }
    }

    /// Locale-independent Fluent identifier; never an operator's free-text reason.
    #[must_use]
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::Authentication => "console-reason-authentication",
            Self::AdminAddition => "console-reason-admin-addition",
            Self::AdminRecovery => "console-reason-admin-recovery",
            Self::AdminRevoked => "console-reason-admin-revoked",
            Self::ScheduledPublication => "console-reason-scheduled-publication",
            Self::FoundationSeed => "console-reason-foundation-seed",
            Self::DraftChange => "console-reason-draft-change",
            Self::PreviewAssessment => "console-reason-preview-assessment",
        }
    }
}

impl FromStr for ReasonCode {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "authentication" => Ok(Self::Authentication),
            "admin_addition" => Ok(Self::AdminAddition),
            "admin_recovery" => Ok(Self::AdminRecovery),
            "admin_revoked" => Ok(Self::AdminRevoked),
            "scheduled_publication" => Ok(Self::ScheduledPublication),
            "foundation_seed" => Ok(Self::FoundationSeed),
            "draft_change" => Ok(Self::DraftChange),
            "preview_assessment" => Ok(Self::PreviewAssessment),
            _ => Err(invalid()),
        }
    }
}

/// Allowlisted before/after and attribution; no arbitrary JSON or text payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditMetadata {
    None,
    Factor(Factor),
    Invitation {
        before: Option<crate::domain::admin_state::Eligibility>,
        after: crate::domain::admin_state::Eligibility,
    },
    Enrollment {
        before: crate::domain::admin_state::Eligibility,
        after: crate::domain::admin_state::Eligibility,
        factor: Factor,
    },
    Eligibility {
        before: bool,
        after: bool,
    },
    Scheduled(ScheduledMetadata),
    Draft(DraftMetadata),
    Preview(PreviewMetadata),
}

/// Exact reviewed revision, baseline and canonical digest; no evidence payload text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewMetadata {
    revision_id: RevisionId,
    baseline_version: VersionId,
    digest: crate::domain::change_hash::RevisionDigest,
}

impl PreviewMetadata {
    #[must_use]
    pub const fn new(
        revision_id: RevisionId,
        baseline_version: VersionId,
        digest: crate::domain::change_hash::RevisionDigest,
    ) -> Self {
        Self {
            revision_id,
            baseline_version,
            digest,
        }
    }
    #[must_use]
    pub const fn revision_id(&self) -> RevisionId {
        self.revision_id
    }
    #[must_use]
    pub const fn baseline_version(&self) -> VersionId {
        self.baseline_version
    }
    #[must_use]
    pub const fn digest(&self) -> crate::domain::change_hash::RevisionDigest {
        self.digest
    }
}

/// Exact typed draft identity and before/after values; no operator free text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftMetadata {
    before: Option<crate::domain::change_revision::ChangeValue>,
    after: crate::domain::change_revision::ChangeValue,
    baseline_version: VersionId,
    revision_id: RevisionId,
    scheduled_at: Option<crate::domain::scheduled_time::ScheduledTime>,
}

impl DraftMetadata {
    #[must_use]
    pub const fn new(
        before: Option<crate::domain::change_revision::ChangeValue>,
        after: crate::domain::change_revision::ChangeValue,
        baseline_version: VersionId,
        revision_id: RevisionId,
        scheduled_at: Option<crate::domain::scheduled_time::ScheduledTime>,
    ) -> Self {
        Self {
            before,
            after,
            baseline_version,
            revision_id,
            scheduled_at,
        }
    }
    #[must_use]
    pub const fn before(&self) -> Option<crate::domain::change_revision::ChangeValue> {
        self.before
    }
    #[must_use]
    pub const fn after(&self) -> crate::domain::change_revision::ChangeValue {
        self.after
    }
    #[must_use]
    pub const fn baseline_version(&self) -> VersionId {
        self.baseline_version
    }
    #[must_use]
    pub const fn revision_id(&self) -> RevisionId {
        self.revision_id
    }
    #[must_use]
    pub const fn scheduled_at(&self) -> Option<&crate::domain::scheduled_time::ScheduledTime> {
        self.scheduled_at.as_ref()
    }
}

/// Fields are private so no external executor identity can be inserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledMetadata {
    scheduler: AdminId,
    approver: AdminId,
    executor: Executor,
    scheduled_at: OffsetDateTime,
    executed_at: OffsetDateTime,
}

impl ScheduledMetadata {
    #[must_use]
    pub const fn scheduled_at(&self) -> OffsetDateTime {
        self.scheduled_at
    }

    #[must_use]
    pub const fn executed_at(&self) -> OffsetDateTime {
        self.executed_at
    }
}

impl AuditMetadata {
    #[must_use]
    pub const fn scheduled(
        scheduler: AdminId,
        approver: AdminId,
        scheduled_at: OffsetDateTime,
        executed_at: OffsetDateTime,
    ) -> Self {
        Self::Scheduled(ScheduledMetadata {
            scheduler,
            approver,
            executor: Executor::SystemWorker,
            scheduled_at,
            executed_at,
        })
    }

    /// Parse only the safe factor field. Other metadata requires typed constructors.
    ///
    /// # Errors
    /// Rejects unknown/duplicate keys, free-text snapshots and secret-bearing values.
    pub fn from_fields(fields: &[(&str, &str)]) -> Result<Self, DomainError> {
        match fields {
            [] => Ok(Self::None),
            [("factor", "passkey")] => Ok(Self::Factor(Factor::Passkey)),
            [("factor", "totp")] => Ok(Self::Factor(Factor::Totp)),
            _ => Err(invalid()),
        }
    }

    #[must_use]
    pub const fn executor(&self) -> Option<Executor> {
        match self {
            Self::Scheduled(metadata) => Some(metadata.executor),
            _ => None,
        }
    }

    #[must_use]
    pub const fn approver(&self) -> Option<AdminId> {
        match self {
            Self::Scheduled(metadata) => Some(metadata.approver),
            _ => None,
        }
    }
}

/// Incomplete boundary input cannot be persisted as an `AuditEntry`.
#[derive(Debug, Clone)]
pub struct AuditInput {
    pub id: AuditId,
    pub event_key: AuditId,
    pub actor: Option<Actor>,
    pub action: Option<AuditAction>,
    pub subject: Option<Subject>,
    pub occurred_at: Option<OffsetDateTime>,
    pub reason: Option<ReasonCode>,
    pub metadata: AuditMetadata,
}

/// Immutable validated event with complete safe attribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    id: AuditId,
    event_key: AuditId,
    actor: Actor,
    action: AuditAction,
    subject: Subject,
    occurred_at: OffsetDateTime,
    reason: ReasonCode,
    metadata: AuditMetadata,
}

impl TryFrom<AuditInput> for AuditEntry {
    type Error = DomainError;

    fn try_from(input: AuditInput) -> Result<Self, Self::Error> {
        let entry = Self {
            id: input.id,
            event_key: input.event_key,
            actor: input.actor.ok_or_else(invalid)?,
            action: input.action.ok_or_else(invalid)?,
            subject: input.subject.ok_or_else(invalid)?,
            occurred_at: input
                .occurred_at
                .ok_or_else(invalid)?
                .to_offset(time::UtcOffset::UTC),
            reason: input.reason.ok_or_else(invalid)?,
            metadata: input.metadata,
        };
        let valid = match (&entry.action, &entry.subject, &entry.metadata) {
            (
                AuditAction::SignedIn | AuditAction::EnrollmentStarted,
                Subject::Admin(target),
                AuditMetadata::Factor(_),
            ) => entry.actor == Actor::Admin(*target) && entry.reason == ReasonCode::Authentication,
            (
                AuditAction::EnrollmentConfirmed,
                Subject::Admin(target),
                AuditMetadata::Enrollment { before, after, .. },
            ) => {
                entry.actor == Actor::Admin(*target)
                    && entry.reason == ReasonCode::Authentication
                    && matches!(
                        before,
                        crate::domain::admin_state::Eligibility::Invited
                            | crate::domain::admin_state::Eligibility::Recovering
                    )
                    && *after == crate::domain::admin_state::Eligibility::Active
            }
            (
                AuditAction::Invited,
                Subject::Admin(target),
                AuditMetadata::Invitation {
                    before: None,
                    after,
                },
            ) => {
                entry.actor != Actor::Admin(*target)
                    && entry.reason == ReasonCode::AdminAddition
                    && *after == crate::domain::admin_state::Eligibility::Invited
            }
            (AuditAction::Recovered, Subject::Admin(_), AuditMetadata::Eligibility { .. }) => {
                entry.reason == ReasonCode::AdminRecovery
            }
            (
                AuditAction::Published | AuditAction::ScheduledCancelled,
                Subject::Publication(_),
                AuditMetadata::Scheduled(metadata),
            ) => {
                entry.actor == Actor::Admin(metadata.scheduler)
                    && entry.occurred_at == metadata.executed_at
                    && metadata.executed_at >= metadata.scheduled_at
                    && entry.reason
                        == if entry.action == AuditAction::ScheduledCancelled {
                            ReasonCode::AdminRevoked
                        } else {
                            ReasonCode::ScheduledPublication
                        }
            }
            (AuditAction::FoundationSeeded, Subject::Admin(system), AuditMetadata::None) => {
                entry.actor == Actor::Admin(*system) && entry.reason == ReasonCode::FoundationSeed
            }
            (AuditAction::DraftSaved, Subject::Change(_), AuditMetadata::Draft(_)) => {
                entry.reason == ReasonCode::DraftChange
            }
            (AuditAction::Previewed, Subject::Change(_), AuditMetadata::Preview(_)) => {
                entry.reason == ReasonCode::PreviewAssessment
            }
            _ => false,
        };
        if !valid {
            return Err(invalid());
        }
        Ok(entry)
    }
}

impl AuditEntry {
    #[must_use]
    pub const fn id(&self) -> AuditId {
        self.id
    }

    #[must_use]
    pub const fn event_key(&self) -> AuditId {
        self.event_key
    }

    #[must_use]
    pub const fn actor(&self) -> Actor {
        self.actor
    }

    #[must_use]
    pub const fn action(&self) -> AuditAction {
        self.action
    }

    #[must_use]
    pub const fn subject(&self) -> Subject {
        self.subject
    }

    #[must_use]
    pub const fn occurred_at(&self) -> OffsetDateTime {
        self.occurred_at
    }

    #[must_use]
    pub const fn reason(&self) -> ReasonCode {
        self.reason
    }

    #[must_use]
    pub const fn metadata(&self) -> &AuditMetadata {
        &self.metadata
    }
}

fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::{
        Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, Executor, Factor, ReasonCode,
        Subject,
    };
    use crate::domain::secrets::SecretBytes;
    use kernel::ids::{AdminId, AuditId, PublicationId};
    use std::str::FromStr;
    use time::OffsetDateTime;

    fn admin(suffix: u8) -> AdminId {
        AdminId::from_str(&format!("019a53a0-0000-7000-8000-{suffix:012}")).unwrap()
    }

    fn fixture() -> AuditInput {
        AuditInput {
            id: AuditId::from_str("019a53a0-0000-7000-8000-000000000010").unwrap(),
            event_key: AuditId::from_str("019a53a0-0000-7000-8000-000000000011").unwrap(),
            actor: Some(Actor::Admin(admin(1))),
            action: Some(AuditAction::SignedIn),
            subject: Some(Subject::Admin(admin(1))),
            occurred_at: Some(OffsetDateTime::UNIX_EPOCH),
            reason: Some(ReasonCode::Authentication),
            metadata: AuditMetadata::Factor(Factor::Passkey),
        }
    }

    #[test]
    fn audit_excludes_authentication_secrets_and_private_data() {
        let entry = AuditEntry::try_from(fixture()).unwrap();
        assert_eq!(entry.actor(), Actor::Admin(admin(1)));
        assert_eq!(entry.action(), AuditAction::SignedIn);
        assert_eq!(entry.subject(), Subject::Admin(admin(1)));
        assert_eq!(entry.occurred_at(), OffsetDateTime::UNIX_EPOCH);
        assert_eq!(entry.reason(), ReasonCode::Authentication);
        assert_eq!(entry.metadata(), &AuditMetadata::Factor(Factor::Passkey));
        assert_eq!(entry.id(), fixture().id);
        assert_eq!(entry.event_key(), fixture().event_key);

        let mut recovery = fixture();
        recovery.action = Some(AuditAction::Recovered);
        recovery.subject = Some(Subject::Admin(admin(2)));
        recovery.reason = Some(ReasonCode::AdminRecovery);
        recovery.metadata = AuditMetadata::Eligibility {
            before: true,
            after: false,
        };
        assert_eq!(
            AuditEntry::try_from(recovery).unwrap().metadata(),
            &AuditMetadata::Eligibility {
                before: true,
                after: false
            }
        );

        assert_required_fields();
        assert_scheduled_attribution();
        assert_secret_canaries();
    }

    fn assert_required_fields() {
        for missing in 0..6 {
            let mut input = fixture();
            match missing {
                0 => input.actor = None,
                1 => input.action = None,
                2 => input.subject = None,
                3 => input.occurred_at = None,
                4 => input.reason = None,
                5 => input.metadata = AuditMetadata::None,
                _ => unreachable!(),
            }
            assert!(AuditEntry::try_from(input).is_err());
        }
        for reason in [ReasonCode::Authentication, ReasonCode::AdminRevoked] {
            assert!(!reason.message_key().is_empty());
            assert_eq!(ReasonCode::from_str(reason.as_str()), Ok(reason));
        }
        for text in [
            "",
            "arbitrary operator text",
            "phone=private-canary",
            "secret=123456",
        ] {
            assert!(ReasonCode::from_str(text).is_err());
        }
    }

    fn assert_scheduled_attribution() {
        let scheduler = admin(1);
        let approver = admin(2);
        let publication = PublicationId::from_str("019a53a0-0000-7000-8000-000000000020").unwrap();
        for action in [AuditAction::Published, AuditAction::ScheduledCancelled] {
            let mut input = fixture();
            input.action = Some(action);
            input.subject = Some(Subject::Publication(publication));
            input.reason = Some(if action == AuditAction::ScheduledCancelled {
                ReasonCode::AdminRevoked
            } else {
                ReasonCode::ScheduledPublication
            });
            input.metadata = AuditMetadata::scheduled(
                scheduler,
                approver,
                OffsetDateTime::UNIX_EPOCH,
                OffsetDateTime::UNIX_EPOCH,
            );
            let entry = AuditEntry::try_from(input.clone()).unwrap();
            assert_eq!(entry.actor(), Actor::Admin(scheduler));
            assert_eq!(entry.metadata().executor(), Some(Executor::SystemWorker));
            assert_eq!(entry.metadata().approver(), Some(approver));
            assert!(format!("{entry:?}").contains("SystemWorker"));
            input.actor = Some(Actor::Admin(admin(3)));
            assert!(AuditEntry::try_from(input).is_err());
        }
        let mut input = fixture();
        input.action = Some(AuditAction::ScheduledCancelled);
        assert!(AuditEntry::try_from(input).is_err());
        assert!(AuditMetadata::from_fields(&[("executed_by", "client-admin")]).is_err());
    }

    fn assert_secret_canaries() {
        for canary in [
            "phone-private-canary",
            "otp-private-canary",
            "key-private-canary",
            "photo-private-canary",
            "payment-private-canary",
            "totp-secret-canary",
            "otpauth://totp/provisioning-qr-canary",
        ] {
            let secret = SecretBytes::new(canary.as_bytes().to_vec());
            assert_eq!(secret.expose(), canary.as_bytes());
            assert!(!format!("{secret:?}").contains(canary));
            for key in [
                "before", "after", "phone", "otp", "key", "photo", "payment", "qr",
            ] {
                let error = AuditMetadata::from_fields(&[(key, canary)]).unwrap_err();
                assert!(!format!("{error:?} {error}").contains(canary));
            }
            assert!(ReasonCode::from_str(canary).is_err());
        }
        assert_eq!(
            AuditMetadata::from_fields(&[("factor", "passkey")]).unwrap(),
            AuditMetadata::Factor(Factor::Passkey)
        );
        assert!(AuditMetadata::from_fields(&[("factor", "secret-canary")]).is_err());
        assert!(AuditMetadata::from_fields(&[("factor", "passkey"), ("factor", "totp")]).is_err());
    }
}
