//! Digest-only OTP challenges with conditional, transaction-scoped transitions.

use admin::domain::admin_state::{EnrollmentPurpose, SecurityEpoch};
use admin::domain::auth_state::{AuthPurpose, BindingDigest};
use admin::domain::otp_state::{OtpBinding, OtpDigest, OtpInput, OtpState};
use admin::ports::otp_challenge_repository::{
    OtpChallengeRepository, OtpCheck, SubmissionCategory,
};
use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{AdminId, ChallengeId};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle};
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::error::{map_sql, stale};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresOtpChallengeRepository {
    uow: PostgresUnitOfWork,
}

impl PostgresOtpChallengeRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

#[async_trait::async_trait]
impl OtpChallengeRepository for PostgresOtpChallengeRepository {
    async fn revoke_all(&self, tx: &mut TxHandle, target: AdminId) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        sqlx::query!(
            "UPDATE admin_otp_challenges SET revoked_at = NOW()
             WHERE admin_id = $1 AND consumed_at IS NULL
               AND superseded_at IS NULL AND revoked_at IS NULL",
            target.as_uuid(),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }

    async fn issue(
        &self,
        tx: &mut TxHandle,
        state: OtpState,
        predecessor: Option<ChallengeId>,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let duplicate = sqlx::query_scalar!(
            "SELECT id FROM admin_otp_challenges WHERE id = $1",
            state.id().as_uuid(),
        )
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if duplicate.is_some() {
            return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
        }
        if let Some(id) = predecessor {
            let Some(previous) = load_locked(connection, id).await? else {
                return Err(stale());
            };
            let previous_state = previous.clone().into_state()?;
            if !previous.pending()
                || previous_state.binding() != state.binding()
                || previous.max_attempts
                    != i32::try_from(state.max_attempts())
                        .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?
                || previous.failed_attempts
                    != i32::try_from(state.failed_attempts())
                        .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?
            {
                return Err(stale());
            }
            sqlx::query!(
                "UPDATE admin_otp_challenges SET superseded_at = $2
                 WHERE id = $1 AND consumed_at IS NULL
                   AND superseded_at IS NULL AND revoked_at IS NULL",
                id.as_uuid(),
                state.issued_at(),
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
        } else {
            let existing = sqlx::query_scalar!(
                "SELECT id FROM admin_otp_challenges
                 WHERE admin_id = $1 AND browser_digest = $2 AND purpose = $3
                   AND security_epoch = $4::text::numeric LIMIT 1",
                state.binding().target.as_uuid(),
                state.binding().browser.as_bytes().as_slice(),
                purpose_text(state.binding().purpose),
                state.binding().epoch.value().to_string(),
            )
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
            if existing.is_some() {
                return Err(stale());
            }
        }
        let inserted = sqlx::query!(
            "INSERT INTO admin_otp_challenges (
                 id, admin_id, browser_digest, purpose, security_epoch, code_digest,
                 issued_at, expires_at, failed_attempts, max_attempts,
                 submission_category, predecessor_id
             ) VALUES ($1, $2, $3, $4, $5::text::numeric, $6, $7, $8, $9, $10,
                       'unknown', $11)
             ON CONFLICT DO NOTHING",
            state.id().as_uuid(),
            state.binding().target.as_uuid(),
            state.binding().browser.as_bytes().as_slice(),
            purpose_text(state.binding().purpose),
            state.binding().epoch.value().to_string(),
            state.digest().as_bytes().as_slice(),
            state.issued_at(),
            state.expires_at(),
            i32::try_from(state.failed_attempts())
                .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?,
            i32::try_from(state.max_attempts())
                .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?,
            predecessor.map(ChallengeId::as_uuid),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if inserted.rows_affected() == 1 {
            Ok(())
        } else {
            Err(DomainError::conflict(ErrorCode::IdempotencyConflict))
        }
    }

    async fn lock(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
    ) -> Result<Option<OtpState>, DomainError> {
        let mode = tx.mode();
        let connection = self.uow.readable_connection(tx)?;
        let row = if mode == TransactionMode::ReadWrite {
            load_locked(connection, id).await?
        } else {
            sqlx::query_as!(
                OtpRow,
                "SELECT id, admin_id, browser_digest, purpose, security_epoch::text AS \"epoch!\",
                        code_digest, issued_at, expires_at, failed_attempts, max_attempts,
                        consumed_at, superseded_at, revoked_at
                 FROM admin_otp_challenges WHERE id = $1",
                id.as_uuid(),
            )
            .fetch_optional(connection)
            .await
            .map_err(|error| map_sql(&error))?
        };
        row.map(OtpRow::into_state).transpose()
    }

    async fn record_failure(&self, tx: &mut TxHandle, check: OtpCheck) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let updated = sqlx::query!(
            "UPDATE admin_otp_challenges SET failed_attempts = failed_attempts + 1
             WHERE id = $1 AND admin_id = $2 AND browser_digest = $3 AND purpose = $4
               AND security_epoch = $5::text::numeric AND failed_attempts = $6
               AND failed_attempts < max_attempts AND issued_at <= $7 AND expires_at > $7
               AND consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
            check.id.as_uuid(),
            check.binding.target.as_uuid(),
            check.binding.browser.as_bytes().as_slice(),
            purpose_text(check.binding.purpose),
            check.binding.epoch.value().to_string(),
            i32::try_from(check.expected_failed_attempts)
                .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?,
            check.now,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        exactly_one(updated.rows_affected())
    }

    async fn consume_once(
        &self,
        tx: &mut TxHandle,
        check: OtpCheck,
        candidate: OtpDigest,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let updated = sqlx::query!(
            "UPDATE admin_otp_challenges SET consumed_at = $8
             WHERE id = $1 AND admin_id = $2 AND browser_digest = $3 AND purpose = $4
               AND security_epoch = $5::text::numeric AND failed_attempts = $6
               AND failed_attempts < max_attempts AND issued_at <= $7 AND expires_at > $7
               AND code_digest = $9 AND consumed_at IS NULL
               AND superseded_at IS NULL AND revoked_at IS NULL",
            check.id.as_uuid(),
            check.binding.target.as_uuid(),
            check.binding.browser.as_bytes().as_slice(),
            purpose_text(check.binding.purpose),
            check.binding.epoch.value().to_string(),
            i32::try_from(check.expected_failed_attempts)
                .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?,
            check.now,
            check.now,
            candidate.as_bytes().as_slice(),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        exactly_one(updated.rows_affected())
    }

    async fn record_submission(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
        category: SubmissionCategory,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let updated = sqlx::query!(
            "UPDATE admin_otp_challenges SET submission_category = $2 WHERE id = $1",
            id.as_uuid(),
            submission_text(category),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        exactly_one(updated.rows_affected())
    }

    async fn submission(
        &self,
        tx: &mut TxHandle,
        id: ChallengeId,
    ) -> Result<Option<SubmissionCategory>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        sqlx::query_scalar!(
            "SELECT submission_category FROM admin_otp_challenges WHERE id = $1",
            id.as_uuid(),
        )
        .fetch_optional(connection)
        .await
        .map_err(|error| map_sql(&error))?
        .map(|value| parse_submission(&value))
        .transpose()
    }
}

#[derive(Clone)]
struct OtpRow {
    id: uuid::Uuid,
    admin_id: uuid::Uuid,
    browser_digest: Vec<u8>,
    purpose: String,
    epoch: String,
    code_digest: Vec<u8>,
    issued_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    failed_attempts: i32,
    max_attempts: i32,
    consumed_at: Option<OffsetDateTime>,
    superseded_at: Option<OffsetDateTime>,
    revoked_at: Option<OffsetDateTime>,
}

impl OtpRow {
    fn pending(&self) -> bool {
        self.consumed_at.is_none() && self.superseded_at.is_none() && self.revoked_at.is_none()
    }

    fn into_state(self) -> Result<OtpState, DomainError> {
        let input = OtpInput {
            id: ChallengeId::try_new(self.id).map_err(|_| stale())?,
            binding: OtpBinding {
                target: AdminId::try_new(self.admin_id).map_err(|_| stale())?,
                browser: BindingDigest::new(self.browser_digest.try_into().map_err(|_| stale())?),
                purpose: parse_purpose(&self.purpose)?,
                epoch: SecurityEpoch::new(self.epoch.parse().map_err(|_| stale())?),
            },
            digest: OtpDigest::new(self.code_digest.try_into().map_err(|_| stale())?),
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            max_attempts: self.max_attempts.try_into().map_err(|_| stale())?,
            failed_attempts: self.failed_attempts.try_into().map_err(|_| stale())?,
        };
        let mut state = OtpState::try_from(input)?;
        let binding = state.binding().clone();
        let digest = state.digest().clone();
        match (self.consumed_at, self.superseded_at, self.revoked_at) {
            (Some(_), None, None) => state
                .consume(&binding, &digest, self.issued_at)
                .map_err(|_| stale())?,
            (None, Some(_), None) => state.supersede(),
            (None, None, Some(_)) => state.revoke(),
            (None, None, None) => {}
            _ => return Err(stale()),
        }
        Ok(state)
    }
}

async fn load_locked(
    connection: &mut PgConnection,
    id: ChallengeId,
) -> Result<Option<OtpRow>, DomainError> {
    sqlx::query_as!(
        OtpRow,
        "SELECT id, admin_id, browser_digest, purpose, security_epoch::text AS \"epoch!\",
                code_digest, issued_at, expires_at, failed_attempts, max_attempts,
                consumed_at, superseded_at, revoked_at
         FROM admin_otp_challenges WHERE id = $1 FOR UPDATE",
        id.as_uuid(),
    )
    .fetch_optional(connection)
    .await
    .map_err(|error| map_sql(&error))
}

fn exactly_one(count: u64) -> Result<(), DomainError> {
    if count == 1 { Ok(()) } else { Err(stale()) }
}

const fn purpose_text(purpose: AuthPurpose) -> &'static str {
    match purpose {
        AuthPurpose::SignIn => "sign_in",
        AuthPurpose::Enrollment(EnrollmentPurpose::Addition) => "addition",
        AuthPurpose::Enrollment(EnrollmentPurpose::Recovery) => "recovery",
    }
}

fn parse_purpose(value: &str) -> Result<AuthPurpose, DomainError> {
    match value {
        "sign_in" => Ok(AuthPurpose::SignIn),
        "addition" => Ok(AuthPurpose::Enrollment(EnrollmentPurpose::Addition)),
        "recovery" => Ok(AuthPurpose::Enrollment(EnrollmentPurpose::Recovery)),
        _ => Err(stale()),
    }
}

const fn submission_text(category: SubmissionCategory) -> &'static str {
    match category {
        SubmissionCategory::Accepted => "accepted",
        SubmissionCategory::Rejected => "rejected",
        SubmissionCategory::Unavailable => "unavailable",
        SubmissionCategory::Unknown => "unknown",
    }
}

fn parse_submission(value: &str) -> Result<SubmissionCategory, DomainError> {
    match value {
        "accepted" => Ok(SubmissionCategory::Accepted),
        "rejected" => Ok(SubmissionCategory::Rejected),
        "unavailable" => Ok(SubmissionCategory::Unavailable),
        "unknown" => Ok(SubmissionCategory::Unknown),
        _ => Err(stale()),
    }
}
