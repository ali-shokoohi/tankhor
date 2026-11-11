//! Append-only audit fake using the caller's isolated shared working set.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use admin::domain::audit::AuditEntry;
use admin::ports::audit_repository::{AuditCursor, AuditPage, AuditQuery, AuditRepository};
use kernel::error::{DomainError, ErrorCode};
use kernel::ports::unit_of_work::TxHandle;

use super::store::FakeStore;

#[derive(Clone, Debug)]
pub struct FakeAuditRepository {
    store: FakeStore,
    fail_next_append: Arc<AtomicBool>,
}

impl FakeAuditRepository {
    #[must_use]
    pub fn new(store: FakeStore) -> Self {
        Self {
            store,
            fail_next_append: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Inject a one-shot unavailable append; never writes committed state directly.
    pub fn fail_next_append(&self) {
        self.fail_next_append.store(true, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl AuditRepository for FakeAuditRepository {
    async fn append(&self, tx: &mut TxHandle, entry: AuditEntry) -> Result<(), DomainError> {
        self.store.with_write(tx, |state| {
            if self.fail_next_append.swap(false, Ordering::SeqCst) {
                return Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable));
            }
            if let Some(existing) = state.audit_entries.values().find(|existing| {
                existing.id() == entry.id() || existing.event_key() == entry.event_key()
            }) {
                return if existing == &entry {
                    Ok(())
                } else {
                    Err(DomainError::conflict(ErrorCode::IdempotencyConflict))
                };
            }
            state.audit_entries.insert(entry.id(), entry);
            Ok(())
        })
    }

    async fn search(&self, tx: &mut TxHandle, query: AuditQuery) -> Result<AuditPage, DomainError> {
        self.store.with_read(tx, |state| {
            let filter = query.filter();
            let mut entries: Vec<_> = state
                .audit_entries
                .values()
                .filter(|entry| {
                    filter.actor().is_none_or(|value| entry.actor() == value)
                        && filter.action().is_none_or(|value| entry.action() == value)
                        && filter
                            .subject()
                            .is_none_or(|value| entry.subject() == value)
                        && filter
                            .from()
                            .is_none_or(|value| entry.occurred_at() >= value)
                        && filter
                            .until()
                            .is_none_or(|value| entry.occurred_at() < value)
                        && query.after().is_none_or(|cursor| {
                            (entry.occurred_at(), entry.id()) > (cursor.occurred_at, cursor.id)
                        })
                })
                .cloned()
                .collect();
            entries.sort_by_key(|entry| (entry.occurred_at(), entry.id()));
            let limit = usize::from(query.limit());
            let has_more = entries.len() > limit;
            entries.truncate(limit);
            let next = if has_more {
                entries.last().map(|entry| AuditCursor {
                    occurred_at: entry.occurred_at(),
                    id: entry.id(),
                })
            } else {
                None
            };
            Ok(AuditPage { entries, next })
        })
    }
}
