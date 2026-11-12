#![allow(dead_code)]

use std::{process::Command, str::FromStr, time::Duration as StdDuration};

use admin::{
    application::{
        passkey::{BeginPasskey, BeginPasskeyInput, FinishPasskeyInput},
        request_admin_otp::{RequestAdminOtp, RequestOtp},
        totp::FinishTotpInput,
    },
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        audit::Factor,
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState, BindingDigest},
        credential_state::{CredentialKind, CredentialState},
        otp_state::OtpDigest,
        secrets::SecretBytes,
        security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput},
        session_state::{SessionInput, SessionState},
    },
    ports::{
        admin_authenticator::{
            AdminAuthenticator, CeremonyContext, IssuedOtp, PasskeyChallenge, PasskeyFinish,
            PasskeyStart, ProtectionContext, TokenPurpose, TotpCheck, TotpCredentialBinding,
            TotpEnrollment, VerifiedPasskey,
        },
        admin_credential_repository::{
            AdminCredentialRepository, CredentialMaterial, CredentialRecord,
        },
        admin_repository::{AdminRepository, IdentityDigest},
        session_store::{PendingCheck, PendingRecord, SessionRecord, SessionStore},
    },
};
use infra_auth::{
    admin_authenticator::{RealAdminAuthenticator, totp_credential_aad},
    envelopes::EnvelopeCipher,
    totp::TotpEngine,
};
use infra_postgres::{
    admin_credential_repository::PostgresAdminCredentialRepository,
    admin_repository::PostgresAdminRepository, audit_repository::PostgresAuditRepository,
    otp_challenge_repository::PostgresOtpChallengeRepository,
    rate_limit_repository::PostgresRateLimitRepository, session_store::PostgresSessionStore,
    unit_of_work::PostgresUnitOfWork,
};
use infra_sms::{kavenegar::KavenegarSender, sms_ir::SmsIrSender};
use kernel::{
    ids::{AdminId, ChallengeId, CredentialId, SessionId},
    ports::unit_of_work::{TransactionMode, UnitOfWork},
};
use sqlx::Row;
use testkit::{
    fakes::{clock::FixedClock, id_generator::SequenceIdGenerator},
    fixtures::database::FixtureDatabase,
};
use time::{Duration, OffsetDateTime};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const PHONE: &str = "+989120000001";
const KEY_ID: &str = "real-access-contract";
const API_KEY: &str = "local-wiremock-key";

pub fn now() -> OffsetDateTime {
    time::macros::datetime!(2026-01-01 00:00 UTC)
}

pub fn secret(value: &str) -> SecretBytes {
    SecretBytes::new(value.as_bytes().to_vec())
}

pub fn admin_id() -> AdminId {
    AdminId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap()
}

pub fn actor_id() -> AdminId {
    AdminId::from_str("019a53a0-0000-7000-8000-000000000101").unwrap()
}

pub fn credential_id() -> CredentialId {
    CredentialId::from_str("019a53a0-0000-7000-8000-000000000002").unwrap()
}

pub fn active_admin() -> AdminState {
    AdminState::new(admin_id(), Eligibility::Active, SecurityEpoch::new(0))
}

pub fn active_actor() -> AdminState {
    AdminState::new(actor_id(), Eligibility::Active, SecurityEpoch::new(0))
}

pub fn policy() -> SecurityPolicy {
    SecurityPolicy::try_from(SecurityPolicyInput {
        otp_ttl_seconds: Some(60),
        primary_proof_ttl_seconds: Some(60),
        ceremony_ttl_seconds: Some(60),
        enrollment_ttl_seconds: Some(300),
        session_idle_ttl_seconds: Some(60),
        session_absolute_ttl_seconds: Some(300),
        otp_max_attempts: Some(3),
        subject_rate_limit: Some(RateLimitInput {
            max_attempts: Some(20),
            window_seconds: Some(300),
        }),
        origin_rate_limit: Some(RateLimitInput {
            max_attempts: Some(20),
            window_seconds: Some(300),
        }),
        rp_id: Some("console.example.test".into()),
        origin: Some("https://console.example.test".into()),
    })
    .unwrap()
}

pub struct RealAccess {
    pub database: FixtureDatabase,
    pub uow: PostgresUnitOfWork,
    pub admins: PostgresAdminRepository,
    pub credentials: PostgresAdminCredentialRepository,
    pub sessions: PostgresSessionStore,
    pub otps: PostgresOtpChallengeRepository,
    pub rates: PostgresRateLimitRepository,
    pub audit: PostgresAuditRepository,
    pub authenticator: RealAdminAuthenticator,
    pub clock: FixedClock,
    pub ids: SequenceIdGenerator,
    pub policy: SecurityPolicy,
}

impl RealAccess {
    pub async fn connect(database: FixtureDatabase) -> Self {
        let pool = database
            .connect_runtime_pool(12)
            .await
            .expect("connect restricted real-access pool");
        let uow = PostgresUnitOfWork::new(pool);
        Self {
            database,
            admins: PostgresAdminRepository::new(uow.clone()),
            credentials: PostgresAdminCredentialRepository::new(uow.clone()),
            sessions: PostgresSessionStore::new(uow.clone()),
            otps: PostgresOtpChallengeRepository::new(uow.clone()),
            rates: PostgresRateLimitRepository::new(uow.clone()),
            audit: PostgresAuditRepository::new(uow.clone()),
            authenticator: RealAdminAuthenticator::new(
                [41; 32],
                KEY_ID,
                "console.example.test",
                "https://console.example.test",
            )
            .unwrap(),
            clock: FixedClock::new(now()),
            ids: SequenceIdGenerator::new(1_000),
            policy: policy(),
            uow,
        }
    }

    pub async fn seed_admin(&self, admin: &AdminState) {
        let identity = self
            .authenticator
            .identity_digest(secret(PHONE))
            .await
            .unwrap();
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await.unwrap();
        self.admins
            .invite(&mut tx, admin.clone(), identity)
            .await
            .unwrap();
        self.uow.commit(tx).await.unwrap();
    }

    pub async fn request_otp(
        &self,
        sms: &SmsFixture,
        admin: &AdminState,
    ) -> (ChallengeId, SecretBytes) {
        RequestAdminOtp {
            uow: &self.uow,
            admins: &self.admins,
            rates: &self.rates,
            otps: &self.otps,
            authenticator: &self.authenticator,
            clock: &self.clock,
            ids: &self.ids,
            primary: &sms.primary,
            secondary: &sms.secondary,
            policy: &self.policy,
        }
        .execute(RequestOtp {
            phone: secret("09120000001"),
            browser: BindingDigest::new([7; 32]),
            origin: BindingDigest::new([8; 32]),
            purpose: AuthPurpose::SignIn,
            predecessor: None,
        })
        .await
        .unwrap();
        let requests = sms.server.received_requests().await.unwrap();
        let request = requests
            .iter()
            .find(|request| request.url.path().contains("/verify/lookup.json"))
            .expect("Kavenegar request must reach wiremock");
        let body = std::str::from_utf8(&request.body).unwrap();
        let code = body
            .split('&')
            .find_map(|field| field.strip_prefix("token="))
            .expect("provider form must contain the OTP")
            .to_owned();
        let row = sqlx::query("SELECT id FROM admin_otp_challenges WHERE admin_id = $1")
            .bind(admin.id().as_uuid())
            .fetch_one(self.uow.pool())
            .await
            .unwrap();
        let id = ChallengeId::try_new(row.get::<uuid::Uuid, _>("id")).unwrap();
        (id, secret(&code))
    }

    pub async fn register_passkey(&self, admin: &AdminState) -> CredentialRecord {
        let context = CeremonyContext {
            challenge: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000201").unwrap(),
            binding: AuthBinding {
                target: admin.id(),
                browser: BindingDigest::new([3; 32]),
                session: SessionId::from_str("019a53a0-0000-7000-8000-000000000202").unwrap(),
                purpose: AuthPurpose::SignIn,
                stage: AuthStage::Passkey,
                epoch: admin.epoch(),
                credential_revision: Some(1),
            },
            credential: Some(credential_id()),
            primary_verified_at: now(),
            created_at: now(),
            expires_at: now() + Duration::MINUTE,
        };
        let request = PasskeyStart {
            context,
            rp_id: self.policy.rp_id().into(),
            origin: self.policy.origin().into(),
            credentials: Vec::new(),
        };
        let challenge = self
            .authenticator
            .begin_passkey(request.clone())
            .await
            .unwrap();
        let response = passkey_response("register", &challenge.public_options, 0);
        let verified = self
            .authenticator
            .finish_passkey(PasskeyFinish {
                request,
                protected_state: challenge.protected_state,
                response,
                now: now(),
            })
            .await
            .unwrap();
        CredentialRecord::new(
            CredentialState::new(
                credential_id(),
                admin.id(),
                CredentialKind::Passkey,
                admin.epoch(),
                1,
            )
            .unwrap(),
            CredentialMaterial::Passkey {
                credential_id: verified.credential_id,
                credential: verified.credential,
            },
        )
        .unwrap()
    }

    pub async fn insert_credential(&self, credential: &CredentialRecord) {
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await.unwrap();
        self.credentials
            .insert(&mut tx, credential.clone())
            .await
            .unwrap();
        self.uow.commit(tx).await.unwrap();
    }

    pub async fn prepare_recovery_actor(&self) -> BindingDigest {
        let actor = active_actor();
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await.unwrap();
        self.admins
            .invite(&mut tx, actor.clone(), IdentityDigest::new([90; 32]))
            .await
            .unwrap();
        let token = self
            .authenticator
            .issue_token(TokenPurpose::Session)
            .await
            .unwrap();
        let digest = self
            .authenticator
            .token_digest(TokenPurpose::Session, token)
            .await
            .unwrap();
        self.sessions
            .insert(
                &mut tx,
                SessionRecord {
                    digest: digest.clone(),
                    state: SessionState::try_from(SessionInput {
                        id: SessionId::from_str("019a53a0-0000-7000-8000-000000000501").unwrap(),
                        admin_id: actor.id(),
                        epoch: actor.epoch(),
                        primary_verified: true,
                        factor: Some(Factor::Passkey),
                        issued_at: now(),
                        expires_at: now() + Duration::minutes(5),
                        idle_timeout: self.policy.session_idle_ttl().try_into().unwrap(),
                    })
                    .unwrap(),
                },
            )
            .await
            .unwrap();
        self.uow.commit(tx).await.unwrap();
        digest
    }

    pub async fn admin_epoch(&self, id: AdminId) -> u64 {
        sqlx::query_scalar::<_, String>("SELECT security_epoch::text FROM admins WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_one(self.uow.pool())
            .await
            .unwrap()
            .parse()
            .unwrap()
    }

    pub async fn credential_is_revoked(&self, id: CredentialId) -> bool {
        sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM admin_credentials WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_one(self.uow.pool())
            .await
            .unwrap()
    }

    pub async fn prepare_passkey_login(&self) -> FinishPasskeyInput {
        let admin = active_admin();
        self.seed_admin(&admin).await;
        self.insert_credential(&self.register_passkey(&admin).await)
            .await;
        let binding = AuthBinding {
            target: admin.id(),
            browser: BindingDigest::new([3; 32]),
            session: SessionId::from_str("019a53a0-0000-7000-8000-000000000302").unwrap(),
            purpose: AuthPurpose::SignIn,
            stage: AuthStage::PrimaryVerified,
            epoch: admin.epoch(),
            credential_revision: None,
        };
        let pending = PendingRecord::new(
            BindingDigest::new([5; 32]),
            AuthState::try_from(AuthInput {
                id: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000301").unwrap(),
                binding,
                primary_verified_at: Some(now()),
                created_at: now(),
                expires_at: now() + Duration::MINUTE,
            })
            .unwrap(),
            None,
            None,
        )
        .unwrap();
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await.unwrap();
        self.sessions
            .create_pending(&mut tx, pending)
            .await
            .unwrap();
        self.uow.commit(tx).await.unwrap();
        let started = BeginPasskey {
            uow: &self.uow,
            admins: &self.admins,
            credentials: &self.credentials,
            sessions: &self.sessions,
            authenticator: &self.authenticator,
            clock: &self.clock,
            ids: &self.ids,
            policy: &self.policy,
        }
        .execute(BeginPasskeyInput {
            proof: PendingCheck {
                reference: BindingDigest::new([5; 32]),
                binding: AuthBinding {
                    target: admin.id(),
                    browser: BindingDigest::new([3; 32]),
                    session: SessionId::from_str("019a53a0-0000-7000-8000-000000000302").unwrap(),
                    purpose: AuthPurpose::SignIn,
                    stage: AuthStage::PrimaryVerified,
                    epoch: admin.epoch(),
                    credential_revision: None,
                },
                now: now(),
            },
            credential: credential_id(),
        })
        .await
        .unwrap();
        let response = passkey_response("assert", &started.public_options, 1);
        let reference = self
            .authenticator
            .token_digest(TokenPurpose::Ceremony, started.token)
            .await
            .unwrap();
        FinishPasskeyInput {
            proof: PendingCheck {
                reference,
                binding: started.binding,
                now: now(),
            },
            credential: credential_id(),
            response,
        }
    }

    pub async fn prepare_totp_login(&self) -> (FinishTotpInput, u64) {
        let admin = active_admin();
        self.seed_admin(&admin).await;
        let binding = AuthBinding {
            target: admin.id(),
            browser: BindingDigest::new([3; 32]),
            session: SessionId::from_str("019a53a0-0000-7000-8000-000000000402").unwrap(),
            purpose: AuthPurpose::SignIn,
            stage: AuthStage::PrimaryVerified,
            epoch: admin.epoch(),
            credential_revision: None,
        };
        let challenge = ChallengeId::from_str("019a53a0-0000-7000-8000-000000000401").unwrap();
        let pending = PendingRecord::new(
            BindingDigest::new([11; 32]),
            AuthState::try_from(AuthInput {
                id: challenge,
                binding: binding.clone(),
                primary_verified_at: Some(now()),
                created_at: now(),
                expires_at: now() + Duration::MINUTE,
            })
            .unwrap(),
            None,
            None,
        )
        .unwrap();
        let mut ceremony_binding = binding.clone();
        ceremony_binding.stage = AuthStage::Totp;
        ceremony_binding.credential_revision = Some(1);
        let context = CeremonyContext {
            challenge,
            binding: ceremony_binding,
            credential: Some(credential_id()),
            primary_verified_at: now(),
            created_at: now(),
            expires_at: now() + Duration::MINUTE,
        };
        let enrollment = self
            .authenticator
            .begin_totp(context.clone())
            .await
            .unwrap();
        let secret = EnvelopeCipher::new([41; 32])
            .open(
                KEY_ID,
                &totp_credential_aad(&TotpCredentialBinding::try_from(&context).unwrap()),
                &enrollment.envelope,
            )
            .unwrap();
        let engine = TotpEngine::new();
        let code = engine.current_code(&secret, now()).unwrap();
        let step = engine.verify(&secret, &code, now()).unwrap();
        self.insert_credential(
            &CredentialRecord::new(
                CredentialState::new(
                    credential_id(),
                    admin.id(),
                    CredentialKind::Totp,
                    admin.epoch(),
                    1,
                )
                .unwrap(),
                CredentialMaterial::Totp {
                    envelope: enrollment.envelope,
                },
            )
            .unwrap(),
        )
        .await;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await.unwrap();
        self.sessions
            .create_pending(&mut tx, pending)
            .await
            .unwrap();
        self.uow.commit(tx).await.unwrap();
        (
            FinishTotpInput {
                proof: PendingCheck {
                    reference: BindingDigest::new([11; 32]),
                    binding,
                    now: now(),
                },
                credential: credential_id(),
                code,
            },
            step,
        )
    }

    pub async fn last_totp_step(&self, id: CredentialId) -> Option<u64> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT last_totp_step::text FROM admin_credentials WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_one(self.uow.pool())
        .await
        .unwrap()
        .map(|value| value.parse().unwrap())
    }

    pub async fn live_pending_count(&self) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM admin_session_states
             WHERE consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
        )
        .fetch_one(self.uow.pool())
        .await
        .unwrap()
    }

    pub async fn credential_revision(&self, id: CredentialId) -> u64 {
        sqlx::query_scalar::<_, String>(
            "SELECT credential_revision::text FROM admin_credentials WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_one(self.uow.pool())
        .await
        .unwrap()
        .parse()
        .unwrap()
    }

    pub async fn session_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM admin_sessions")
            .fetch_one(self.uow.pool())
            .await
            .unwrap()
    }

    pub async fn audit_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(self.uow.pool())
            .await
            .unwrap()
    }

    pub async fn active_credential_count(&self, admin: AdminId) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM admin_credentials WHERE admin_id = $1 AND revoked_at IS NULL",
        )
        .bind(admin.as_uuid())
        .fetch_one(self.uow.pool())
        .await
        .unwrap()
    }

    pub async fn close(self) {
        self.uow.pool().close().await;
        self.database.close().await;
    }
}

pub struct CoordinatedAuthenticator<'a> {
    inner: &'a RealAdminAuthenticator,
    passkey_barrier: Option<&'a tokio::sync::Barrier>,
    otp_barrier: Option<&'a tokio::sync::Barrier>,
    totp_barrier: Option<&'a tokio::sync::Barrier>,
    totp_ready: Option<&'a tokio::sync::Notify>,
    totp_resume: Option<&'a tokio::sync::Notify>,
}

impl<'a> CoordinatedAuthenticator<'a> {
    pub fn passkey_race(
        inner: &'a RealAdminAuthenticator,
        barrier: &'a tokio::sync::Barrier,
    ) -> Self {
        Self {
            inner,
            passkey_barrier: Some(barrier),
            otp_barrier: None,
            totp_barrier: None,
            totp_ready: None,
            totp_resume: None,
        }
    }

    pub fn otp_race(inner: &'a RealAdminAuthenticator, barrier: &'a tokio::sync::Barrier) -> Self {
        Self {
            inner,
            passkey_barrier: None,
            otp_barrier: Some(barrier),
            totp_barrier: None,
            totp_ready: None,
            totp_resume: None,
        }
    }

    pub fn totp_race(inner: &'a RealAdminAuthenticator, barrier: &'a tokio::sync::Barrier) -> Self {
        Self {
            inner,
            passkey_barrier: None,
            otp_barrier: None,
            totp_barrier: Some(barrier),
            totp_ready: None,
            totp_resume: None,
        }
    }

    pub fn paused_totp(
        inner: &'a RealAdminAuthenticator,
        ready: &'a tokio::sync::Notify,
        resume: &'a tokio::sync::Notify,
    ) -> Self {
        Self {
            inner,
            passkey_barrier: None,
            otp_barrier: None,
            totp_barrier: None,
            totp_ready: Some(ready),
            totp_resume: Some(resume),
        }
    }
}

#[async_trait::async_trait]
impl AdminAuthenticator for CoordinatedAuthenticator<'_> {
    async fn identity_digest(
        &self,
        phone: SecretBytes,
    ) -> Result<IdentityDigest, kernel::error::DomainError> {
        self.inner.identity_digest(phone).await
    }

    async fn issue_otp(
        &self,
        challenge: ChallengeId,
    ) -> Result<IssuedOtp, kernel::error::DomainError> {
        self.inner.issue_otp(challenge).await
    }

    async fn verify_otp(
        &self,
        challenge: ChallengeId,
        code: SecretBytes,
        digest: OtpDigest,
    ) -> Result<bool, kernel::error::DomainError> {
        let valid = self.inner.verify_otp(challenge, code, digest).await?;
        if let Some(barrier) = self.otp_barrier {
            barrier.wait().await;
        }
        Ok(valid)
    }

    async fn issue_token(
        &self,
        purpose: TokenPurpose,
    ) -> Result<SecretBytes, kernel::error::DomainError> {
        self.inner.issue_token(purpose).await
    }

    async fn token_digest(
        &self,
        purpose: TokenPurpose,
        token: SecretBytes,
    ) -> Result<BindingDigest, kernel::error::DomainError> {
        self.inner.token_digest(purpose, token).await
    }

    async fn protect(
        &self,
        context: ProtectionContext,
        plaintext: SecretBytes,
    ) -> Result<SecretBytes, kernel::error::DomainError> {
        self.inner.protect(context, plaintext).await
    }

    async fn unprotect(
        &self,
        context: ProtectionContext,
        envelope: SecretBytes,
    ) -> Result<SecretBytes, kernel::error::DomainError> {
        self.inner.unprotect(context, envelope).await
    }

    async fn begin_passkey(
        &self,
        request: PasskeyStart,
    ) -> Result<PasskeyChallenge, kernel::error::DomainError> {
        self.inner.begin_passkey(request).await
    }

    async fn finish_passkey(
        &self,
        request: PasskeyFinish,
    ) -> Result<VerifiedPasskey, kernel::error::DomainError> {
        let verified = self.inner.finish_passkey(request).await?;
        if let Some(barrier) = self.passkey_barrier {
            barrier.wait().await;
        }
        Ok(verified)
    }

    async fn begin_totp(
        &self,
        context: CeremonyContext,
    ) -> Result<TotpEnrollment, kernel::error::DomainError> {
        self.inner.begin_totp(context).await
    }

    async fn verify_totp(&self, request: TotpCheck) -> Result<u64, kernel::error::DomainError> {
        let step = self.inner.verify_totp(request).await?;
        if let Some(barrier) = self.totp_barrier {
            barrier.wait().await;
        }
        if let Some(ready) = self.totp_ready {
            ready.notify_one();
            self.totp_resume.unwrap().notified().await;
        }
        Ok(step)
    }
}

pub struct SmsFixture {
    server: MockServer,
    primary: KavenegarSender,
    secondary: SmsIrSender,
}

impl SmsFixture {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("/v1/{API_KEY}/verify/lookup.json")))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"return":{"status":200}}"#),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/send/verify"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":1}"#))
            .mount(&server)
            .await;
        let primary = KavenegarSender::new(
            &server.uri(),
            API_KEY,
            "admin-otp",
            StdDuration::from_secs(2),
        )
        .unwrap();
        let secondary = SmsIrSender::new(
            &server.uri(),
            API_KEY,
            42,
            "code",
            StdDuration::from_secs(2),
        )
        .unwrap();
        Self {
            server,
            primary,
            secondary,
        }
    }
}

pub fn passkey_response(operation: &str, options: &SecretBytes, counter: u64) -> SecretBytes {
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../infra-auth/tests/fixtures/webauthn_fixture.py"
        ))
        .arg(operation)
        .arg(String::from_utf8(options.expose().to_vec()).unwrap())
        .arg("https://console.example.test")
        .arg(counter.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "local signed passkey fixture failed"
    );
    SecretBytes::new(output.stdout)
}
