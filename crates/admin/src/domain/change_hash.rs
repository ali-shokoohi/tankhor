//! Canonical digest of reviewed draft content; never the revision primary key.

use super::change_revision::{ChangeDraft, ChangeSubject, ChangeValue};
use kernel::error::{DomainError, ErrorCode};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RevisionDigest([u8; 32]);

impl RevisionDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for RevisionDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RevisionDigest")
            .field(&self.0)
            .finish()
    }
}

/// # Errors
/// Rejects drafts without a current revision.
pub fn draft_hash(draft: &ChangeDraft) -> Result<RevisionDigest, DomainError> {
    let revision = draft
        .current()
        .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))?;
    let mut hash = Sha256::new();
    match draft.subject() {
        ChangeSubject::Setting(key) => {
            hash.update(b"setting\0");
            hash.update(key.as_str().as_bytes());
        }
        ChangeSubject::Flag(key) => {
            hash.update(b"flag\0");
            hash.update(key.as_str().as_bytes());
        }
    }
    hash.update(draft.baseline_version().as_uuid().as_bytes());
    match revision.value() {
        ChangeValue::Number(value) => {
            hash.update([0]);
            hash.update(value.to_be_bytes());
        }
        ChangeValue::Formats(value) => {
            hash.update([1, value.mask()]);
        }
        ChangeValue::Boolean(value) => {
            hash.update([2, u8::from(value)]);
        }
    }
    if let Some(schedule) = revision.scheduled_at() {
        hash.update([1]);
        hash.update(schedule.utc().unix_timestamp_nanos().to_be_bytes());
        hash.update((schedule.zone().len() as u64).to_be_bytes());
        hash.update(schedule.zone().as_bytes());
    } else {
        hash.update([0]);
    }
    Ok(RevisionDigest(hash.finalize().into()))
}

#[cfg(test)]
mod tests {
    use super::draft_hash;
    use crate::domain::{
        change_revision::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue},
        formats::FormatSet,
        scheduled_time::ScheduledTime,
        settings::SettingKey,
    };
    use kernel::ids::{AdminId, ChangeId, RevisionId, VersionId};
    use std::str::FromStr;
    use time::macros::datetime;

    fn draft(
        change_suffix: &str,
        value: ChangeValue,
        schedule: Option<ScheduledTime>,
    ) -> ChangeDraft {
        let change_id = ChangeId::from_str(&format!(
            "018f2f7c-8a3c-7000-9000-0000000000{change_suffix}"
        ))
        .unwrap();
        let revision_id = RevisionId::from_str("018f2f7c-8a3c-7000-9000-000000000101").unwrap();
        let baseline = VersionId::from_str("018f2f7c-8a3c-7000-9000-000000000102").unwrap();
        let subject = ChangeSubject::new(Some(SettingKey::UploadFormats), None).unwrap();
        ChangeDraft::new(
            change_id,
            subject,
            baseline,
            AdminId::from_str("018f2f7c-8a3c-7000-9000-0000000000a1").unwrap(),
        )
        .append_revision(
            ChangeRevision::new(
                revision_id,
                change_id,
                1,
                value,
                schedule,
                AdminId::from_str("018f2f7c-8a3c-7000-9000-0000000000a1").unwrap(),
                datetime!(2026-01-01 00:00 UTC),
            ),
            None,
        )
        .unwrap()
    }

    #[test]
    fn draft_hash_is_canonical() {
        let now = datetime!(2026-01-01 00:00 UTC);
        let formats_a = ChangeValue::Formats(FormatSet::from_str("JPEG,PNG,WEBP,HEIC").unwrap());
        let formats_b = ChangeValue::Formats(FormatSet::from_str("HEIC,WEBP,PNG,JPEG").unwrap());
        assert_eq!(
            draft_hash(&draft("01", formats_a, None)),
            draft_hash(&draft("02", formats_b, None))
        );

        let z = ScheduledTime::from_json("2026-06-01T12:00:00Z", now).unwrap();
        let offset = ScheduledTime::from_json("2026-06-01T17:30:00+05:30", now).unwrap();
        assert_eq!(
            draft_hash(&draft("03", formats_a, Some(z.clone()))),
            draft_hash(&draft("04", formats_a, Some(offset)))
        );

        let tehran =
            ScheduledTime::from_console("2026-06-01T15:30", Some("Asia/Tehran"), now).unwrap();
        assert_eq!(z.utc(), tehran.utc());
        assert_ne!(
            draft_hash(&draft("05", formats_a, Some(z.clone()))),
            draft_hash(&draft("06", formats_a, Some(tehran)))
        );
        let later = ScheduledTime::from_json("2026-06-01T12:00:01Z", now).unwrap();
        assert_ne!(
            draft_hash(&draft("07", formats_a, Some(z))),
            draft_hash(&draft("08", formats_a, Some(later)))
        );
        let jpeg_only = ChangeValue::Formats(FormatSet::from_str("JPEG").unwrap());
        assert_ne!(
            draft_hash(&draft("09", formats_a, None)),
            draft_hash(&draft("10", jpeg_only, None))
        );
        assert!(
            ScheduledTime::from_console("2026-03-29T02:30", Some("Europe/Berlin"), now).is_err()
        );
    }
}
