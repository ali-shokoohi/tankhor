//! Single-subject change drafts and immutable, head-CAS revisions.

use super::{
    flag::FlagKey, formats::FormatSet, scheduled_time::ScheduledTime, settings::SettingKey,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, ChangeId, RevisionId, VersionId},
};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeSubject {
    Setting(SettingKey),
    Flag(FlagKey),
}

impl ChangeSubject {
    /// # Errors
    /// Requires exactly one setting or flag subject.
    pub fn new(setting: Option<SettingKey>, flag: Option<FlagKey>) -> Result<Self, DomainError> {
        match (setting, flag) {
            (Some(setting), None) => Ok(Self::Setting(setting)),
            (None, Some(flag)) => Ok(Self::Flag(flag)),
            _ => Err(invalid()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeValue {
    Number(u32),
    Formats(FormatSet),
    Boolean(bool),
}

impl ChangeValue {
    const fn matches(self, subject: ChangeSubject) -> bool {
        matches!(
            (subject, self),
            (
                ChangeSubject::Setting(SettingKey::UploadFormats),
                Self::Formats(_)
            ) | (ChangeSubject::Flag(_), Self::Boolean(_))
                | (ChangeSubject::Setting(_), Self::Number(_))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRevision {
    id: RevisionId,
    change_id: ChangeId,
    revision_number: u32,
    value: ChangeValue,
    scheduled_at: Option<ScheduledTime>,
    created_by: AdminId,
    created_at: OffsetDateTime,
}

impl ChangeRevision {
    #[must_use]
    pub const fn new(
        id: RevisionId,
        change_id: ChangeId,
        revision_number: u32,
        value: ChangeValue,
        scheduled_at: Option<ScheduledTime>,
        created_by: AdminId,
        created_at: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            change_id,
            revision_number,
            value,
            scheduled_at,
            created_by,
            created_at,
        }
    }

    #[must_use]
    pub const fn id(&self) -> RevisionId {
        self.id
    }

    #[must_use]
    pub const fn change_id(&self) -> ChangeId {
        self.change_id
    }

    #[must_use]
    pub const fn revision_number(&self) -> u32 {
        self.revision_number
    }

    #[must_use]
    pub const fn value(&self) -> ChangeValue {
        self.value
    }

    #[must_use]
    pub const fn scheduled_at(&self) -> Option<&ScheduledTime> {
        self.scheduled_at.as_ref()
    }

    #[must_use]
    pub const fn created_by(&self) -> AdminId {
        self.created_by
    }

    #[must_use]
    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDraft {
    id: ChangeId,
    subject: ChangeSubject,
    baseline_version: VersionId,
    creator: AdminId,
    revisions: Vec<ChangeRevision>,
    head: Option<RevisionId>,
}

impl ChangeDraft {
    #[must_use]
    pub const fn new(
        id: ChangeId,
        subject: ChangeSubject,
        baseline_version: VersionId,
        creator: AdminId,
    ) -> Self {
        Self {
            id,
            subject,
            baseline_version,
            creator,
            revisions: Vec::new(),
            head: None,
        }
    }

    /// # Errors
    /// Rejects stale heads, mismatched change IDs, non-sequential numbers, or invalid subject values.
    pub fn append_revision(
        &self,
        revision: ChangeRevision,
        expected_head: Option<RevisionId>,
    ) -> Result<Self, DomainError> {
        let expected_number = u32::try_from(self.revisions.len())
            .ok()
            .and_then(|len| len.checked_add(1));
        if self.head != expected_head
            || revision.change_id != self.id
            || Some(revision.revision_number) != expected_number
        {
            return Err(DomainError::conflict(ErrorCode::StaleRevision));
        }
        if !revision.value.matches(self.subject) {
            return Err(invalid());
        }
        let mut next = self.clone();
        next.head = Some(revision.id);
        next.revisions.push(revision);
        Ok(next)
    }

    #[must_use]
    pub const fn id(&self) -> ChangeId {
        self.id
    }

    #[must_use]
    pub const fn subject(&self) -> ChangeSubject {
        self.subject
    }

    #[must_use]
    pub const fn baseline_version(&self) -> VersionId {
        self.baseline_version
    }

    #[must_use]
    pub const fn creator(&self) -> AdminId {
        self.creator
    }

    #[must_use]
    pub const fn head(&self) -> Option<RevisionId> {
        self.head
    }

    #[must_use]
    pub fn current(&self) -> Option<&ChangeRevision> {
        self.revisions.last()
    }

    #[must_use]
    pub fn revisions(&self) -> &[ChangeRevision] {
        &self.revisions
    }
}

const fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::{ChangeDraft, ChangeRevision, ChangeSubject, ChangeValue};
    use crate::domain::{flag::FlagKey, settings::SettingKey};
    use kernel::{
        error::ErrorCode,
        ids::{AdminId, ChangeId, RevisionId, VersionId},
    };
    use std::str::FromStr;
    use time::macros::datetime;

    fn id<T: FromStr>(value: &str) -> T {
        value.parse().ok().unwrap()
    }

    #[test]
    fn single_subject_revision_is_immutable_and_head_cas_is_explicit() {
        assert!(
            ChangeSubject::new(Some(SettingKey::StudioMaxCards), Some(FlagKey::Ticketing)).is_err()
        );
        assert!(ChangeSubject::new(None, None).is_err());
        let subject = ChangeSubject::new(Some(SettingKey::StudioMaxCards), None).unwrap();
        let change_id = id::<ChangeId>("018f2f7c-8a3c-7000-9000-000000000001");
        let baseline = id::<VersionId>("018f2f7c-8a3c-7000-9000-000000000002");
        let creator = id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a1");
        let editor = id::<AdminId>("018f2f7c-8a3c-7000-9000-0000000000a2");
        let draft = ChangeDraft::new(change_id, subject, baseline, creator);
        let revision = ChangeRevision::new(
            id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000003"),
            change_id,
            1,
            ChangeValue::Number(25),
            None,
            creator,
            datetime!(2026-01-01 00:00 UTC),
        );
        let revised = draft.append_revision(revision.clone(), None).unwrap();
        assert_eq!(draft.head(), None);
        assert_eq!(revised.head(), Some(revised.current().unwrap().id()));
        assert_eq!(revised.current().unwrap().value(), ChangeValue::Number(25));
        assert_eq!(revised.subject(), subject);
        assert_eq!(revised.baseline_version(), baseline);
        assert_eq!(draft.creator(), creator);
        assert_eq!(revised.creator(), creator);
        assert_eq!(revised.current().unwrap().created_by(), creator);

        let edited = revised
            .append_revision(
                ChangeRevision::new(
                    id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000005"),
                    change_id,
                    2,
                    ChangeValue::Number(26),
                    None,
                    editor,
                    datetime!(2026-01-01 00:02 UTC),
                ),
                revised.head(),
            )
            .unwrap();
        assert_eq!(edited.creator(), creator);
        assert_eq!(edited.revisions()[0].created_by(), creator);
        assert_eq!(edited.current().unwrap().created_by(), editor);

        let second = ChangeRevision::new(
            id::<RevisionId>("018f2f7c-8a3c-7000-9000-000000000004"),
            change_id,
            2,
            ChangeValue::Number(30),
            None,
            editor,
            datetime!(2026-01-01 00:01 UTC),
        );
        let error = revised.append_revision(second, None).unwrap_err();
        assert_eq!(error.code(), ErrorCode::StaleRevision);
        assert_eq!(revised.current().unwrap().revision_number(), 1);

        let flag_subject = ChangeSubject::new(None, Some(FlagKey::Ticketing)).unwrap();
        let flag_draft = ChangeDraft::new(change_id, flag_subject, baseline, creator);
        assert!(flag_draft.append_revision(revision, None).is_err());
    }
}
