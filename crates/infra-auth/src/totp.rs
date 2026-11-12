//! RFC 6238 SHA-1 TOTP with explicit timestamps and matched steps.

use admin::domain::secrets::SecretBytes;
use kernel::error::{DomainError, ErrorCode};
use rand::{RngCore, rng};
use time::OffsetDateTime;
use totp_rs::{Algorithm, Builder, Totp};

#[derive(Debug, Default)]
pub struct TotpEngine;

impl TotpEngine {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn generate_secret(&self) -> SecretBytes {
        let mut secret = [0_u8; 20];
        rng().fill_bytes(&mut secret);
        SecretBytes::new(secret.to_vec())
    }

    /// # Errors
    /// Rejects an invalid secret or account metadata.
    pub fn provisioning(
        &self,
        secret: &SecretBytes,
        account_name: &str,
    ) -> Result<SecretBytes, DomainError> {
        let totp = Self::totp(secret, Some(account_name))?;
        let url = totp
            .to_url()
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
        Ok(SecretBytes::new(url.into_bytes()))
    }

    /// # Errors
    /// Rejects invalid secret material or a pre-epoch timestamp.
    pub fn current_code(
        &self,
        secret: &SecretBytes,
        now: OffsetDateTime,
    ) -> Result<SecretBytes, DomainError> {
        let timestamp = timestamp(now)?;
        Ok(SecretBytes::new(
            Self::totp(secret, None)?
                .generate(timestamp)
                .to_string()
                .into_bytes(),
        ))
    }

    /// Verify only the mathematical proof; persistence must CAS the returned step.
    /// # Errors
    /// Rejects malformed/invalid codes, invalid secrets and pre-epoch timestamps.
    pub fn verify(
        &self,
        secret: &SecretBytes,
        code: &SecretBytes,
        now: OffsetDateTime,
    ) -> Result<u64, DomainError> {
        let timestamp = timestamp(now)?;
        let code = std::str::from_utf8(code.expose()).map_err(|_| denied())?;
        Self::totp(secret, None)?
            .check(code, timestamp)
            .ok_or_else(denied)
    }

    fn totp(secret: &SecretBytes, account_name: Option<&str>) -> Result<Totp, DomainError> {
        let mut builder = Builder::new()
            .with_algorithm(Algorithm::SHA1)
            .with_digits(6)
            .with_skew(1)
            .with_step_duration(30)
            .with_secret(secret.expose().to_vec());
        if let Some(account_name) = account_name {
            builder = builder
                .with_issuer(Some("Tankhor"))
                .with_account_name(account_name.to_owned());
        }
        builder
            .build()
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))
    }
}

fn timestamp(now: OffsetDateTime) -> Result<u64, DomainError> {
    u64::try_from(now.unix_timestamp())
        .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
