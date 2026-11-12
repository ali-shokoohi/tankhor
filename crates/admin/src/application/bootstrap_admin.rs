//! FR-004/024: one-time owner-authorized initial-admin provisioning.
use crate::{
    domain::{
        admin_state::{AdminState, Eligibility, EnrollmentPurpose, SecurityEpoch},
        audit::{Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, ReasonCode, Subject},
        secrets::SecretBytes,
        security_policy::SecurityPolicy,
    },
    ports::{
        admin_authenticator::AdminAuthenticator,
        admin_repository::{AdminListQuery, AdminRepository},
        audit_repository::AuditRepository,
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

/// Trusted context constructed only by the protected operator provisioning command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorContext {
    id: AdminId,
    owner_authorized: bool,
}

impl OperatorContext {
    #[must_use]
    pub const fn new(id: AdminId, owner_authorized: bool) -> Self {
        Self {
            id,
            owner_authorized,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BootstrapAdminInput {
    pub operator: OperatorContext,
    pub phone: SecretBytes,
}

pub struct BootstrapAdmin<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub audit: &'a dyn AuditRepository,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}

impl BootstrapAdmin<'_> {
    /// # Errors
    /// Rejects unauthorized, repeated or self-recovery provisioning. Eligibility,
    /// protected enrollment authorization and audit commit atomically; no factor or session is issued.
    pub async fn execute(&self, input: BootstrapAdminInput) -> Result<AdminId, DomainError> {
        if !input.operator.owner_authorized {
            return Err(denied());
        }
        let identity = self
            .authenticator
            .identity_digest(super::request_admin_otp::normalize_phone(&input.phone)?)
            .await?;
        let target_id = self.ids.next_admin_id().await?;
        let audit_id = self.ids.next_audit_id().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            let existing = self
                .admins
                .list(&mut tx, AdminListQuery::new(None, 1)?)
                .await?;
            if !existing.entries.is_empty() || input.operator.id == target_id {
                return Err(denied());
            }
            let now = self.clock.now().await?;
            let expires_at = now
                .checked_add(
                    time::Duration::try_from(self.policy.enrollment_ttl()).map_err(|_| denied())?,
                )
                .ok_or_else(denied)?;
            let operator = AdminState::new(
                input.operator.id,
                Eligibility::Active,
                SecurityEpoch::new(0),
            );
            let mut target =
                AdminState::new(target_id, Eligibility::Invited, SecurityEpoch::new(0));
            target.authorize_enrollment(
                &operator,
                EnrollmentPurpose::Addition,
                SecurityEpoch::new(0),
                now,
                expires_at,
            )?;
            self.admins.invite(&mut tx, target, identity).await?;
            self.audit
                .append(
                    &mut tx,
                    AuditEntry::try_from(AuditInput {
                        id: audit_id,
                        event_key: audit_id,
                        actor: Some(Actor::Admin(input.operator.id)),
                        action: Some(AuditAction::Invited),
                        subject: Some(Subject::Admin(target_id)),
                        occurred_at: Some(now),
                        reason: Some(ReasonCode::AdminAddition),
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
        Ok(target_id)
    }
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
