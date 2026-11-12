//! Complete local cryptographic implementation of the admin authenticator port.

use std::fmt;

use admin::{
    domain::{
        admin_state::EnrollmentPurpose,
        auth_state::{AuthPurpose, AuthStage, BindingDigest},
        otp_state::OtpDigest,
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::{
            AdminAuthenticator, CeremonyContext, IssuedOtp, PasskeyChallenge, PasskeyFinish,
            PasskeyStart, ProtectionContext, TokenPurpose, TotpCheck, TotpCredentialBinding,
            TotpEnrollment, VerifiedPasskey,
        },
        admin_repository::IdentityDigest,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::ChallengeId,
};
use time::OffsetDateTime;

use crate::{
    capabilities::CapabilityGenerator, envelopes::EnvelopeCipher, passkey::PasskeyEngine,
    totp::TotpEngine,
};

pub struct RealAdminAuthenticator {
    capabilities: CapabilityGenerator,
    envelopes: EnvelopeCipher,
    passkeys: PasskeyEngine,
    totp: TotpEngine,
    key_id: String,
    rp_id: String,
    origin: String,
}

impl RealAdminAuthenticator {
    /// Construct from externally managed keys and the configured relying party.
    ///
    /// # Errors
    /// Rejects an invalid key identifier or `WebAuthn` relying-party configuration.
    pub fn new(
        key: [u8; 32],
        key_id: &str,
        rp_id: &str,
        origin: &str,
    ) -> Result<Self, DomainError> {
        if key_id.is_empty() || key_id.len() > usize::from(u8::MAX) {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self {
            capabilities: CapabilityGenerator::new(key),
            envelopes: EnvelopeCipher::new(key),
            passkeys: PasskeyEngine::new(rp_id, origin)?,
            totp: TotpEngine::new(),
            key_id: key_id.to_owned(),
            rp_id: rp_id.to_owned(),
            origin: origin.to_owned(),
        })
    }

    fn protect_bytes(
        &self,
        aad: &[u8],
        plaintext: &SecretBytes,
    ) -> Result<SecretBytes, DomainError> {
        self.envelopes
            .seal(&self.key_id, aad, plaintext)
            .map_err(|_| denied())
    }

    fn unprotect_bytes(
        &self,
        aad: &[u8],
        envelope: &SecretBytes,
    ) -> Result<SecretBytes, DomainError> {
        self.envelopes
            .open(&self.key_id, aad, envelope)
            .map_err(|_| denied())
    }

    fn check_passkey_request(&self, request: &PasskeyStart) -> Result<(), DomainError> {
        if request.rp_id != self.rp_id || request.origin != self.origin {
            return Err(denied());
        }
        check_ceremony(
            &request.context,
            request.context.created_at,
            AuthStage::Passkey,
        )
    }
}

#[async_trait::async_trait]
impl AdminAuthenticator for RealAdminAuthenticator {
    async fn identity_digest(
        &self,
        canonical_phone: SecretBytes,
    ) -> Result<IdentityDigest, DomainError> {
        self.capabilities.identity_digest(&canonical_phone)
    }

    async fn issue_otp(&self, challenge: ChallengeId) -> Result<IssuedOtp, DomainError> {
        self.capabilities.issue_otp(challenge)
    }

    async fn verify_otp(
        &self,
        challenge: ChallengeId,
        code: SecretBytes,
        digest: OtpDigest,
    ) -> Result<bool, DomainError> {
        self.capabilities.verify_otp(challenge, &code, &digest)
    }

    async fn issue_token(&self, purpose: TokenPurpose) -> Result<SecretBytes, DomainError> {
        self.capabilities.issue_token(purpose)
    }

    async fn token_digest(
        &self,
        purpose: TokenPurpose,
        token: SecretBytes,
    ) -> Result<BindingDigest, DomainError> {
        self.capabilities.token_digest(purpose, &token)
    }

    async fn protect(
        &self,
        context: ProtectionContext,
        plaintext: SecretBytes,
    ) -> Result<SecretBytes, DomainError> {
        self.protect_bytes(&protection_aad(&context), &plaintext)
    }

    async fn unprotect(
        &self,
        context: ProtectionContext,
        envelope: SecretBytes,
    ) -> Result<SecretBytes, DomainError> {
        self.unprotect_bytes(&protection_aad(&context), &envelope)
    }

    async fn begin_passkey(&self, request: PasskeyStart) -> Result<PasskeyChallenge, DomainError> {
        self.check_passkey_request(&request)?;
        let (public_options, state) = if request.credentials.is_empty() {
            self.passkeys.begin_registration(
                request.context.binding.target.into(),
                &request.context.binding.target.to_string(),
            )?
        } else {
            self.passkeys.begin_authentication(&request.credentials)?
        };
        let protected_state = self.protect_bytes(&ceremony_aad(&request.context), &state)?;
        Ok(PasskeyChallenge {
            public_options,
            protected_state,
        })
    }

    async fn finish_passkey(&self, request: PasskeyFinish) -> Result<VerifiedPasskey, DomainError> {
        self.check_passkey_request(&request.request)?;
        check_ceremony(&request.request.context, request.now, AuthStage::Passkey)?;
        let state = self.unprotect_bytes(
            &ceremony_aad(&request.request.context),
            &request.protected_state,
        )?;
        let credential = if request.request.credentials.is_empty() {
            self.passkeys
                .finish_registration(&state, &request.response)?
        } else {
            self.passkeys.finish_authentication_any(
                &state,
                &request.response,
                &request.request.credentials,
            )?
        };
        Ok(VerifiedPasskey {
            credential_id: credential.credential_id,
            credential: credential.credential,
        })
    }

    async fn begin_totp(&self, context: CeremonyContext) -> Result<TotpEnrollment, DomainError> {
        check_ceremony(&context, context.created_at, AuthStage::Totp)?;
        let binding = TotpCredentialBinding::try_from(&context)?;
        let secret = self.totp.generate_secret();
        let envelope = self.protect_bytes(&totp_credential_aad(&binding), &secret)?;
        let provisioning = self
            .totp
            .provisioning(&secret, &context.binding.target.to_string())?;
        let protected_provisioning = self.protect_bytes(&ceremony_aad(&context), &provisioning)?;
        Ok(TotpEnrollment {
            envelope,
            protected_provisioning,
        })
    }

    async fn verify_totp(&self, request: TotpCheck) -> Result<u64, DomainError> {
        check_ceremony(&request.context, request.now, AuthStage::Totp)?;
        let binding = TotpCredentialBinding::try_from(&request.context)?;
        let secret = self.unprotect_bytes(&totp_credential_aad(&binding), &request.envelope)?;
        self.totp.verify(&secret, &request.code, request.now)
    }
}

fn check_ceremony(
    context: &CeremonyContext,
    now: OffsetDateTime,
    stage: AuthStage,
) -> Result<(), DomainError> {
    if context.binding.stage != stage
        || context.expires_at <= context.created_at
        || context.primary_verified_at > context.created_at
        || now < context.created_at
        || now >= context.expires_at
    {
        return Err(denied());
    }
    Ok(())
}

fn protection_aad(context: &ProtectionContext) -> Vec<u8> {
    match context {
        ProtectionContext::Identity(admin) => {
            let mut aad = b"tankhor.identity.v1".to_vec();
            push_uuid(&mut aad, *admin);
            aad
        }
        ProtectionContext::Ceremony(context) => ceremony_aad(context),
    }
}

#[must_use]
pub fn totp_credential_aad(binding: &TotpCredentialBinding) -> Vec<u8> {
    let mut aad = b"tankhor.totp-credential.v1".to_vec();
    push_uuid(&mut aad, binding.owner);
    push_uuid(&mut aad, binding.credential);
    aad.extend_from_slice(&binding.epoch.value().to_be_bytes());
    aad
}

fn ceremony_aad(context: &CeremonyContext) -> Vec<u8> {
    let mut aad = b"tankhor.ceremony.v1".to_vec();
    push_uuid(&mut aad, context.challenge);
    push_uuid(&mut aad, context.binding.target);
    aad.extend_from_slice(context.binding.browser.as_bytes());
    push_uuid(&mut aad, context.binding.session);
    match context.binding.purpose {
        AuthPurpose::SignIn => aad.push(0),
        AuthPurpose::Enrollment(EnrollmentPurpose::Addition) => aad.extend_from_slice(&[1, 0]),
        AuthPurpose::Enrollment(EnrollmentPurpose::Recovery) => aad.extend_from_slice(&[1, 1]),
    }
    aad.push(match context.binding.stage {
        AuthStage::PrimaryVerified => 0,
        AuthStage::Passkey => 1,
        AuthStage::Totp => 2,
    });
    aad.extend_from_slice(&context.binding.epoch.value().to_be_bytes());
    match context.binding.credential_revision {
        Some(revision) => {
            aad.push(1);
            aad.extend_from_slice(&revision.to_be_bytes());
        }
        None => aad.push(0),
    }
    match context.credential {
        Some(credential) => {
            aad.push(1);
            push_uuid(&mut aad, credential);
        }
        None => aad.push(0),
    }
    for instant in [
        context.primary_verified_at,
        context.created_at,
        context.expires_at,
    ] {
        aad.extend_from_slice(&instant.unix_timestamp_nanos().to_be_bytes());
    }
    aad
}

fn push_uuid(output: &mut Vec<u8>, id: impl Into<uuid::Uuid>) {
    output.extend_from_slice(id.into().as_bytes());
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}

impl fmt::Debug for RealAdminAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RealAdminAuthenticator([REDACTED])")
    }
}
