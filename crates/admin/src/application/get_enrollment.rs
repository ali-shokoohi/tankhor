//! Enrollment choices require current protected authorization and primary proof.
use crate::{
    application::verify_admin_otp::PrimaryProof,
    domain::{
        auth_state::{AuthPurpose, AuthStage},
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        admin_repository::AdminRepository,
        session_store::{PendingCheck, SessionStore},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::AdminId,
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentFactor {
    Passkey,
    Totp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentView {
    pub target: AdminId,
    pub factors: Vec<EnrollmentFactor>,
}

pub struct GetEnrollmentInput {
    pub proof: PrimaryProof,
    pub browser: SecretBytes,
}

pub struct GetEnrollment<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
}

impl GetEnrollment<'_> {
    /// # Errors
    /// Refuses a wrong purpose/stage/browser, stale authority, invalid primary proof,
    /// missing enrollment authorization, and expired or consumed ceremony state.
    pub async fn execute(&self, input: GetEnrollmentInput) -> Result<EnrollmentView, DomainError> {
        let binding = input.proof.binding;
        if binding.stage != AuthStage::PrimaryVerified
            || !matches!(binding.purpose, AuthPurpose::Enrollment(_))
        {
            return Err(denied());
        }
        let reference = self
            .authenticator
            .token_digest(TokenPurpose::PrimaryProof, input.proof.token)
            .await?;
        let browser = self
            .authenticator
            .token_digest(TokenPurpose::Browser, input.browser)
            .await?;
        if browser != binding.browser {
            return Err(denied());
        }

        let now = self.clock.now().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let target = self
                .admins
                .get(&mut tx, binding.target)
                .await?
                .ok_or_else(denied)?;
            let authorization = self
                .admins
                .enrollment(&mut tx, binding.target)
                .await?
                .ok_or_else(denied)?;
            if target.enrollment() != Some(&authorization) {
                return Err(denied());
            }
            let AuthPurpose::Enrollment(purpose) = binding.purpose else {
                return Err(denied());
            };
            target.check_enrollment(binding.target, purpose, binding.epoch, now)?;
            self.sessions
                .read_pending(
                    &mut tx,
                    PendingCheck {
                        reference,
                        binding: binding.clone(),
                        now,
                    },
                )
                .await?;
            Ok(EnrollmentView {
                target: binding.target,
                factors: vec![EnrollmentFactor::Passkey, EnrollmentFactor::Totp],
            })
        }
        .await;

        match result {
            Ok(view) => {
                self.uow.commit(tx).await?;
                Ok(view)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
