use kernel::error::{DomainError, ErrorCode};

pub(crate) fn map_sql(error: &sqlx::Error) -> DomainError {
    match error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .as_deref()
    {
        Some("40001" | "40P01") => DomainError::retryable(ErrorCode::TransientConflict),
        Some("23505") => DomainError::conflict(ErrorCode::IdempotencyConflict),
        Some("25006") => DomainError::invalid_handle(ErrorCode::ReadOnlyTransaction),
        _ => unavailable(),
    }
}

pub(crate) const fn unavailable() -> DomainError {
    DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)
}

pub(crate) const fn stale() -> DomainError {
    DomainError::conflict(ErrorCode::StaleRevision)
}
