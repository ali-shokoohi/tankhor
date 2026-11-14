//! Embedded-IANA-zone schedule parsing with explicit current time.

use chrono::{LocalResult, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use kernel::error::{DomainError, ErrorCode};
use std::str::FromStr;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTime {
    utc: OffsetDateTime,
    zone: String,
}

impl ScheduledTime {
    /// # Errors
    /// Rejects unknown zones, invalid local timestamps, DST gaps/ambiguities, or non-future times.
    pub fn from_console(
        local: &str,
        zone: Option<&str>,
        now: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let zone = zone.unwrap_or("Asia/Tehran");
        let timezone = Tz::from_str(zone).map_err(|_| invalid())?;
        let local =
            NaiveDateTime::parse_from_str(local, "%Y-%m-%dT%H:%M").map_err(|_| invalid())?;
        let resolved = match timezone.from_local_datetime(&local) {
            LocalResult::Single(value) => value,
            LocalResult::Ambiguous(_, _) | LocalResult::None => return Err(invalid()),
        };
        let nanos = i128::from(resolved.timestamp()) * 1_000_000_000
            + i128::from(resolved.timestamp_subsec_nanos());
        let utc = OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| invalid())?;
        Self::new_submission(utc, zone.to_owned(), now)
    }

    /// # Errors
    /// Rejects non-RFC3339 strings, missing explicit offsets, or non-future times.
    pub fn from_json(value: &str, now: OffsetDateTime) -> Result<Self, DomainError> {
        let bytes = value.as_bytes();
        let explicit_offset = value.ends_with('Z')
            || (bytes.len() >= 6
                && matches!(bytes[bytes.len() - 6], b'+' | b'-')
                && bytes[bytes.len() - 3] == b':');
        if bytes.get(10) != Some(&b'T') || !explicit_offset {
            return Err(invalid());
        }
        let utc = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| invalid())?;
        Self::new_submission(utc, "UTC".to_owned(), now)
    }

    fn new_submission(
        utc: OffsetDateTime,
        zone: String,
        now: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        if utc <= now {
            return Err(invalid());
        }
        Ok(Self { utc, zone })
    }

    #[must_use]
    pub const fn authorized_execution(utc: OffsetDateTime, zone: String) -> Self {
        Self { utc, zone }
    }

    #[must_use]
    pub const fn utc(&self) -> OffsetDateTime {
        self.utc
    }

    #[must_use]
    pub fn zone(&self) -> &str {
        &self.zone
    }
}

const fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::ScheduledTime;
    use time::macros::datetime;

    const NOW: time::OffsetDateTime = datetime!(2026-01-01 00:00 UTC);

    #[test]
    fn schedule_time_rejects_ambiguous_nonexistent_or_past() {
        let tehran = ScheduledTime::from_console("2026-01-01T04:30", None, NOW).unwrap();
        assert_eq!(tehran.utc(), datetime!(2026-01-01 01:00 UTC));
        assert_eq!(tehran.zone(), "Asia/Tehran");

        for valid in [
            "2026-03-29T01:30",
            "2026-03-29T03:30",
            "2026-10-25T01:30",
            "2026-10-25T03:30",
        ] {
            assert!(ScheduledTime::from_console(valid, Some("Europe/Berlin"), NOW).is_ok());
        }
        assert!(
            ScheduledTime::from_console("2026-03-29T02:30", Some("Europe/Berlin"), NOW).is_err()
        );
        assert!(
            ScheduledTime::from_console("2026-10-25T02:30", Some("Europe/Berlin"), NOW).is_err()
        );
        assert!(ScheduledTime::from_console("2026-01-01T00:00", Some("UTC"), NOW).is_err());
        assert!(ScheduledTime::from_console("2025-12-31T23:59", Some("UTC"), NOW).is_err());
        assert!(ScheduledTime::from_console("2026-01-01T00:01", Some("UTC"), NOW).is_ok());
        assert!(ScheduledTime::from_console("2026-01-01 00:01", Some("UTC"), NOW).is_err());
        assert!(ScheduledTime::from_console("2026-01-01T00:01", Some("Not/AZone"), NOW).is_err());

        let overdue = ScheduledTime::authorized_execution(
            datetime!(2025-01-01 00:00 UTC),
            "Asia/Tehran".to_owned(),
        );
        assert_eq!(overdue.utc(), datetime!(2025-01-01 00:00 UTC));
    }

    #[test]
    fn json_schedule_time_requires_explicit_offset() {
        let z = ScheduledTime::from_json("2026-06-01T12:00:00Z", NOW).unwrap();
        let positive = ScheduledTime::from_json("2026-06-01T17:30:00+05:30", NOW).unwrap();
        let negative = ScheduledTime::from_json("2026-06-01T08:00:00-04:00", NOW).unwrap();
        assert_eq!(z.utc(), positive.utc());
        assert_eq!(z.utc(), negative.utc());
        assert_eq!(z.utc(), datetime!(2026-06-01 12:00 UTC));
        for invalid in [
            "2026-06-01T12:00:00",
            "2026-06-01 12:00:00Z",
            "not-a-time",
            "2025-06-01T12:00:00Z",
            "2026-01-01T00:00:00Z",
        ] {
            assert!(
                ScheduledTime::from_json(invalid, NOW).is_err(),
                "accepted {invalid}"
            );
        }
    }
}
