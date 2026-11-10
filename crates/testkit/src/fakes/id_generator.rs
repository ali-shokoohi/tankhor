//! Deterministic sequence-based `UUIDv7` identifier generator fake.

use std::sync::Mutex;

use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{
    AdminId, ApprovalId, AuditId, ChallengeId, ChangeId, CredentialId, PreviewId, PublicationId,
    RevisionId, SessionId, VersionId,
};
use kernel::ports::id_generator::IdGenerator;
use uuid::Uuid;

#[derive(Debug)]
struct GeneratorState {
    next_seq: u64,
    remaining: Option<usize>,
}

/// Deterministic in-memory [`IdGenerator`] producing strictly increasing RFC 9562 `UUIDv7` values.
#[derive(Debug)]
pub struct SequenceIdGenerator {
    state: Mutex<GeneratorState>,
}

impl SequenceIdGenerator {
    /// Construct an unbounded deterministic [`SequenceIdGenerator`] starting at `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            state: Mutex::new(GeneratorState {
                next_seq: seed,
                remaining: None,
            }),
        }
    }

    /// Construct a bounded deterministic [`SequenceIdGenerator`] that fails after `limit` allocations.
    #[must_use]
    pub const fn with_limit(seed: u64, limit: usize) -> Self {
        Self {
            state: Mutex::new(GeneratorState {
                next_seq: seed,
                remaining: Some(limit),
            }),
        }
    }

    fn allocate_raw(&self) -> Result<Uuid, DomainError> {
        let seq = {
            let mut guard = self
                .state
                .lock()
                .expect("SequenceIdGenerator mutex poisoned");
            if let Some(rem) = &mut guard.remaining {
                if *rem == 0 {
                    return Err(DomainError::unavailable(ErrorCode::SourceExhausted));
                }
                *rem -= 1;
            }
            let current = guard.next_seq;
            guard.next_seq = guard
                .next_seq
                .checked_add(1)
                .ok_or_else(|| DomainError::unavailable(ErrorCode::SourceExhausted))?;
            current
        };

        // Construct a canonical RFC 9562 UUIDv7 without touching wall clock or OS randomness:
        // 48-bit monotonic timestamp starting at 2025-11-01T00:00:00Z + sequence ms,
        // version = 0b0111, variant = 0b10, deterministic sequence payload.
        let unix_ms = 1_761_955_200_000u64.saturating_add(seq);
        let ts_be = unix_ms.to_be_bytes();
        let seq_be = seq.to_be_bytes();
        let mut bytes = [0u8; 16];
        bytes[0..6].copy_from_slice(&ts_be[2..8]);
        bytes[6] = 0x70 | (seq_be[0] & 0x0f);
        bytes[7] = seq_be[1];
        bytes[8] = 0x80 | (seq_be[2] & 0x3f);
        bytes[9..15].copy_from_slice(&seq_be[2..8]);
        bytes[15] = seq_be[7];
        Ok(Uuid::from_bytes(bytes))
    }
}

#[async_trait::async_trait]
impl IdGenerator for SequenceIdGenerator {
    async fn next_admin_id(&self) -> Result<AdminId, DomainError> {
        let raw = self.allocate_raw()?;
        AdminId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_credential_id(&self) -> Result<CredentialId, DomainError> {
        let raw = self.allocate_raw()?;
        CredentialId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_session_id(&self) -> Result<SessionId, DomainError> {
        let raw = self.allocate_raw()?;
        SessionId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_challenge_id(&self) -> Result<ChallengeId, DomainError> {
        let raw = self.allocate_raw()?;
        ChallengeId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_change_id(&self) -> Result<ChangeId, DomainError> {
        let raw = self.allocate_raw()?;
        ChangeId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_revision_id(&self) -> Result<RevisionId, DomainError> {
        let raw = self.allocate_raw()?;
        RevisionId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_preview_id(&self) -> Result<PreviewId, DomainError> {
        let raw = self.allocate_raw()?;
        PreviewId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_approval_id(&self) -> Result<ApprovalId, DomainError> {
        let raw = self.allocate_raw()?;
        ApprovalId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_publication_id(&self) -> Result<PublicationId, DomainError> {
        let raw = self.allocate_raw()?;
        PublicationId::try_new(raw)
            .map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_version_id(&self) -> Result<VersionId, DomainError> {
        let raw = self.allocate_raw()?;
        VersionId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }

    async fn next_audit_id(&self) -> Result<AuditId, DomainError> {
        let raw = self.allocate_raw()?;
        AuditId::try_new(raw).map_err(|_| DomainError::unavailable(ErrorCode::SourceExhausted))
    }
}
