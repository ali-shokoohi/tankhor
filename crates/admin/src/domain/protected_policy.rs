//! Fixed editable registry and non-negotiable product guardrail boundary.

use crate::domain::flag::FlagKey;
use crate::domain::settings::SettingKey;
use kernel::error::{DomainError, ErrorCode};
use std::str::FromStr;

/// Names only: the owning product features must provide the executable evidence.
pub const OWNED_GUARDRAIL_BOUNDARY: [&str; 9] = [
    "GR1", "GR2", "GR3", "GR4", "GR5", "GR6", "GR7", "GR8", "GR9",
];

/// # Errors
/// Rejects unknown or non-editable keys.
pub fn ensure_editable_key(key: &str) -> Result<(), DomainError> {
    SettingKey::from_str(key)
        .map(|_| ())
        .or_else(|_| FlagKey::from_str(key).map(|_| ()))
        .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        flag::launch_flags,
        settings::{initial_numeric_settings, initial_upload_formats},
    };

    #[test]
    fn settings_cannot_weaken_security_or_financial_integrity() {
        let mut settings = initial_numeric_settings()
            .into_iter()
            .map(|setting| setting.key().as_str())
            .collect::<Vec<_>>();
        settings.push(initial_upload_formats().key().as_str());
        for key in settings {
            assert!(ensure_editable_key(key).is_ok());
        }
        for flag in launch_flags() {
            assert!(ensure_editable_key(flag.key().as_str()).is_ok());
        }
        for bypass in [
            "mandatory_second_factor",
            "admin_access_restrictions",
            "audit_protection",
            "financial_integrity",
            "security_off",
            "audit_off",
            "double_debit_allowed",
            "payment_credit_bypass",
            "unknown_or_arbitrary_key",
        ] {
            assert!(ensure_editable_key(bypass).is_err());
        }
        assert_eq!(OWNED_GUARDRAIL_BOUNDARY.len(), 9);
        assert!(OWNED_GUARDRAIL_BOUNDARY.contains(&"GR9"));
    }
}
