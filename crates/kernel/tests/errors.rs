use kernel::error::{DomainError, ErrorCode};

#[test]
fn domain_errors_do_not_expose_infrastructure_secrets() {
    let cases = [
        (
            DomainError::validation(ErrorCode::InvalidValue),
            "validation",
            ErrorCode::InvalidValue,
            false,
            false,
        ),
        (
            DomainError::unauthorised(ErrorCode::AuthenticationRequired),
            "unauthorised",
            ErrorCode::AuthenticationRequired,
            false,
            false,
        ),
        (
            DomainError::forbidden(ErrorCode::PermissionDenied),
            "forbidden",
            ErrorCode::PermissionDenied,
            false,
            false,
        ),
        (
            DomainError::conflict(ErrorCode::StaleRevision),
            "conflict",
            ErrorCode::StaleRevision,
            false,
            false,
        ),
        (
            DomainError::unavailable(ErrorCode::PrerequisiteUnavailable),
            "unavailable",
            ErrorCode::PrerequisiteUnavailable,
            false,
            false,
        ),
        (
            DomainError::invalid_handle(ErrorCode::ForeignTransactionHandle),
            "invalid_handle",
            ErrorCode::ForeignTransactionHandle,
            false,
            false,
        ),
        (
            DomainError::retryable(ErrorCode::TransientConflict),
            "retryable",
            ErrorCode::TransientConflict,
            true,
            false,
        ),
        (
            DomainError::unknown_commit(ErrorCode::CommitOutcomeUnknown),
            "unknown_commit",
            ErrorCode::CommitOutcomeUnknown,
            false,
            true,
        ),
    ];

    assert_eq!(cases.len(), 8);

    for (err, category, expected_code, expected_retryable, expected_unknown_commit) in cases {
        assert_eq!(err.category().as_str(), category);
        assert_eq!(err.code(), expected_code);
        assert_eq!(err.is_retryable(), expected_retryable);
        assert_eq!(err.is_unknown_commit(), expected_unknown_commit);

        let display_out = err.to_string();
        let debug_out = format!("{err:?}");
        for rendered in [&display_out, &debug_out] {
            assert!(!rendered.contains("postgres://"));
            assert!(!rendered.contains("password"));
            assert!(!rendered.contains("SELECT"));
            assert!(!rendered.contains("api_key"));
        }
    }
}
