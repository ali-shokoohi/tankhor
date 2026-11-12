//! Secure random capabilities and purpose-separated keyed digests.

use std::fmt;

use admin::{
    domain::{auth_state::BindingDigest, otp_state::OtpDigest, secrets::SecretBytes},
    ports::{
        admin_authenticator::{IssuedOtp, TokenPurpose},
        admin_repository::IdentityDigest,
    },
};
use kernel::{error::DomainError, ids::ChallengeId};
use rand::{Rng, RngCore, rng};
use sha2::{Digest, Sha256};

const HEX: &[u8; 16] = b"0123456789abcdef";

pub struct CapabilityGenerator {
    key: [u8; 32],
}

impl CapabilityGenerator {
    #[must_use]
    pub const fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// # Errors
    /// Reserved for the authenticator port's safe error boundary.
    pub fn issue_otp(&self, challenge: ChallengeId) -> Result<IssuedOtp, DomainError> {
        let code = format!("{:06}", rng().random_range(0..1_000_000));
        let code = SecretBytes::new(code.into_bytes());
        let digest = self.digest(b"otp-v1", &[challenge.as_uuid().as_bytes(), code.expose()]);
        Ok(IssuedOtp {
            code,
            digest: OtpDigest::new(digest),
        })
    }

    /// # Errors
    /// Reserved for the authenticator port's safe error boundary.
    pub fn verify_otp(
        &self,
        challenge: ChallengeId,
        code: &SecretBytes,
        expected: &OtpDigest,
    ) -> Result<bool, DomainError> {
        let actual = self.digest(b"otp-v1", &[challenge.as_uuid().as_bytes(), code.expose()]);
        let difference = actual
            .iter()
            .zip(expected.as_bytes())
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            });
        Ok(difference == 0)
    }

    /// # Errors
    /// Reserved for the authenticator port's safe error boundary.
    pub fn issue_token(&self, _purpose: TokenPurpose) -> Result<SecretBytes, DomainError> {
        let mut token = [0_u8; 32];
        rng().fill_bytes(&mut token);
        let mut encoded = Vec::with_capacity(64);
        for byte in token {
            encoded.push(HEX[usize::from(byte >> 4)]);
            encoded.push(HEX[usize::from(byte & 0x0f)]);
        }
        Ok(SecretBytes::new(encoded))
    }

    /// # Errors
    /// Reserved for the authenticator port's safe error boundary.
    pub fn token_digest(
        &self,
        purpose: TokenPurpose,
        token: &SecretBytes,
    ) -> Result<BindingDigest, DomainError> {
        let label: &[u8] = match purpose {
            TokenPurpose::Session => b"session-v1",
            TokenPurpose::Csrf => b"csrf-v1",
            TokenPurpose::Browser => b"browser-v1",
            TokenPurpose::PrimaryProof => b"primary-proof-v1",
            TokenPurpose::Ceremony => b"ceremony-v1",
            TokenPurpose::EnrollmentAuthorization => b"enrollment-authorization-v1",
        };
        Ok(BindingDigest::new(self.digest(label, &[token.expose()])))
    }

    /// # Errors
    /// Reserved for the authenticator port's safe error boundary.
    pub fn identity_digest(
        &self,
        canonical_phone: &SecretBytes,
    ) -> Result<IdentityDigest, DomainError> {
        Ok(IdentityDigest::new(self.digest(
            b"canonical-phone-v1",
            &[canonical_phone.expose()],
        )))
    }

    fn digest(&self, label: &[u8], values: &[&[u8]]) -> [u8; 32] {
        let mut inner = Vec::new();
        inner.extend_from_slice(label);
        inner.push(0);
        for value in values {
            inner.extend_from_slice(&(value.len() as u64).to_be_bytes());
            inner.extend_from_slice(value);
        }
        let inner_hash = Sha256::digest(
            self.key
                .iter()
                .map(|byte| byte ^ 0x36)
                .chain(inner)
                .collect::<Vec<_>>(),
        );
        let outer = self
            .key
            .iter()
            .map(|byte| byte ^ 0x5c)
            .chain(inner_hash)
            .collect::<Vec<_>>();
        Sha256::digest(outer).into()
    }
}

impl fmt::Debug for CapabilityGenerator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CapabilityGenerator([REDACTED])")
    }
}
