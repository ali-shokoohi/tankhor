//! FR-004/005, AC-ADM-004/007/043: protected admin state in the caller's `UoW`.

use crate::domain::admin_state::{
    AdminState, EnrollmentAuthorization, EnrollmentPurpose, Invalidation, SecurityEpoch,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::AdminId,
    ports::unit_of_work::TxHandle,
};
use std::{fmt, num::NonZeroU16};
use time::OffsetDateTime;

/// Keyed normalized-identity digest. No plaintext phone or implicit formatting.
#[derive(Clone, PartialEq, Eq)]
pub struct IdentityDigest([u8; 32]);

impl IdentityDigest {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for IdentityDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("IdentityDigest([REDACTED])")
    }
}

/// Safe ascending-ID keyset query with an explicit finite page bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminListQuery {
    after: Option<AdminId>,
    limit: NonZeroU16,
}

impl AdminListQuery {
    /// # Errors
    /// Rejects zero-sized pages.
    pub fn new(after: Option<AdminId>, limit: u16) -> Result<Self, DomainError> {
        Ok(Self {
            after,
            limit: NonZeroU16::new(limit)
                .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))?,
        })
    }

    #[must_use]
    pub const fn after(&self) -> Option<AdminId> {
        self.after
    }

    #[must_use]
    pub const fn limit(&self) -> u16 {
        self.limit.get()
    }
}

/// Internal identity and protected state only; never phone or credential material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminPage {
    pub entries: Vec<AdminState>,
    pub next: Option<AdminId>,
}

/// Identity-bound authorization, not a bearer token or browser ceremony.
/// `expected_authorization` compares the complete previous protected value so a
/// superseded authorization cannot be consumed by an old concurrent request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizeEnrollment {
    pub actor: AdminId,
    pub target: AdminId,
    pub purpose: EnrollmentPurpose,
    pub expected_epoch: SecurityEpoch,
    pub expected_authorization: Option<EnrollmentAuthorization>,
    pub authorized_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

/// Operations always use the supplied transaction; no independent commit.
/// Browser/session/stage proof validation remains in `SessionStore` and services.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait AdminRepository: Send + Sync {
    async fn find_by_identity(
        &self,
        tx: &mut TxHandle,
        identity: IdentityDigest,
    ) -> Result<Option<AdminState>, DomainError>;

    async fn list(
        &self,
        tx: &mut TxHandle,
        query: AdminListQuery,
    ) -> Result<AdminPage, DomainError>;

    async fn get(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
    ) -> Result<Option<AdminState>, DomainError>;

    /// Lock unique targets in ascending ID order, regardless of input order.
    /// Return present targets in that same stable order.
    async fn lock(
        &self,
        tx: &mut TxHandle,
        targets: Vec<AdminId>,
    ) -> Result<Vec<AdminState>, DomainError>;

    /// Insert a unique ID and keyed identity, never a factor or a session.
    async fn invite(
        &self,
        tx: &mut TxHandle,
        state: AdminState,
        identity: IdentityDigest,
    ) -> Result<(), DomainError>;

    /// Atomically change eligibility and advance epoch under expected-epoch CAS.
    /// Reject stale/non-increasing versions and exhaustion; invalidate protected
    /// authorization in the same write. The caller owns child revocations/audit.
    async fn invalidate_epoch(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: SecurityEpoch,
        next: SecurityEpoch,
        kind: Invalidation,
    ) -> Result<(), DomainError>;

    /// Restore eligibility under CAS without resetting or reusing any older epoch.
    async fn restore(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: SecurityEpoch,
    ) -> Result<(), DomainError>;

    /// Lock/recheck current actor and target; require another eligible admin,
    /// target eligibility/purpose/current epoch, finite expiry and exact prior
    /// authorization. Creating and superseding share this conditional operation.
    async fn authorize_enrollment(
        &self,
        tx: &mut TxHandle,
        command: AuthorizeEnrollment,
    ) -> Result<(), DomainError>;

    async fn enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
    ) -> Result<Option<EnrollmentAuthorization>, DomainError>;

    /// One conditional winner matching the complete pending authorization,
    /// target/purpose/current epoch and time. Never consumes a replacement.
    async fn consume_enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: EnrollmentAuthorization,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;

    /// Conditional revoke of the exact protected authorization, never a replacement.
    async fn revoke_enrollment(
        &self,
        tx: &mut TxHandle,
        target: AdminId,
        expected: EnrollmentAuthorization,
    ) -> Result<(), DomainError>;
}

#[cfg(test)]
mod tests {
    use super::{
        AdminListQuery, AdminPage, AdminRepository, AuthorizeEnrollment, IdentityDigest,
        MockAdminRepository,
    };
    use crate::domain::admin_state::{
        AdminState, Eligibility, EnrollmentPurpose, EnrollmentStatus, Invalidation, SecurityEpoch,
    };
    use kernel::{
        ids::AdminId,
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn id(number: u16) -> AdminId {
        AdminId::from_str(&format!("019a53a0-0000-7000-8000-{number:012x}")).unwrap()
    }

    #[tokio::test]
    async fn admin_repository_port_is_async_and_mockable() {
        let mut mock = MockAdminRepository::new();
        let store = StoreId::new();
        let target = AdminState::new(id(2), Eligibility::Invited, SecurityEpoch::new(4));
        let actor = AdminState::new(id(1), Eligibility::Active, SecurityEpoch::new(8));
        let digest = IdentityDigest::new([17; 32]);
        assert_eq!(digest.as_bytes(), &[17; 32]);
        assert_eq!(format!("{digest:?}"), "IdentityDigest([REDACTED])");
        let mut tx = TxHandle::new(store, TransactionMode::ReadWrite, ());
        let mut sequence = mockall::Sequence::new();
        let expected_target = target.clone();
        mock.expect_invite()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |tx, state, digest| {
                tx.store_id() == store
                    && state == &expected_target
                    && digest.as_bytes() == &[17; 32]
            })
            .return_once(|_, _, _| Ok(()));
        mock.invite(&mut tx, target.clone(), digest.clone())
            .await
            .unwrap();
        let result = target.clone();
        mock.expect_find_by_identity()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |tx, value| tx.store_id() == store && value.as_bytes() == &[17; 32])
            .return_once(move |_, _| Ok(Some(result)));
        assert_eq!(
            mock.find_by_identity(&mut tx, digest).await.unwrap(),
            Some(target.clone())
        );
        let result = target.clone();
        mock.expect_get()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |tx, target| tx.store_id() == store && *target == id(2))
            .return_once(move |_, _| Ok(Some(result)));
        assert_eq!(
            mock.get(&mut tx, id(2)).await.unwrap(),
            Some(target.clone())
        );
        mock.expect_list()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |tx, query| {
                tx.store_id() == store && query.after() == Some(id(1)) && query.limit() == 1
            })
            .return_once(move |_, _| {
                Ok(AdminPage {
                    entries: vec![target],
                    next: Some(id(2)),
                })
            });
        let page = mock
            .list(&mut tx, AdminListQuery::new(Some(id(1)), 1).unwrap())
            .await
            .unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].id(), id(2));
        assert_eq!(page.next, Some(id(2)));
        assert!(AdminListQuery::new(None, 0).is_err());
        let targets = vec![id(2), id(1)];
        mock.expect_lock()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |tx, ids| tx.store_id() == store && ids == &[id(2), id(1)])
            .return_once(move |_, _| Ok(vec![actor]));
        assert_eq!(mock.lock(&mut tx, targets).await.unwrap()[0].id(), id(1));
        assert_epoch_surface(&mut mock, &mut tx).await;
        assert_authorization_surface(&mut mock, &mut tx).await;
    }

    async fn assert_epoch_surface(mock: &mut MockAdminRepository, tx: &mut TxHandle) {
        mock.expect_invalidate_epoch()
            .times(1)
            .withf(|_, target, expected, next, kind| {
                *target == id(2)
                    && expected.value() == 4
                    && next.value() == 5
                    && *kind == Invalidation::Recover
            })
            .return_once(|_, _, _, _, _| Ok(()));
        mock.invalidate_epoch(
            tx,
            id(2),
            SecurityEpoch::new(4),
            SecurityEpoch::new(5),
            Invalidation::Recover,
        )
        .await
        .unwrap();
        mock.expect_restore()
            .times(1)
            .withf(|_, target, expected| *target == id(2) && expected.value() == 5)
            .return_once(|_, _, _| Ok(()));
        mock.restore(tx, id(2), SecurityEpoch::new(5))
            .await
            .unwrap();
    }

    async fn assert_authorization_surface(mock: &mut MockAdminRepository, tx: &mut TxHandle) {
        let now = OffsetDateTime::UNIX_EPOCH;
        let actor = AdminState::new(id(1), Eligibility::Active, SecurityEpoch::new(8));
        let mut target = AdminState::new(id(2), Eligibility::Recovering, SecurityEpoch::new(5));
        target
            .authorize_enrollment(
                &actor,
                EnrollmentPurpose::Recovery,
                target.epoch(),
                now,
                now + Duration::minutes(5),
            )
            .unwrap();
        let authorization = target.enrollment().unwrap().clone();
        let command = AuthorizeEnrollment {
            actor: id(1),
            target: id(2),
            purpose: EnrollmentPurpose::Recovery,
            expected_epoch: SecurityEpoch::new(5),
            expected_authorization: None,
            authorized_at: now,
            expires_at: now + Duration::minutes(5),
        };
        let expected = command.clone();
        mock.expect_authorize_enrollment()
            .times(1)
            .withf(move |_, command| command == &expected)
            .return_once(|_, _| Ok(()));
        mock.authorize_enrollment(tx, command).await.unwrap();
        let result = authorization.clone();
        mock.expect_enrollment()
            .times(1)
            .withf(|_, target| *target == id(2))
            .return_once(move |_, _| Ok(Some(result)));
        assert_eq!(
            mock.enrollment(tx, id(2)).await.unwrap(),
            Some(authorization.clone())
        );
        assert_eq!(authorization.status(), EnrollmentStatus::Pending);
        let expected = authorization.clone();
        mock.expect_consume_enrollment()
            .times(1)
            .withf(move |_, target, value, at| *target == id(2) && value == &expected && *at == now)
            .return_once(|_, _, _, _| Ok(()));
        mock.consume_enrollment(tx, id(2), authorization.clone(), now)
            .await
            .unwrap();
        let expected = authorization.clone();
        mock.expect_revoke_enrollment()
            .times(1)
            .withf(move |_, target, value| *target == id(2) && value == &expected)
            .return_once(|_, _, _| Ok(()));
        mock.revoke_enrollment(tx, id(2), authorization.clone())
            .await
            .unwrap();
        let replacement = AuthorizeEnrollment {
            actor: id(1),
            target: id(2),
            purpose: EnrollmentPurpose::Recovery,
            expected_epoch: SecurityEpoch::new(5),
            expected_authorization: Some(authorization),
            authorized_at: now + Duration::SECOND,
            expires_at: now + Duration::minutes(6),
        };
        let expected = replacement.clone();
        mock.expect_authorize_enrollment()
            .times(1)
            .withf(move |_, command| command == &expected)
            .return_once(|_, _| Ok(()));
        mock.authorize_enrollment(tx, replacement).await.unwrap();
    }
}
