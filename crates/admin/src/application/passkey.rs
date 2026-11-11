//! FR-002/024: browser-bound second factor; crypto outside the final audited `UoW`.
use crate::{
    domain::{
        admin_state::AdminState,
        audit::{
            Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, Factor, ReasonCode, Subject,
        },
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState},
        credential_state::CredentialKind,
        secrets::SecretBytes,
        security_policy::SecurityPolicy,
        session_state::{SessionInput, SessionState},
    },
    ports::{
        admin_authenticator::{
            AdminAuthenticator, CeremonyContext, PasskeyFinish, PasskeyStart, TokenPurpose,
        },
        admin_credential_repository::{
            AdminCredentialRepository, CredentialMaterial, CredentialRecord, PasskeyRevision,
        },
        admin_repository::AdminRepository,
        audit_repository::AuditRepository,
        session_store::{PendingCheck, PendingRecord, PendingView, SessionRecord, SessionStore},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::CredentialId,
    ports::{
        clock::Clock,
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, TxHandle, UnitOfWork},
    },
};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct BeginPasskeyInput {
    pub proof: PendingCheck,
    pub credential: CredentialId,
}
#[derive(Debug)]
pub struct PasskeyOptions {
    pub token: SecretBytes,
    pub binding: AuthBinding,
    pub public_options: SecretBytes,
}
#[derive(Debug, Clone)]
pub struct FinishPasskeyInput {
    pub proof: PendingCheck,
    pub credential: CredentialId,
    pub response: SecretBytes,
}
#[derive(Debug)]
pub struct CompletedSession {
    pub token: SecretBytes,
}

pub struct BeginPasskey<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub credentials: &'a dyn AdminCredentialRepository,
    pub sessions: &'a dyn SessionStore,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}

impl BeginPasskey<'_> {
    /// # Errors
    /// Requires a current primary proof and active passkey; refuses stale/replayed state.
    pub async fn execute(
        &self,
        mut input: BeginPasskeyInput,
    ) -> Result<PasskeyOptions, DomainError> {
        input.proof.now = self.clock.now().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let snapshot = read_snapshot(
            self.admins,
            self.credentials,
            self.sessions,
            &mut tx,
            &input.proof,
            input.credential,
            AuthStage::PrimaryVerified,
        )
        .await;
        let (admin, credential, primary) = match snapshot {
            Ok(value) => {
                self.uow.commit(tx).await?;
                value
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                return Err(error);
            }
        };
        let created_at = self.clock.now().await?;
        let mut binding = primary.state.binding().clone();
        binding.stage = AuthStage::Passkey;
        binding.credential_revision = Some(credential.state().revision());
        let state = AuthState::try_from(AuthInput {
            id: self.ids.next_challenge_id().await?,
            binding: binding.clone(),
            primary_verified_at: Some(primary.state.primary_verified_at()),
            created_at,
            expires_at: expiry(created_at, self.policy.ceremony_ttl())?
                .min(primary.state.expires_at()),
        })?;
        let request = start(&state, &credential, self.policy)?;
        let challenge = self.authenticator.begin_passkey(request).await?;
        let token = self
            .authenticator
            .issue_token(TokenPurpose::Ceremony)
            .await?;
        let reference = self
            .authenticator
            .token_digest(TokenPurpose::Ceremony, token.clone())
            .await?;
        let pending = PendingRecord::new(reference, state, Some(challenge.protected_state), None)?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let update = async {
            let current = self.admins.lock(&mut tx, vec![admin.id()]).await?;
            if current.as_slice() != [admin.clone()] {
                return Err(denied());
            }
            let current = self
                .credentials
                .lock(&mut tx, input.credential)
                .await?
                .ok_or_else(denied)?;
            if current != credential {
                return Err(denied());
            }
            input.proof.now = self.clock.now().await?;
            let current = self
                .sessions
                .read_pending(&mut tx, input.proof.clone())
                .await?;
            if current != primary {
                return Err(denied());
            }
            pending
                .state()
                .check(pending.state().binding(), &admin, input.proof.now)?;
            self.sessions.consume_pending(&mut tx, input.proof).await?;
            self.sessions.create_pending(&mut tx, pending).await
        }
        .await;
        if let Err(error) = update {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;
        Ok(PasskeyOptions {
            token,
            binding,
            public_options: challenge.public_options,
        })
    }
}

pub struct FinishPasskey<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub credentials: &'a dyn AdminCredentialRepository,
    pub sessions: &'a dyn SessionStore,
    pub audit: &'a dyn AuditRepository,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}

impl FinishPasskey<'_> {
    /// # Errors
    /// Invalid crypto, stale authority/revision, consumed/expired state, or failed audit
    /// never return access. Unknown commit remains an error, not claimed success.
    pub async fn execute(
        &self,
        mut input: FinishPasskeyInput,
    ) -> Result<CompletedSession, DomainError> {
        input.proof.now = self.clock.now().await?;
        let ((admin, credential, pending), protected_state) = self.snapshot(&input).await?;
        let verified = self
            .authenticator
            .finish_passkey(PasskeyFinish {
                request: start(&pending.state, &credential, self.policy)?,
                protected_state,
                response: input.response,
                now: self.clock.now().await?,
            })
            .await?;
        let CredentialMaterial::Passkey { credential_id, .. } = credential.material() else {
            return Err(denied());
        };
        if &verified.credential_id != credential_id {
            return Err(denied());
        }
        let next_revision = credential
            .state()
            .revision()
            .checked_add(1)
            .ok_or_else(denied)?;
        let token = self
            .authenticator
            .issue_token(TokenPurpose::Session)
            .await?;
        let digest = self
            .authenticator
            .token_digest(TokenPurpose::Session, token.clone())
            .await?;
        let session_id = self.ids.next_session_id().await?;
        let event_id = self.ids.next_audit_id().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let update = async {
            let current = self.admins.lock(&mut tx, vec![admin.id()]).await?;
            if current.as_slice() != [admin.clone()] {
                return Err(denied());
            }
            let current = self
                .credentials
                .lock(&mut tx, input.credential)
                .await?
                .ok_or_else(denied)?;
            if current != credential {
                return Err(denied());
            }
            let now = self.clock.now().await?;
            input.proof.now = now;
            if self
                .sessions
                .read_pending(&mut tx, input.proof.clone())
                .await?
                != pending
            {
                return Err(denied());
            }
            self.sessions.consume_pending(&mut tx, input.proof).await?;
            self.credentials
                .compare_revision(
                    &mut tx,
                    PasskeyRevision {
                        id: input.credential,
                        epoch: admin.epoch(),
                        expected_revision: credential.state().revision(),
                        next_revision,
                        credential: verified.credential,
                    },
                )
                .await?;
            let state = session_state(session_id, &admin, now, self.policy)?;
            state.check_access(&admin, now)?;
            self.sessions
                .insert(&mut tx, SessionRecord { digest, state })
                .await?;
            self.audit
                .append(
                    &mut tx,
                    AuditEntry::try_from(AuditInput {
                        id: event_id,
                        event_key: event_id,
                        actor: Some(Actor::Admin(admin.id())),
                        action: Some(AuditAction::SignedIn),
                        subject: Some(Subject::Admin(admin.id())),
                        occurred_at: Some(now),
                        reason: Some(ReasonCode::Authentication),
                        metadata: AuditMetadata::Factor(Factor::Passkey),
                    })?,
                )
                .await
        }
        .await;
        if let Err(error) = update {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;
        Ok(CompletedSession { token })
    }

    async fn snapshot(
        &self,
        input: &FinishPasskeyInput,
    ) -> Result<((AdminState, CredentialRecord, PendingView), SecretBytes), DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let snapshot = async {
            let snapshot = read_snapshot(
                self.admins,
                self.credentials,
                self.sessions,
                &mut tx,
                &input.proof,
                input.credential,
                AuthStage::Passkey,
            )
            .await?;
            let protected = self
                .sessions
                .verification_state(&mut tx, input.proof.clone())
                .await?;
            Ok::<_, DomainError>((snapshot, protected))
        }
        .await;
        match snapshot {
            Ok(value) => {
                self.uow.commit(tx).await?;
                Ok(value)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}

fn session_state(
    id: kernel::ids::SessionId,
    admin: &AdminState,
    now: OffsetDateTime,
    policy: &SecurityPolicy,
) -> Result<SessionState, DomainError> {
    SessionState::try_from(SessionInput {
        id,
        admin_id: admin.id(),
        epoch: admin.epoch(),
        primary_verified: true,
        factor: Some(Factor::Passkey),
        issued_at: now,
        expires_at: expiry(now, policy.session_absolute_ttl())?,
        idle_timeout: policy.session_idle_ttl().try_into().map_err(|_| denied())?,
    })
}

async fn read_snapshot(
    admins: &dyn AdminRepository,
    credentials: &dyn AdminCredentialRepository,
    sessions: &dyn SessionStore,
    tx: &mut TxHandle,
    proof: &PendingCheck,
    id: CredentialId,
    expected_stage: AuthStage,
) -> Result<(AdminState, CredentialRecord, PendingView), DomainError> {
    if proof.binding.stage != expected_stage || proof.binding.purpose != AuthPurpose::SignIn {
        return Err(denied());
    }
    let admin = admins
        .get(tx, proof.binding.target)
        .await?
        .ok_or_else(denied)?;
    if !admin.is_eligible() || admin.epoch() != proof.binding.epoch {
        return Err(denied());
    }
    let pending = sessions.read_pending(tx, proof.clone()).await?;
    let credential = credentials.lock(tx, id).await?.ok_or_else(denied)?;
    let state = credential.state();
    if state.admin_id() != admin.id() || state.kind() != CredentialKind::Passkey {
        return Err(denied());
    }
    state.check_current(
        admin.epoch(),
        proof
            .binding
            .credential_revision
            .unwrap_or(state.revision()),
    )?;
    Ok((admin, credential, pending))
}

fn start(
    state: &AuthState,
    credential: &CredentialRecord,
    policy: &SecurityPolicy,
) -> Result<PasskeyStart, DomainError> {
    let CredentialMaterial::Passkey {
        credential: material,
        ..
    } = credential.material()
    else {
        return Err(denied());
    };
    Ok(PasskeyStart {
        context: CeremonyContext {
            challenge: state.id(),
            binding: state.binding().clone(),
            credential: Some(credential.state().id()),
            primary_verified_at: state.primary_verified_at(),
            created_at: state.created_at(),
            expires_at: state.expires_at(),
        },
        rp_id: policy.rp_id().into(),
        origin: policy.origin().into(),
        credentials: vec![material.clone()],
    })
}
fn expiry(now: OffsetDateTime, ttl: std::time::Duration) -> Result<OffsetDateTime, DomainError> {
    now.checked_add(ttl.try_into().map_err(|_| denied())?)
        .ok_or_else(denied)
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
