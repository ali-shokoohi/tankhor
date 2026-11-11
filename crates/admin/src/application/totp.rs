//! FR-002/021: primary proof plus a fresh TOTP step creates one audited session.
use crate::{
    domain::{
        admin_state::AdminState,
        audit::{
            Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, Factor, ReasonCode, Subject,
        },
        auth_state::{AuthPurpose, AuthStage},
        secrets::SecretBytes,
        security_policy::SecurityPolicy,
        session_state::{SessionInput, SessionState},
    },
    ports::{
        admin_authenticator::{AdminAuthenticator, CeremonyContext, TokenPurpose, TotpCheck},
        admin_credential_repository::{
            AdminCredentialRepository, CredentialMaterial, CredentialRecord,
        },
        admin_repository::AdminRepository,
        audit_repository::AuditRepository,
        session_store::{PendingCheck, PendingView, SessionRecord, SessionStore},
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::CredentialId,
    ports::{
        clock::Clock,
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

#[derive(Debug, Clone)]
pub struct FinishTotpInput {
    pub proof: PendingCheck,
    pub credential: CredentialId,
    pub code: SecretBytes,
}

#[derive(Debug)]
pub struct CompletedSession {
    pub token: SecretBytes,
}

pub struct FinishTotp<'a> {
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

impl FinishTotp<'_> {
    /// # Errors
    /// Refuses missing primary proof, invalid crypto, stale authority/step, expiry and
    /// audit failure. Unknown commit is never represented as successful authentication.
    pub async fn execute(
        &self,
        mut input: FinishTotpInput,
    ) -> Result<CompletedSession, DomainError> {
        input.proof.now = self.clock.now().await?;
        let (admin, credential, pending) = self.snapshot(&input).await?;
        let CredentialMaterial::Totp { envelope } = credential.material() else {
            return Err(denied());
        };
        let step = self
            .authenticator
            .verify_totp(TotpCheck {
                context: context(&pending, &credential),
                envelope: envelope.clone(),
                code: input.code,
                now: self.clock.now().await?,
            })
            .await?;
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
            if self
                .admins
                .lock(&mut tx, vec![admin.id()])
                .await?
                .as_slice()
                != [admin.clone()]
            {
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
                .accept_totp_step(
                    &mut tx,
                    input.credential,
                    admin.epoch(),
                    credential.state().revision(),
                    step,
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
                        metadata: AuditMetadata::Factor(Factor::Totp),
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
        input: &FinishTotpInput,
    ) -> Result<(AdminState, CredentialRecord, PendingView), DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let snapshot = async {
            if input.proof.binding.stage != AuthStage::PrimaryVerified
                || input.proof.binding.purpose != AuthPurpose::SignIn
            {
                return Err(denied());
            }
            let admin = self
                .admins
                .get(&mut tx, input.proof.binding.target)
                .await?
                .ok_or_else(denied)?;
            if !admin.is_eligible() || admin.epoch() != input.proof.binding.epoch {
                return Err(denied());
            }
            let pending = self
                .sessions
                .read_pending(&mut tx, input.proof.clone())
                .await?;
            let credential = self
                .credentials
                .lock(&mut tx, input.credential)
                .await?
                .ok_or_else(denied)?;
            if credential.state().admin_id() != admin.id()
                || !matches!(credential.material(), CredentialMaterial::Totp { .. })
            {
                return Err(denied());
            }
            credential
                .state()
                .check_current(admin.epoch(), credential.state().revision())?;
            Ok((admin, credential, pending))
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

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}

fn context(pending: &PendingView, credential: &CredentialRecord) -> CeremonyContext {
    let mut binding = pending.state.binding().clone();
    binding.stage = AuthStage::Totp;
    binding.credential_revision = Some(credential.state().revision());
    CeremonyContext {
        challenge: pending.state.id(),
        binding,
        credential: Some(credential.state().id()),
        primary_verified_at: pending.state.primary_verified_at(),
        created_at: pending.state.created_at(),
        expires_at: pending.state.expires_at(),
    }
}

fn session_state(
    id: kernel::ids::SessionId,
    admin: &AdminState,
    now: time::OffsetDateTime,
    policy: &SecurityPolicy,
) -> Result<SessionState, DomainError> {
    SessionState::try_from(SessionInput {
        id,
        admin_id: admin.id(),
        epoch: admin.epoch(),
        primary_verified: true,
        factor: Some(Factor::Totp),
        issued_at: now,
        expires_at: now
            .checked_add(
                policy
                    .session_absolute_ttl()
                    .try_into()
                    .map_err(|_| denied())?,
            )
            .ok_or_else(denied)?,
        idle_timeout: policy.session_idle_ttl().try_into().map_err(|_| denied())?,
    })
}
