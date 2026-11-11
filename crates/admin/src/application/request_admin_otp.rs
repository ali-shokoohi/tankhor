//! FR-002, FR-CON-02, AC-ADM-002, Q09: bounded admin-only OTP transport orchestration.

use crate::{
    domain::{
        admin_state::AdminState,
        auth_state::{AuthPurpose, BindingDigest},
        otp_state::{OtpBinding, OtpInput, OtpState},
        secrets::SecretBytes,
        security_policy::SecurityPolicy,
    },
    ports::{
        admin_authenticator::AdminAuthenticator,
        admin_repository::{AdminRepository, IdentityDigest},
        otp_challenge_repository::{OtpChallengeRepository, SubmissionCategory},
        rate_limit_repository::{
            RateLimitKey, RateLimitPurpose, RateLimitRepository, RateLimitScope,
        },
        sms_sender::{OtpSms, SmsSender, SmsTemplate},
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
pub struct RequestOtp {
    pub phone: SecretBytes,
    pub browser: BindingDigest,
    /// Keyed network-origin digest from the trusted request context, never raw IP.
    pub origin: BindingDigest,
    pub purpose: AuthPurpose,
    pub predecessor: Option<ChallengeId>,
}

pub struct RequestAdminOtp<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub rates: &'a dyn RateLimitRepository,
    pub otps: &'a dyn OtpChallengeRepository,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub primary: &'a dyn SmsSender,
    pub secondary: &'a dyn SmsSender,
    pub policy: &'a SecurityPolicy,
}

impl RequestAdminOtp<'_> {
    /// Returns the same empty acknowledgement for eligible, absent and throttled identities.
    /// No proof, credential or session is issued by requesting a code.
    /// # Errors
    /// Rejects malformed input, stale replacement state and sanitized dependency failures.
    pub async fn execute(&self, request: RequestOtp) -> Result<(), DomainError> {
        let phone = normalize_phone(&request.phone)?;
        let identity = self.authenticator.identity_digest(phone.clone()).await?;
        let now = self.clock.now().await?;
        let Some(admin) = self.reserve(&request, identity, now).await? else {
            return Ok(());
        };
        let id = self.ids.next_challenge_id().await?;
        let otp = self.authenticator.issue_otp(id).await?;
        let expires_at = now
            .checked_add(self.policy.otp_ttl().try_into().map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let binding = OtpBinding {
            target: admin.id(),
            browser: request.browser.clone(),
            purpose: request.purpose,
            epoch: admin.epoch(),
        };
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let persist = async {
            let current = self.admins.lock(&mut tx, vec![admin.id()]).await?;
            if current.first() != Some(&admin) || !eligible(&admin, request.purpose, now) {
                return Err(denied());
            }
            let failures = if let Some(previous) = request.predecessor {
                let old = self
                    .otps
                    .lock(&mut tx, previous)
                    .await?
                    .ok_or_else(denied)?;
                if old.binding() != &binding {
                    return Err(denied());
                }
                old.failed_attempts()
            } else {
                0
            };
            let state = OtpState::try_from(OtpInput {
                id,
                binding,
                digest: otp.digest,
                issued_at: now,
                expires_at,
                max_attempts: self
                    .policy
                    .otp_max_attempts()
                    .get()
                    .try_into()
                    .map_err(|_| invalid())?,
                failed_attempts: failures,
            })?;
            self.otps.issue(&mut tx, state, request.predecessor).await
        }
        .await;
        if let Err(error) = persist {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;

        let sms = OtpSms {
            challenge: id,
            recipient: phone,
            code: otp.code,
            template: SmsTemplate::AdminOtp,
            expires_at,
        };
        let primary = self.primary.send_otp(sms.clone()).await;
        let category = if matches!(
            primary,
            SubmissionCategory::Unavailable | SubmissionCategory::Unknown
        ) {
            let secondary = self.secondary.send_otp(sms).await;
            // A later rejection cannot prove that an ambiguous first attempt was not delivered.
            if primary == SubmissionCategory::Unknown && secondary != SubmissionCategory::Accepted {
                SubmissionCategory::Unknown
            } else {
                secondary
            }
        } else {
            primary
        };
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        if let Err(error) = self.otps.record_submission(&mut tx, id, category).await {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await
    }

    async fn reserve(
        &self,
        request: &RequestOtp,
        identity: IdentityDigest,
        now: OffsetDateTime,
    ) -> Result<Option<AdminState>, DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let check = async {
            let subject = self
                .rates
                .increment(
                    &mut tx,
                    RateLimitKey::new(
                        RateLimitScope::Subject,
                        RateLimitPurpose::Send,
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
                        RateLimitPurpose::Send,
                        *request.origin.as_bytes(),
                    ),
                    self.policy.origin_rate_limit(),
                    now,
                )
                .await?;
            if !subject.allowed || !origin.allowed {
                return Ok(None);
            }
            Ok(self
                .admins
                .find_by_identity(&mut tx, identity)
                .await?
                .filter(|admin| eligible(admin, request.purpose, now)))
        }
        .await;
        match check {
            Ok(admin) => {
                self.uow.commit(tx).await?;
                Ok(admin)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}

fn eligible(admin: &AdminState, purpose: AuthPurpose, now: OffsetDateTime) -> bool {
    match purpose {
        AuthPurpose::SignIn => admin.is_eligible(),
        AuthPurpose::Enrollment(purpose) => admin.enrollment().is_some_and(|authorization| {
            authorization.purpose() == purpose
                && authorization.epoch() == admin.epoch()
                && authorization.status() == crate::domain::admin_state::EnrollmentStatus::Pending
                && authorization.authorized_at() <= now
                && now < authorization.expires_at()
        }),
    }
}

fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}

pub(super) fn normalize_phone(phone: &SecretBytes) -> Result<SecretBytes, DomainError> {
    let input = std::str::from_utf8(phone.expose()).map_err(|_| invalid())?;
    let normalized: String = input
        .trim()
        .chars()
        .map(|c| match c {
            '۰'..='۹' => {
                char::from(b'0' + u8::try_from(u32::from(c) - u32::from('۰')).unwrap_or(0))
            }
            '٠'..='٩' => {
                char::from(b'0' + u8::try_from(u32::from(c) - u32::from('٠')).unwrap_or(0))
            }
            _ => c,
        })
        .collect();
    let national = normalized
        .strip_prefix("+98")
        .or_else(|| normalized.strip_prefix('0'))
        .ok_or_else(invalid)?;
    if national.len() != 10
        || !national.starts_with('9')
        || !national.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(invalid());
    }
    Ok(SecretBytes::new(format!("+98{national}").into_bytes()))
}
