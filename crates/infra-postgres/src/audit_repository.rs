//! Append-only PostgreSQL audit persistence in the caller's transaction.

use std::str::FromStr;

use admin::domain::admin_state::Eligibility;
use admin::domain::audit::{
    Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, Factor, ReasonCode, Subject,
};
use admin::ports::audit_repository::{AuditCursor, AuditPage, AuditQuery, AuditRepository};
use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{AdminId, AuditId, PublicationId};
use kernel::ports::unit_of_work::TxHandle;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{map_sql, unavailable};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresAuditRepository {
    uow: PostgresUnitOfWork,
}

impl PostgresAuditRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

#[async_trait::async_trait]
impl AuditRepository for PostgresAuditRepository {
    async fn append(&self, tx: &mut TxHandle, entry: AuditEntry) -> Result<(), DomainError> {
        let values = InsertValues::from(&entry);
        let connection = self.uow.writable_connection(tx)?;
        let inserted = sqlx::query!(
            "INSERT INTO audit_log (
                 id, event_key, actor_type, actor_id, action, subject_type, subject_id,
                 occurred_at, reason_code, before_eligibility, after_eligibility,
                 before_eligible, after_eligible, factor, approver_id, executed_by,
                 scheduled_at, executed_at
             ) VALUES (
                 $1, $2, 'admin', $3, $4, $5, $6, $7, $8, $9, $10,
                 $11, $12, $13, $14, $15, $16, $17
             ) ON CONFLICT DO NOTHING",
            entry.id().as_uuid(),
            entry.event_key().as_uuid(),
            values.actor_id,
            values.action,
            values.subject_type,
            values.subject_id,
            entry.occurred_at(),
            entry.reason().as_str(),
            values.before_eligibility,
            values.after_eligibility,
            values.before_eligible,
            values.after_eligible,
            values.factor,
            values.approver_id,
            values.executed_by,
            values.scheduled_at,
            values.executed_at,
        )
        .execute(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if inserted.rows_affected() == 1 {
            return Ok(());
        }

        let existing = sqlx::query_as!(
            AuditRow,
            "SELECT id, event_key, actor_type, actor_id, action, subject_type, subject_id,
                    occurred_at, reason_code, before_eligibility, after_eligibility,
                    before_eligible, after_eligible, factor, approver_id, executed_by,
                    scheduled_at, executed_at
             FROM audit_log WHERE id = $1 OR event_key = $2",
            entry.id().as_uuid(),
            entry.event_key().as_uuid(),
        )
        .fetch_all(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if existing.len() == 1 {
            let persisted = existing
                .into_iter()
                .next()
                .expect("length checked")
                .into_entry()?;
            if persisted == entry {
                return Ok(());
            }
        }
        Err(DomainError::conflict(ErrorCode::IdempotencyConflict))
    }

    async fn search(&self, tx: &mut TxHandle, query: AuditQuery) -> Result<AuditPage, DomainError> {
        let filter = query.filter();
        let actor_id = filter.actor().map(|Actor::Admin(id)| id.as_uuid());
        let action = filter.action().map(action_name);
        let (subject_type, subject_id) = filter.subject().map_or((None, None), |subject| {
            let (kind, id) = subject_parts(subject);
            (Some(kind), Some(id))
        });
        let after = query.after();
        let connection = self.uow.readable_connection(tx)?;
        let mut rows = sqlx::query_as!(
            AuditRow,
            "SELECT id, event_key, actor_type, actor_id, action, subject_type, subject_id,
                    occurred_at, reason_code, before_eligibility, after_eligibility,
                    before_eligible, after_eligible, factor, approver_id, executed_by,
                    scheduled_at, executed_at
             FROM audit_log
             WHERE ($1::uuid IS NULL OR actor_id = $1)
               AND ($2::text IS NULL OR action = $2)
               AND ($3::text IS NULL OR subject_type = $3)
               AND ($4::uuid IS NULL OR subject_id = $4)
               AND ($5::timestamptz IS NULL OR occurred_at >= $5)
               AND ($6::timestamptz IS NULL OR occurred_at < $6)
               AND (NOT $7::boolean OR (occurred_at, id) > ($8::timestamptz, $9::uuid))
             ORDER BY occurred_at, id
             LIMIT $10",
            actor_id,
            action,
            subject_type,
            subject_id,
            filter.from(),
            filter.until(),
            after.is_some(),
            after.map_or(OffsetDateTime::UNIX_EPOCH, |cursor| cursor.occurred_at),
            after.map_or(Uuid::nil(), |cursor| cursor.id.as_uuid()),
            i64::from(query.limit()) + 1,
        )
        .fetch_all(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        let has_more = rows.len() > usize::from(query.limit());
        rows.truncate(usize::from(query.limit()));
        let next = if has_more {
            let row = rows
                .last()
                .expect("a nonempty page is required for a cursor");
            Some(AuditCursor {
                occurred_at: row.occurred_at,
                id: audit_id(row.id)?,
            })
        } else {
            None
        };
        let entries = rows
            .into_iter()
            .map(AuditRow::into_entry)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AuditPage { entries, next })
    }
}

struct InsertValues {
    actor_id: Uuid,
    action: &'static str,
    subject_type: &'static str,
    subject_id: Uuid,
    before_eligibility: Option<&'static str>,
    after_eligibility: Option<&'static str>,
    before_eligible: Option<bool>,
    after_eligible: Option<bool>,
    factor: Option<&'static str>,
    approver_id: Option<Uuid>,
    executed_by: Option<&'static str>,
    scheduled_at: Option<OffsetDateTime>,
    executed_at: Option<OffsetDateTime>,
}

impl From<&AuditEntry> for InsertValues {
    fn from(entry: &AuditEntry) -> Self {
        let Actor::Admin(actor) = entry.actor();
        let (subject_type, subject_id) = subject_parts(entry.subject());
        let mut values = Self {
            actor_id: actor.as_uuid(),
            action: action_name(entry.action()),
            subject_type,
            subject_id,
            before_eligibility: None,
            after_eligibility: None,
            before_eligible: None,
            after_eligible: None,
            factor: None,
            approver_id: None,
            executed_by: None,
            scheduled_at: None,
            executed_at: None,
        };
        match entry.metadata() {
            AuditMetadata::None => {}
            AuditMetadata::Factor(factor) => values.factor = Some(factor_name(*factor)),
            AuditMetadata::Invitation { before, after } => {
                values.before_eligibility = before.map(eligibility_name);
                values.after_eligibility = Some(eligibility_name(*after));
            }
            AuditMetadata::Enrollment {
                before,
                after,
                factor,
            } => {
                values.before_eligibility = Some(eligibility_name(*before));
                values.after_eligibility = Some(eligibility_name(*after));
                values.factor = Some(factor_name(*factor));
            }
            AuditMetadata::Eligibility { before, after } => {
                values.before_eligible = Some(*before);
                values.after_eligible = Some(*after);
            }
            AuditMetadata::Scheduled(metadata) => {
                values.approver_id =
                    Some(entry.metadata().approver().expect("scheduled").as_uuid());
                values.executed_by = Some("system_worker");
                values.scheduled_at = Some(metadata.scheduled_at());
                values.executed_at = Some(metadata.executed_at());
            }
        }
        values
    }
}

struct AuditRow {
    id: Uuid,
    event_key: Uuid,
    actor_type: String,
    actor_id: Uuid,
    action: String,
    subject_type: String,
    subject_id: Uuid,
    occurred_at: OffsetDateTime,
    reason_code: String,
    before_eligibility: Option<String>,
    after_eligibility: Option<String>,
    before_eligible: Option<bool>,
    after_eligible: Option<bool>,
    factor: Option<String>,
    approver_id: Option<Uuid>,
    executed_by: Option<String>,
    scheduled_at: Option<OffsetDateTime>,
    executed_at: Option<OffsetDateTime>,
}

impl AuditRow {
    fn into_entry(self) -> Result<AuditEntry, DomainError> {
        if self.actor_type != "admin" {
            return Err(unavailable());
        }
        let actor = Actor::Admin(admin_id(self.actor_id)?);
        let action = parse_action(&self.action)?;
        let subject = match self.subject_type.as_str() {
            "admin" => Subject::Admin(admin_id(self.subject_id)?),
            "publication" => Subject::Publication(
                PublicationId::try_new(self.subject_id).map_err(|_| unavailable())?,
            ),
            _ => return Err(unavailable()),
        };
        let metadata = match action {
            AuditAction::SignedIn | AuditAction::EnrollmentStarted => {
                AuditMetadata::Factor(parse_factor(self.factor.as_deref())?)
            }
            AuditAction::EnrollmentConfirmed => AuditMetadata::Enrollment {
                before: parse_eligibility(self.before_eligibility.as_deref())?,
                after: parse_eligibility(self.after_eligibility.as_deref())?,
                factor: parse_factor(self.factor.as_deref())?,
            },
            AuditAction::Invited => AuditMetadata::Invitation {
                before: self
                    .before_eligibility
                    .as_deref()
                    .map(parse_optional_eligibility)
                    .transpose()?,
                after: parse_eligibility(self.after_eligibility.as_deref())?,
            },
            AuditAction::Recovered => AuditMetadata::Eligibility {
                before: self.before_eligible.ok_or_else(unavailable)?,
                after: self.after_eligible.ok_or_else(unavailable)?,
            },
            AuditAction::Published | AuditAction::ScheduledCancelled => {
                if self.executed_by.as_deref() != Some("system_worker") {
                    return Err(unavailable());
                }
                AuditMetadata::scheduled(
                    admin_id(self.actor_id)?,
                    admin_id(self.approver_id.ok_or_else(unavailable)?)?,
                    self.scheduled_at.ok_or_else(unavailable)?,
                    self.executed_at.ok_or_else(unavailable)?,
                )
            }
        };
        AuditEntry::try_from(AuditInput {
            id: audit_id(self.id)?,
            event_key: audit_id(self.event_key)?,
            actor: Some(actor),
            action: Some(action),
            subject: Some(subject),
            occurred_at: Some(self.occurred_at),
            reason: Some(ReasonCode::from_str(&self.reason_code).map_err(|_| unavailable())?),
            metadata,
        })
        .map_err(|_| unavailable())
    }
}

fn subject_parts(subject: Subject) -> (&'static str, Uuid) {
    match subject {
        Subject::Admin(id) => ("admin", id.as_uuid()),
        Subject::Publication(id) => ("publication", id.as_uuid()),
    }
}

const fn action_name(action: AuditAction) -> &'static str {
    match action {
        AuditAction::SignedIn => "signed_in",
        AuditAction::EnrollmentStarted => "enrollment_started",
        AuditAction::EnrollmentConfirmed => "enrollment_confirmed",
        AuditAction::Invited => "invited",
        AuditAction::Recovered => "recovered",
        AuditAction::Published => "published",
        AuditAction::ScheduledCancelled => "scheduled_cancelled",
    }
}

fn parse_action(value: &str) -> Result<AuditAction, DomainError> {
    match value {
        "signed_in" => Ok(AuditAction::SignedIn),
        "enrollment_started" => Ok(AuditAction::EnrollmentStarted),
        "enrollment_confirmed" => Ok(AuditAction::EnrollmentConfirmed),
        "invited" => Ok(AuditAction::Invited),
        "recovered" => Ok(AuditAction::Recovered),
        "published" => Ok(AuditAction::Published),
        "scheduled_cancelled" => Ok(AuditAction::ScheduledCancelled),
        _ => Err(unavailable()),
    }
}

const fn factor_name(factor: Factor) -> &'static str {
    match factor {
        Factor::Passkey => "passkey",
        Factor::Totp => "totp",
    }
}

fn parse_factor(value: Option<&str>) -> Result<Factor, DomainError> {
    match value {
        Some("passkey") => Ok(Factor::Passkey),
        Some("totp") => Ok(Factor::Totp),
        _ => Err(unavailable()),
    }
}

const fn eligibility_name(value: Eligibility) -> &'static str {
    match value {
        Eligibility::Invited => "invited",
        Eligibility::Active => "active",
        Eligibility::Recovering => "recovering",
        Eligibility::Deactivated => "deactivated",
        Eligibility::Revoked => "revoked",
    }
}

fn parse_eligibility(value: Option<&str>) -> Result<Eligibility, DomainError> {
    parse_optional_eligibility(value.ok_or_else(unavailable)?)
}

fn parse_optional_eligibility(value: &str) -> Result<Eligibility, DomainError> {
    match value {
        "invited" => Ok(Eligibility::Invited),
        "active" => Ok(Eligibility::Active),
        "recovering" => Ok(Eligibility::Recovering),
        "deactivated" => Ok(Eligibility::Deactivated),
        "revoked" => Ok(Eligibility::Revoked),
        _ => Err(unavailable()),
    }
}

fn admin_id(value: Uuid) -> Result<AdminId, DomainError> {
    AdminId::try_new(value).map_err(|_| unavailable())
}

fn audit_id(value: Uuid) -> Result<AuditId, DomainError> {
    AuditId::try_new(value).map_err(|_| unavailable())
}
