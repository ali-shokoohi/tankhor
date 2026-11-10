//! Validated operator HTTP transport limits (`TANKHOR_HTTP_MAX_BODY_BYTES`, `TANKHOR_HTTP_MAX_INFLIGHT`).

use std::collections::BTreeMap;
use std::num::NonZeroUsize;

/// Upper finite ceiling for configured HTTP request body size (64 MiB).
pub const MAX_ALLOWED_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Upper finite ceiling for configured concurrent in-flight HTTP requests.
pub const MAX_ALLOWED_INFLIGHT: usize = 65_536;

/// Sanitized transport policy validation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportPolicyError {
    #[error("missing required TANKHOR_HTTP_MAX_BODY_BYTES variable")]
    MissingMaxBodyBytes,
    #[error("invalid TANKHOR_HTTP_MAX_BODY_BYTES; expected finite positive byte count")]
    InvalidMaxBodyBytes,
    #[error("missing required TANKHOR_HTTP_MAX_INFLIGHT variable")]
    MissingMaxInflight,
    #[error("invalid TANKHOR_HTTP_MAX_INFLIGHT; expected finite positive concurrency limit")]
    InvalidMaxInflight,
}

/// Owned, validated HTTP transport policy passed from composition root to `web`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportPolicy {
    max_body_bytes: NonZeroUsize,
    max_inflight: NonZeroUsize,
}

impl TransportPolicy {
    /// Constructs a validated transport policy from explicit finite bounds.
    ///
    /// # Errors
    ///
    /// Returns [`TransportPolicyError`] if either bound exceeds the finite ceiling.
    pub const fn new(
        max_body_bytes: NonZeroUsize,
        max_inflight: NonZeroUsize,
    ) -> Result<Self, TransportPolicyError> {
        if max_body_bytes.get() > MAX_ALLOWED_BODY_BYTES {
            return Err(TransportPolicyError::InvalidMaxBodyBytes);
        }
        if max_inflight.get() > MAX_ALLOWED_INFLIGHT {
            return Err(TransportPolicyError::InvalidMaxInflight);
        }
        Ok(Self {
            max_body_bytes,
            max_inflight,
        })
    }

    /// Parses and validates operator transport policy from process environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`TransportPolicyError`] when either variable is missing, zero, negative,
    /// non-integer, or exceeds the finite ceiling.
    pub fn from_env() -> Result<Self, TransportPolicyError> {
        Self::from_pairs(std::env::vars())
    }

    /// Parses and validates operator transport policy from `(key, value)` pairs.
    ///
    /// # Errors
    ///
    /// Returns [`TransportPolicyError`] when either variable is missing, zero, negative,
    /// non-integer, or exceeds the finite ceiling.
    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self, TransportPolicyError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut map = BTreeMap::new();
        for (key, val) in pairs {
            map.insert(key.as_ref().to_owned(), val.as_ref().to_owned());
        }

        let raw_body = map
            .get("TANKHOR_HTTP_MAX_BODY_BYTES")
            .ok_or(TransportPolicyError::MissingMaxBodyBytes)?;
        let max_body_bytes = parse_bounded_positive(raw_body, MAX_ALLOWED_BODY_BYTES)
            .ok_or(TransportPolicyError::InvalidMaxBodyBytes)?;

        let raw_inflight = map
            .get("TANKHOR_HTTP_MAX_INFLIGHT")
            .ok_or(TransportPolicyError::MissingMaxInflight)?;
        let max_inflight = parse_bounded_positive(raw_inflight, MAX_ALLOWED_INFLIGHT)
            .ok_or(TransportPolicyError::InvalidMaxInflight)?;

        Self::new(max_body_bytes, max_inflight)
    }

    #[must_use]
    pub const fn max_body_bytes(self) -> NonZeroUsize {
        self.max_body_bytes
    }

    #[must_use]
    pub const fn max_inflight(self) -> NonZeroUsize {
        self.max_inflight
    }
}

fn parse_bounded_positive(raw: &str, max_allowed: usize) -> Option<NonZeroUsize> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let parsed = raw.parse::<usize>().ok()?;
    if parsed > max_allowed {
        return None;
    }
    NonZeroUsize::new(parsed)
}
