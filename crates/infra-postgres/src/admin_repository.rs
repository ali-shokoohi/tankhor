//! Protected administrator state and enrollment in caller-owned transactions.

use admin::domain::admin_state::{
    AdminState, Eligibility, EnrollmentAuthorization, EnrollmentPurpose, EnrollmentStatus,
    Invalidation, SecurityEpoch,
};
use admin::ports::admin_repository::{
    AdminListQuery, AdminPage, AdminRepository, AuthorizeEnrollment, IdentityDigest,
};
use kernel::error::{DomainError, ErrorCode};
use kernel::ids::AdminId;
use kernel::ports::unit_of_work::TxHandle;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{map_sql, stale, unavailable};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresAdminRepository {
    uow: PostgresUnitOfWork,
}

impl PostgresAdminRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

#[async_trait::async_trait]
impl AdminRepository for PostgresAdminRepository {
    async fn find_by_identity(
        &self,
        tx: &mut TxHandle,
        identity: IdentityDigest,
    ) -> Result<Option<AdminState>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        sqlx::query_as!(
            AdminRow,
            "SELECT id, eligibility, security_epoch::text AS \"security_epoch!\",
                    enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
                    enrollment_expires_at, enrollment_epoch::text AS enrollment_epoch,
                    enrollment_status
             FROM admins WHERE phone_lookup_digest = $1",
            identity.as_bytes().as_slice(),
        )
        .fetch_optional(connection)
        .await
        .map_err(|error| map_sql(&error))?
        .map(AdminRow::into_state)
        .transpose()
    }

    async fn list(
        &self,
        tx: &mut TxHandle,
        query: AdminListQuery,
    ) -> Result<AdminPage, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        let mut rows = sqlx::query_as!(
            AdminRow,
            "SELECT id, eligibility, security_epoch::text AS \"security_epoch!\",
                    enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
                    enrollment_expires_at, enrollment_epoch::text AS enrollment_epoch,
                    enrollment_status
             FROM admins
             WHERE ($1::uuid IS NULL OR id > $1)
             ORDER BY id LIMIT $2",
            query.after().map(AdminId::as_uuid),
            i64::from(query.limit()) + 1,
        )
        .fetch_all(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        let has_more = rows.len() > usize::from(query.limit());
        rows.truncate(usize::from(query.limit()));
        let next = if has_more {
            rows.last().map(|row| admin_id(row.id)).transpose()?
        } else {
            None
        };
        let entries = rows
            .into_iter()
            .map(AdminRow::into_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AdminPage { entries, next })
    }

    async fn get(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
    ) -> Result<Option<AdminState>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        select_admin(connection, target, false).await
    }

    async fn lock(
        &self,
        tx: &mut TxHandle,
        mut targets: Vec<AdminId>,
    ) -> Result<Vec<AdminState>, DomainError> {
        targets.sort_unstable();
        targets.dedup();
        let ids: Vec<Uuid> = targets.into_iter().map(AdminId::as_uuid).collect();
        let connection = self.uow.readable_connection(tx)?;
        sqlx::query_as!(
            AdminRow,
            "SELECT id, eligibility, security_epoch::text AS \"security_epoch!\",
                    enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
                    enrollment_expires_at, enrollment_epoch::text AS enrollment_epoch,
                    enrollment_status
             FROM admins WHERE id = ANY($1::uuid[]) ORDER BY id FOR UPDATE",
            &ids,
        )
        .fetch_all(connection)
        .await
        .map_err(|error| map_sql(&error))?
        .into_iter()
        .map(AdminRow::into_state)
        .collect()
    }

    async fn invite(
        &self,
        tx: &mut TxHandle,
        state: AdminState,
        identity: IdentityDigest,
    ) -> Result<(), DomainError> {
        let enrollment = state.enrollment();
        let connection = self.uow.writable_connection(tx)?;
        sqlx::query!(
            "INSERT INTO admins (
                 id, phone_key_id, phone_nonce, phone_ciphertext, phone_lookup_digest,
                 eligibility, security_epoch, created_by, created_at,
                 enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
                 enrollment_expires_at, enrollment_epoch, enrollment_status
             ) VALUES (
                 $1, 'identity-only', $2, $3, $4, $5, CAST($6 AS TEXT)::NUMERIC, $1,
                 '1970-01-01T00:00:00Z', $7, $8, $9, $10, CAST($11 AS TEXT)::NUMERIC, $12
             )",
            state.id().as_uuid(),
            &[0_u8; 12].as_slice(),
            &[0_u8; 16].as_slice(),
            identity.as_bytes().as_slice(),
            eligibility_name(&state),
            epoch_value(state.epoch()),
            enrollment.map(|value| purpose_name(value.purpose())),
            enrollment.map(|value| value.authorizer().as_uuid()),
            enrollment.map(EnrollmentAuthorization::authorized_at),
            enrollment.map(EnrollmentAuthorization::expires_at),
            enrollment.map(|value| epoch_value(value.epoch())),
            enrollment.map(|value| status_name(value.status())),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }

    async fn invalidate_epoch(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: SecurityEpoch,
        next: SecurityEpoch,
        kind: Invalidation,
    ) -> Result<(), DomainError> {
        if next <= expected {
            return Err(stale());
        }
        let eligibility = match kind {
            Invalidation::Deactivate => "deactivated",
            Invalidation::Revoke => "revoked",
            Invalidation::Recover => "recovering",
        };
        let connection = self.uow.writable_connection(tx)?;
        let result = sqlx::query!(
            "UPDATE admins SET eligibility = $4, security_epoch = CAST($3 AS TEXT)::NUMERIC,
                    enrollment_status = CASE WHEN enrollment_status IS NULL
                        THEN NULL ELSE 'revoked' END
             WHERE id = $1 AND security_epoch = CAST($2 AS TEXT)::NUMERIC
               AND CAST($3 AS TEXT)::NUMERIC > security_epoch",
            target.as_uuid(),
            epoch_value(expected),
            epoch_value(next),
            eligibility,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(stale())
        }
    }

    async fn restore(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: SecurityEpoch,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let result = sqlx::query!(
            "UPDATE admins SET eligibility = 'active'
             WHERE id = $1 AND security_epoch = CAST($2 AS TEXT)::NUMERIC",
            target.as_uuid(),
            epoch_value(expected),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(stale())
        }
    }

    async fn authorize_enrollment(
        &self,
        tx: &mut TxHandle,
        command: AuthorizeEnrollment,
    ) -> Result<(), DomainError> {
        if command.expires_at <= command.authorized_at {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        let locked = self.lock(tx, vec![command.actor, command.target]).await?;
        let actor = locked.iter().find(|state| state.id() == command.actor);
        let target = locked.iter().find(|state| state.id() == command.target);
        authorization_error(&command, actor, target)?;
        let expected = command.expected_authorization.as_ref();
        let has_expected = expected.is_some();
        let connection = self.uow.writable_connection(tx)?;
        let result = sqlx::query!(
            "UPDATE admins AS target SET
                 enrollment_purpose = $2, enrollment_authorizer = $3,
                 enrollment_authorized_at = $4, enrollment_expires_at = $5,
                 enrollment_epoch = CAST($6 AS TEXT)::NUMERIC, enrollment_status = 'pending'
             WHERE target.id = $1
               AND target.security_epoch = CAST($6 AS TEXT)::NUMERIC
               AND target.eligibility = $7
               AND EXISTS (
                   SELECT 1 FROM admins AS actor
                   WHERE actor.id = $3 AND actor.eligibility = 'active' AND actor.id <> target.id
               )
               AND (
                   (NOT $8 AND target.enrollment_purpose IS NULL)
                   OR ($8 AND target.enrollment_purpose = $9
                       AND target.enrollment_authorizer = $10
                       AND target.enrollment_authorized_at = $11
                       AND target.enrollment_expires_at = $12
                       AND target.enrollment_epoch = CAST($13 AS TEXT)::NUMERIC
                       AND target.enrollment_status = $14)
               )",
            command.target.as_uuid(),
            purpose_name(command.purpose),
            command.actor.as_uuid(),
            command.authorized_at,
            command.expires_at,
            epoch_value(command.expected_epoch),
            eligibility_for_purpose(command.purpose),
            has_expected,
            expected.map(|value| purpose_name(value.purpose())),
            expected.map(|value| value.authorizer().as_uuid()),
            expected.map(EnrollmentAuthorization::authorized_at),
            expected.map(EnrollmentAuthorization::expires_at),
            expected.map(|value| epoch_value(value.epoch())),
            expected.map(|value| status_name(value.status())),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(stale())
        }
    }

    async fn enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
    ) -> Result<Option<EnrollmentAuthorization>, DomainError> {
        Ok(self
            .get(tx, target)
            .await?
            .and_then(|state| state.enrollment().cloned()))
    }

    async fn consume_enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: EnrollmentAuthorization,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let result = sqlx::query!(
            "UPDATE admins SET enrollment_status = 'consumed'
             WHERE id = $1 AND security_epoch = CAST($2 AS TEXT)::NUMERIC
               AND eligibility = $3 AND enrollment_purpose = $4
               AND enrollment_authorizer = $5 AND enrollment_authorized_at = $6
               AND enrollment_expires_at = $7 AND enrollment_epoch = CAST($2 AS TEXT)::NUMERIC
               AND enrollment_status = 'pending' AND enrollment_authorized_at <= $8
               AND enrollment_expires_at > $8",
            target.as_uuid(),
            epoch_value(expected.epoch()),
            eligibility_for_purpose(expected.purpose()),
            purpose_name(expected.purpose()),
            expected.authorizer().as_uuid(),
            expected.authorized_at(),
            expected.expires_at(),
            now,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(stale())
        }
    }

    async fn revoke_enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: EnrollmentAuthorization,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let result = sqlx::query!(
            "UPDATE admins SET enrollment_status = 'revoked'
             WHERE id = $1 AND security_epoch = CAST($2 AS TEXT)::NUMERIC
               AND enrollment_purpose = $3 AND enrollment_authorizer = $4
               AND enrollment_authorized_at = $5 AND enrollment_expires_at = $6
               AND enrollment_epoch = CAST($2 AS TEXT)::NUMERIC AND enrollment_status = $7",
            target.as_uuid(),
            epoch_value(expected.epoch()),
            purpose_name(expected.purpose()),
            expected.authorizer().as_uuid(),
            expected.authorized_at(),
            expected.expires_at(),
            status_name(expected.status()),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(stale())
        }
    }
}

async fn select_admin(
    connection: &mut sqlx::PgConnection,
    target: AdminId,
    for_update: bool,
) -> Result<Option<AdminState>, DomainError> {
    if for_update {
        sqlx::query_as!(
            AdminRow,
            "SELECT id, eligibility, security_epoch::text AS \"security_epoch!\",
                    enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
                    enrollment_expires_at, enrollment_epoch::text AS enrollment_epoch,
                    enrollment_status
             FROM admins WHERE id = $1 FOR UPDATE",
            target.as_uuid(),
        )
        .fetch_optional(connection)
        .await
    } else {
        sqlx::query_as!(
            AdminRow,
            "SELECT id, eligibility, security_epoch::text AS \"security_epoch!\",
                    enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
                    enrollment_expires_at, enrollment_epoch::text AS enrollment_epoch,
                    enrollment_status
             FROM admins WHERE id = $1",
            target.as_uuid(),
        )
        .fetch_optional(connection)
        .await
    }
    .map_err(|error| map_sql(&error))?
    .map(AdminRow::into_state)
    .transpose()
}

struct AdminRow {
    id: Uuid,
    eligibility: String,
    security_epoch: String,
    enrollment_purpose: Option<String>,
    enrollment_authorizer: Option<Uuid>,
    enrollment_authorized_at: Option<OffsetDateTime>,
    enrollment_expires_at: Option<OffsetDateTime>,
    enrollment_epoch: Option<String>,
    enrollment_status: Option<String>,
}

impl AdminRow {
    fn into_state(self) -> Result<AdminState, DomainError> {
        let id = admin_id(self.id)?;
        let epoch = parse_epoch(&self.security_epoch)?;
        let eligibility = parse_eligibility(&self.eligibility)?;
        let Some(purpose) = self.enrollment_purpose else {
            return Ok(AdminState::new(id, eligibility, epoch));
        };
        let authorizer = admin_id(self.enrollment_authorizer.ok_or_else(unavailable)?)?;
        let authorization_epoch = parse_epoch(&self.enrollment_epoch.ok_or_else(unavailable)?)?;
        let authorized_at = self.enrollment_authorized_at.ok_or_else(unavailable)?;
        let expires_at = self.enrollment_expires_at.ok_or_else(unavailable)?;
        let purpose = parse_purpose(&purpose)?;
        let initial_eligibility = match purpose {
            EnrollmentPurpose::Addition => Eligibility::Invited,
            EnrollmentPurpose::Recovery => Eligibility::Recovering,
        };
        let mut state = AdminState::new(id, initial_eligibility, authorization_epoch);
        let actor = AdminState::new(authorizer, Eligibility::Active, SecurityEpoch::new(0));
        state
            .authorize_enrollment(
                &actor,
                purpose,
                authorization_epoch,
                authorized_at,
                expires_at,
            )
            .map_err(|_| unavailable())?;
        let status = match self.enrollment_status.as_deref() {
            Some("pending") => EnrollmentStatus::Pending,
            Some("consumed") => {
                state
                    .consume_enrollment(id, purpose, authorization_epoch, authorized_at)
                    .map_err(|_| unavailable())?;
                EnrollmentStatus::Consumed
            }
            Some("revoked") => {
                state
                    .revoke_enrollment(authorization_epoch)
                    .map_err(|_| unavailable())?;
                EnrollmentStatus::Revoked
            }
            _ => return Err(unavailable()),
        };
        if epoch != authorization_epoch {
            if status == EnrollmentStatus::Pending {
                return Err(unavailable());
            }
            let invalidation = match eligibility {
                Eligibility::Deactivated => Invalidation::Deactivate,
                Eligibility::Revoked => Invalidation::Revoke,
                Eligibility::Active | Eligibility::Recovering => Invalidation::Recover,
                Eligibility::Invited => return Err(unavailable()),
            };
            state
                .invalidate(authorization_epoch, epoch, invalidation)
                .map_err(|_| unavailable())?;
        }
        if eligibility != state.eligibility() {
            if eligibility != Eligibility::Active {
                return Err(unavailable());
            }
            state.restore(epoch).map_err(|_| unavailable())?;
        }
        Ok(state)
    }
}

fn authorization_error(
    command: &AuthorizeEnrollment,
    actor: Option<&AdminState>,
    target: Option<&AdminState>,
) -> Result<(), DomainError> {
    let Some(target) = target else {
        return Err(DomainError::validation(ErrorCode::NotFound));
    };
    if command.actor == command.target || !actor.is_some_and(AdminState::is_eligible) {
        return Err(DomainError::forbidden(ErrorCode::PermissionDenied));
    }
    if target.epoch() != command.expected_epoch
        || target.enrollment() != command.expected_authorization.as_ref()
    {
        return Err(stale());
    }
    if !matches!(
        (target.eligibility(), command.purpose),
        (Eligibility::Invited, EnrollmentPurpose::Addition)
            | (Eligibility::Recovering, EnrollmentPurpose::Recovery)
    ) {
        return Err(DomainError::forbidden(ErrorCode::PermissionDenied));
    }
    Ok(())
}

fn admin_id(value: Uuid) -> Result<AdminId, DomainError> {
    AdminId::try_new(value).map_err(|_| unavailable())
}

fn epoch_value(value: SecurityEpoch) -> String {
    value.value().to_string()
}

fn parse_epoch(value: &str) -> Result<SecurityEpoch, DomainError> {
    value
        .parse::<u64>()
        .map(SecurityEpoch::new)
        .map_err(|_| unavailable())
}

const fn eligibility_name(state: &AdminState) -> &'static str {
    match state.eligibility() {
        Eligibility::Invited => "invited",
        Eligibility::Active => "active",
        Eligibility::Recovering => "recovering",
        Eligibility::Deactivated => "deactivated",
        Eligibility::Revoked => "revoked",
    }
}

fn parse_eligibility(value: &str) -> Result<Eligibility, DomainError> {
    match value {
        "invited" => Ok(Eligibility::Invited),
        "active" => Ok(Eligibility::Active),
        "recovering" => Ok(Eligibility::Recovering),
        "deactivated" => Ok(Eligibility::Deactivated),
        "revoked" => Ok(Eligibility::Revoked),
        _ => Err(unavailable()),
    }
}

const fn eligibility_for_purpose(purpose: EnrollmentPurpose) -> &'static str {
    match purpose {
        EnrollmentPurpose::Addition => "invited",
        EnrollmentPurpose::Recovery => "recovering",
    }
}

const fn purpose_name(purpose: EnrollmentPurpose) -> &'static str {
    match purpose {
        EnrollmentPurpose::Addition => "addition",
        EnrollmentPurpose::Recovery => "recovery",
    }
}

fn parse_purpose(value: &str) -> Result<EnrollmentPurpose, DomainError> {
    match value {
        "addition" => Ok(EnrollmentPurpose::Addition),
        "recovery" => Ok(EnrollmentPurpose::Recovery),
        _ => Err(unavailable()),
    }
}

const fn status_name(status: EnrollmentStatus) -> &'static str {
    match status {
        EnrollmentStatus::Pending => "pending",
        EnrollmentStatus::Consumed => "consumed",
        EnrollmentStatus::Revoked => "revoked",
    }
}
