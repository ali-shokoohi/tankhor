//! FR-021–024, AC-ADM-021–025: identical assertions for fake and real repositories.

use admin::domain::audit::{
    Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, DraftMetadata, Executor, Factor,
    PreviewMetadata, ReasonCode, Subject,
};
use admin::ports::audit_repository::{AuditCursor, AuditFilter, AuditQuery, AuditRepository};
use kernel::error::{DomainError, ErrorCategory};
use kernel::ids::{AdminId, AuditId, ChangeId, PublicationId, RevisionId, VersionId};
use kernel::ports::unit_of_work::{TransactionMode, TxHandle, UnitOfWork};
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

/// A fresh isolated backend, fault control and a companion transactional resource.
/// No assertion inspects backend tables, fake maps, or adapter-specific payloads.
#[async_trait::async_trait]
pub trait AuditRepositoryContractHarness: Send + Sync {
    type Uow: UnitOfWork;
    type Repository: AuditRepository;

    fn uow(&self) -> &Self::Uow;
    fn repository(&self) -> &Self::Repository;

    /// Fail exactly the next append with a dependency-unavailable error.
    async fn fail_next_append(&self) -> Result<(), DomainError>;
    async fn write_companion(&self, tx: &mut TxHandle) -> Result<(), DomainError>;
    async fn companion_exists(&self, tx: &mut TxHandle) -> Result<bool, DomainError>;
    async fn begin_foreign(&self) -> Result<TxHandle, DomainError>;
}

fn audit_id(number: u16) -> AuditId {
    AuditId::from_str(&format!("019a53a0-0000-7000-8000-{number:012x}")).unwrap()
}

fn admin(number: u16) -> AdminId {
    AdminId::from_str(&format!("019a53a0-0000-7000-8000-{number:012x}")).unwrap()
}

fn entry(number: u16, actor: u16, seconds: i64, action: AuditAction) -> AuditEntry {
    let at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds);
    let (subject, reason, metadata) = match action {
        AuditAction::SignedIn | AuditAction::EnrollmentStarted => (
            Subject::Admin(admin(actor)),
            ReasonCode::Authentication,
            AuditMetadata::Factor(Factor::Passkey),
        ),
        AuditAction::EnrollmentConfirmed => (
            Subject::Admin(admin(actor)),
            ReasonCode::Authentication,
            AuditMetadata::Enrollment {
                before: admin::domain::admin_state::Eligibility::Invited,
                after: admin::domain::admin_state::Eligibility::Active,
                factor: Factor::Passkey,
            },
        ),
        AuditAction::Invited => (
            Subject::Admin(admin(actor + 1)),
            ReasonCode::AdminAddition,
            AuditMetadata::Invitation {
                before: None,
                after: admin::domain::admin_state::Eligibility::Invited,
            },
        ),
        AuditAction::Recovered => (
            Subject::Admin(admin(2)),
            ReasonCode::AdminRecovery,
            AuditMetadata::Eligibility {
                before: true,
                after: false,
            },
        ),
        AuditAction::FoundationSeeded => (
            Subject::Admin(admin(actor)),
            ReasonCode::FoundationSeed,
            AuditMetadata::None,
        ),
        AuditAction::DraftSaved => (
            Subject::Change(ChangeId::from_str("019a53a0-0000-7000-8000-000000000097").unwrap()),
            ReasonCode::DraftChange,
            AuditMetadata::Draft(DraftMetadata::new(
                Some(admin::domain::change_revision::ChangeValue::Number(15)),
                admin::domain::change_revision::ChangeValue::Number(20),
                VersionId::from_str("019a53a0-0000-7000-8000-000000000096").unwrap(),
                RevisionId::from_str("019a53a0-0000-7000-8000-000000000095").unwrap(),
                None,
            )),
        ),
        AuditAction::Previewed => (
            Subject::Change(ChangeId::from_str("019a53a0-0000-7000-8000-000000000097").unwrap()),
            ReasonCode::PreviewAssessment,
            AuditMetadata::Preview(PreviewMetadata::new(
                RevisionId::from_str("019a53a0-0000-7000-8000-000000000095").unwrap(),
                VersionId::from_str("019a53a0-0000-7000-8000-000000000096").unwrap(),
                admin::domain::change_hash::RevisionDigest::from_bytes([7; 32]),
            )),
        ),
        AuditAction::Published | AuditAction::ScheduledCancelled => (
            Subject::Publication(
                PublicationId::from_str(if action == AuditAction::Published {
                    "019a53a0-0000-7000-8000-000000000098"
                } else {
                    "019a53a0-0000-7000-8000-000000000099"
                })
                .unwrap(),
            ),
            if action == AuditAction::Published {
                ReasonCode::ScheduledPublication
            } else {
                ReasonCode::AdminRevoked
            },
            AuditMetadata::scheduled(admin(actor), admin(3), at - Duration::SECOND, at),
        ),
    };
    AuditEntry::try_from(AuditInput {
        id: audit_id(number),
        event_key: audit_id(number + 1000),
        actor: Some(Actor::Admin(admin(actor))),
        action: Some(action),
        subject: Some(subject),
        occurred_at: Some(at),
        reason: Some(reason),
        metadata,
    })
    .unwrap()
}

fn all() -> AuditFilter {
    AuditFilter::new(None, None, None, None, None).unwrap()
}

async fn read<H: AuditRepositoryContractHarness>(h: &H, filter: AuditFilter) -> Vec<AuditEntry> {
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    let mut after = None;
    let mut result = Vec::new();
    loop {
        let page = h
            .repository()
            .search(&mut tx, AuditQuery::new(filter.clone(), after, 1).unwrap())
            .await
            .unwrap();
        assert!(page.entries.len() <= 1, "page exceeds its bound");
        for event in &page.entries {
            let key = (event.occurred_at(), event.id());
            if let Some(previous) = result.last() {
                let previous: &AuditEntry = previous;
                assert!(
                    key > (previous.occurred_at(), previous.id()),
                    "unstable cursor order"
                );
            }
            if let Some(cursor) = after {
                assert!(key > (cursor.occurred_at, cursor.id));
            }
        }
        if let Some(next) = page.next {
            let last = page
                .entries
                .last()
                .expect("cursor must advance over an entry");
            assert_eq!(
                next,
                AuditCursor {
                    occurred_at: last.occurred_at(),
                    id: last.id()
                }
            );
            assert_ne!(Some(next), after);
        }
        result.extend(page.entries);
        assert!(
            result.len() <= 20,
            "cursor failed to terminate on bounded fixture"
        );
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    h.uow().rollback(tx).await.unwrap();
    result
}

async fn verify_filters<H: AuditRepositoryContractHarness>(h: &H, events: &[AuditEntry]) {
    let at = OffsetDateTime::UNIX_EPOCH;
    let filters = [
        AuditFilter::new(Some(Actor::Admin(admin(1))), None, None, None, None).unwrap(),
        AuditFilter::new(None, Some(AuditAction::SignedIn), None, None, None).unwrap(),
        AuditFilter::new(None, None, Some(Subject::Admin(admin(2))), None, None).unwrap(),
        AuditFilter::new(
            None,
            None,
            None,
            Some(at + Duration::seconds(10)),
            Some(at + Duration::seconds(20)),
        )
        .unwrap(),
        AuditFilter::new(
            Some(Actor::Admin(admin(1))),
            Some(AuditAction::Recovered),
            Some(Subject::Admin(admin(2))),
            Some(at),
            Some(at + Duration::seconds(30)),
        )
        .unwrap(),
        AuditFilter::new(Some(Actor::Admin(admin(9))), None, None, None, None).unwrap(),
        AuditFilter::new(None, None, None, Some(at + Duration::seconds(20)), None).unwrap(),
        AuditFilter::new(None, None, None, None, Some(at + Duration::seconds(20))).unwrap(),
    ];
    for filter in filters {
        let expected: Vec<_> = events
            .iter()
            .filter(|event| {
                filter.actor().is_none_or(|value| event.actor() == value)
                    && filter.action().is_none_or(|value| event.action() == value)
                    && filter
                        .subject()
                        .is_none_or(|value| event.subject() == value)
                    && filter
                        .from()
                        .is_none_or(|value| event.occurred_at() >= value)
                    && filter
                        .until()
                        .is_none_or(|value| event.occurred_at() < value)
            })
            .cloned()
            .collect();
        assert_eq!(read(h, filter).await, expected);
    }
}

async fn verify_uniqueness<H: AuditRepositoryContractHarness>(h: &H, original: &AuditEntry) {
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.repository()
        .append(&mut tx, original.clone())
        .await
        .expect("identical retry");
    h.uow().commit(tx).await.unwrap();
    let mut conflicting = entry(100, 2, 80, AuditAction::SignedIn);
    // Preserve a valid typed payload while changing just its stable event key.
    conflicting = AuditEntry::try_from(AuditInput {
        id: conflicting.id(),
        event_key: original.event_key(),
        actor: Some(conflicting.actor()),
        action: Some(conflicting.action()),
        subject: Some(conflicting.subject()),
        occurred_at: Some(conflicting.occurred_at()),
        reason: Some(conflicting.reason()),
        metadata: conflicting.metadata().clone(),
    })
    .unwrap();
    let reused_id = AuditEntry::try_from(AuditInput {
        id: original.id(),
        event_key: audit_id(500),
        actor: Some(original.actor()),
        action: Some(original.action()),
        subject: Some(original.subject()),
        occurred_at: Some(original.occurred_at()),
        reason: Some(original.reason()),
        metadata: original.metadata().clone(),
    })
    .unwrap();
    for duplicate in [conflicting, reused_id] {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        assert_eq!(
            h.repository()
                .append(&mut tx, duplicate)
                .await
                .unwrap_err()
                .category(),
            ErrorCategory::Conflict
        );
        h.uow().rollback(tx).await.unwrap();
    }
}

async fn verify_rollback<H: AuditRepositoryContractHarness>(h: &H, expected: &[AuditEntry]) {
    let cancel = entry(60, 1, 60, AuditAction::ScheduledCancelled);
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    h.write_companion(&mut tx).await.unwrap();
    h.repository()
        .append(&mut tx, cancel.clone())
        .await
        .unwrap();
    h.fail_next_append().await.unwrap();
    let error = h
        .repository()
        .append(&mut tx, entry(61, 1, 61, AuditAction::SignedIn))
        .await
        .unwrap_err();
    assert_eq!(error.category(), ErrorCategory::Unavailable);
    h.uow().rollback(tx).await.unwrap();
    assert_eq!(read(h, all()).await, expected);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert!(
        !h.companion_exists(&mut tx).await.unwrap(),
        "companion write escaped rollback"
    );
    h.uow().rollback(tx).await.unwrap();
    {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.repository()
            .append(&mut tx, cancel.clone())
            .await
            .unwrap();
    }
    assert_eq!(
        read(h, all()).await,
        expected,
        "drop must discard pending audit"
    );
    for _ in 0..2 {
        let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
        h.repository()
            .append(&mut tx, cancel.clone())
            .await
            .unwrap();
        h.uow().commit(tx).await.unwrap();
    }
    let entries = read(
        h,
        AuditFilter::new(None, None, Some(cancel.subject()), None, None).unwrap(),
    )
    .await;
    assert_eq!(
        entries,
        std::slice::from_ref(&cancel),
        "cancellation retries append once, never a success event"
    );
    assert_eq!(entries[0].actor(), Actor::Admin(admin(1)));
    assert_eq!(
        entries[0].metadata().executor(),
        Some(Executor::SystemWorker)
    );
    assert_eq!(entries[0].metadata().approver(), Some(admin(3)));
    assert_eq!(entries[0].reason(), ReasonCode::AdminRevoked);
}

async fn verify_handles<H: AuditRepositoryContractHarness>(h: &H) {
    let mut foreign = h.begin_foreign().await.unwrap();
    assert_eq!(
        h.repository()
            .append(&mut foreign, entry(70, 1, 70, AuditAction::SignedIn))
            .await
            .unwrap_err()
            .category(),
        ErrorCategory::InvalidHandle
    );
    assert_eq!(
        h.repository()
            .search(&mut foreign, AuditQuery::new(all(), None, 2).unwrap())
            .await
            .unwrap_err()
            .category(),
        ErrorCategory::InvalidHandle
    );
    drop(foreign);
    let mut tx = h.uow().begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        h.repository()
            .append(&mut tx, entry(70, 1, 70, AuditAction::SignedIn))
            .await
            .unwrap_err()
            .category(),
        ErrorCategory::InvalidHandle
    );
    h.uow().rollback(tx).await.unwrap();
}

/// Run unchanged against a fresh fake or real backend.
///
/// # Panics
/// Any violation of uniqueness, safe round-trip, filtering, paging or caller-UoW atomicity.
pub async fn audit_repository_contract<H: AuditRepositoryContractHarness>(h: &H) {
    assert!(read(h, all()).await.is_empty());
    let events = vec![
        entry(1, 1, 10, AuditAction::SignedIn),
        entry(2, 2, 10, AuditAction::SignedIn),
        entry(3, 1, 20, AuditAction::Recovered),
        entry(8, 2, 30, AuditAction::SignedIn),
        entry(4, 1, 35, AuditAction::Published),
        entry(5, 1, 40, AuditAction::SignedIn),
    ];
    let mut tx = h.uow().begin(TransactionMode::ReadWrite).await.unwrap();
    for event in events.iter().rev() {
        h.repository().append(&mut tx, event.clone()).await.unwrap();
    }
    h.uow().commit(tx).await.unwrap();
    assert_eq!(read(h, all()).await, events);
    verify_filters(h, &events).await;
    verify_uniqueness(h, &events[0]).await;
    assert_eq!(
        read(h, all()).await,
        events,
        "retry/conflict cannot rewrite history"
    );
    verify_rollback(h, &events).await;
    verify_handles(h).await;
}
