//! Digest-only sessions and transactional restricted authentication ceremonies.

use admin::domain::admin_state::{AdminState, Eligibility, EnrollmentPurpose, SecurityEpoch};
use admin::domain::audit::Factor;
use admin::domain::auth_state::{
    AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest,
};
use admin::domain::secrets::SecretBytes;
use admin::domain::session_state::{SessionInput, SessionState};
use admin::ports::session_store::{
    PendingCheck, PendingRecord, PendingView, SessionRecord, SessionStore,
};
use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{AdminId, SessionId};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::error::{map_sql, unavailable};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresSessionStore {
    uow: PostgresUnitOfWork,
}

impl PostgresSessionStore {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }

    async fn pending_row(
        &self,
        tx: &mut TxHandle,
        reference: &BindingDigest,
        lock: bool,
    ) -> Result<PendingRow, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        let digest = reference.as_bytes().as_slice();
        let row = if lock {
            sqlx::query_as!(
                PendingRow,
                "SELECT s.id, s.admin_id AS \"admin_id!\",
                        s.browser_digest, s.session_id, s.purpose, s.phase,
                        s.security_epoch::text AS \"epoch!\", s.primary_verified_at,
                        s.created_at, s.expires_at, s.consumed_at, s.superseded_at, s.revoked_at,
                        s.credential_revision::text AS credential_revision,
                        s.verification_ciphertext, s.provisioning_ciphertext,
                        s.provisioning_presented_at, a.eligibility,
                        a.security_epoch::text AS \"admin_epoch!\",
                        a.enrollment_purpose, a.enrollment_authorizer,
                        a.enrollment_authorized_at, a.enrollment_expires_at,
                        a.enrollment_epoch::text AS enrollment_epoch, a.enrollment_status
                 FROM admin_session_states s JOIN admins a ON a.id = s.admin_id
                 WHERE s.reference_digest = $1 FOR UPDATE OF s, a",
                digest,
            )
            .fetch_optional(connection)
            .await
        } else {
            sqlx::query_as!(
                PendingRow,
                "SELECT s.id, s.admin_id AS \"admin_id!\",
                        s.browser_digest, s.session_id, s.purpose, s.phase,
                        s.security_epoch::text AS \"epoch!\", s.primary_verified_at,
                        s.created_at, s.expires_at, s.consumed_at, s.superseded_at, s.revoked_at,
                        s.credential_revision::text AS credential_revision,
                        s.verification_ciphertext, s.provisioning_ciphertext,
                        s.provisioning_presented_at, a.eligibility,
                        a.security_epoch::text AS \"admin_epoch!\",
                        a.enrollment_purpose, a.enrollment_authorizer,
                        a.enrollment_authorized_at, a.enrollment_expires_at,
                        a.enrollment_epoch::text AS enrollment_epoch, a.enrollment_status
                 FROM admin_session_states s JOIN admins a ON a.id = s.admin_id
                 WHERE s.reference_digest = $1",
                digest,
            )
            .fetch_optional(connection)
            .await
        };
        row.map_err(|error| map_sql(&error))?.ok_or_else(denied)
    }
}

#[async_trait::async_trait]
impl SessionStore for PostgresSessionStore {
    async fn insert(&self, tx: &mut TxHandle, session: SessionRecord) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let state = session.state;
        let idle = state.idle_timeout().whole_seconds();
        if idle <= 0 || Duration::seconds(idle) != state.idle_timeout() {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        let inserted = sqlx::query!(
            "INSERT INTO admin_sessions (
                 id, token_digest, admin_id, security_epoch, primary_verified,
                 authenticated_factor, issued_at, expires_at, idle_timeout_seconds, last_activity_at
             ) SELECT $1, $2, $3, $4::text::numeric, $5, $6, $7, $8, $9, $10
             FROM admins WHERE id = $3 ON CONFLICT DO NOTHING",
            state.id().as_uuid(),
            session.digest.as_bytes().as_slice(),
            state.admin_id().as_uuid(),
            state.epoch().value().to_string(),
            state.factor().is_some(),
            state.factor().map(factor_name),
            state.issued_at(),
            state.expires_at(),
            idle,
            state.last_activity_at(),
        )
        .execute(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if inserted.rows_affected() == 0 {
            let exists = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM admins WHERE id = $1)",
                state.admin_id().as_uuid(),
            )
            .fetch_one(connection)
            .await
            .map_err(|error| map_sql(&error))?;
            return if exists.unwrap_or(false) {
                Err(DomainError::conflict(ErrorCode::IdempotencyConflict))
            } else {
                Err(DomainError::validation(ErrorCode::NotFound))
            };
        }
        Ok(())
    }

    async fn lookup(
        &self,
        tx: &mut TxHandle,
        digest: BindingDigest,
    ) -> Result<Option<SessionState>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        sqlx::query_as!(
            SessionRow,
            "SELECT id, admin_id, security_epoch::text AS \"epoch!\", primary_verified,
                    authenticated_factor, issued_at, expires_at, idle_timeout_seconds,
                    last_activity_at, revoked_at
             FROM admin_sessions WHERE token_digest = $1",
            digest.as_bytes().as_slice(),
        )
        .fetch_optional(connection)
        .await
        .map_err(|error| map_sql(&error))?
        .map(SessionRow::into_state)
        .transpose()
    }

    async fn check_current(
        &self,
        tx: &mut TxHandle,
        digest: BindingDigest,
        now: OffsetDateTime,
    ) -> Result<SessionState, DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let row = sqlx::query_as!(
            CurrentSessionRow,
            "SELECT s.id, s.admin_id, s.security_epoch::text AS \"epoch!\",
                    s.primary_verified, s.authenticated_factor, s.issued_at, s.expires_at,
                    s.idle_timeout_seconds, s.last_activity_at, s.revoked_at,
                    a.eligibility, a.security_epoch::text AS \"admin_epoch!\"
             FROM admin_sessions s JOIN admins a ON a.id = s.admin_id
             WHERE s.token_digest = $1 FOR UPDATE OF s, a",
            digest.as_bytes().as_slice(),
        )
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| map_sql(&error))?
        .ok_or_else(|| DomainError::unauthorised(ErrorCode::AuthenticationRequired))?;
        let admin = AdminState::new(
            admin_id(row.admin_id)?,
            eligibility(&row.eligibility)?,
            parse_epoch(&row.admin_epoch)?,
        );
        let mut state = row.session().into_state()?;
        state.record_activity(&admin, now)?;
        sqlx::query!(
            "UPDATE admin_sessions SET last_activity_at = $2 WHERE id = $1",
            state.id().as_uuid(),
            now,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(state)
    }

    async fn revoke_one(
        &self,
        tx: &mut TxHandle,
        id: SessionId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        sqlx::query!(
            "UPDATE admin_sessions SET revoked_at = COALESCE(revoked_at, $2) WHERE id = $1",
            id.as_uuid(),
            now,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }

    async fn revoke_all(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        sqlx::query!(
            "UPDATE admin_sessions SET revoked_at = COALESCE(revoked_at, $2) WHERE admin_id = $1",
            admin.as_uuid(),
            now,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }

    async fn create_pending(
        &self,
        tx: &mut TxHandle,
        pending: PendingRecord,
    ) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let state = pending.state();
        let binding = state.binding();
        let admin = current_admin(&mut *connection, binding.target).await?;
        state.check(binding, &admin, state.created_at())?;
        let verification = pending.verification_envelope().map(SecretBytes::expose);
        let provisioning = pending.provisioning_envelope().map(SecretBytes::expose);
        let (purpose, phase) = purpose_phase(binding);
        let inserted = sqlx::query!(
            "INSERT INTO admin_session_states (
                 id, reference_digest, admin_id, browser_digest, session_id, purpose, phase,
                 security_epoch, primary_verified_at, created_at, expires_at, credential_revision,
                 verification_key_id, verification_nonce, verification_ciphertext,
                 provisioning_key_id, provisioning_nonce, provisioning_ciphertext
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8::text::numeric, $9, $10, $11,
                       $12::text::numeric,
                       CASE WHEN $13::bytea IS NULL THEN NULL ELSE 'opaque-v1' END,
                       CASE WHEN $13::bytea IS NULL THEN NULL ELSE '\\x00'::bytea END, $13,
                       CASE WHEN $14::bytea IS NULL THEN NULL ELSE 'opaque-v1' END,
                       CASE WHEN $14::bytea IS NULL THEN NULL ELSE '\\x00'::bytea END, $14)
             ON CONFLICT DO NOTHING",
            state.id().as_uuid(),
            pending.reference().as_bytes().as_slice(),
            binding.target.as_uuid(),
            binding.browser.as_bytes().as_slice(),
            binding.session.as_uuid(),
            purpose,
            phase,
            binding.epoch.value().to_string(),
            state.primary_verified_at(),
            state.created_at(),
            state.expires_at(),
            binding.credential_revision.map(|value| value.to_string()),
            verification,
            provisioning,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if inserted.rows_affected() == 0 {
            return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
        }
        Ok(())
    }

    async fn read_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<PendingView, DomainError> {
        let row = self.pending_row(tx, &check.reference, false).await?;
        let (state, admin) = row.state_and_admin()?;
        state.check(&check.binding, &admin, check.now)?;
        Ok(PendingView {
            reference: check.reference,
            state,
            provisioning_presented: row.provisioning_presented_at.is_some(),
        })
    }

    async fn verification_state(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<SecretBytes, DomainError> {
        let lock = tx.mode() == TransactionMode::ReadWrite;
        let row = self.pending_row(tx, &check.reference, lock).await?;
        let (state, admin) = row.state_and_admin()?;
        state.check(&check.binding, &admin, check.now)?;
        row.verification_ciphertext
            .map(SecretBytes::new)
            .ok_or_else(denied)
    }

    async fn consume_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<(), DomainError> {
        transition_pending(self, tx, &check, "consumed").await
    }

    async fn supersede_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<(), DomainError> {
        transition_pending(self, tx, &check, "superseded").await
    }

    async fn revoke_pending(&self, tx: &mut TxHandle, admin: AdminId) -> Result<(), DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        sqlx::query!(
            "UPDATE admin_session_states
             SET revoked_at = created_at, provisioning_key_id = NULL,
                 provisioning_nonce = NULL, provisioning_ciphertext = NULL
             WHERE admin_id = $1 AND consumed_at IS NULL AND superseded_at IS NULL
               AND revoked_at IS NULL",
            admin.as_uuid(),
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        Ok(())
    }

    async fn present_provisioning(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<SecretBytes, DomainError> {
        self.uow.writable_connection(tx)?;
        let row = self.pending_row(tx, &check.reference, true).await?;
        let connection = self.uow.writable_connection(tx)?;
        let (state, admin) = row.state_and_admin()?;
        state.check(&check.binding, &admin, check.now)?;
        if row.provisioning_presented_at.is_some() {
            return Err(denied());
        }
        let envelope = row.provisioning_ciphertext.ok_or_else(denied)?;
        let changed = sqlx::query!(
            "UPDATE admin_session_states
             SET provisioning_presented_at = $2, provisioning_key_id = NULL,
                 provisioning_nonce = NULL, provisioning_ciphertext = NULL
             WHERE id = $1 AND provisioning_presented_at IS NULL
               AND consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
            row.id,
            check.now,
        )
        .execute(connection)
        .await
        .map_err(|error| map_sql(&error))?;
        if changed.rows_affected() != 1 {
            return Err(denied());
        }
        Ok(SecretBytes::new(envelope))
    }
}

async fn transition_pending(
    store: &PostgresSessionStore,
    tx: &mut TxHandle,
    check: &PendingCheck,
    transition: &str,
) -> Result<(), DomainError> {
    store.uow.writable_connection(tx)?;
    let row = store.pending_row(tx, &check.reference, true).await?;
    let connection = store.uow.writable_connection(tx)?;
    let (state, admin) = row.state_and_admin()?;
    state.check(&check.binding, &admin, check.now)?;
    let changed = match transition {
        "consumed" => {
            sqlx::query!(
                "UPDATE admin_session_states SET consumed_at = $2,
                     provisioning_key_id = NULL, provisioning_nonce = NULL,
                     provisioning_ciphertext = NULL
                 WHERE id = $1 AND consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
                row.id,
                check.now,
            )
            .execute(connection)
            .await
        }
        "superseded" => {
            sqlx::query!(
                "UPDATE admin_session_states SET superseded_at = $2,
                     provisioning_key_id = NULL, provisioning_nonce = NULL,
                     provisioning_ciphertext = NULL
                 WHERE id = $1 AND consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
                row.id,
                check.now,
            )
            .execute(connection)
            .await
        }
        _ => return Err(DomainError::validation(ErrorCode::InvalidValue)),
    }
    .map_err(|error| map_sql(&error))?;
    if changed.rows_affected() == 1 {
        Ok(())
    } else {
        Err(denied())
    }
}

#[derive(Debug)]
struct SessionRow {
    id: Uuid,
    admin_id: Uuid,
    epoch: String,
    primary_verified: bool,
    authenticated_factor: Option<String>,
    issued_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    idle_timeout_seconds: i64,
    last_activity_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
}

impl SessionRow {
    fn into_state(self) -> Result<SessionState, DomainError> {
        let mut state = SessionState::try_from(SessionInput {
            id: session_id(self.id)?,
            admin_id: admin_id(self.admin_id)?,
            epoch: parse_epoch(&self.epoch)?,
            primary_verified: self.primary_verified,
            factor: self
                .authenticated_factor
                .as_deref()
                .map(factor)
                .transpose()?,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            idle_timeout: Duration::seconds(self.idle_timeout_seconds),
        })?;
        if self.last_activity_at != self.issued_at {
            let admin = AdminState::new(state.admin_id(), Eligibility::Active, state.epoch());
            state.record_activity(&admin, self.last_activity_at)?;
        }
        if let Some(at) = self.revoked_at {
            state.revoke(at);
        }
        Ok(state)
    }
}

#[derive(Debug)]
struct CurrentSessionRow {
    id: Uuid,
    admin_id: Uuid,
    epoch: String,
    primary_verified: bool,
    authenticated_factor: Option<String>,
    issued_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    idle_timeout_seconds: i64,
    last_activity_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
    eligibility: String,
    admin_epoch: String,
}

impl CurrentSessionRow {
    fn session(self) -> SessionRow {
        SessionRow {
            id: self.id,
            admin_id: self.admin_id,
            epoch: self.epoch,
            primary_verified: self.primary_verified,
            authenticated_factor: self.authenticated_factor,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            idle_timeout_seconds: self.idle_timeout_seconds,
            last_activity_at: self.last_activity_at,
            revoked_at: self.revoked_at,
        }
    }
}

#[derive(Debug)]
struct PendingRow {
    id: Uuid,
    admin_id: Uuid,
    browser_digest: Vec<u8>,
    session_id: Uuid,
    purpose: String,
    phase: String,
    epoch: String,
    primary_verified_at: OffsetDateTime,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    consumed_at: Option<OffsetDateTime>,
    superseded_at: Option<OffsetDateTime>,
    revoked_at: Option<OffsetDateTime>,
    credential_revision: Option<String>,
    verification_ciphertext: Option<Vec<u8>>,
    provisioning_ciphertext: Option<Vec<u8>>,
    provisioning_presented_at: Option<OffsetDateTime>,
    eligibility: String,
    admin_epoch: String,
    enrollment_purpose: Option<String>,
    enrollment_authorizer: Option<Uuid>,
    enrollment_authorized_at: Option<OffsetDateTime>,
    enrollment_expires_at: Option<OffsetDateTime>,
    enrollment_epoch: Option<String>,
    enrollment_status: Option<String>,
}

impl PendingRow {
    fn state_and_admin(&self) -> Result<(AuthState, AdminState), DomainError> {
        if self.consumed_at.is_some() || self.superseded_at.is_some() || self.revoked_at.is_some() {
            return Err(denied());
        }
        let purpose = match self.purpose.as_str() {
            "sign_in" => AuthPurpose::SignIn,
            "addition" => AuthPurpose::Enrollment(EnrollmentPurpose::Addition),
            "recovery" => AuthPurpose::Enrollment(EnrollmentPurpose::Recovery),
            _ => return Err(DomainError::validation(ErrorCode::InvalidValue)),
        };
        let auth_stage = match self.phase.as_str() {
            "primary_verified" => AuthStage::PrimaryVerified,
            "passkey" => AuthStage::Passkey,
            "totp" => AuthStage::Totp,
            _ => return Err(DomainError::validation(ErrorCode::InvalidValue)),
        };
        let binding = AuthBinding {
            target: admin_id(self.admin_id)?,
            browser: digest(&self.browser_digest)?,
            session: session_id(self.session_id)?,
            purpose,
            stage: auth_stage,
            epoch: parse_epoch(&self.epoch)?,
            credential_revision: self
                .credential_revision
                .as_deref()
                .map(parse_u64)
                .transpose()?,
        };
        let state = AuthState::try_from(AuthInput {
            id: challenge_id(self.id)?,
            binding,
            primary_verified_at: Some(self.primary_verified_at),
            created_at: self.created_at,
            expires_at: self.expires_at,
        })?;
        let mut admin = AdminState::new(
            admin_id(self.admin_id)?,
            eligibility(&self.eligibility)?,
            parse_epoch(&self.admin_epoch)?,
        );
        if self.enrollment_status.as_deref() == Some("pending") {
            let authorizer = AdminState::new(
                admin_id(self.enrollment_authorizer.ok_or_else(denied)?)?,
                Eligibility::Active,
                SecurityEpoch::new(0),
            );
            admin.authorize_enrollment(
                &authorizer,
                enrollment_purpose(self.enrollment_purpose.as_deref())?,
                parse_epoch(self.enrollment_epoch.as_deref().ok_or_else(denied)?)?,
                self.enrollment_authorized_at.ok_or_else(denied)?,
                self.enrollment_expires_at.ok_or_else(denied)?,
            )?;
        }
        Ok((state, admin))
    }
}

async fn current_admin(
    connection: &mut sqlx::PgConnection,
    id: AdminId,
) -> Result<AdminState, DomainError> {
    let row = sqlx::query_as!(
        CurrentAdminRow,
        "SELECT id, eligibility, security_epoch::text AS \"epoch!\",
                enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
                enrollment_expires_at, enrollment_epoch::text AS enrollment_epoch,
                enrollment_status
         FROM admins WHERE id = $1",
        id.as_uuid(),
    )
    .fetch_optional(connection)
    .await
    .map_err(|error| map_sql(&error))?
    .ok_or_else(|| DomainError::validation(ErrorCode::NotFound))?;
    let mut admin = AdminState::new(
        admin_id(row.id)?,
        eligibility(&row.eligibility)?,
        parse_epoch(&row.epoch)?,
    );
    if row.enrollment_status.as_deref() == Some("pending") {
        let authorizer = AdminState::new(
            admin_id(row.enrollment_authorizer.ok_or_else(denied)?)?,
            Eligibility::Active,
            SecurityEpoch::new(0),
        );
        admin.authorize_enrollment(
            &authorizer,
            enrollment_purpose(row.enrollment_purpose.as_deref())?,
            parse_epoch(row.enrollment_epoch.as_deref().ok_or_else(denied)?)?,
            row.enrollment_authorized_at.ok_or_else(denied)?,
            row.enrollment_expires_at.ok_or_else(denied)?,
        )?;
    }
    Ok(admin)
}

#[derive(Debug)]
struct CurrentAdminRow {
    id: Uuid,
    eligibility: String,
    epoch: String,
    enrollment_purpose: Option<String>,
    enrollment_authorizer: Option<Uuid>,
    enrollment_authorized_at: Option<OffsetDateTime>,
    enrollment_expires_at: Option<OffsetDateTime>,
    enrollment_epoch: Option<String>,
    enrollment_status: Option<String>,
}

fn parse_epoch(value: &str) -> Result<SecurityEpoch, DomainError> {
    parse_u64(value).map(SecurityEpoch::new)
}

fn parse_u64(value: &str) -> Result<u64, DomainError> {
    value
        .parse()
        .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))
}

fn digest(value: &[u8]) -> Result<BindingDigest, DomainError> {
    let bytes: [u8; 32] = value
        .try_into()
        .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
    Ok(BindingDigest::new(bytes))
}

fn eligibility(value: &str) -> Result<Eligibility, DomainError> {
    match value {
        "invited" => Ok(Eligibility::Invited),
        "active" => Ok(Eligibility::Active),
        "recovering" => Ok(Eligibility::Recovering),
        "deactivated" => Ok(Eligibility::Deactivated),
        "revoked" => Ok(Eligibility::Revoked),
        _ => Err(DomainError::validation(ErrorCode::InvalidValue)),
    }
}

fn enrollment_purpose(value: Option<&str>) -> Result<EnrollmentPurpose, DomainError> {
    match value {
        Some("addition") => Ok(EnrollmentPurpose::Addition),
        Some("recovery") => Ok(EnrollmentPurpose::Recovery),
        _ => Err(DomainError::validation(ErrorCode::InvalidValue)),
    }
}

fn purpose_phase(binding: &AuthBinding) -> (&'static str, &'static str) {
    let purpose = match binding.purpose {
        AuthPurpose::SignIn => "sign_in",
        AuthPurpose::Enrollment(EnrollmentPurpose::Addition) => "addition",
        AuthPurpose::Enrollment(EnrollmentPurpose::Recovery) => "recovery",
    };
    let phase = match binding.stage {
        AuthStage::PrimaryVerified => "primary_verified",
        AuthStage::Passkey => "passkey",
        AuthStage::Totp => "totp",
    };
    (purpose, phase)
}

fn factor_name(value: Factor) -> &'static str {
    match value {
        Factor::Passkey => "passkey",
        Factor::Totp => "totp",
    }
}

fn factor(value: &str) -> Result<Factor, DomainError> {
    match value {
        "passkey" => Ok(Factor::Passkey),
        "totp" => Ok(Factor::Totp),
        _ => Err(DomainError::validation(ErrorCode::InvalidValue)),
    }
}

fn admin_id(value: Uuid) -> Result<AdminId, DomainError> {
    AdminId::try_new(value).map_err(|_| unavailable())
}

fn session_id(value: Uuid) -> Result<SessionId, DomainError> {
    SessionId::try_new(value).map_err(|_| unavailable())
}

fn challenge_id(value: Uuid) -> Result<kernel::ids::ChallengeId, DomainError> {
    kernel::ids::ChallengeId::try_new(value).map_err(|_| unavailable())
}

fn denied() -> DomainError {
    DomainError::forbidden(ErrorCode::PermissionDenied)
}
