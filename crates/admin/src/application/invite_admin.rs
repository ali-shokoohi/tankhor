//! Another current administrator creates identity-bound authorization, never a bearer grant.
use crate::{
    domain::{
        admin_state::{AdminState, Eligibility, EnrollmentPurpose, SecurityEpoch},
        audit::{Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, ReasonCode, Subject},
        auth_state::BindingDigest,
        secrets::SecretBytes,
        security_policy::SecurityPolicy,
    },
    ports::{
        admin_authenticator::AdminAuthenticator,
        admin_repository::{AdminRepository, AuthorizeEnrollment},
        audit_repository::AuditRepository,
        session_store::SessionStore,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::AdminId,
    ports::{
        clock::Clock,
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};
#[derive(Debug, Clone)]
pub struct InviteAdminInput {
    pub actor: BindingDigest,
    pub phone: SecretBytes,
    pub reason: Option<ReasonCode>,
}
pub struct InviteAdmin<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub audit: &'a dyn AuditRepository,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}
impl InviteAdmin<'_> {
    /// # Errors
    /// Rejects incomplete/currently invalid actors, duplicate identities and missing managed reason.
    /// All creation/authorization/audit effects roll back together; unknown commit is not success.
    pub async fn execute(&self, input: InviteAdminInput) -> Result<AdminId, DomainError> {
        if input.reason != Some(ReasonCode::AdminAddition) {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        let phone = super::request_admin_otp::normalize_phone(&input.phone)?;
        let identity = self.authenticator.identity_digest(phone).await?;
        let target = self.ids.next_admin_id().await?;
        let event = self.ids.next_audit_id().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            let session = self
                .sessions
                .check_current(&mut tx, input.actor, self.clock.now().await?)
                .await?;
            if self
                .admins
                .find_by_identity(&mut tx, identity.clone())
                .await?
                .is_some()
            {
                return Err(DomainError::conflict(ErrorCode::IdempotencyConflict));
            }
            let actors = self.admins.lock(&mut tx, vec![session.admin_id()]).await?;
            let actor = actors.first().ok_or_else(denied)?;
            let now = self.clock.now().await?;
            session.check_access(actor, now)?;
            if actor.id() == target {
                return Err(denied());
            }
            let state = AdminState::new(target, Eligibility::Invited, SecurityEpoch::new(0));
            let expiry = now
                .checked_add(
                    time::Duration::try_from(self.policy.enrollment_ttl()).map_err(|_| denied())?,
                )
                .ok_or_else(denied)?;
            self.admins.invite(&mut tx, state, identity).await?;
            self.admins
                .authorize_enrollment(
                    &mut tx,
                    AuthorizeEnrollment {
                        actor: actor.id(),
                        target,
                        purpose: EnrollmentPurpose::Addition,
                        expected_epoch: SecurityEpoch::new(0),
                        expected_authorization: None,
                        authorized_at: now,
                        expires_at: expiry,
                    },
                )
                .await?;
            self.audit
                .append(
                    &mut tx,
                    AuditEntry::try_from(AuditInput {
                        id: event,
                        event_key: event,
                        actor: Some(Actor::Admin(actor.id())),
                        action: Some(AuditAction::Invited),
                        subject: Some(Subject::Admin(target)),
                        occurred_at: Some(now),
                        reason: input.reason,
                        metadata: AuditMetadata::Invitation {
                            before: None,
                            after: Eligibility::Invited,
                        },
                    })?,
                )
                .await
        }
        .await;
        if let Err(error) = result {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;
        Ok(target)
    }
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
