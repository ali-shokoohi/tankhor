//! FR-001, FR-002: issue browser-bound form presentation without granting access.
use crate::{
    domain::{auth_state::BindingDigest, secrets::SecretBytes},
    ports::admin_authenticator::{AdminAuthenticator, TokenPurpose},
};
use kernel::error::DomainError;

/// Opaque form capabilities; neither value is a primary proof or admin session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessForm {
    pub csrf: SecretBytes,
    pub browser: BindingDigest,
}

pub struct PrepareAccessForm<'a> {
    pub authenticator: &'a dyn AdminAuthenticator,
}

impl PrepareAccessForm<'_> {
    /// # Errors
    /// Propagates sanitized capability-generation and browser-binding failures.
    pub async fn execute(&self, browser: SecretBytes) -> Result<AccessForm, DomainError> {
        let browser = self
            .authenticator
            .token_digest(TokenPurpose::Browser, browser)
            .await?;
        let csrf = self.authenticator.issue_token(TokenPurpose::Csrf).await?;
        Ok(AccessForm { csrf, browser })
    }
}
