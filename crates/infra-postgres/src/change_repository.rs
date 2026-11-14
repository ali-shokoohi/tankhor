//! Single-subject change draft, immutable revision, preview and approval adapter.

use std::str::FromStr;

use admin::{
    domain::{
        change_approval::Approval,
        change_hash::{RevisionDigest, draft_hash},
        change_preview::{EffectRule, ImpactProvenance, PopulationEvidence, PreviewReport},
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        flag::FlagKey,
        flag_readiness::ReadinessStatus,
        formats::FormatSet,
        scheduled_time::ScheduledTime,
        settings::SettingKey,
    },
    ports::change_repository::ChangeRepository,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ApprovalId, ChangeId, PreviewId, RevisionId, VersionId},
    ports::unit_of_work::TxHandle,
};
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    error::{map_sql, stale},
    unit_of_work::PostgresUnitOfWork,
};

#[derive(Clone, Debug)]
pub struct PostgresChangeRepository {
    uow: PostgresUnitOfWork,
}

impl PostgresChangeRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

const fn corrupt() -> DomainError {
    DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)
}

const fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

fn subject_columns(
    subject: ChangeSubject,
    baseline: VersionId,
) -> (
    Option<&'static str>,
    Option<&'static str>,
    Option<Uuid>,
    Option<Uuid>,
) {
    match subject {
        ChangeSubject::Setting(key) => (Some(key.as_str()), None, Some(baseline.as_uuid()), None),
        ChangeSubject::Flag(key) => (None, Some(key.as_str()), None, Some(baseline.as_uuid())),
    }
}

fn decode_subject(
    setting_key: Option<&str>,
    flag_key: Option<&str>,
) -> Result<ChangeSubject, DomainError> {
    let setting = setting_key.map(SettingKey::from_str).transpose()?;
    let flag = flag_key.map(FlagKey::from_str).transpose()?;
    ChangeSubject::new(setting, flag)
}

fn decode_baseline(
    subject: ChangeSubject,
    setting_version: Option<Uuid>,
    flag_version: Option<Uuid>,
) -> Result<VersionId, DomainError> {
    let ((ChangeSubject::Setting(_), Some(raw), None) | (ChangeSubject::Flag(_), None, Some(raw))) =
        (subject, setting_version, flag_version)
    else {
        return Err(corrupt());
    };
    VersionId::try_new(raw).map_err(|_| corrupt())
}

fn value_columns(value: ChangeValue) -> (Option<i64>, Option<i32>, Option<bool>) {
    match value {
        ChangeValue::Number(number) => (Some(i64::from(number)), None, None),
        ChangeValue::Formats(formats) => (None, Some(i32::from(formats.mask())), None),
        ChangeValue::Boolean(enabled) => (None, None, Some(enabled)),
    }
}

fn encode_readiness(readiness: ReadinessStatus) -> &'static str {
    match readiness {
        ReadinessStatus::NotDelivered => "not-delivered",
        ReadinessStatus::Unknown => "unknown",
        ReadinessStatus::Failed => "failed",
        ReadinessStatus::Ready => "ready",
    }
}

fn decode_readiness(raw: &str) -> Result<ReadinessStatus, DomainError> {
    match raw {
        "not-delivered" => Ok(ReadinessStatus::NotDelivered),
        "unknown" => Ok(ReadinessStatus::Unknown),
        "failed" => Ok(ReadinessStatus::Failed),
        "ready" => Ok(ReadinessStatus::Ready),
        _ => Err(corrupt()),
    }
}

fn prefix_draft(draft: &ChangeDraft, count: usize) -> Result<ChangeDraft, DomainError> {
    let mut prefix = ChangeDraft::new(
        draft.id(),
        draft.subject(),
        draft.baseline_version(),
        draft.creator(),
    );
    for revision in draft.revisions().iter().take(count).cloned() {
        let head = prefix.head();
        prefix = prefix.append_revision(revision, head)?;
    }
    Ok(prefix)
}

struct DraftRow {
    id: Uuid,
    setting_key: Option<String>,
    flag_key: Option<String>,
    baseline_setting_version: Option<Uuid>,
    baseline_flag_version: Option<Uuid>,
    head_revision_id: Option<Uuid>,
    creator_id: Uuid,
}

struct RevisionRow {
    id: Uuid,
    change_id: Uuid,
    revision_number: i64,
    setting_key: Option<String>,
    flag_key: Option<String>,
    numeric_value: Option<i64>,
    format_mask: Option<i32>,
    boolean_value: Option<bool>,
    scheduled_at: Option<OffsetDateTime>,
    scheduled_zone: Option<String>,
    created_by: Uuid,
    created_at: OffsetDateTime,
}

impl RevisionRow {
    fn into_revision(self, subject: ChangeSubject) -> Result<ChangeRevision, DomainError> {
        if decode_subject(self.setting_key.as_deref(), self.flag_key.as_deref())? != subject {
            return Err(corrupt());
        }
        let value = match (self.numeric_value, self.format_mask, self.boolean_value) {
            (Some(number), None, None) => {
                ChangeValue::Number(u32::try_from(number).map_err(|_| corrupt())?)
            }
            (None, Some(mask), None) => {
                let raw = u8::try_from(mask).map_err(|_| corrupt())?;
                ChangeValue::Formats(FormatSet::from_mask(raw).map_err(|_| corrupt())?)
            }
            (None, None, Some(enabled)) => ChangeValue::Boolean(enabled),
            _ => return Err(corrupt()),
        };
        let scheduled_at = match (self.scheduled_at, self.scheduled_zone) {
            (Some(utc), Some(zone)) => Some(ScheduledTime::authorized_execution(utc, zone)),
            (None, None) => None,
            _ => return Err(corrupt()),
        };
        Ok(ChangeRevision::new(
            RevisionId::try_new(self.id).map_err(|_| corrupt())?,
            ChangeId::try_new(self.change_id).map_err(|_| corrupt())?,
            u32::try_from(self.revision_number).map_err(|_| corrupt())?,
            value,
            scheduled_at,
            AdminId::try_new(self.created_by).map_err(|_| corrupt())?,
            self.created_at,
        ))
    }
}

async fn load_draft(
    connection: &mut PgConnection,
    id: ChangeId,
) -> Result<Option<ChangeDraft>, DomainError> {
    let Some(row) = sqlx::query_as!(
        DraftRow,
        "SELECT id, setting_key, flag_key, baseline_setting_version, baseline_flag_version,
                head_revision_id, creator_id
         FROM change_drafts WHERE id = $1",
        id.as_uuid()
    )
    .fetch_optional(&mut *connection)
    .await
    .map_err(|error| map_sql(&error))?
    else {
        return Ok(None);
    };
    let subject = decode_subject(row.setting_key.as_deref(), row.flag_key.as_deref())?;
    let baseline = decode_baseline(
        subject,
        row.baseline_setting_version,
        row.baseline_flag_version,
    )?;
    let creator = AdminId::try_new(row.creator_id).map_err(|_| corrupt())?;
    let mut draft = ChangeDraft::new(
        ChangeId::try_new(row.id).map_err(|_| corrupt())?,
        subject,
        baseline,
        creator,
    );
    let revisions = sqlx::query_as!(
        RevisionRow,
        "SELECT id, change_id, revision_number, setting_key, flag_key, numeric_value,
                format_mask, boolean_value, scheduled_at, scheduled_zone, created_by, created_at
         FROM change_revisions WHERE change_id = $1 ORDER BY revision_number ASC",
        id.as_uuid()
    )
    .fetch_all(connection)
    .await
    .map_err(|error| map_sql(&error))?;
    for revision_row in revisions {
        let revision = revision_row.into_revision(subject)?;
        let expected = draft.head();
        draft = draft
            .append_revision(revision, expected)
            .map_err(|_| corrupt())?;
    }
    let stored_head = row
        .head_revision_id
        .map(RevisionId::try_new)
        .transpose()
        .map_err(|_| corrupt())?;
    if draft.head() != stored_head {
        return Err(corrupt());
    }
    Ok(Some(draft))
}

async fn insert_revision(
    connection: &mut PgConnection,
    draft: &ChangeDraft,
    index: usize,
) -> Result<(), DomainError> {
    let revision = draft.revisions().get(index).ok_or_else(invalid)?;
    let prefix = prefix_draft(draft, index + 1)?;
    let digest = draft_hash(&prefix)?;
    let (setting_key, flag_key, _, _) = subject_columns(draft.subject(), draft.baseline_version());
    let (numeric_value, format_mask, boolean_value) = value_columns(revision.value());
    let scheduled_at = revision.scheduled_at().map(ScheduledTime::utc);
    let scheduled_zone = revision.scheduled_at().map(ScheduledTime::zone);
    sqlx::query!(
        "INSERT INTO change_revisions (
             id, change_id, revision_number, setting_key, flag_key, numeric_value, format_mask,
             boolean_value, reason_code, scheduled_at, scheduled_zone, version_hash, created_by,
             created_at
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'draft_change', $9, $10, $11, $12, $13)",
        revision.id().as_uuid(),
        draft.id().as_uuid(),
        i64::from(revision.revision_number()),
        setting_key,
        flag_key,
        numeric_value,
        format_mask,
        boolean_value,
        scheduled_at,
        scheduled_zone,
        digest.as_bytes().as_slice(),
        revision.created_by().as_uuid(),
        revision.created_at()
    )
    .execute(connection)
    .await
    .map_err(|error| map_sql(&error))?;
    Ok(())
}

struct PreviewRow {
    id: Uuid,
    change_id: Uuid,
    revision_id: Uuid,
    setting_key: Option<String>,
    flag_key: Option<String>,
    baseline_setting_version: Option<Uuid>,
    baseline_flag_version: Option<Uuid>,
    policy_identity: String,
    source_identity: String,
    effect_rule: String,
    assessed_at: OffsetDateTime,
    reviewed_by: Uuid,
    version_hash: Vec<u8>,
    readiness: Option<String>,
}

struct EvidenceRow {
    population: String,
    known_count: Option<i64>,
}

async fn build_preview(
    connection: &mut PgConnection,
    row: PreviewRow,
) -> Result<PreviewReport, DomainError> {
    let preview_id = PreviewId::try_new(row.id).map_err(|_| corrupt())?;
    let change_id = ChangeId::try_new(row.change_id).map_err(|_| corrupt())?;
    let revision_id = RevisionId::try_new(row.revision_id).map_err(|_| corrupt())?;
    let subject = decode_subject(row.setting_key.as_deref(), row.flag_key.as_deref())?;
    let baseline = decode_baseline(
        subject,
        row.baseline_setting_version,
        row.baseline_flag_version,
    )?;
    let reviewed_by = AdminId::try_new(row.reviewed_by).map_err(|_| corrupt())?;
    let stored_digest: [u8; 32] = row.version_hash.try_into().map_err(|_| corrupt())?;
    let provenance =
        ImpactProvenance::new(row.policy_identity, row.source_identity, row.assessed_at)
            .map_err(|_| corrupt())?;
    let effect_rule = EffectRule::new(row.effect_rule).map_err(|_| corrupt())?;
    let readiness = row.readiness.as_deref().map(decode_readiness).transpose()?;
    let evidence_rows = sqlx::query_as!(
        EvidenceRow,
        "SELECT population, known_count FROM change_preview_evidence
         WHERE preview_id = $1 ORDER BY evidence_index ASC",
        row.id
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| map_sql(&error))?;
    let mut evidence = Vec::with_capacity(evidence_rows.len());
    for item in evidence_rows {
        let parsed = match item.known_count {
            Some(count) => PopulationEvidence::known(
                item.population,
                u64::try_from(count).map_err(|_| corrupt())?,
            ),
            None => PopulationEvidence::unavailable(item.population),
        };
        evidence.push(parsed);
    }
    let full_draft = load_draft(connection, change_id)
        .await?
        .ok_or_else(corrupt)?;
    let position = full_draft
        .revisions()
        .iter()
        .position(|revision| revision.id() == revision_id)
        .ok_or_else(corrupt)?;
    let mut snapshot = ChangeDraft::new(change_id, subject, baseline, full_draft.creator());
    for revision in full_draft.revisions().iter().take(position + 1).cloned() {
        let head = snapshot.head();
        snapshot = snapshot
            .append_revision(revision, head)
            .map_err(|_| corrupt())?;
    }
    let report = PreviewReport::new_with_readiness(
        preview_id,
        &snapshot,
        reviewed_by,
        provenance,
        effect_rule,
        evidence,
        readiness,
    )
    .map_err(|_| corrupt())?;
    if report.digest() != RevisionDigest::from_bytes(stored_digest) {
        return Err(corrupt());
    }
    Ok(report)
}

async fn load_preview(
    connection: &mut PgConnection,
    id: PreviewId,
) -> Result<Option<PreviewReport>, DomainError> {
    let Some(row) = sqlx::query_as!(
        PreviewRow,
        "SELECT id, change_id, revision_id, setting_key, flag_key, baseline_setting_version,
                baseline_flag_version, policy_identity, source_identity, effect_rule,
                assessed_at, reviewed_by, version_hash, readiness
         FROM change_previews WHERE id = $1",
        id.as_uuid()
    )
    .fetch_optional(&mut *connection)
    .await
    .map_err(|error| map_sql(&error))?
    else {
        return Ok(None);
    };
    build_preview(connection, row).await.map(Some)
}

struct ApprovalRow {
    id: Uuid,
    preview_id: Uuid,
    approver_id: Uuid,
    approved_at: OffsetDateTime,
}

async fn build_approval(
    connection: &mut PgConnection,
    row: ApprovalRow,
) -> Result<Approval, DomainError> {
    let report = load_preview(
        connection,
        PreviewId::try_new(row.preview_id).map_err(|_| corrupt())?,
    )
    .await?
    .ok_or_else(corrupt)?;
    Approval::new(
        ApprovalId::try_new(row.id).map_err(|_| corrupt())?,
        AdminId::try_new(row.approver_id).map_err(|_| corrupt())?,
        &report,
        row.approved_at,
    )
    .map_err(|_| corrupt())
}

#[async_trait::async_trait]
impl ChangeRepository for PostgresChangeRepository {
    async fn draft(
        &self,
        tx: &mut TxHandle,
        id: ChangeId,
    ) -> Result<Option<ChangeDraft>, DomainError> {
        load_draft(self.uow.readable_connection(tx)?, id).await
    }

    async fn create_draft(&self, tx: &mut TxHandle, draft: ChangeDraft) -> Result<(), DomainError> {
        let first = draft.revisions().first().ok_or_else(invalid)?;
        let head = draft.head().ok_or_else(invalid)?;
        let (setting_key, flag_key, baseline_setting, baseline_flag) =
            subject_columns(draft.subject(), draft.baseline_version());
        let connection = self.uow.writable_connection(tx)?;
        let present = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM change_drafts WHERE id = $1) AS present",
            draft.id().as_uuid()
        )
        .fetch_one(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?
        .present
        .unwrap_or(false);
        if present {
            return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
        }
        sqlx::query!(
            "INSERT INTO change_drafts (
                 id, setting_key, flag_key, baseline_setting_version, baseline_flag_version,
                 head_revision_id, edit_counter, creator_id, created_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            draft.id().as_uuid(),
            setting_key,
            flag_key,
            baseline_setting,
            baseline_flag,
            head.as_uuid(),
            i64::try_from(draft.revisions().len()).map_err(|_| invalid())? - 1,
            draft.creator().as_uuid(),
            first.created_at()
        )
        .execute(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        for index in 0..draft.revisions().len() {
            insert_revision(&mut *connection, &draft, index).await?;
        }
        Ok(())
    }

    async fn append_revision(
        &self,
        tx: &mut TxHandle,
        draft: ChangeDraft,
        expected_head: Option<RevisionId>,
    ) -> Result<(), DomainError> {
        let next_head = draft.head().ok_or_else(stale)?;
        let connection = self.uow.writable_connection(tx)?;
        let current = load_draft(&mut *connection, draft.id())
            .await?
            .ok_or_else(stale)?;
        if current.head() != expected_head
            || current.subject() != draft.subject()
            || current.baseline_version() != draft.baseline_version()
            || current.creator() != draft.creator()
            || draft.revisions().len() != current.revisions().len() + 1
            || current.revisions() != &draft.revisions()[..current.revisions().len()]
        {
            return Err(stale());
        }
        let updated = sqlx::query!(
            "UPDATE change_drafts
             SET head_revision_id = $2, edit_counter = edit_counter + 1
             WHERE id = $1 AND head_revision_id IS NOT DISTINCT FROM $3",
            draft.id().as_uuid(),
            next_head.as_uuid(),
            expected_head.map(RevisionId::as_uuid)
        )
        .execute(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if updated.rows_affected() != 1 {
            return Err(stale());
        }
        insert_revision(connection, &draft, draft.revisions().len() - 1).await
    }

    async fn save_preview(
        &self,
        tx: &mut TxHandle,
        report: PreviewReport,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let present = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM change_previews WHERE id = $1) AS present",
            report.id().as_uuid()
        )
        .fetch_one(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?
        .present
        .unwrap_or(false);
        if present {
            return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
        }
        let draft = load_draft(&mut *connection, report.change_id())
            .await?
            .ok_or_else(stale)?;
        if draft.head() != Some(report.revision_id())
            || draft.subject() != report.subject()
            || draft.baseline_version() != report.baseline_version()
        {
            return Err(stale());
        }
        let (setting_key, flag_key, baseline_setting, baseline_flag) =
            subject_columns(report.subject(), report.baseline_version());
        let digest = report.digest();
        sqlx::query!(
            "INSERT INTO change_previews (
                 id, change_id, revision_id, setting_key, flag_key, baseline_setting_version,
                 baseline_flag_version, policy_identity, source_identity, effect_rule,
                 assessed_at, reviewed_by, version_hash, readiness
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
            report.id().as_uuid(),
            report.change_id().as_uuid(),
            report.revision_id().as_uuid(),
            setting_key,
            flag_key,
            baseline_setting,
            baseline_flag,
            report.provenance().policy(),
            report.provenance().source(),
            report.effect_rule().as_str(),
            report.generated_at(),
            report.reviewed_by().as_uuid(),
            digest.as_bytes().as_slice(),
            report.readiness().map(encode_readiness)
        )
        .execute(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        for (index, item) in report.evidence().iter().enumerate() {
            let (population, known_count) = match item {
                PopulationEvidence::Known { population, count } => (
                    population.as_str(),
                    Some(i64::try_from(*count).map_err(|_| invalid())?),
                ),
                PopulationEvidence::Unavailable { population } => (population.as_str(), None),
            };
            sqlx::query!(
                "INSERT INTO change_preview_evidence (
                     preview_id, evidence_index, population, known_count
                 ) VALUES ($1, $2, $3, $4)",
                report.id().as_uuid(),
                i32::try_from(index).map_err(|_| invalid())?,
                population,
                known_count
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
        }
        Ok(())
    }

    async fn preview(
        &self,
        tx: &mut TxHandle,
        id: PreviewId,
    ) -> Result<Option<PreviewReport>, DomainError> {
        load_preview(self.uow.readable_connection(tx)?, id).await
    }

    async fn latest_preview(
        &self,
        tx: &mut TxHandle,
        change: ChangeId,
        revision: RevisionId,
    ) -> Result<Option<PreviewReport>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        let Some(row) = sqlx::query_as!(
            PreviewRow,
            "SELECT id, change_id, revision_id, setting_key, flag_key, baseline_setting_version,
                    baseline_flag_version, policy_identity, source_identity, effect_rule,
                    assessed_at, reviewed_by, version_hash, readiness
             FROM change_previews
             WHERE change_id = $1 AND revision_id = $2
             ORDER BY assessed_at DESC, id DESC LIMIT 1",
            change.as_uuid(),
            revision.as_uuid()
        )
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?
        else {
            return Ok(None);
        };
        build_preview(connection, row).await.map(Some)
    }

    async fn save_approval(
        &self,
        tx: &mut TxHandle,
        approval: Approval,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let present = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM change_approvals WHERE id = $1) AS present",
            approval.id().as_uuid()
        )
        .fetch_one(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?
        .present
        .unwrap_or(false);
        if present {
            return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
        }
        let report = approval.approved_report();
        let stored_preview = load_preview(&mut *connection, report.id())
            .await?
            .ok_or_else(stale)?;
        if &stored_preview != report {
            return Err(stale());
        }
        let (setting_key, flag_key, baseline_setting, baseline_flag) =
            subject_columns(report.subject(), report.baseline_version());
        sqlx::query!(
            "INSERT INTO change_approvals (
                 id, preview_id, change_id, revision_id, setting_key, flag_key,
                 baseline_setting_version, baseline_flag_version, approver_id, approved_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            approval.id().as_uuid(),
            report.id().as_uuid(),
            report.change_id().as_uuid(),
            report.revision_id().as_uuid(),
            setting_key,
            flag_key,
            baseline_setting,
            baseline_flag,
            approval.approver_id().as_uuid(),
            approval.approved_at()
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }

    async fn approval(
        &self,
        tx: &mut TxHandle,
        id: ApprovalId,
    ) -> Result<Option<Approval>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        let Some(row) = sqlx::query_as!(
            ApprovalRow,
            "SELECT id, preview_id, approver_id, approved_at
             FROM change_approvals WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?
        else {
            return Ok(None);
        };
        build_approval(connection, row).await.map(Some)
    }

    async fn latest_approval(
        &self,
        tx: &mut TxHandle,
        change: ChangeId,
        revision: RevisionId,
    ) -> Result<Option<Approval>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        let Some(row) = sqlx::query_as!(
            ApprovalRow,
            "SELECT id, preview_id, approver_id, approved_at
             FROM change_approvals
             WHERE change_id = $1 AND revision_id = $2
             ORDER BY approved_at DESC, id DESC LIMIT 1",
            change.as_uuid(),
            revision.as_uuid()
        )
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?
        else {
            return Ok(None);
        };
        build_approval(connection, row).await.map(Some)
    }
}
