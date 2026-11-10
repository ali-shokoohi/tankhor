//! Sanitized domain and port error taxonomy.
//!
//! Infrastructure, SQL, cryptographic and vendor payloads never cross port boundaries;
//! adapters map failures to closed [`ErrorCategory`] and [`ErrorCode`] values, and web
//! translates them into localized copy or RFC 9457 `application/problem+json`.

use std::fmt;

use thiserror::Error;

/// High-level error classification shared across kernel, contexts and adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Input or domain invariant validation failed.
    Validation,
    /// Caller lacks a valid primary or full administrator proof.
    Unauthorised,
    /// Authenticated caller is not permitted to perform the action.
    Forbidden,
    /// State conflict, stale revision/baseline, CAS mismatch or duplicate payload mismatch.
    Conflict,
    /// Required store, seed, capability or dependency is unavailable.
    Unavailable,
    /// Transaction handle is finished, foreign to the store or from another backend.
    InvalidHandle,
    /// Transient contention or serialization failure that is safe to retry.
    Retryable,
    /// Commit outcome is indeterminate and must be resolved via stable operation identity.
    UnknownCommit,
}

impl ErrorCategory {
    /// Canonical lowercase category identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Unauthorised => "unauthorised",
            Self::Forbidden => "forbidden",
            Self::Conflict => "conflict",
            Self::Unavailable => "unavailable",
            Self::InvalidHandle => "invalid_handle",
            Self::Retryable => "retryable",
            Self::UnknownCommit => "unknown_commit",
        }
    }
}

/// Sanitized machine-readable error codes carrying zero raw infrastructure text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Field or policy value is invalid or out of range.
    InvalidValue,
    /// Primary or full authentication proof is missing, expired or invalid.
    AuthenticationRequired,
    /// Caller is authenticated but barred from performing this transition.
    PermissionDenied,
    /// Expected revision, baseline, security epoch or resource state changed.
    StaleRevision,
    /// Resource was not found.
    NotFound,
    /// Operation identity was reused with a mismatched payload.
    IdempotencyConflict,
    /// Required storage, seed or upstream dependency is unavailable.
    PrerequisiteUnavailable,
    /// Deterministic/bounded source exhausted or clock unavailable.
    SourceExhausted,
    /// Transaction handle has already been consumed or closed.
    FinishedTransactionHandle,
    /// Transaction handle belongs to a different store or adapter backend.
    ForeignTransactionHandle,
    /// Read-only transaction rejected a mutating operation.
    ReadOnlyTransaction,
    /// Transient serialization/lock failure safe to retry.
    TransientConflict,
    /// Commit acknowledgement was lost; check stable operation identity before retrying.
    CommitOutcomeUnknown,
}

impl ErrorCode {
    /// Canonical stable token for logging and problem mapping.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidValue => "invalid_value",
            Self::AuthenticationRequired => "authentication_required",
            Self::PermissionDenied => "permission_denied",
            Self::StaleRevision => "stale_revision",
            Self::NotFound => "not_found",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::PrerequisiteUnavailable => "prerequisite_unavailable",
            Self::SourceExhausted => "source_exhausted",
            Self::FinishedTransactionHandle => "finished_transaction_handle",
            Self::ForeignTransactionHandle => "foreign_transaction_handle",
            Self::ReadOnlyTransaction => "read_only_transaction",
            Self::TransientConflict => "transient_conflict",
            Self::CommitOutcomeUnknown => "commit_outcome_unknown",
        }
    }
}

/// Sanitized error value returned by domain rules, ports and application services.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Error)]
#[error("{}: {}", self.category.as_str(), self.code.as_str())]
pub struct DomainError {
    category: ErrorCategory,
    code: ErrorCode,
}

impl fmt::Debug for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DomainError")
            .field("category", &self.category.as_str())
            .field("code", &self.code.as_str())
            .finish()
    }
}

impl DomainError {
    /// Construct a sanitized error from an explicit category and code.
    #[must_use]
    pub const fn new(category: ErrorCategory, code: ErrorCode) -> Self {
        Self { category, code }
    }

    /// Construct a validation error.
    #[must_use]
    pub const fn validation(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::Validation, code)
    }

    /// Construct an unauthorised error.
    #[must_use]
    pub const fn unauthorised(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::Unauthorised, code)
    }

    /// Construct a forbidden error.
    #[must_use]
    pub const fn forbidden(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::Forbidden, code)
    }

    /// Construct a conflict / stale-state error.
    #[must_use]
    pub const fn conflict(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::Conflict, code)
    }

    /// Construct an unavailable prerequisite/store error.
    #[must_use]
    pub const fn unavailable(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::Unavailable, code)
    }

    /// Construct an invalid transaction-handle error.
    #[must_use]
    pub const fn invalid_handle(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::InvalidHandle, code)
    }

    /// Construct a safely retryable transient error.
    #[must_use]
    pub const fn retryable(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::Retryable, code)
    }

    /// Construct an indeterminate-commit error requiring operation-ID resolution.
    #[must_use]
    pub const fn unknown_commit(code: ErrorCode) -> Self {
        Self::new(ErrorCategory::UnknownCommit, code)
    }

    /// Return the high-level error category.
    #[must_use]
    pub const fn category(self) -> ErrorCategory {
        self.category
    }

    /// Return the sanitized error code.
    #[must_use]
    pub const fn code(self) -> ErrorCode {
        self.code
    }

    /// True only when the error represents a safely retryable transient failure.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self.category, ErrorCategory::Retryable)
    }

    /// True when commit acknowledgement was lost and outcome is unknown.
    #[must_use]
    pub const fn is_unknown_commit(self) -> bool {
        matches!(self.category, ErrorCategory::UnknownCommit)
    }
}
