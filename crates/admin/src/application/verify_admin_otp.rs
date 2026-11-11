//! FR-002, FR-CON-02, AC-ADM-002: primary verification is not console access.
use super::request_admin_otp::normalize_phone;
use crate::{
    domain::{
        admin_state::AdminState,
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        otp_state::OtpState,
        secrets::SecretBytes,
        security_policy::SecurityPolicy,
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        admin_repository::AdminRepository,
        otp_challenge_repository::{OtpChallengeRepository, OtpCheck},
        rate_limit_repository::{
            RateLimitKey, RateLimitPurpose, RateLimitRepository, RateLimitScope,
        },
        session_store::{PendingRecord, SessionStore},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::ChallengeId,
    ports::{
        clock::Clock,
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct VerifyOtp {
    pub phone: SecretBytes,
    pub challenge: ChallengeId,
    pub code: SecretBytes,
    pub browser: BindingDigest,
    pub origin: BindingDigest,
    pub purpose: AuthPurpose,
}

/// Opaque capability for a short-lived, browser-bound second-factor flow only.
#[derive(Debug)]
pub struct PrimaryProof {
    pub token: SecretBytes,
    pub binding: AuthBinding,
}

pub struct VerifyAdminOtp<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub rates: &'a dyn RateLimitRepository,
    pub otps: &'a dyn OtpChallengeRepository,
    pub sessions: &'a dyn SessionStore,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}

impl VerifyAdminOtp<'_> {
    /// # Errors
    /// Refuses invalid/expired/replayed proofs, stale authority, throttling and dependency failures.
    pub async fn execute(&self, request: VerifyOtp) -> Result<PrimaryProof, DomainError> {
        let (admin, otp) = self.snapshot(&request).await?;
        let valid = self
            .authenticator
            .verify_otp(otp.id(), request.code, otp.digest().clone())
            .await?;
        let now = self.clock.now().await?;
        let proof = if valid {
            let token = self
                .authenticator
                .issue_token(TokenPurpose::PrimaryProof)
                .await?;
            let reference = self
                .authenticator
                .token_digest(TokenPurpose::PrimaryProof, token.clone())
                .await?;
            let binding = AuthBinding {
                target: admin.id(),
                browser: request.browser,
                session: self.ids.next_session_id().await?,
                purpose: request.purpose,
                stage: AuthStage::PrimaryVerified,
                epoch: admin.epoch(),
                credential_revision: None,
            };
            let state = AuthState::try_from(AuthInput {
                id: self.ids.next_challenge_id().await?,
                binding: binding.clone(),
                primary_verified_at: Some(now),
                created_at: now,
                expires_at: now
                    .checked_add(
                        self.policy
                            .primary_proof_ttl()
                            .try_into()
                            .map_err(|_| denied())?,
                    )
                    .ok_or_else(denied)?,
            })?;
            Some((
                PrimaryProof { token, binding },
                PendingRecord::new(reference, state, None, None)?,
            ))
        } else {
            None
        };
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let update = async {
            let current = self.admins.lock(&mut tx, vec![admin.id()]).await?;
            if current.first() != Some(&admin) {
                return Err(denied());
            }
            let current = self
                .otps
                .lock(&mut tx, otp.id())
                .await?
                .ok_or_else(denied)?;
            if current != otp {
                return Err(denied());
            }
            let now = self.clock.now().await?;
            authorize(&admin, request.purpose, now)?;
            let check = OtpCheck {
                id: otp.id(),
                binding: otp.binding().clone(),
                expected_failed_attempts: otp.failed_attempts(),
                now,
            };
            if let Some((_, pending)) = &proof {
                pending
                    .state()
                    .check(pending.state().binding(), &admin, now)?;
                self.otps
                    .consume_once(&mut tx, check, otp.digest().clone())
                    .await?;
                self.sessions.create_pending(&mut tx, pending.clone()).await
            } else {
                self.otps.record_failure(&mut tx, check).await
            }
        }
        .await;
        if let Err(error) = update {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;
        proof.map(|(proof, _)| proof).ok_or_else(denied)
    }

    async fn snapshot(&self, request: &VerifyOtp) -> Result<(AdminState, OtpState), DomainError> {
        let identity = self
            .authenticator
            .identity_digest(normalize_phone(&request.phone)?)
            .await?;
        let now = self.clock.now().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let read = async {
            let subject = self
                .rates
                .increment(
                    &mut tx,
                    RateLimitKey::new(
                        RateLimitScope::Subject,
                        RateLimitPurpose::Verify,
                        *identity.as_bytes(),
                    ),
                    self.policy.subject_rate_limit(),
                    now,
                )
                .await?;
            let origin = self
                .rates
                .increment(
                    &mut tx,
                    RateLimitKey::new(
                        RateLimitScope::Origin,
                        RateLimitPurpose::Verify,
                        *request.origin.as_bytes(),
                    ),
                    self.policy.origin_rate_limit(),
                    now,
                )
                .await?;
            if !subject.allowed || !origin.allowed {
                return Ok(None);
            }
            let Some(admin) = self.admins.find_by_identity(&mut tx, identity).await? else {
                return Ok(None);
            };
            let Some(otp) = self.otps.lock(&mut tx, request.challenge).await? else {
                return Ok(None);
            };
            if otp.binding().target != admin.id()
                || otp.binding().browser != request.browser
                || otp.binding().purpose != request.purpose
                || otp.binding().epoch != admin.epoch()
                || authorize(&admin, request.purpose, now).is_err()
            {
                return Ok(None);
            }
            // Validate live status/expiry/budget on a copy without mutating persisted attempts.
            if otp
                .clone()
                .record_failure(otp.binding(), otp.failed_attempts(), now)
                .is_err()
            {
                return Ok(None);
            }
            Ok(Some((admin, otp)))
        }
        .await;
        match read {
            Ok(snapshot) => {
                self.uow.commit(tx).await?;
                snapshot.ok_or_else(denied)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}

fn authorize(
    admin: &AdminState,
    purpose: AuthPurpose,
    now: OffsetDateTime,
) -> Result<(), DomainError> {
    match purpose {
        AuthPurpose::SignIn if admin.is_eligible() => Ok(()),
        AuthPurpose::Enrollment(purpose) => {
            admin.check_enrollment(admin.id(), purpose, admin.epoch(), now)
        }
        AuthPurpose::SignIn => Err(denied()),
    }
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
