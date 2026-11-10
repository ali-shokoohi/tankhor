//! Real RFC 9562 `UUIDv7` identifier generator implementing [`IdGenerator`].

use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{
    AdminId, ApprovalId, AuditId, ChallengeId, ChangeId, CredentialId, PreviewId, PublicationId,
    RevisionId, SessionId, VersionId,
};
use kernel::ports::id_generator::IdGenerator;
use uuid::Uuid;

/// Real system `UUIDv7` generator.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemIdGenerator;

impl SystemIdGenerator {
    /// Construct a new [`SystemIdGenerator`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn generate<T>(
        construct: fn(Uuid) -> Result<T, kernel::ids::IdError>,
    ) -> Result<T, DomainError> {
        construct(Uuid::now_v7()).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }
}

#[async_trait::async_trait]
impl IdGenerator for SystemIdGenerator {
    async fn next_admin_id(&self) -> Result<AdminId, DomainError> {
        Self::generate(AdminId::try_new)
    }

    async fn next_credential_id(&self) -> Result<CredentialId, DomainError> {
        Self::generate(CredentialId::try_new)
    }

    async fn next_session_id(&self) -> Result<SessionId, DomainError> {
        Self::generate(SessionId::try_new)
    }

    async fn next_challenge_id(&self) -> Result<ChallengeId, DomainError> {
        Self::generate(ChallengeId::try_new)
    }

    async fn next_change_id(&self) -> Result<ChangeId, DomainError> {
        Self::generate(ChangeId::try_new)
    }

    async fn next_revision_id(&self) -> Result<RevisionId, DomainError> {
        Self::generate(RevisionId::try_new)
    }

    async fn next_preview_id(&self) -> Result<PreviewId, DomainError> {
        Self::generate(PreviewId::try_new)
    }

    async fn next_approval_id(&self) -> Result<ApprovalId, DomainError> {
        Self::generate(ApprovalId::try_new)
    }

    async fn next_publication_id(&self) -> Result<PublicationId, DomainError> {
        Self::generate(PublicationId::try_new)
    }

    async fn next_version_id(&self) -> Result<VersionId, DomainError> {
        Self::generate(VersionId::try_new)
    }

    async fn next_audit_id(&self) -> Result<AuditId, DomainError> {
        Self::generate(AuditId::try_new)
    }
}
