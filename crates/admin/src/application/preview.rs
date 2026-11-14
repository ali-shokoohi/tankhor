//! Exact-revision impact/readiness preview with no lifecycle or vendor effects.
use crate::{
    domain::{
        audit::{
            Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, PreviewMetadata, ReasonCode,
            Subject,
        },
        auth_state::BindingDigest,
        change_preview::{EffectRule, ImpactProvenance, PopulationEvidence, PreviewReport},
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        flag_readiness::CapabilityReadiness,
        impact_attempts::{AttemptLimitImpact, GenerationUnitSnapshot, assess_attempt_limit},
        impact_cards::{CardLimitImpact, CardProjectSnapshot, assess_card_limit},
        impact_lifetime::{LifetimeImpact, LifetimePolicy, LifetimePopulation, assess_lifetime},
        impact_photos::{PhotoAccountSnapshot, PhotoLimitImpact, assess_photo_limit},
        impact_recovery::{PendingRecoverySnapshot, RecoveryImpact, assess_recovery_window},
        impact_uploads::{UploadPolicyImpact, UploadPolicySnapshot, assess_upload_policy},
        settings::{SettingKey, SettingValue},
    },
    ports::{
        admin_repository::AdminRepository, audit_repository::AuditRepository,
        change_repository::ChangeRepository, flag_repository::FlagRepository,
        session_store::SessionStore, settings_repository::SettingsRepository,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{ChangeId, RevisionId},
    ports::{
        clock::Clock,
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewEvidence {
    Lifetime(LifetimePopulation),
    Card(CardProjectSnapshot),
    Photo(PhotoAccountSnapshot),
    Upload(UploadPolicySnapshot),
    Attempt(GenerationUnitSnapshot),
    Recovery(PendingRecoverySnapshot),
    None,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpactAssessment {
    Lifetime(LifetimeImpact),
    Card(CardLimitImpact),
    Photo(PhotoLimitImpact),
    Upload(UploadPolicyImpact),
    Attempt(AttemptLimitImpact),
    Recovery(RecoveryImpact),
    Capability(CapabilityReadiness),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewOutcome {
    pub report: PreviewReport,
    pub impact: ImpactAssessment,
}

#[derive(Debug, Clone)]
pub struct PreviewInput {
    pub actor: BindingDigest,
    pub change: String,
    pub revision: String,
    pub evidence: PreviewEvidence,
}

pub struct Preview<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub sessions: &'a dyn SessionStore,
    pub admins: &'a dyn AdminRepository,
    pub settings: &'a dyn SettingsRepository,
    pub flags: &'a dyn FlagRepository,
    pub changes: &'a dyn ChangeRepository,
    pub audit: &'a dyn AuditRepository,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
}

impl Preview<'_> {
    /// Persists one exact report and audit; input evidence is gathered before the transaction.
    ///
    /// # Errors
    /// Returns validation, authorisation, conflict, or persistence errors.
    pub async fn execute(&self, input: PreviewInput) -> Result<PreviewOutcome, DomainError> {
        let change = parse::<ChangeId>(&input.change)?;
        let revision = parse::<RevisionId>(&input.revision)?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            let session = self
                .sessions
                .lookup(&mut tx, input.actor)
                .await?
                .ok_or_else(denied)?;
            let admin = self
                .admins
                .get(&mut tx, session.admin_id())
                .await?
                .ok_or_else(denied)?;
            let now = self.clock.now().await?;
            session.check_access(&admin, now)?;
            let draft = self
                .changes
                .draft(&mut tx, change)
                .await?
                .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?;
            if draft.head() != Some(revision)
                || draft.current().map(ChangeRevision::id) != Some(revision)
            {
                return Err(DomainError::conflict(ErrorCode::StaleRevision));
            }
            let current = match draft.subject() {
                ChangeSubject::Setting(key) => {
                    let record = self
                        .settings
                        .get(&mut tx, key)
                        .await?
                        .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?;
                    if record.version().version() != draft.baseline_version() {
                        return Err(DomainError::conflict(ErrorCode::StaleRevision));
                    }
                    Some(record.version().value().clone())
                }
                ChangeSubject::Flag(key) => {
                    let record = self
                        .flags
                        .get(&mut tx, key)
                        .await?
                        .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?;
                    if record.version().version() != draft.baseline_version() {
                        return Err(DomainError::conflict(ErrorCode::StaleRevision));
                    }
                    None
                }
            };
            let preview_id = self.ids.next_preview_id().await?;
            let (impact, provenance, effect, evidence, readiness) =
                assess(&draft, current.as_ref(), input.evidence, now)?;
            let report = PreviewReport::new_with_readiness(
                preview_id,
                &draft,
                admin.id(),
                provenance,
                effect,
                evidence,
                readiness,
            )?;
            self.changes.save_preview(&mut tx, report.clone()).await?;
            let audit_id = self.ids.next_audit_id().await?;
            self.audit
                .append(
                    &mut tx,
                    AuditEntry::try_from(AuditInput {
                        id: audit_id,
                        event_key: audit_id,
                        actor: Some(Actor::Admin(admin.id())),
                        action: Some(AuditAction::Previewed),
                        subject: Some(Subject::Change(change)),
                        occurred_at: Some(now),
                        reason: Some(ReasonCode::PreviewAssessment),
                        metadata: AuditMetadata::Preview(PreviewMetadata::new(
                            revision,
                            draft.baseline_version(),
                            report.digest(),
                        )),
                    })?,
                )
                .await?;
            Ok(PreviewOutcome { report, impact })
        }
        .await;
        match result {
            Ok(output) => {
                self.uow.commit(tx).await?;
                Ok(output)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}

type Assessment = (
    ImpactAssessment,
    ImpactProvenance,
    EffectRule,
    Vec<PopulationEvidence>,
    Option<crate::domain::flag_readiness::ReadinessStatus>,
);

#[allow(clippy::too_many_lines)]
fn assess(
    draft: &ChangeDraft,
    current: Option<&SettingValue>,
    evidence: PreviewEvidence,
    now: OffsetDateTime,
) -> Result<Assessment, DomainError> {
    let subject = draft.subject();
    let proposed = draft.current().ok_or_else(invalid)?.value();
    if matches!(evidence, PreviewEvidence::Unavailable) {
        let (policy, source, effect) = policy_identity(subject);
        return Ok((
            ImpactAssessment::Unavailable,
            ImpactProvenance::new(policy, source, now)?,
            EffectRule::new(effect)?,
            vec![PopulationEvidence::unavailable("required_evidence")],
            None,
        ));
    }
    match (subject, proposed, evidence) {
        (ChangeSubject::Flag(key), ChangeValue::Boolean(_), PreviewEvidence::None) => {
            let provenance = ImpactProvenance::new(
                "flag-capability-readiness/v1",
                "registered-capability-evidence",
                now,
            )?;
            let readiness = CapabilityReadiness::not_delivered(
                key,
                draft.head().ok_or_else(invalid)?,
                draft.baseline_version(),
                provenance.clone(),
            );
            Ok((
                ImpactAssessment::Capability(readiness.clone()),
                provenance,
                EffectRule::new("capability-not-delivered/enablement-blocked")?,
                vec![],
                Some(readiness.status()),
            ))
        }
        (
            ChangeSubject::Setting(
                key @ (SettingKey::BuyerTempPhotoDays
                | SettingKey::StudioTempUploadDays
                | SettingKey::StudioAutoDraftDays),
            ),
            ChangeValue::Number(proposed),
            PreviewEvidence::Lifetime(population),
        ) => {
            let previous = number(current)?;
            let policy = match key {
                SettingKey::BuyerTempPhotoDays => LifetimePolicy::BuyerTempPhotos,
                SettingKey::StudioTempUploadDays => LifetimePolicy::StudioTempUploads,
                SettingKey::StudioAutoDraftDays => LifetimePolicy::StudioAutoDrafts,
                _ => return Err(invalid()),
            };
            if proposed < previous {
                let impact = assess_lifetime(policy, previous, proposed, population, now)?;
                Ok((
                    ImpactAssessment::Lifetime(impact.clone()),
                    impact.provenance().clone(),
                    impact.effect_rule().clone(),
                    impact.evidence().to_vec(),
                    None,
                ))
            } else {
                let (policy, source, _) = policy_identity(subject);
                Ok((
                    ImpactAssessment::Unavailable,
                    ImpactProvenance::new(policy, source, now)?,
                    EffectRule::new("no-existing-expiry-reduction/new-items-use-new-lifetime")?,
                    vec![
                        PopulationEvidence::known("changed_count", 0),
                        PopulationEvidence::known("already_due_count", 0),
                    ],
                    None,
                ))
            }
        }
        (
            ChangeSubject::Setting(SettingKey::StudioMaxCards),
            ChangeValue::Number(limit),
            PreviewEvidence::Card(snapshot),
        ) => {
            let impact = assess_card_limit(limit, snapshot, now)?;
            Ok((
                ImpactAssessment::Card(impact.clone()),
                impact.provenance().clone(),
                impact.effect_rule().clone(),
                vec![],
                None,
            ))
        }
        (
            ChangeSubject::Setting(SettingKey::BuyerMaxPermanentPhotos),
            ChangeValue::Number(limit),
            PreviewEvidence::Photo(snapshot),
        ) => {
            let impact = assess_photo_limit(limit, snapshot, now)?;
            Ok((
                ImpactAssessment::Photo(impact.clone()),
                impact.provenance().clone(),
                impact.effect_rule().clone(),
                vec![],
                None,
            ))
        }
        (
            ChangeSubject::Setting(
                SettingKey::UploadMaxMb
                | SettingKey::UploadMinShortestEdgePx
                | SettingKey::UploadFormats,
            ),
            _,
            PreviewEvidence::Upload(snapshot),
        ) => {
            let impact = assess_upload_policy(snapshot, now)?;
            Ok((
                ImpactAssessment::Upload(impact.clone()),
                impact.provenance().clone(),
                impact.effect_rule().clone(),
                vec![],
                None,
            ))
        }
        (
            ChangeSubject::Setting(SettingKey::GenerationMaxAttempts),
            ChangeValue::Number(limit),
            PreviewEvidence::Attempt(snapshot),
        ) => {
            let impact = assess_attempt_limit(limit, snapshot, now)?;
            Ok((
                ImpactAssessment::Attempt(impact.clone()),
                impact.provenance().clone(),
                impact.effect_rule().clone(),
                vec![],
                None,
            ))
        }
        (
            ChangeSubject::Setting(SettingKey::AccountRecoveryDays),
            ChangeValue::Number(days),
            PreviewEvidence::Recovery(snapshot),
        ) => {
            let impact = assess_recovery_window(days, snapshot, now, now)?;
            Ok((
                ImpactAssessment::Recovery(impact.clone()),
                impact.provenance().clone(),
                impact.effect_rule().clone(),
                vec![],
                None,
            ))
        }
        _ => Err(invalid()),
    }
}

fn number(current: Option<&SettingValue>) -> Result<u32, DomainError> {
    match current {
        Some(SettingValue::Number(value)) => u32::try_from(*value).map_err(|_| invalid()),
        _ => Err(invalid()),
    }
}

fn policy_identity(subject: ChangeSubject) -> (&'static str, &'static str, &'static str) {
    match subject {
        ChangeSubject::Setting(SettingKey::BuyerTempPhotoDays) => (
            "buyer-temp-photo-lifetime/v1",
            "buyer-media-ledger",
            "recalculate-original-upload/next-owning-lifecycle-sweep",
        ),
        ChangeSubject::Setting(SettingKey::StudioTempUploadDays) => (
            "studio-temp-upload-lifetime/v1",
            "studio-upload-ledger",
            "recalculate-original-upload/next-owning-lifecycle-sweep",
        ),
        ChangeSubject::Setting(SettingKey::StudioAutoDraftDays) => (
            "studio-auto-draft-lifetime/v1",
            "studio-activity-ledger",
            "recalculate-last-activity/next-owning-lifecycle-sweep",
        ),
        ChangeSubject::Setting(SettingKey::StudioMaxCards) => (
            "studio-card-limit/v1",
            "studio-project-policy",
            "preserve-existing/confirmed-unaffected/at-limit-generate-no-addition",
        ),
        ChangeSubject::Setting(SettingKey::BuyerMaxPermanentPhotos) => (
            "buyer-photo-limit/v1",
            "buyer-media-policy",
            "preserve-all-existing/add-only-below-limit",
        ),
        ChangeSubject::Setting(SettingKey::GenerationMaxAttempts) => (
            "generation-attempt-limit/v1",
            "generation-queue-policy",
            "preserve-previously-queued-and-processing/future-queued-use-new-limit",
        ),
        ChangeSubject::Setting(SettingKey::AccountRecoveryDays) => (
            "account-recovery-window/v1",
            "identity-recovery-policy",
            "preserve-pending-D/new-deletion-requests-use-new-window",
        ),
        ChangeSubject::Setting(
            SettingKey::UploadMaxMb
            | SettingKey::UploadMinShortestEdgePx
            | SettingKey::UploadFormats,
        ) => (
            "upload-validation/v1",
            "upload-validation-policy",
            "preserve-stored/validate-new-only",
        ),
        ChangeSubject::Flag(_) => (
            "flag-capability-readiness/v1",
            "registered-capability-evidence",
            "capability-not-delivered/enablement-blocked",
        ),
    }
}

fn parse<T: std::str::FromStr>(value: &str) -> Result<T, DomainError> {
    value.parse().map_err(|_| invalid())
}
const fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
