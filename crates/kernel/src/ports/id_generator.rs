//! Asynchronous typed `UUIDv7` identifier generator port.
#![cfg_attr(
    any(test, feature = "mocks"),
    expect(
        clippy::struct_field_names,
        reason = "Mockall generates one expectation field per typed identifier method"
    )
)]

use crate::error::DomainError;
use crate::ids::{
    AdminId, ApprovalId, AuditId, ChallengeId, ChangeId, CredentialId, PreviewId, PublicationId,
    RevisionId, SessionId, VersionId,
};

/// Asynchronous source of checked `UUIDv7` identifiers supplied by the composition root or fixture.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait IdGenerator: Send + Sync {
    /// Allocate a fresh [`AdminId`].
    async fn next_admin_id(&self) -> Result<AdminId, DomainError>;
    /// Allocate a fresh [`CredentialId`].
    async fn next_credential_id(&self) -> Result<CredentialId, DomainError>;
    /// Allocate a fresh [`SessionId`].
    async fn next_session_id(&self) -> Result<SessionId, DomainError>;
    /// Allocate a fresh [`ChallengeId`].
    async fn next_challenge_id(&self) -> Result<ChallengeId, DomainError>;
    /// Allocate a fresh [`ChangeId`].
    async fn next_change_id(&self) -> Result<ChangeId, DomainError>;
    /// Allocate a fresh [`RevisionId`].
    async fn next_revision_id(&self) -> Result<RevisionId, DomainError>;
    /// Allocate a fresh [`PreviewId`].
    async fn next_preview_id(&self) -> Result<PreviewId, DomainError>;
    /// Allocate a fresh [`ApprovalId`].
    async fn next_approval_id(&self) -> Result<ApprovalId, DomainError>;
    /// Allocate a fresh [`PublicationId`].
    async fn next_publication_id(&self) -> Result<PublicationId, DomainError>;
    /// Allocate a fresh [`VersionId`].
    async fn next_version_id(&self) -> Result<VersionId, DomainError>;
    /// Allocate a fresh [`AuditId`].
    async fn next_audit_id(&self) -> Result<AuditId, DomainError>;
}
