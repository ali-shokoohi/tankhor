//! Another current administrator atomically replaces the target's authorization and invalidates old access.
use crate::{
    domain::{
        admin_state::{EnrollmentPurpose, Invalidation},
        audit::{Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, ReasonCode, Subject},
        auth_state::BindingDigest,
        security_policy::SecurityPolicy,
    },
    ports::{
        admin_credential_repository::AdminCredentialRepository,
        admin_repository::{AdminRepository, AuthorizeEnrollment},
        audit_repository::AuditRepository,
        otp_challenge_repository::OtpChallengeRepository,
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
pub struct RecoverAdminInput {
    pub actor: BindingDigest,
    pub target: AdminId,
    pub reason: Option<ReasonCode>,
}

pub struct RecoverAdmin<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub credentials: &'a dyn AdminCredentialRepository,
    pub otp: &'a dyn OtpChallengeRepository,
    pub audit: &'a dyn AuditRepository,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}

impl RecoverAdmin<'_> {
    /// # Errors
    /// Refuses self-recovery, invalid current actors, missing targets, epoch exhaustion,
    /// and any failed transactional write. Unknown commit outcome is never success.
    /// No session, transferable token or factor is returned or created.
    pub async fn execute(&self, input: RecoverAdminInput) -> Result<(), DomainError> {
        if input.reason != Some(ReasonCode::AdminRecovery) {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        let event = self.ids.next_audit_id().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            let session = self
                .sessions
                .check_current(&mut tx, input.actor, self.clock.now().await?)
                .await?;
            if session.admin_id() == input.target {
                return Err(denied());
            }
            let mut ids = vec![session.admin_id(), input.target];
            ids.sort_unstable();
            let locked = self.admins.lock(&mut tx, ids).await?;
            let actor = locked
                .iter()
                .find(|a| a.id() == session.admin_id())
                .ok_or_else(denied)?;
            let target = locked
                .iter()
                .find(|a| a.id() == input.target)
                .ok_or_else(denied)?;
            let now = self.clock.now().await?;
            session.check_access(actor, now)?;
            let next = target.epoch().advance()?;
            let expiry = now
                .checked_add(
                    time::Duration::try_from(self.policy.enrollment_ttl()).map_err(|_| denied())?,
                )
                .ok_or_else(denied)?;
            // The same epoch transition revokes the previous protected authorization.
            let mut invalidated = target.clone();
            invalidated.invalidate(target.epoch(), next, Invalidation::Recover)?;
            self.admins
                .invalidate_epoch(
                    &mut tx,
                    target.id(),
                    target.epoch(),
                    next,
                    Invalidation::Recover,
                )
                .await?;
            self.sessions.revoke_all(&mut tx, target.id(), now).await?;
            self.credentials
                .revoke_all(&mut tx, target.id(), now)
                .await?;
            self.sessions.revoke_pending(&mut tx, target.id()).await?;
            self.otp.revoke_all(&mut tx, target.id()).await?;
            self.admins
                .authorize_enrollment(
                    &mut tx,
                    AuthorizeEnrollment {
                        actor: actor.id(),
                        target: target.id(),
                        purpose: EnrollmentPurpose::Recovery,
                        expected_epoch: next,
                        expected_authorization: invalidated.enrollment().cloned(),
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
                        action: Some(AuditAction::Recovered),
                        subject: Some(Subject::Admin(target.id())),
                        occurred_at: Some(now),
                        reason: input.reason,
                        metadata: AuditMetadata::Eligibility {
                            before: target.is_eligible(),
                            after: false,
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
        self.uow.commit(tx).await
    }
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
