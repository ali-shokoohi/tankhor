//! Append-only audit persistence through the caller's shared transaction.

use crate::domain::audit::{Actor, AuditAction, AuditEntry, Subject};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::AuditId,
    ports::unit_of_work::TxHandle,
};
use std::num::NonZeroU16;
use time::OffsetDateTime;

/// Stable ascending `(occurred_at, id)` cursor; neither member is a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditCursor {
    pub occurred_at: OffsetDateTime,
    pub id: AuditId,
}

/// Optional exact filters and a half-open UTC time range `[from, until)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFilter {
    actor: Option<Actor>,
    action: Option<AuditAction>,
    subject: Option<Subject>,
    from: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
}

impl AuditFilter {
    /// # Errors
    /// Refuses an empty or backwards time range.
    pub fn new(
        actor: Option<Actor>,
        action: Option<AuditAction>,
        subject: Option<Subject>,
        from: Option<OffsetDateTime>,
        until: Option<OffsetDateTime>,
    ) -> Result<Self, DomainError> {
        if matches!((from, until), (Some(start), Some(end)) if end <= start) {
            return Err(invalid());
        }
        Ok(Self {
            actor,
            action,
            subject,
            from,
            until,
        })
    }

    #[must_use]
    pub const fn actor(&self) -> Option<Actor> {
        self.actor
    }

    #[must_use]
    pub const fn action(&self) -> Option<AuditAction> {
        self.action
    }

    #[must_use]
    pub const fn subject(&self) -> Option<Subject> {
        self.subject
    }

    #[must_use]
    pub const fn from(&self) -> Option<OffsetDateTime> {
        self.from
    }

    #[must_use]
    pub const fn until(&self) -> Option<OffsetDateTime> {
        self.until
    }
}

/// Explicit finite page size; no unbounded search operation or implicit defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditQuery {
    filter: AuditFilter,
    after: Option<AuditCursor>,
    limit: NonZeroU16,
}

impl AuditQuery {
    /// # Errors
    /// Refuses zero-sized pages.
    pub fn new(
        filter: AuditFilter,
        after: Option<AuditCursor>,
        limit: u16,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            filter,
            after,
            limit: NonZeroU16::new(limit).ok_or_else(invalid)?,
        })
    }

    #[must_use]
    pub const fn filter(&self) -> &AuditFilter {
        &self.filter
    }

    #[must_use]
    pub const fn after(&self) -> Option<AuditCursor> {
        self.after
    }

    #[must_use]
    pub const fn limit(&self) -> u16 {
        self.limit.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditPage {
    pub entries: Vec<AuditEntry>,
    pub next: Option<AuditCursor>,
}

/// Append and query only. Implementations preserve event-key uniqueness and
/// participate in the caller's transaction; they never open or commit a substitute.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait AuditRepository: Send + Sync {
    /// Append once by stable event key; incompatible reuse is a typed conflict.
    async fn append(&self, tx: &mut TxHandle, entry: AuditEntry) -> Result<(), DomainError>;

    /// Search in ascending cursor order within the explicit page bound.
    async fn search(&self, tx: &mut TxHandle, query: AuditQuery) -> Result<AuditPage, DomainError>;
}

fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::{
        AuditCursor, AuditFilter, AuditPage, AuditQuery, AuditRepository, MockAuditRepository,
    };
    use crate::domain::audit::{
        Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, Factor, ReasonCode, Subject,
    };
    use kernel::{
        error::{DomainError, ErrorCode},
        ids::{AdminId, AuditId},
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn entry() -> AuditEntry {
        let actor = AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap();
        AuditEntry::try_from(AuditInput {
            id: AuditId::from_str("019a53a0-0000-7000-8000-000000000010").unwrap(),
            event_key: AuditId::from_str("019a53a0-0000-7000-8000-000000000011").unwrap(),
            actor: Some(Actor::Admin(actor)),
            action: Some(AuditAction::SignedIn),
            subject: Some(Subject::Admin(actor)),
            occurred_at: Some(OffsetDateTime::UNIX_EPOCH),
            reason: Some(ReasonCode::Authentication),
            metadata: AuditMetadata::Factor(Factor::Passkey),
        })
        .unwrap()
    }

    #[tokio::test]
    async fn audit_repository_port_is_mockable_and_append_only() {
        let event = entry();
        let cursor = AuditCursor {
            occurred_at: event.occurred_at(),
            id: event.id(),
        };
        let filter = AuditFilter::new(
            Some(event.actor()),
            Some(event.action()),
            Some(event.subject()),
            Some(event.occurred_at()),
            Some(event.occurred_at() + Duration::SECOND),
        )
        .unwrap();
        assert_eq!(filter.actor(), Some(event.actor()));
        assert_eq!(filter.action(), Some(event.action()));
        assert_eq!(filter.subject(), Some(event.subject()));
        assert_eq!(filter.from(), Some(event.occurred_at()));
        assert_eq!(filter.until(), Some(event.occurred_at() + Duration::SECOND));
        let query = AuditQuery::new(filter.clone(), Some(cursor), 2).unwrap();
        assert_eq!(query.limit(), 2);
        assert_eq!(query.after(), Some(cursor));
        assert_eq!(query.filter(), &filter);
        assert!(AuditQuery::new(filter, None, 0).is_err());
        for until in [event.occurred_at(), event.occurred_at() - Duration::SECOND] {
            assert!(
                AuditFilter::new(None, None, None, Some(event.occurred_at()), Some(until)).is_err()
            );
        }
        let store = StoreId::new();
        let mut tx = TxHandle::new(store, TransactionMode::ReadWrite, ());
        let expected = event.clone();
        let mut mock = MockAuditRepository::new();
        mock.expect_append()
            .times(1)
            .withf(move |handle, value| handle.store_id() == store && value == &expected)
            .returning(|_, _| Ok(()));
        let expected_query = query.clone();
        let returned = event.clone();
        mock.expect_search()
            .times(1)
            .withf(move |handle, value| handle.store_id() == store && value == &expected_query)
            .returning(move |_, _| {
                Ok(AuditPage {
                    entries: vec![returned.clone()],
                    next: Some(cursor),
                })
            });
        let repo: &dyn AuditRepository = &mock;
        repo.append(&mut tx, event.clone()).await.unwrap();
        let page = repo.search(&mut tx, query.clone()).await.unwrap();
        assert_eq!(page.entries, vec![event]);
        assert_eq!(page.next, Some(cursor));
        let mut failing = MockAuditRepository::new();
        failing
            .expect_append()
            .times(1)
            .returning(|_, _| Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)));
        assert_eq!(
            failing.append(&mut tx, entry()).await.unwrap_err().code(),
            ErrorCode::PrerequisiteUnavailable
        );
        let production = include_str!("audit_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let methods: Vec<_> = production
            .split("async fn ")
            .skip(1)
            .map(|part| part.split('(').next().unwrap().trim())
            .collect();
        assert_eq!(
            methods,
            ["append", "search"],
            "the audit port must not expose edit/delete operations"
        );
    }
}
