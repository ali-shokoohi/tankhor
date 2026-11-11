//! Non-disableable operator authentication policy (FR-002/011, Q09/Q23).
//!
//! Values are explicit operator input, never editable business settings or defaults.

use kernel::error::{DomainError, ErrorCode};
use std::{num::NonZeroU64, time::Duration};

/// Unvalidated, optional operator abuse-budget input.
#[derive(Clone)]
pub struct RateLimitInput {
    pub max_attempts: Option<i64>,
    pub window_seconds: Option<i64>,
}

/// Unvalidated operator input; absence never means an unlimited check.
/// Deliberately has no diagnostic representation for raw configuration.
#[derive(Clone)]
pub struct SecurityPolicyInput {
    pub otp_ttl_seconds: Option<i64>,
    pub primary_proof_ttl_seconds: Option<i64>,
    pub ceremony_ttl_seconds: Option<i64>,
    pub enrollment_ttl_seconds: Option<i64>,
    pub session_idle_ttl_seconds: Option<i64>,
    pub session_absolute_ttl_seconds: Option<i64>,
    pub otp_max_attempts: Option<i64>,
    pub subject_rate_limit: Option<RateLimitInput>,
    pub origin_rate_limit: Option<RateLimitInput>,
    pub rp_id: Option<String>,
    pub origin: Option<String>,
}

/// Finite positive abuse budget and window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    max_attempts: NonZeroU64,
    window: Duration,
}

impl RateLimit {
    #[must_use]
    pub const fn max_attempts(self) -> NonZeroU64 {
        self.max_attempts
    }

    #[must_use]
    pub const fn window(self) -> Duration {
        self.window
    }
}

/// Validated, immutable policy with mandatory primary and second-factor checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityPolicy {
    otp_ttl: Duration,
    primary_proof_ttl: Duration,
    ceremony_ttl: Duration,
    enrollment_ttl: Duration,
    session_idle_ttl: Duration,
    session_absolute_ttl: Duration,
    otp_max_attempts: NonZeroU64,
    subject_rate_limit: RateLimit,
    origin_rate_limit: RateLimit,
    rp_id: String,
    origin: String,
}

impl TryFrom<SecurityPolicyInput> for SecurityPolicy {
    type Error = DomainError;

    fn try_from(input: SecurityPolicyInput) -> Result<Self, Self::Error> {
        let rp_id = input.rp_id.ok_or_else(invalid)?;
        let origin = input.origin.ok_or_else(invalid)?;
        validate_origin(&rp_id, &origin)?;
        let policy = Self {
            otp_ttl: duration(input.otp_ttl_seconds)?,
            primary_proof_ttl: duration(input.primary_proof_ttl_seconds)?,
            ceremony_ttl: duration(input.ceremony_ttl_seconds)?,
            enrollment_ttl: duration(input.enrollment_ttl_seconds)?,
            session_idle_ttl: duration(input.session_idle_ttl_seconds)?,
            session_absolute_ttl: duration(input.session_absolute_ttl_seconds)?,
            otp_max_attempts: positive(input.otp_max_attempts)?,
            subject_rate_limit: rate_limit(input.subject_rate_limit)?,
            origin_rate_limit: rate_limit(input.origin_rate_limit)?,
            rp_id,
            origin,
        };
        if policy.session_idle_ttl > policy.session_absolute_ttl {
            return Err(invalid());
        }
        Ok(policy)
    }
}

impl SecurityPolicy {
    #[must_use]
    pub const fn otp_ttl(&self) -> Duration {
        self.otp_ttl
    }

    #[must_use]
    pub const fn primary_proof_ttl(&self) -> Duration {
        self.primary_proof_ttl
    }

    #[must_use]
    pub const fn ceremony_ttl(&self) -> Duration {
        self.ceremony_ttl
    }

    #[must_use]
    pub const fn enrollment_ttl(&self) -> Duration {
        self.enrollment_ttl
    }

    #[must_use]
    pub const fn session_idle_ttl(&self) -> Duration {
        self.session_idle_ttl
    }

    #[must_use]
    pub const fn session_absolute_ttl(&self) -> Duration {
        self.session_absolute_ttl
    }

    #[must_use]
    pub const fn otp_max_attempts(&self) -> NonZeroU64 {
        self.otp_max_attempts
    }

    #[must_use]
    pub const fn subject_rate_limit(&self) -> RateLimit {
        self.subject_rate_limit
    }

    #[must_use]
    pub const fn origin_rate_limit(&self) -> RateLimit {
        self.origin_rate_limit
    }

    #[must_use]
    pub fn rp_id(&self) -> &str {
        &self.rp_id
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

fn positive(value: Option<i64>) -> Result<NonZeroU64, DomainError> {
    value
        .and_then(|value| u64::try_from(value).ok())
        .and_then(NonZeroU64::new)
        .ok_or_else(invalid)
}

fn duration(value: Option<i64>) -> Result<Duration, DomainError> {
    Ok(Duration::from_secs(positive(value)?.get()))
}

fn rate_limit(input: Option<RateLimitInput>) -> Result<RateLimit, DomainError> {
    let input = input.ok_or_else(invalid)?;
    Ok(RateLimit {
        max_attempts: positive(input.max_attempts)?,
        window: duration(input.window_seconds)?,
    })
}

// Only a serialized HTTPS DNS origin is accepted: no URL normalization that could
// silently discard a path, credentials, fragment, whitespace or wildcard.
fn validate_origin(rp_id: &str, origin: &str) -> Result<(), DomainError> {
    let authority = origin.strip_prefix("https://").ok_or_else(invalid)?;
    let host = if let Some((host, port)) = authority.split_once(':') {
        if port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port.parse::<u16>().ok().is_none_or(|port| port == 0)
        {
            return Err(invalid());
        }
        host
    } else {
        authority
    };
    if !valid_dns_name(rp_id) || !valid_dns_name(host) || host != rp_id {
        return Err(invalid());
    }
    Ok(())
}

fn valid_dns_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::{RateLimitInput, SecurityPolicy, SecurityPolicyInput};
    use kernel::error::{DomainError, ErrorCode};
    use std::time::Duration;

    // Explicit operator fixtures, not business defaults (FR-002/011, Q09/Q23).
    fn fixture() -> SecurityPolicyInput {
        SecurityPolicyInput {
            otp_ttl_seconds: Some(60),
            primary_proof_ttl_seconds: Some(90),
            ceremony_ttl_seconds: Some(120),
            enrollment_ttl_seconds: Some(600),
            session_idle_ttl_seconds: Some(300),
            session_absolute_ttl_seconds: Some(900),
            otp_max_attempts: Some(3),
            subject_rate_limit: Some(RateLimitInput {
                max_attempts: Some(5),
                window_seconds: Some(60),
            }),
            origin_rate_limit: Some(RateLimitInput {
                max_attempts: Some(10),
                window_seconds: Some(120),
            }),
            rp_id: Some("console.example.test".into()),
            origin: Some("https://console.example.test".into()),
        }
    }

    #[test]
    fn security_policy_is_finite_and_nondisableable() {
        let policy = SecurityPolicy::try_from(fixture()).unwrap();
        assert_eq!(policy.otp_ttl(), Duration::from_secs(60));
        assert_eq!(policy.primary_proof_ttl(), Duration::from_secs(90));
        assert_eq!(policy.ceremony_ttl(), Duration::from_secs(120));
        assert_eq!(policy.enrollment_ttl(), Duration::from_secs(600));
        assert_eq!(policy.session_idle_ttl(), Duration::from_secs(300));
        assert_eq!(policy.session_absolute_ttl(), Duration::from_mins(15));
        assert_eq!(policy.otp_max_attempts().get(), 3);
        assert_eq!(policy.subject_rate_limit().max_attempts().get(), 5);
        assert_eq!(
            policy.subject_rate_limit().window(),
            Duration::from_secs(60)
        );
        assert_eq!(policy.origin_rate_limit().max_attempts().get(), 10);
        assert_eq!(
            policy.origin_rate_limit().window(),
            Duration::from_secs(120)
        );
        assert_eq!(policy.rp_id(), "console.example.test");
        assert_eq!(policy.origin(), "https://console.example.test");

        let invalid = Err(DomainError::validation(ErrorCode::InvalidValue));
        for field in 0..11 {
            for value in [None, Some(0), Some(-1)] {
                let mut input = fixture();
                match field {
                    0 => input.otp_ttl_seconds = value,
                    1 => input.primary_proof_ttl_seconds = value,
                    2 => input.ceremony_ttl_seconds = value,
                    3 => input.enrollment_ttl_seconds = value,
                    4 => input.session_idle_ttl_seconds = value,
                    5 => input.session_absolute_ttl_seconds = value,
                    6 => input.otp_max_attempts = value,
                    7 => input.subject_rate_limit.as_mut().unwrap().max_attempts = value,
                    8 => input.subject_rate_limit.as_mut().unwrap().window_seconds = value,
                    9 => input.origin_rate_limit.as_mut().unwrap().max_attempts = value,
                    10 => input.origin_rate_limit.as_mut().unwrap().window_seconds = value,
                    _ => unreachable!(),
                }
                assert_eq!(SecurityPolicy::try_from(input), invalid, "field {field}");
            }
        }
        for subject in [true, false] {
            let mut input = fixture();
            if subject {
                input.subject_rate_limit = None;
            } else {
                input.origin_rate_limit = None;
            }
            assert_eq!(SecurityPolicy::try_from(input), invalid);
        }
        let mut input = fixture();
        input.session_idle_ttl_seconds = Some(901);
        assert_eq!(SecurityPolicy::try_from(input), invalid);

        assert_invalid_origins();
        assert_invalid_rp_ids();
        let mut input = fixture();
        input.origin = Some("https://console.example.test:8443".into());
        assert_eq!(
            SecurityPolicy::try_from(input).unwrap().origin(),
            "https://console.example.test:8443"
        );
        let mut input = fixture();
        input.session_idle_ttl_seconds = Some(900);
        assert!(SecurityPolicy::try_from(input).is_ok());

        // Rejected configuration cannot turn into a diagnostic data leak.
        let mut input = fixture();
        input.origin = Some("https://user:secret-canary@console.example.test".into());
        let error = SecurityPolicy::try_from(input).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("secret-canary"));
    }

    fn assert_invalid_origins() {
        for origin in [
            None,
            Some(""),
            Some("http://console.example.test"),
            Some("https://*.example.test"),
            Some("https://console.example.test/console"),
            Some("https://console.example.test?secret=canary"),
            Some("https://console.example.test#fragment"),
            Some("https://user:secret@console.example.test"),
            Some("https://console.example.test.evil.test"),
            Some("https://console.example.test:0"),
            Some("https://console.example.test:65536"),
            Some("https://console.example.test:"),
            Some("https://console.example.test:abc"),
            Some("https://console.example.test/"),
            Some("https://console.example.test\\evil"),
            Some(" https://console.example.test"),
            Some("https://console.example.test\n"),
        ] {
            let mut input = fixture();
            input.origin = origin.map(str::to_owned);
            assert_eq!(
                SecurityPolicy::try_from(input),
                Err(DomainError::validation(ErrorCode::InvalidValue)),
                "unsafe origin"
            );
        }
    }

    fn assert_invalid_rp_ids() {
        let invalid = Err(DomainError::validation(ErrorCode::InvalidValue));
        for rp_id in [
            None,
            Some(""),
            Some("*.example.test"),
            Some("https://console.example.test"),
            Some("console.example.test:443"),
            Some("console.example.test/"),
            Some("evil.test"),
            Some("-console.example.test"),
            Some("console..example.test"),
            Some("console.example.test."),
            Some("console_example.test"),
        ] {
            let mut input = fixture();
            input.rp_id = rp_id.map(str::to_owned);
            assert_eq!(SecurityPolicy::try_from(input), invalid, "unsafe RP");
        }
        for host in [format!("{}.test", "a".repeat(64)), "a.".repeat(128)] {
            let mut input = fixture();
            input.rp_id = Some(host.clone());
            input.origin = Some(format!("https://{host}"));
            assert_eq!(SecurityPolicy::try_from(input), invalid);
        }
    }
}
