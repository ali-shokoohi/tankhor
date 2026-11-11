//! Target phone proof and protected identity authorization jointly permit enrollment.
use crate::{
    domain::{
        admin_state::AdminState,
        audit::{
            Actor, AuditAction, AuditEntry, AuditInput, AuditMetadata, Factor, ReasonCode, Subject,
        },
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState},
        secrets::SecretBytes,
        security_policy::SecurityPolicy,
    },
    ports::{
        admin_authenticator::{
            AdminAuthenticator, CeremonyContext, PasskeyFinish, PasskeyStart, ProtectionContext,
            TokenPurpose, TotpCheck,
        },
        admin_credential_repository::{
            AdminCredentialRepository, CredentialMaterial, CredentialRecord,
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
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

#[derive(Debug, Clone)]
pub struct BeginPasskeyEnrollmentInput {
    pub proof: PendingCheck,
}
#[derive(Debug)]
pub struct PasskeyEnrollmentOptions {
    pub token: SecretBytes,
    pub binding: AuthBinding,
    pub credential: CredentialId,
    pub public_options: SecretBytes,
}
pub struct BeginPasskeyEnrollment<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub audit: &'a dyn AuditRepository,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}
impl BeginPasskeyEnrollment<'_> {
    /// # Errors
    /// Refuses missing/stale identity authorization or invalid/replayed primary proof.
    pub async fn execute(
        &self,
        mut input: BeginPasskeyEnrollmentInput,
    ) -> Result<PasskeyEnrollmentOptions, DomainError> {
        input.proof.now = self.clock.now().await?;
        let (admin, primary) = self.snapshot(&input.proof).await?;
        let credential = self.ids.next_credential_id().await?;
        let created_at = self.clock.now().await?;
        let mut binding = primary.state.binding().clone();
        binding.stage = AuthStage::Passkey;
        binding.credential_revision = Some(1);
        let authorization = admin.enrollment().ok_or_else(denied)?;
        let expires_at = created_at
            .checked_add(
                time::Duration::try_from(self.policy.ceremony_ttl()).map_err(|_| denied())?,
            )
            .ok_or_else(denied)?
            .min(primary.state.expires_at())
            .min(authorization.expires_at());
        let state = AuthState::try_from(AuthInput {
            id: self.ids.next_challenge_id().await?,
            binding: binding.clone(),
            primary_verified_at: Some(primary.state.primary_verified_at()),
            created_at,
            expires_at,
        })?;
        let challenge = self
            .authenticator
            .begin_passkey(PasskeyStart {
                context: CeremonyContext {
                    challenge: state.id(),
                    binding: binding.clone(),
                    credential: Some(credential),
                    primary_verified_at: state.primary_verified_at(),
                    created_at,
                    expires_at,
                },
                rp_id: self.policy.rp_id().into(),
                origin: self.policy.origin().into(),
                credentials: Vec::new(),
            })
            .await?;
        let token = self
            .authenticator
            .issue_token(TokenPurpose::Ceremony)
            .await?;
        let reference = self
            .authenticator
            .token_digest(TokenPurpose::Ceremony, token.clone())
            .await?;
        let pending = PendingRecord::new(reference, state, Some(challenge.protected_state), None)?;
        let audit_id = self.ids.next_audit_id().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            let current = self.admins.lock(&mut tx, vec![admin.id()]).await?;
            if current.as_slice() != [admin.clone()] {
                return Err(denied());
            }
            input.proof.now = self.clock.now().await?;
            authorize(&admin, &input.proof)?;
            if self
                .sessions
                .read_pending(&mut tx, input.proof.clone())
                .await?
                != primary
            {
                return Err(denied());
            }
            pending
                .state()
                .check(pending.state().binding(), &admin, input.proof.now)?;
            self.sessions.consume_pending(&mut tx, input.proof).await?;
            self.sessions.create_pending(&mut tx, pending).await?;
            self.audit
                .append(
                    &mut tx,
                    AuditEntry::try_from(AuditInput {
                        id: audit_id,
                        event_key: audit_id,
                        actor: Some(Actor::Admin(admin.id())),
                        action: Some(AuditAction::EnrollmentStarted),
                        subject: Some(Subject::Admin(admin.id())),
                        occurred_at: Some(self.clock.now().await?),
                        reason: Some(ReasonCode::Authentication),
                        metadata: AuditMetadata::Factor(Factor::Passkey),
                    })?,
                )
                .await
        }
        .await;
        if let Err(error) = result {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;
        Ok(PasskeyEnrollmentOptions {
            token,
            binding,
            credential,
            public_options: challenge.public_options,
        })
    }

    async fn snapshot(
        &self,
        proof: &PendingCheck,
    ) -> Result<(AdminState, PendingView), DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let admin = self
                .admins
                .get(&mut tx, proof.binding.target)
                .await?
                .ok_or_else(denied)?;
            authorize(&admin, proof)?;
            let primary = self.sessions.read_pending(&mut tx, proof.clone()).await?;
            Ok((admin, primary))
        }
        .await;
        match result {
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
fn authorize(admin: &AdminState, proof: &PendingCheck) -> Result<(), DomainError> {
    if proof.binding.stage != AuthStage::PrimaryVerified
        || proof.binding.credential_revision.is_some()
    {
        return Err(denied());
    }
    let AuthPurpose::Enrollment(purpose) = proof.binding.purpose else {
        return Err(denied());
    };
    admin.check_enrollment(
        proof.binding.target,
        purpose,
        proof.binding.epoch,
        proof.now,
    )
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}

#[derive(Debug, Clone)]
pub struct BeginTotpEnrollmentInput {
    pub proof: PendingCheck,
    /// Authenticated old ceremony to supersede when restarting after a lost response.
    pub predecessor: Option<PendingCheck>,
}

/// Only the authenticated one-time presentation response may render `provisioning`.
/// Its secret wrapper redacts Debug; never serialize this DTO into a URL or a query.
#[derive(Debug)]
pub struct TotpEnrollmentPresentation {
    pub token: SecretBytes,
    pub binding: AuthBinding,
    pub credential: CredentialId,
    pub provisioning: SecretBytes,
    pub no_store: bool,
}

pub struct BeginTotpEnrollment<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub audit: &'a dyn AuditRepository,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub policy: &'a SecurityPolicy,
}

impl BeginTotpEnrollment<'_> {
    /// # Errors
    /// Refuses invalid authority, stale/replayed primary state or unresolved commits.
    /// Decryption failure after commit does not undo the presentation mark.
    pub async fn execute(
        &self,
        mut input: BeginTotpEnrollmentInput,
    ) -> Result<TotpEnrollmentPresentation, DomainError> {
        input.proof.now = self.clock.now().await?;
        let (admin, primary) = self.snapshot(&input.proof).await?;
        let credential = self.ids.next_credential_id().await?;
        let (state, context) = self.ceremony(&admin, &primary, credential).await?;
        let binding = state.binding().clone();
        let enrollment = self.authenticator.begin_totp(context.clone()).await?;
        let token = self
            .authenticator
            .issue_token(TokenPurpose::Ceremony)
            .await?;
        let reference = self
            .authenticator
            .token_digest(TokenPurpose::Ceremony, token.clone())
            .await?;
        let pending = PendingRecord::new(
            reference.clone(),
            state,
            Some(enrollment.envelope),
            Some(enrollment.protected_provisioning),
        )?;
        let audit_id = self.ids.next_audit_id().await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            if self
                .admins
                .lock(&mut tx, vec![admin.id()])
                .await?
                .as_slice()
                != [admin.clone()]
            {
                return Err(denied());
            }
            input.proof.now = self.clock.now().await?;
            authorize(&admin, &input.proof)?;
            if self
                .sessions
                .read_pending(&mut tx, input.proof.clone())
                .await?
                != primary
            {
                return Err(denied());
            }
            pending.state().check(&binding, &admin, input.proof.now)?;
            if let Some(mut predecessor) = input.predecessor {
                if predecessor.binding != binding {
                    return Err(denied());
                }
                predecessor.now = input.proof.now;
                self.sessions
                    .supersede_pending(&mut tx, predecessor)
                    .await?;
            }
            self.sessions
                .consume_pending(&mut tx, input.proof.clone())
                .await?;
            self.sessions.create_pending(&mut tx, pending).await?;
            let envelope = self
                .sessions
                .present_provisioning(
                    &mut tx,
                    PendingCheck {
                        reference,
                        binding: binding.clone(),
                        now: input.proof.now,
                    },
                )
                .await?;
            self.audit
                .append(
                    &mut tx,
                    totp_start_audit(audit_id, &admin, input.proof.now)?,
                )
                .await?;
            Ok(envelope)
        }
        .await;
        let envelope = match result {
            Ok(envelope) => envelope,
            Err(error) => {
                self.uow.rollback(tx).await?;
                return Err(error);
            }
        };
        self.uow.commit(tx).await?;
        let provisioning = self
            .authenticator
            .unprotect(ProtectionContext::Ceremony(context), envelope)
            .await?;
        Ok(TotpEnrollmentPresentation {
            token,
            binding,
            credential,
            provisioning,
            no_store: true,
        })
    }

    async fn ceremony(
        &self,
        admin: &AdminState,
        primary: &PendingView,
        credential: CredentialId,
    ) -> Result<(AuthState, CeremonyContext), DomainError> {
        let created_at = self.clock.now().await?;
        let mut binding = primary.state.binding().clone();
        binding.stage = AuthStage::Totp;
        binding.credential_revision = Some(1);
        let expires_at = created_at
            .checked_add(
                time::Duration::try_from(self.policy.ceremony_ttl()).map_err(|_| denied())?,
            )
            .ok_or_else(denied)?
            .min(primary.state.expires_at())
            .min(admin.enrollment().ok_or_else(denied)?.expires_at());
        let state = AuthState::try_from(AuthInput {
            id: self.ids.next_challenge_id().await?,
            binding: binding.clone(),
            primary_verified_at: Some(primary.state.primary_verified_at()),
            created_at,
            expires_at,
        })?;
        let context = CeremonyContext {
            challenge: state.id(),
            binding,
            credential: Some(credential),
            primary_verified_at: state.primary_verified_at(),
            created_at,
            expires_at,
        };
        Ok((state, context))
    }

    async fn snapshot(
        &self,
        proof: &PendingCheck,
    ) -> Result<(AdminState, PendingView), DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let admin = self
                .admins
                .get(&mut tx, proof.binding.target)
                .await?
                .ok_or_else(denied)?;
            authorize(&admin, proof)?;
            let primary = self.sessions.read_pending(&mut tx, proof.clone()).await?;
            Ok((admin, primary))
        }
        .await;
        match result {
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

fn totp_start_audit(
    id: kernel::ids::AuditId,
    admin: &AdminState,
    now: time::OffsetDateTime,
) -> Result<AuditEntry, DomainError> {
    AuditEntry::try_from(AuditInput {
        id,
        event_key: id,
        actor: Some(Actor::Admin(admin.id())),
        action: Some(AuditAction::EnrollmentStarted),
        subject: Some(Subject::Admin(admin.id())),
        occurred_at: Some(now),
        reason: Some(ReasonCode::Authentication),
        metadata: AuditMetadata::Factor(Factor::Totp),
    })
}

#[derive(Debug, Clone)]
pub struct FinishPasskeyEnrollmentInput {
    pub proof: PendingCheck,
    pub credential: CredentialId,
    pub response: SecretBytes,
}

#[derive(Debug)]
pub struct CompletedEnrollment {
    pub token: SecretBytes,
}

pub struct FinishPasskeyEnrollment<'a> {
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

impl FinishPasskeyEnrollment<'_> {
    /// # Errors
    /// Invalid, stale or replayed ceremonies and failed audit never grant access.
    /// Unknown commit remains an error rather than a returned session capability.
    pub async fn execute(
        &self,
        mut input: FinishPasskeyEnrollmentInput,
    ) -> Result<CompletedEnrollment, DomainError> {
        input.proof.now = self.clock.now().await?;
        let (admin, pending, protected_state) = self.snapshot(&input.proof).await?;
        let verified = self
            .authenticator
            .finish_passkey(PasskeyFinish {
                request: registration_request(&pending.state, input.credential, self.policy),
                protected_state,
                response: input.response,
                now: self.clock.now().await?,
            })
            .await?;
        let credential = CredentialRecord::new(
            crate::domain::credential_state::CredentialState::new(
                input.credential,
                admin.id(),
                crate::domain::credential_state::CredentialKind::Passkey,
                admin.epoch(),
                1,
            )?,
            CredentialMaterial::Passkey {
                credential_id: verified.credential_id,
                credential: verified.credential,
            },
        )?;
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
        let result = async {
            let current = self.admins.lock(&mut tx, vec![admin.id()]).await?;
            if current.as_slice() != [admin.clone()] {
                return Err(denied());
            }
            let now = self.clock.now().await?;
            input.proof.now = now;
            authorize_confirmation(&admin, &input.proof)?;
            if self
                .sessions
                .read_pending(&mut tx, input.proof.clone())
                .await?
                != pending
            {
                return Err(denied());
            }
            self.sessions.consume_pending(&mut tx, input.proof).await?;
            self.admins
                .consume_enrollment(
                    &mut tx,
                    admin.id(),
                    admin.enrollment().ok_or_else(denied)?.clone(),
                    now,
                )
                .await?;
            self.credentials.insert(&mut tx, credential).await?;
            self.admins
                .restore(&mut tx, admin.id(), admin.epoch())
                .await?;
            let state = enrollment_session(session_id, &admin, now, self.policy, Factor::Passkey)?;
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
                        action: Some(AuditAction::EnrollmentConfirmed),
                        subject: Some(Subject::Admin(admin.id())),
                        occurred_at: Some(now),
                        reason: Some(ReasonCode::Authentication),
                        metadata: enrollment_metadata(&admin, Factor::Passkey)?,
                    })?,
                )
                .await
        }
        .await;
        if let Err(error) = result {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;
        Ok(CompletedEnrollment { token })
    }

    async fn snapshot(
        &self,
        proof: &PendingCheck,
    ) -> Result<(AdminState, PendingView, SecretBytes), DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let admin = self
                .admins
                .get(&mut tx, proof.binding.target)
                .await?
                .ok_or_else(denied)?;
            authorize_confirmation(&admin, proof)?;
            let pending = self.sessions.read_pending(&mut tx, proof.clone()).await?;
            let protected_state = self
                .sessions
                .verification_state(&mut tx, proof.clone())
                .await?;
            Ok((admin, pending, protected_state))
        }
        .await;
        match result {
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

fn authorize_confirmation(admin: &AdminState, proof: &PendingCheck) -> Result<(), DomainError> {
    if proof.binding.stage != AuthStage::Passkey || proof.binding.credential_revision != Some(1) {
        return Err(denied());
    }
    let AuthPurpose::Enrollment(purpose) = proof.binding.purpose else {
        return Err(denied());
    };
    admin.check_enrollment(
        proof.binding.target,
        purpose,
        proof.binding.epoch,
        proof.now,
    )
}

fn enrollment_metadata(admin: &AdminState, factor: Factor) -> Result<AuditMetadata, DomainError> {
    use crate::domain::admin_state::{Eligibility, EnrollmentPurpose};
    Ok(AuditMetadata::Enrollment {
        before: match admin.enrollment().ok_or_else(denied)?.purpose() {
            EnrollmentPurpose::Addition => Eligibility::Invited,
            EnrollmentPurpose::Recovery => Eligibility::Recovering,
        },
        after: Eligibility::Active,
        factor,
    })
}

fn registration_request(
    state: &AuthState,
    credential: CredentialId,
    policy: &SecurityPolicy,
) -> PasskeyStart {
    PasskeyStart {
        context: CeremonyContext {
            challenge: state.id(),
            binding: state.binding().clone(),
            credential: Some(credential),
            primary_verified_at: state.primary_verified_at(),
            created_at: state.created_at(),
            expires_at: state.expires_at(),
        },
        rp_id: policy.rp_id().into(),
        origin: policy.origin().into(),
        credentials: Vec::new(),
    }
}

fn enrollment_session(
    id: kernel::ids::SessionId,
    admin: &AdminState,
    now: time::OffsetDateTime,
    policy: &SecurityPolicy,
    factor: Factor,
) -> Result<crate::domain::session_state::SessionState, DomainError> {
    let state = crate::domain::session_state::SessionState::try_from(
        crate::domain::session_state::SessionInput {
            id,
            admin_id: admin.id(),
            epoch: admin.epoch(),
            primary_verified: true,
            factor: Some(factor),
            issued_at: now,
            expires_at: now
                .checked_add(
                    time::Duration::try_from(policy.session_absolute_ttl())
                        .map_err(|_| denied())?,
                )
                .ok_or_else(denied)?,
            idle_timeout: time::Duration::try_from(policy.session_idle_ttl())
                .map_err(|_| denied())?,
        },
    )?;
    let mut active = admin.clone();
    active.restore(admin.epoch())?;
    state.check_access(&active, now)?;
    Ok(state)
}

#[derive(Debug, Clone)]
pub struct FinishTotpEnrollmentInput {
    pub proof: PendingCheck,
    pub credential: CredentialId,
    pub code: SecretBytes,
}
pub struct FinishTotpEnrollment<'a> {
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
impl FinishTotpEnrollment<'_> {
    /// # Errors
    /// Missing authority, stale/replayed proofs, rejected steps or failed audit grant no access.
    /// Unknown commit never returns a session capability.
    pub async fn execute(
        &self,
        mut input: FinishTotpEnrollmentInput,
    ) -> Result<CompletedEnrollment, DomainError> {
        input.proof.now = self.clock.now().await?;
        let (admin, pending, envelope) = self.snapshot(&input.proof).await?;
        let step = self
            .authenticator
            .verify_totp(TotpCheck {
                context: CeremonyContext {
                    challenge: pending.state.id(),
                    binding: pending.state.binding().clone(),
                    credential: Some(input.credential),
                    primary_verified_at: pending.state.primary_verified_at(),
                    created_at: pending.state.created_at(),
                    expires_at: pending.state.expires_at(),
                },
                envelope: envelope.clone(),
                code: input.code,
                now: self.clock.now().await?,
            })
            .await?;
        let credential = CredentialRecord::new(
            crate::domain::credential_state::CredentialState::new(
                input.credential,
                admin.id(),
                crate::domain::credential_state::CredentialKind::Totp,
                admin.epoch(),
                1,
            )?,
            CredentialMaterial::Totp { envelope },
        )?;
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
        let result = async {
            let current = self.admins.lock(&mut tx, vec![admin.id()]).await?;
            if current.as_slice() != [admin.clone()] {
                return Err(denied());
            }
            let now = self.clock.now().await?;
            input.proof.now = now;
            authorize_totp_confirmation(&admin, &input.proof)?;
            if self
                .sessions
                .read_pending(&mut tx, input.proof.clone())
                .await?
                != pending
            {
                return Err(denied());
            }
            self.sessions.consume_pending(&mut tx, input.proof).await?;
            self.admins
                .consume_enrollment(
                    &mut tx,
                    admin.id(),
                    admin.enrollment().ok_or_else(denied)?.clone(),
                    now,
                )
                .await?;
            self.credentials.insert(&mut tx, credential).await?;
            self.credentials
                .accept_totp_step(&mut tx, input.credential, admin.epoch(), 1, step)
                .await?;
            self.admins
                .restore(&mut tx, admin.id(), admin.epoch())
                .await?;
            let state = enrollment_session(session_id, &admin, now, self.policy, Factor::Totp)?;
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
                        action: Some(AuditAction::EnrollmentConfirmed),
                        subject: Some(Subject::Admin(admin.id())),
                        occurred_at: Some(now),
                        reason: Some(ReasonCode::Authentication),
                        metadata: enrollment_metadata(&admin, Factor::Totp)?,
                    })?,
                )
                .await
        }
        .await;
        if let Err(error) = result {
            self.uow.rollback(tx).await?;
            return Err(error);
        }
        self.uow.commit(tx).await?;
        Ok(CompletedEnrollment { token })
    }
    async fn snapshot(
        &self,
        proof: &PendingCheck,
    ) -> Result<(AdminState, PendingView, SecretBytes), DomainError> {
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let admin = self
                .admins
                .get(&mut tx, proof.binding.target)
                .await?
                .ok_or_else(denied)?;
            authorize_totp_confirmation(&admin, proof)?;
            let pending = self.sessions.read_pending(&mut tx, proof.clone()).await?;
            let envelope = self
                .sessions
                .verification_state(&mut tx, proof.clone())
                .await?;
            Ok((admin, pending, envelope))
        }
        .await;
        match result {
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
fn authorize_totp_confirmation(
    admin: &AdminState,
    proof: &PendingCheck,
) -> Result<(), DomainError> {
    if proof.binding.stage != AuthStage::Totp || proof.binding.credential_revision != Some(1) {
        return Err(denied());
    }
    let AuthPurpose::Enrollment(purpose) = proof.binding.purpose else {
        return Err(denied());
    };
    admin.check_enrollment(
        proof.binding.target,
        purpose,
        proof.binding.epoch,
        proof.now,
    )
}
