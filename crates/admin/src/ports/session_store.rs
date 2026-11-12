//! FR-002/003/004, FR-CON-02: digest-only session and restricted ceremony storage.

use crate::domain::{
    admin_state::AdminState,
    auth_state::{AuthBinding, AuthPurpose, AuthStage, AuthState, BindingDigest},
    secrets::SecretBytes,
    session_state::SessionState,
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, SessionId},
    ports::unit_of_work::TxHandle,
};
use time::OffsetDateTime;

/// Server-only storage input: no raw cookie/capability is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub digest: BindingDigest,
    pub state: SessionState,
}

/// Trusted request context, including the current credential revision when used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCheck {
    pub reference: BindingDigest,
    pub binding: AuthBinding,
    pub now: OffsetDateTime,
}

/// Restricted metadata only. Neither serialized ceremony state nor provisioning
/// material can be retrieved through this read/query result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingView {
    pub reference: BindingDigest,
    pub state: AuthState,
    pub provisioning_presented: bool,
}

/// Server-only record. Both opaque payloads must be authenticated encrypted
/// envelopes supplied by the authenticator, never plaintext or client state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRecord {
    reference: BindingDigest,
    state: AuthState,
    verification: Option<SecretBytes>,
    provisioning: Option<SecretBytes>,
    provisioning_presented: bool,
}

impl PendingRecord {
    /// # Errors
    /// Rejects missing/empty ceremony state, primary-proof payloads and
    /// provisioning outside a TOTP enrollment ceremony.
    pub fn new(
        reference: BindingDigest,
        state: AuthState,
        verification: Option<SecretBytes>,
        provisioning: Option<SecretBytes>,
    ) -> Result<Self, DomainError> {
        let binding = state.binding();
        let primary = binding.stage == AuthStage::PrimaryVerified;
        if primary == verification.is_some()
            || verification.as_ref().is_some_and(|v| v.expose().is_empty())
            || provisioning.as_ref().is_some_and(|v| v.expose().is_empty())
            || (provisioning.is_some()
                && (binding.stage != AuthStage::Totp
                    || !matches!(binding.purpose, AuthPurpose::Enrollment(_))))
        {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self {
            reference,
            state,
            verification,
            provisioning,
            provisioning_presented: false,
        })
    }

    #[must_use]
    pub const fn reference(&self) -> &BindingDigest {
        &self.reference
    }

    #[must_use]
    pub const fn state(&self) -> &AuthState {
        &self.state
    }

    /// Server-only encrypted verifier envelope for adapter persistence.
    #[must_use]
    pub const fn verification_envelope(&self) -> Option<&SecretBytes> {
        self.verification.as_ref()
    }

    /// Server-only encrypted provisioning envelope for adapter persistence.
    #[must_use]
    pub const fn provisioning_envelope(&self) -> Option<&SecretBytes> {
        self.provisioning.as_ref()
    }

    /// # Errors
    /// Refuses a wrong reference, binding, current authority, or expired/terminal state.
    pub fn view(
        &self,
        check: &PendingCheck,
        admin: &AdminState,
    ) -> Result<PendingView, DomainError> {
        self.check(check, admin)?;
        Ok(PendingView {
            reference: self.reference.clone(),
            state: self.state.clone(),
            provisioning_presented: self.provisioning_presented,
        })
    }

    /// Protected verifier-only operation; never used for a client read/query DTO.
    /// # Errors
    /// Refuses invalid current context and primary proofs without verifier state.
    pub fn verification_state(
        &self,
        check: &PendingCheck,
        admin: &AdminState,
    ) -> Result<SecretBytes, DomainError> {
        self.check(check, admin)?;
        self.verification.clone().ok_or_else(denied)
    }

    /// Take the encrypted presentation envelope once, retaining verifier state.
    /// Adapters must persist this mutation atomically under a conditional lock/CAS;
    /// concurrent callers cannot both return material from committed operations.
    /// # Errors
    /// Refuses invalid context, missing provisioning or any repeated presentation.
    pub fn present_provisioning(
        &mut self,
        check: &PendingCheck,
        admin: &AdminState,
    ) -> Result<SecretBytes, DomainError> {
        self.check(check, admin)?;
        if self.provisioning_presented {
            return Err(denied());
        }
        let envelope = self.provisioning.take().ok_or_else(denied)?;
        self.provisioning_presented = true;
        Ok(envelope)
    }

    /// # Errors
    /// Refuses a stale/expired binding or replay; refusal leaves state unchanged.
    pub fn consume(&mut self, check: &PendingCheck, admin: &AdminState) -> Result<(), DomainError> {
        self.check(check, admin)?;
        self.state.consume(&check.binding, admin, check.now)?;
        self.provisioning = None;
        Ok(())
    }

    /// # Errors
    /// Refuses an invalid/terminal context without mutation.
    pub fn supersede(
        &mut self,
        check: &PendingCheck,
        admin: &AdminState,
    ) -> Result<(), DomainError> {
        self.check(check, admin)?;
        self.state.supersede();
        self.provisioning = None;
        Ok(())
    }

    pub fn revoke(&mut self) {
        self.state.revoke();
        self.provisioning = None;
    }

    fn check(&self, check: &PendingCheck, admin: &AdminState) -> Result<(), DomainError> {
        if self.reference != check.reference {
            return Err(denied());
        }
        self.state.check(&check.binding, admin, check.now)
    }
}

fn denied() -> DomainError {
    DomainError::forbidden(ErrorCode::PermissionDenied)
}

/// Every operation uses the caller's transaction and current committed admin
/// authority. Missing/invalid pending state is a typed `PermissionDenied` refusal;
/// duplicate digests/IDs conflict. No operation accepts or persists raw capabilities.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    async fn insert(&self, tx: &mut TxHandle, session: SessionRecord) -> Result<(), DomainError>;

    async fn lookup(
        &self,
        tx: &mut TxHandle,
        digest: BindingDigest,
    ) -> Result<Option<SessionState>, DomainError>;

    /// Re-read current admin eligibility/epoch, check finite absolute/idle expiry,
    /// then record activity without extending the absolute deadline.
    async fn check_current(
        &self,
        tx: &mut TxHandle,
        digest: BindingDigest,
        now: OffsetDateTime,
    ) -> Result<SessionState, DomainError>;

    async fn revoke_one(
        &self,
        tx: &mut TxHandle,
        id: SessionId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;

    /// Idempotent target-only revocation, including no other administrator.
    async fn revoke_all(
        &self,
        tx: &mut TxHandle,
        admin: AdminId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;

    async fn create_pending(
        &self,
        tx: &mut TxHandle,
        pending: PendingRecord,
    ) -> Result<(), DomainError>;

    async fn read_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<PendingView, DomainError>;

    /// Internal verifier access to encrypted state only; never a client DTO.
    async fn verification_state(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<SecretBytes, DomainError>;

    /// Conditional one-winner consumption with all bindings checked under lock.
    async fn consume_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<(), DomainError>;

    async fn supersede_pending(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<(), DomainError>;

    /// Idempotently revoke only this target's pending primary/ceremony state.
    async fn revoke_pending(&self, tx: &mut TxHandle, admin: AdminId) -> Result<(), DomainError>;

    /// Conditional atomic mark-and-take: one winner, no redisplay. Return the
    /// encrypted presentation envelope only, leaving protected verifier state.
    /// Commit the mark before decrypting/rendering; never release material after
    /// rollback or an unresolved commit, and never include it in ordinary reads.
    async fn present_provisioning(
        &self,
        tx: &mut TxHandle,
        check: PendingCheck,
    ) -> Result<SecretBytes, DomainError>;
}

#[cfg(test)]
mod tests {
    use super::{
        MockSessionStore, PendingCheck, PendingRecord, PendingView, SessionRecord, SessionStore,
    };
    use crate::domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        audit::Factor,
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        secrets::SecretBytes,
        session_state::{SessionInput, SessionState},
    };
    use kernel::{
        ids::{AdminId, ChallengeId, SessionId},
        ports::unit_of_work::{StoreId, TransactionMode, TxHandle},
    };
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    fn fixture() -> (SessionRecord, PendingRecord, PendingCheck) {
        let admin = AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap();
        let session = SessionId::from_str("019a53a0-0000-7000-8000-000000000011").unwrap();
        let challenge = ChallengeId::from_str("019a53a0-0000-7000-8000-000000000021").unwrap();
        let epoch = SecurityEpoch::new(4);
        let now = OffsetDateTime::UNIX_EPOCH;
        let state = SessionState::try_from(SessionInput {
            id: session,
            admin_id: admin,
            epoch,
            primary_verified: true,
            factor: Some(Factor::Passkey),
            issued_at: now,
            expires_at: now + Duration::minutes(10),
            idle_timeout: Duration::minutes(2),
        })
        .unwrap();
        let binding = AuthBinding {
            target: admin,
            browser: BindingDigest::new([2; 32]),
            session,
            purpose: AuthPurpose::SignIn,
            stage: AuthStage::PrimaryVerified,
            epoch,
            credential_revision: None,
        };
        let proof = AuthState::try_from(AuthInput {
            id: challenge,
            binding: binding.clone(),
            primary_verified_at: Some(now),
            created_at: now,
            expires_at: now + Duration::minutes(2),
        })
        .unwrap();
        let pending = PendingRecord::new(BindingDigest::new([3; 32]), proof, None, None).unwrap();
        assert!(
            PendingRecord::new(
                BindingDigest::new([3; 32]),
                pending.state().clone(),
                Some(SecretBytes::new(vec![1])),
                None
            )
            .is_err()
        );
        let check = PendingCheck {
            reference: BindingDigest::new([3; 32]),
            binding,
            now,
        };
        (
            SessionRecord {
                digest: BindingDigest::new([1; 32]),
                state,
            },
            pending,
            check,
        )
    }

    fn mock_store(state: SessionState, view: PendingView) -> MockSessionStore {
        let mut mock = MockSessionStore::new();
        mock.expect_insert().times(1).return_once(|_, _| Ok(()));
        mock.expect_lookup().times(1).return_once({
            let state = state.clone();
            move |_, _| Ok(Some(state))
        });
        mock.expect_check_current()
            .times(1)
            .return_once(move |_, _, _| Ok(state));
        mock.expect_revoke_one()
            .times(1)
            .return_once(|_, _, _| Ok(()));
        mock.expect_revoke_all()
            .times(1)
            .return_once(|_, _, _| Ok(()));
        mock.expect_create_pending()
            .times(1)
            .return_once(|_, _| Ok(()));
        mock.expect_read_pending()
            .times(1)
            .return_once(move |_, _| Ok(view));
        mock.expect_verification_state()
            .times(1)
            .return_once(|_, _| Ok(SecretBytes::new(b"synthetic-encrypted-state".to_vec())));
        mock.expect_consume_pending()
            .times(1)
            .return_once(|_, _| Ok(()));
        mock.expect_supersede_pending()
            .times(1)
            .return_once(|_, _| Ok(()));
        mock.expect_revoke_pending()
            .times(1)
            .return_once(|_, _| Ok(()));
        mock.expect_present_provisioning()
            .times(1)
            .return_once(|_, _| Ok(SecretBytes::new(b"synthetic-secret".to_vec())));
        mock
    }

    #[tokio::test]
    async fn session_store_port_is_async_and_mockable() {
        let (session, pending, check) = fixture();
        let admin = session.state.admin_id();
        let id = session.state.id();
        let now = check.now;
        let view = PendingView {
            reference: check.reference.clone(),
            state: pending.state().clone(),
            provisioning_presented: false,
        };
        assert!(!format!("{pending:?}").contains("synthetic-secret"));
        let mock = mock_store(session.state.clone(), view);
        let mut tx = TxHandle::new(StoreId::new(), TransactionMode::ReadWrite, ());
        let store: &dyn SessionStore = &mock;
        store.insert(&mut tx, session.clone()).await.unwrap();
        assert_eq!(
            store.lookup(&mut tx, session.digest.clone()).await.unwrap(),
            Some(session.state.clone())
        );
        assert_eq!(
            store
                .check_current(&mut tx, session.digest, now)
                .await
                .unwrap(),
            session.state
        );
        store.revoke_one(&mut tx, id, now).await.unwrap();
        store.revoke_all(&mut tx, admin, now).await.unwrap();
        store.create_pending(&mut tx, pending).await.unwrap();
        assert_eq!(
            store
                .read_pending(&mut tx, check.clone())
                .await
                .unwrap()
                .state
                .binding(),
            &check.binding
        );
        assert_eq!(
            store
                .verification_state(&mut tx, check.clone())
                .await
                .unwrap()
                .expose(),
            b"synthetic-encrypted-state"
        );
        store.consume_pending(&mut tx, check.clone()).await.unwrap();
        store
            .supersede_pending(&mut tx, check.clone())
            .await
            .unwrap();
        store.revoke_pending(&mut tx, admin).await.unwrap();
        assert_eq!(
            store
                .present_provisioning(&mut tx, check)
                .await
                .unwrap()
                .expose(),
            b"synthetic-secret"
        );
        assert!(
            session
                .state
                .check_access(
                    &AdminState::new(admin, Eligibility::Active, SecurityEpoch::new(4)),
                    now
                )
                .is_ok()
        );
        verify_pending_transitions();
    }

    fn verify_pending_transitions() {
        let (_, primary, check) = fixture();
        let actor = AdminState::new(
            check.binding.target,
            Eligibility::Active,
            check.binding.epoch,
        );
        for field in 0..9 {
            let mut wrong = check.clone();
            match field {
                0 => wrong.reference = BindingDigest::new([99; 32]),
                1 => wrong.binding.browser = BindingDigest::new([99; 32]),
                2 => {
                    wrong.binding.session =
                        SessionId::from_str("019a53a0-0000-7000-8000-000000000099").unwrap();
                }
                3 => {
                    wrong.binding.target =
                        AdminId::from_str("019a53a0-0000-7000-8000-000000000099").unwrap();
                }
                4 => wrong.binding.stage = AuthStage::Passkey,
                5 => {
                    wrong.binding.purpose = AuthPurpose::Enrollment(
                        crate::domain::admin_state::EnrollmentPurpose::Recovery,
                    );
                }
                6 => wrong.binding.epoch = SecurityEpoch::new(99),
                7 => wrong.binding.credential_revision = Some(99),
                _ => wrong.now += Duration::minutes(2),
            }
            let mut record = primary.clone();
            assert_eq!(
                record.view(&wrong, &actor).unwrap_err().code(),
                kernel::error::ErrorCode::PermissionDenied
            );
            assert!(record.consume(&wrong, &actor).is_err());
            assert_eq!(record, primary);
        }
        let mut record = primary.clone();
        record.consume(&check, &actor).unwrap();
        assert!(record.consume(&check, &actor).is_err());
        assert!(record.view(&check, &actor).is_err());
        let mut record = primary.clone();
        record.supersede(&check, &actor).unwrap();
        assert!(record.view(&check, &actor).is_err());
        let mut record = primary;
        record.revoke();
        record.revoke();
        assert!(record.view(&check, &actor).is_err());
        verify_provisioning();
    }

    fn verify_provisioning() {
        use crate::domain::admin_state::{EnrollmentPurpose, Invalidation};
        let (_, _, mut check) = fixture();
        let mut actor = AdminState::new(
            check.binding.target,
            Eligibility::Active,
            check.binding.epoch,
        );
        actor
            .invalidate(
                check.binding.epoch,
                SecurityEpoch::new(5),
                Invalidation::Recover,
            )
            .unwrap();
        let authorizer = AdminState::new(
            AdminId::from_str("019a53a0-0000-7000-8000-000000000099").unwrap(),
            Eligibility::Active,
            SecurityEpoch::new(1),
        );
        actor
            .authorize_enrollment(
                &authorizer,
                EnrollmentPurpose::Recovery,
                actor.epoch(),
                check.now,
                check.now + Duration::minutes(2),
            )
            .unwrap();
        check.binding.epoch = actor.epoch();
        check.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery);
        check.binding.stage = AuthStage::Totp;
        let state = AuthState::try_from(AuthInput {
            id: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000022").unwrap(),
            binding: check.binding.clone(),
            primary_verified_at: Some(check.now),
            created_at: check.now,
            expires_at: check.now + Duration::minutes(2),
        })
        .unwrap();
        let mut record = PendingRecord::new(
            check.reference.clone(),
            state,
            Some(SecretBytes::new(b"synthetic-verifier-envelope".to_vec())),
            Some(SecretBytes::new(
                b"synthetic-provisioning-envelope".to_vec(),
            )),
        )
        .unwrap();
        let original = record.clone();
        verify_presentation_race(original.clone(), &check, &actor);
        assert!(!format!("{record:?}").contains("synthetic-"));
        assert!(!record.view(&check, &actor).unwrap().provisioning_presented);
        assert_eq!(
            record
                .present_provisioning(&check, &actor)
                .unwrap()
                .expose(),
            b"synthetic-provisioning-envelope"
        );
        assert!(record.view(&check, &actor).unwrap().provisioning_presented);
        assert!(record.present_provisioning(&check, &actor).is_err());
        assert_eq!(
            record.verification_state(&check, &actor).unwrap().expose(),
            b"synthetic-verifier-envelope"
        );
        let mut terminal = original;
        terminal.supersede(&check, &actor).unwrap();
        assert!(terminal.present_provisioning(&check, &actor).is_err());
        assert!(terminal.verification_state(&check, &actor).is_err());
        record.revoke();
        assert!(record.present_provisioning(&check, &actor).is_err());
        assert!(record.verification_state(&check, &actor).is_err());
    }

    fn verify_presentation_race(record: PendingRecord, check: &PendingCheck, actor: &AdminState) {
        let record = std::sync::Mutex::new(record);
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let attempt = || {
                barrier.wait();
                record.lock().unwrap().present_provisioning(check, actor)
            };
            let first = scope.spawn(attempt);
            let second = scope.spawn(attempt);
            let results = [first.join().unwrap(), second.join().unwrap()];
            assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
            assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        });
        let record = record.into_inner().unwrap();
        assert!(record.view(check, actor).unwrap().provisioning_presented);
        assert_eq!(
            record.verification_state(check, actor).unwrap().expose(),
            b"synthetic-verifier-envelope"
        );
    }
}
