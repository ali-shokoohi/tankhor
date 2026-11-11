//! FR-002, FR-024, FR-CON-02/Q23: crypto boundary, never a grant of console access.

use super::admin_repository::IdentityDigest;
use crate::domain::{
    admin_state::SecurityEpoch,
    auth_state::{AuthBinding, BindingDigest},
    otp_state::OtpDigest,
    secrets::SecretBytes,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ChallengeId, CredentialId},
};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenPurpose {
    Session,
    Csrf,
    Browser,
    PrimaryProof,
    Ceremony,
    EnrollmentAuthorization,
}

/// Complete trusted binding authenticated into opaque server-state envelopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyContext {
    pub challenge: ChallengeId,
    pub binding: AuthBinding,
    pub credential: Option<CredentialId>,
    pub primary_verified_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

/// Stable protection identity for an enrolled TOTP secret, not an access grant.
/// Browser/session/proof identifiers belong only to the independently checked ceremony.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpCredentialBinding {
    pub owner: AdminId,
    pub credential: CredentialId,
    pub epoch: SecurityEpoch,
}

impl TryFrom<&CeremonyContext> for TotpCredentialBinding {
    type Error = DomainError;

    fn try_from(context: &CeremonyContext) -> Result<Self, Self::Error> {
        Ok(Self {
            owner: context.binding.target,
            credential: context
                .credential
                .ok_or_else(|| DomainError::unauthorised(ErrorCode::AuthenticationRequired))?,
            epoch: context.binding.epoch,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionContext {
    Identity(AdminId),
    Ceremony(CeremonyContext),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedOtp {
    pub code: SecretBytes,
    pub digest: OtpDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyStart {
    pub context: CeremonyContext,
    pub rp_id: String,
    pub origin: String,
    /// Complete serialized public library credentials, never a vendor type.
    pub credentials: Vec<SecretBytes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyChallenge {
    /// Public `WebAuthn` options only, never the complete server state.
    pub public_options: SecretBytes,
    pub protected_state: SecretBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyFinish {
    pub request: PasskeyStart,
    pub protected_state: SecretBytes,
    pub response: SecretBytes,
    pub now: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPasskey {
    pub credential_id: SecretBytes,
    /// Complete verified counter/backup state to persist under revision CAS.
    pub credential: SecretBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpEnrollment {
    pub envelope: SecretBytes,
    /// Protected one-time presentation payload, only through `SessionStore`'s gate.
    pub protected_provisioning: SecretBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpCheck {
    pub context: CeremonyContext,
    pub envelope: SecretBytes,
    pub code: SecretBytes,
    pub now: OffsetDateTime,
}

/// Owns secure randomness, challenges, keyed digests and authenticated envelopes.
/// IDs and instants are supplied by IdGenerator/Clock, never created here.
/// Implementations reject invalid/rebound/expired proofs with sanitized errors.
/// Passkeys require UV and configured RP/origin, encrypt complete server state,
/// and bind all `CeremonyContext` fields. Empty credential lists mean registration;
/// nonempty lists mean authentication. TOTP uses RFC6238 SHA1, 6 digits, 30 seconds,
/// ±1 step and independent 160-bit secrets. Return the matched step for caller CAS;
/// neither crypto verification nor SMS acceptance prevents replay by itself.
/// TOTP secret envelopes authenticate `TotpCredentialBinding`, not enrollment's
/// browser/session/challenge or expiry. Verification checks the current ceremony's
/// stage and time independently; the application validates its primary proof,
/// complete binding, eligibility, epoch, credential revision/revocation and replay
/// CAS. Persistent decryptability never authorizes a ceremony. Provisioning remains
/// ceremony-bound and subject to the one-time `SessionStore` presentation gate.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait AdminAuthenticator: Send + Sync {
    async fn identity_digest(
        &self,
        canonical_phone: SecretBytes,
    ) -> Result<IdentityDigest, DomainError>;
    async fn issue_otp(&self, challenge: ChallengeId) -> Result<IssuedOtp, DomainError>;
    async fn verify_otp(
        &self,
        challenge: ChallengeId,
        code: SecretBytes,
        digest: OtpDigest,
    ) -> Result<bool, DomainError>;
    async fn issue_token(&self, purpose: TokenPurpose) -> Result<SecretBytes, DomainError>;
    async fn token_digest(
        &self,
        purpose: TokenPurpose,
        token: SecretBytes,
    ) -> Result<BindingDigest, DomainError>;
    async fn protect(
        &self,
        context: ProtectionContext,
        plaintext: SecretBytes,
    ) -> Result<SecretBytes, DomainError>;
    async fn unprotect(
        &self,
        context: ProtectionContext,
        envelope: SecretBytes,
    ) -> Result<SecretBytes, DomainError>;
    async fn begin_passkey(&self, request: PasskeyStart) -> Result<PasskeyChallenge, DomainError>;
    async fn finish_passkey(&self, request: PasskeyFinish) -> Result<VerifiedPasskey, DomainError>;
    async fn begin_totp(&self, context: CeremonyContext) -> Result<TotpEnrollment, DomainError>;
    async fn verify_totp(&self, request: TotpCheck) -> Result<u64, DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        admin_state::SecurityEpoch,
        auth_state::{AuthBinding, AuthPurpose, AuthStage},
        secrets::SecretBytes,
    };
    use crate::{
        domain::{auth_state::BindingDigest, otp_state::OtpDigest},
        ports::admin_repository::IdentityDigest,
    };
    use kernel::ids::{AdminId, ChallengeId, CredentialId, SessionId};
    use std::str::FromStr;
    use time::OffsetDateTime;

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn admin_authenticator_port_is_mockable_and_secret_safe() {
        let secret = SecretBytes::new(b"synthetic-private-material".to_vec());
        let context = CeremonyContext {
            challenge: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
            binding: AuthBinding {
                target: AdminId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap(),
                browser: BindingDigest::new([3; 32]),
                session: SessionId::from_str("019a53a0-0000-7000-8000-000000000004").unwrap(),
                purpose: AuthPurpose::SignIn,
                stage: AuthStage::Passkey,
                epoch: SecurityEpoch::new(1),
                credential_revision: Some(1),
            },
            credential: Some(
                CredentialId::from_str("019a53a0-0000-7000-8000-000000000005").unwrap(),
            ),
            primary_verified_at: OffsetDateTime::UNIX_EPOCH,
            created_at: OffsetDateTime::UNIX_EPOCH,
            expires_at: OffsetDateTime::UNIX_EPOCH + time::Duration::MINUTE,
        };
        let passkey = PasskeyStart {
            context: context.clone(),
            rp_id: "console.example.test".into(),
            origin: "https://console.example.test".into(),
            credentials: vec![secret.clone()],
        };
        let started = PasskeyChallenge {
            public_options: secret.clone(),
            protected_state: secret.clone(),
        };
        let proof = PasskeyFinish {
            request: passkey.clone(),
            protected_state: secret.clone(),
            response: secret.clone(),
            now: OffsetDateTime::UNIX_EPOCH,
        };
        let verified = VerifiedPasskey {
            credential_id: secret.clone(),
            credential: secret.clone(),
        };
        let totp = TotpCheck {
            context: context.clone(),
            envelope: secret.clone(),
            code: secret.clone(),
            now: OffsetDateTime::UNIX_EPOCH,
        };
        let provisioning = TotpEnrollment {
            envelope: secret.clone(),
            protected_provisioning: secret.clone(),
        };
        let protection = ProtectionContext::Ceremony(context.clone());
        let mut mock = MockAdminAuthenticator::new();
        mock.expect_identity_digest()
            .times(1)
            .returning(|_| Ok(IdentityDigest::new([1; 32])));
        mock.expect_issue_otp().times(1).returning(|_| {
            Ok(IssuedOtp {
                code: SecretBytes::new(b"123456".to_vec()),
                digest: OtpDigest::new([2; 32]),
            })
        });
        mock.expect_verify_otp()
            .times(1)
            .returning(|_, _, _| Ok(true));
        mock.expect_issue_token()
            .times(1)
            .returning(|_| Ok(SecretBytes::new(b"synthetic-token".to_vec())));
        mock.expect_token_digest()
            .times(1)
            .returning(|_, _| Ok(BindingDigest::new([3; 32])));
        mock.expect_protect()
            .times(1)
            .returning(|_, _| Ok(SecretBytes::new(vec![1])));
        mock.expect_unprotect()
            .times(1)
            .returning(|_, _| Ok(SecretBytes::new(vec![2])));
        let output = started.clone();
        mock.expect_begin_passkey()
            .times(1)
            .returning(move |_| Ok(output.clone()));
        let output = verified.clone();
        mock.expect_finish_passkey()
            .times(1)
            .returning(move |_| Ok(output.clone()));
        let output = provisioning.clone();
        mock.expect_begin_totp()
            .times(1)
            .returning(move |_| Ok(output.clone()));
        mock.expect_verify_totp().times(1).returning(|_| Ok(42));
        let port: &dyn AdminAuthenticator = &mock;
        assert_eq!(
            port.identity_digest(secret.clone()).await.unwrap(),
            IdentityDigest::new([1; 32])
        );
        let otp = port.issue_otp(context.challenge).await.unwrap();
        assert!(
            port.verify_otp(context.challenge, otp.code.clone(), otp.digest.clone())
                .await
                .unwrap()
        );
        let token = port.issue_token(TokenPurpose::Session).await.unwrap();
        assert_eq!(
            port.token_digest(TokenPurpose::Session, token.clone())
                .await
                .unwrap(),
            BindingDigest::new([3; 32])
        );
        let envelope = port
            .protect(protection.clone(), secret.clone())
            .await
            .unwrap();
        assert_eq!(
            port.unprotect(protection, envelope).await.unwrap().expose(),
            &[2]
        );
        assert_eq!(port.begin_passkey(passkey.clone()).await.unwrap(), started);
        assert_eq!(port.finish_passkey(proof.clone()).await.unwrap(), verified);
        assert_eq!(port.begin_totp(context).await.unwrap(), provisioning);
        assert_eq!(port.verify_totp(totp.clone()).await.unwrap(), 42);
        for debug in [
            format!("{passkey:?}"),
            format!("{proof:?}"),
            format!("{totp:?}"),
            format!("{started:?}"),
            format!("{verified:?}"),
            format!("{provisioning:?}"),
            format!("{otp:?}"),
            format!("{token:?}"),
        ] {
            assert!(!debug.contains("synthetic-private-material"));
            assert!(!debug.contains("synthetic-token"));
            assert!(!debug.contains("123456"));
        }
    }
}
