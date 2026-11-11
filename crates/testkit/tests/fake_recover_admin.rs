//! T182: independent committed-state recovery and stale-login contracts (not PostgreSQL races).
use admin::{
    application::recover_admin::{RecoverAdmin, RecoverAdminInput},
    domain::{
        admin_state::{
            AdminState, Eligibility, EnrollmentPurpose, EnrollmentStatus, Invalidation,
            SecurityEpoch,
        },
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
        auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState},
        credential_state::{CredentialKind, CredentialState},
        otp_state::{OtpBinding, OtpDigest, OtpInput, OtpState},
        secrets::SecretBytes,
    },
    ports::{
        admin_credential_repository::{CredentialMaterial, CredentialRecord},
        admin_repository::AdminRepository,
        otp_challenge_repository::{OtpChallengeRepository, OtpCheck},
        rate_limit_repository::{
            RateLimitKey, RateLimitPurpose, RateLimitRepository, RateLimitScope,
        },
        session_store::{PendingCheck, PendingRecord, SessionRecord, SessionStore},
    },
};
use kernel::ports::unit_of_work::{TransactionMode, UnitOfWork};
use testkit::fakes::{
    admin_credential_repository::FakeAdminCredentialRepository,
    admin_repository::FakeAdminRepository,
    audit_repository::FakeAuditRepository,
    clock::FixedClock,
    id_generator::SequenceIdGenerator,
    otp_challenge_repository::FakeOtpChallengeRepository,
    rate_limit_repository::FakeRateLimitRepository,
    session_store::FakeSessionStore,
    store::{FakeState, FakeStore},
    unit_of_work::FakeUnitOfWork,
};
#[path = "support/recovery_fixture.rs"]
mod fixture;

#[tokio::test]
async fn admin_recovery_preserves_second_factor_requirement() {
    drive("valid").await;
}
#[tokio::test]
async fn last_admin_lockout_has_no_bypass() {
    for scenario in [
        "self",
        "missing_actor",
        "incomplete",
        "expired",
        "revoked_actor",
        "reason",
        "wrong_reason",
        "missing_target",
        "exhausted",
    ] {
        drive(scenario).await;
    }
}
#[tokio::test]
async fn recovery_revokes_all_admin_sessions_and_old_factors() {
    drive("valid").await;
    drive("audit").await;
    drive("rollback").await;
}
#[tokio::test]
async fn audit_recovery_has_complete_safe_fields_once() {
    drive("valid").await;
    drive("audit").await;
}
#[tokio::test]
async fn recovery_races_with_login_safely_state_contract() {
    drive("login_first").await;
    recovery_before_final_login_check().await;
}

#[expect(
    clippy::too_many_lines,
    reason = "independent multi-admin recovery fixture covers every persisted authentication child"
)]
fn seed(state: &mut FakeState, scenario: &str) {
    let mut actor = fixture::actor();
    if scenario == "revoked_actor" {
        actor
            .invalidate(
                actor.epoch(),
                actor.epoch().advance().unwrap(),
                Invalidation::Revoke,
            )
            .unwrap();
    }
    let target = if scenario == "exhausted" {
        AdminState::new(
            fixture::target().id(),
            Eligibility::Active,
            SecurityEpoch::new(u64::MAX),
        )
    } else {
        fixture::target()
    };
    for (admin, ns, offset) in [
        (&target, vec![21, 22], 0),
        (&actor, vec![23], 40),
        (
            &AdminState::new(fixture::id(8), Eligibility::Active, SecurityEpoch::new(3)),
            vec![24],
            80,
        ),
    ] {
        state.admins.insert(admin.id(), admin.clone());
        for n in ns {
            state.admin_sessions.insert(
                [n; 32],
                SessionRecord {
                    digest: fixture::digest(n),
                    state: fixture::session(admin, n, true),
                },
            );
        }
        for (n, kind) in [(31, CredentialKind::Passkey), (32, CredentialKind::Totp)] {
            let number = n + offset;
            let id = format!("019a53a0-0000-7000-8000-{number:012}")
                .parse()
                .unwrap();
            let material = match kind {
                CredentialKind::Passkey => CredentialMaterial::Passkey {
                    credential_id: SecretBytes::new(vec![number]),
                    credential: SecretBytes::new(vec![number]),
                },
                CredentialKind::Totp => CredentialMaterial::Totp {
                    envelope: SecretBytes::new(vec![number]),
                },
            };
            state.admin_credentials.insert(
                id,
                CredentialRecord::new(
                    CredentialState::new(id, admin.id(), kind, admin.epoch(), 1).unwrap(),
                    material,
                )
                .unwrap(),
            );
        }
        for (n, stage) in [
            (41, AuthStage::PrimaryVerified),
            (42, AuthStage::Passkey),
            (43, AuthStage::Totp),
        ] {
            let number = n + offset;
            let binding = AuthBinding {
                target: admin.id(),
                browser: fixture::digest(number),
                session: format!("019a53a0-0000-7000-8000-{number:012}")
                    .parse()
                    .unwrap(),
                purpose: AuthPurpose::SignIn,
                stage,
                epoch: admin.epoch(),
                credential_revision: (stage != AuthStage::PrimaryVerified).then_some(1),
            };
            let pending = PendingRecord::new(
                fixture::digest(number),
                AuthState::try_from(AuthInput {
                    id: format!("019a53a0-0000-7000-8000-{number:012}")
                        .parse()
                        .unwrap(),
                    binding,
                    primary_verified_at: Some(fixture::now()),
                    created_at: fixture::now(),
                    expires_at: fixture::now() + time::Duration::MINUTE,
                })
                .unwrap(),
                (stage != AuthStage::PrimaryVerified).then(|| SecretBytes::new(vec![number])),
                None,
            )
            .unwrap();
            state.admin_pending.insert([number; 32], pending);
        }
        for n in [61, 62] {
            let number = n + offset;
            let id = format!("019a53a0-0000-7000-8000-{number:012}")
                .parse()
                .unwrap();
            let binding = OtpBinding {
                target: admin.id(),
                browser: fixture::digest(number),
                purpose: if n == 61 {
                    AuthPurpose::SignIn
                } else {
                    AuthPurpose::Enrollment(EnrollmentPurpose::Recovery)
                },
                epoch: if n == 61 {
                    admin.epoch()
                } else {
                    SecurityEpoch::new(0)
                },
            };
            state.admin_otps.insert(
                id,
                OtpState::try_from(OtpInput {
                    id,
                    binding,
                    digest: OtpDigest::new([number; 32]),
                    issued_at: fixture::now(),
                    expires_at: fixture::now() + time::Duration::MINUTE,
                    max_attempts: 3,
                    failed_attempts: 2,
                })
                .unwrap(),
            );
            state.otp_heads.insert(id, id);
        }
    }
    state.admin_sessions.insert(
        [7; 32],
        SessionRecord {
            digest: fixture::digest(7),
            state: fixture::session(&fixture::actor(), 20, scenario != "incomplete"),
        },
    );
    if scenario == "missing_actor" {
        state.admins.remove(&actor.id());
    }
    if scenario == "missing_target" {
        state.admins.remove(&target.id());
    }
}

fn unchanged(before: &FakeState, after: &FakeState) {
    assert_eq!(after.admins, before.admins);
    assert_eq!(after.admin_sessions, before.admin_sessions);
    assert_eq!(after.admin_credentials, before.admin_credentials);
    assert_eq!(after.admin_pending, before.admin_pending);
    assert_eq!(after.admin_otps, before.admin_otps);
    assert_eq!(after.audit_entries, before.audit_entries);
    assert_eq!(after.admin_identities, before.admin_identities);
    assert_eq!(after.otp_heads, before.otp_heads);
    assert_eq!(
        after.rate_limits.keys().collect::<Vec<_>>(),
        before.rate_limits.keys().collect::<Vec<_>>()
    );
    assert_eq!(after.otp_submissions, before.otp_submissions);
}

#[expect(
    clippy::too_many_lines,
    reason = "complete independent committed-state and stale-login security assertions"
)]
async fn drive(scenario: &str) {
    let store = FakeStore::new();
    store.seed(|state| seed(state, scenario));
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let credentials = FakeAdminCredentialRepository::new(store.clone());
    let otp = FakeOtpChallengeRepository::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let clock = FixedClock::new(fixture::now());
    let ids = SequenceIdGenerator::new(100);
    let policy = fixture::policy();
    let rates = FakeRateLimitRepository::new(store.clone());
    let key = RateLimitKey::new(RateLimitScope::Subject, RateLimitPurpose::Verify, [1; 32]);
    let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
    let mut budget = None;
    for _ in 0..4 {
        budget = Some(
            rates
                .increment(&mut tx, key, policy.subject_rate_limit(), fixture::now())
                .await
                .unwrap(),
        );
    }
    assert!(!budget.unwrap().allowed);
    uow.commit(tx).await.unwrap();
    if scenario == "expired" {
        clock.advance(time::Duration::MINUTE);
    }
    if matches!(scenario, "audit" | "rollback") {
        audit.fail_next_append();
    }
    let service = RecoverAdmin {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        credentials: &credentials,
        otp: &otp,
        audit: &audit,
        clock: &clock,
        ids: &ids,
        policy: &policy,
    };
    let before = store.try_read_committed().unwrap();
    let input = RecoverAdminInput {
        actor: fixture::digest(7),
        target: if scenario == "self" {
            fixture::actor().id()
        } else {
            fixture::target().id()
        },
        reason: match scenario {
            "reason" => None,
            "wrong_reason" => Some(ReasonCode::AdminAddition),
            _ => Some(ReasonCode::AdminRecovery),
        },
    };
    let stale = fixture::target();
    if scenario == "login_first" {
        let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
        assert_eq!(
            admins.lock(&mut tx, vec![stale.id()]).await.unwrap(),
            vec![stale.clone()]
        );
        sessions
            .insert(
                &mut tx,
                SessionRecord {
                    digest: fixture::digest(80),
                    state: fixture::session(&stale, 80, true),
                },
            )
            .await
            .unwrap();
        uow.commit(tx).await.unwrap();
    }
    let result = service.execute(input).await;
    let success = matches!(scenario, "valid" | "login_first" | "recovery_first");
    assert_eq!(result.is_ok(), success, "{scenario}");
    let after = store.try_read_committed().unwrap();
    let mut tx = uow.begin(TransactionMode::ReadOnly).await.unwrap();
    assert_eq!(
        rates.read(&mut tx, key, fixture::now()).await.unwrap(),
        budget
    );
    uow.commit(tx).await.unwrap();
    if !success {
        unchanged(&before, &after);
        return;
    }
    assert_eq!(result.unwrap(), ());
    let target = &after.admins[&stale.id()];
    assert_eq!(target.epoch(), stale.epoch().advance().unwrap());
    assert!(!target.is_eligible());
    let authorization = target.enrollment().unwrap();
    assert_ne!(Some(authorization), stale.enrollment());
    assert_eq!(authorization.authorizer(), fixture::actor().id());
    assert_eq!(authorization.purpose(), EnrollmentPurpose::Recovery);
    assert_eq!(authorization.status(), EnrollmentStatus::Pending);
    assert_eq!(authorization.epoch(), target.epoch());
    assert_eq!(authorization.authorized_at(), fixture::now());
    assert_eq!(
        authorization.expires_at(),
        fixture::now() + time::Duration::minutes(5)
    );
    assert_eq!(
        after.admin_sessions.len(),
        before.admin_sessions.len() + usize::from(scenario == "login_first")
    );
    assert_eq!(
        after.admin_credentials.len(),
        before.admin_credentials.len()
    );
    assert_eq!(after.admin_pending.len(), before.admin_pending.len());
    let mut tx = uow.begin(TransactionMode::ReadWrite).await.unwrap();
    for (key, old) in &before.admin_sessions {
        let actual = &after.admin_sessions[key];
        if old.state.admin_id() == target.id() {
            let mut expected = old.clone();
            expected.state.revoke(fixture::now());
            assert_eq!(actual, &expected);
            assert!(
                sessions
                    .check_current(&mut tx, actual.digest.clone(), fixture::now())
                    .await
                    .is_err()
            );
            assert!(old.state.check_access(target, fixture::now()).is_err());
        } else {
            assert_eq!(actual, old);
        }
    }
    for (key, old) in &before.admin_credentials {
        let actual = &after.admin_credentials[key];
        if old.state().admin_id() == target.id() {
            let mut expected = old.state().clone();
            expected.revoke(fixture::now());
            assert_eq!(actual.state(), &expected);
            assert!(actual.state().check_current(stale.epoch(), 1).is_err());
        } else {
            assert_eq!(actual, old);
        }
    }
    for (key, old) in &before.admin_pending {
        let actual = &after.admin_pending[key];
        if old.state().binding().target == target.id() {
            let mut expected = old.clone();
            expected.revoke();
            assert_eq!(actual, &expected);
            assert!(
                sessions
                    .read_pending(
                        &mut tx,
                        PendingCheck {
                            reference: old.reference().clone(),
                            binding: old.state().binding().clone(),
                            now: fixture::now()
                        }
                    )
                    .await
                    .is_err()
            );
        } else {
            assert_eq!(actual, old);
        }
    }
    for (key, old) in &before.admin_otps {
        let actual = &after.admin_otps[key];
        if old.binding().target == target.id() {
            let mut expected = old.clone();
            expected.revoke();
            assert_eq!(actual, &expected);
            assert!(
                otp.consume_once(
                    &mut tx,
                    OtpCheck {
                        id: *key,
                        binding: old.binding().clone(),
                        expected_failed_attempts: 2,
                        now: fixture::now()
                    },
                    old.digest().clone()
                )
                .await
                .is_err()
            );
        } else {
            assert_eq!(actual, old);
        }
    }
    assert_eq!(after.otp_heads, before.otp_heads);
    for (id, old) in &before.admins {
        if *id != target.id() {
            assert_eq!(&after.admins[id], old);
        }
    }
    if scenario == "login_first" {
        assert!(
            sessions
                .check_current(&mut tx, fixture::digest(80), fixture::now())
                .await
                .is_err()
        );
    }
    // Neither a fresh primary-only session nor any pre-recovery factor grants console access.
    assert!(
        fixture::session(target, 81, false)
            .check_access(target, fixture::now())
            .is_err()
    );
    uow.rollback(tx).await.unwrap();
    assert_eq!(after.audit_entries.len(), 1);
    let entry = after.audit_entries.values().next().unwrap();
    assert_eq!(entry.actor(), Actor::Admin(fixture::actor().id()));
    assert_eq!(entry.subject(), Subject::Admin(target.id()));
    assert_eq!(entry.action(), AuditAction::Recovered);
    assert_eq!(entry.reason(), ReasonCode::AdminRecovery);
    assert_eq!(entry.occurred_at(), fixture::now());
    assert_eq!(
        entry.metadata(),
        &AuditMetadata::Eligibility {
            before: true,
            after: false
        }
    );
    assert!(!format!("{entry:?}").contains("secret"));
}

// Pause the real login service before its second (final, locked) transaction.
// SessionStore::insert deliberately does not own authorization/epoch policy.
struct PausedLoginUow<'a> {
    inner: &'a FakeUnitOfWork,
    begins: std::sync::atomic::AtomicUsize,
    ready: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl UnitOfWork for PausedLoginUow<'_> {
    async fn begin(
        &self,
        mode: TransactionMode,
    ) -> Result<kernel::ports::unit_of_work::TxHandle, kernel::error::DomainError> {
        if self
            .begins
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            == 1
        {
            self.ready.notify_one();
            self.resume.notified().await;
        }
        self.inner.begin(mode).await
    }
    async fn commit(
        &self,
        tx: kernel::ports::unit_of_work::TxHandle,
    ) -> Result<(), kernel::error::DomainError> {
        self.inner.commit(tx).await
    }
    async fn rollback(
        &self,
        tx: kernel::ports::unit_of_work::TxHandle,
    ) -> Result<(), kernel::error::DomainError> {
        self.inner.rollback(tx).await
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "deterministic real application interleaving and complete unchanged-state assertions"
)]
async fn recovery_before_final_login_check() {
    use admin::{
        application::totp::{FinishTotp, FinishTotpInput},
        ports::admin_authenticator::{AdminAuthenticator, CeremonyContext},
    };
    use testkit::fakes::admin_authenticator::FakeAdminAuthenticator;
    let store = FakeStore::new();
    store.seed(|state| seed(state, "valid"));
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let credentials = FakeAdminCredentialRepository::new(store.clone());
    let otp = FakeOtpChallengeRepository::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let clock = FixedClock::new(fixture::now());
    let ids = SequenceIdGenerator::new(200);
    let policy = fixture::policy();
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let initial = store.try_read_committed().unwrap();
    let pending = &initial.admin_pending[&[41; 32]];
    let credential = "019a53a0-0000-7000-8000-000000000032".parse().unwrap();
    let mut binding = pending.state().binding().clone();
    binding.stage = AuthStage::Totp;
    binding.credential_revision = Some(1);
    let enrollment = crypto
        .begin_totp(CeremonyContext {
            challenge: pending.state().id(),
            binding,
            credential: Some(credential),
            primary_verified_at: fixture::now(),
            created_at: fixture::now(),
            expires_at: fixture::now() + time::Duration::MINUTE,
        })
        .await
        .unwrap();
    let (code, _) = crypto
        .totp_fixture(&enrollment.envelope, fixture::now())
        .unwrap();
    store.seed(|state| {
        let old = &state.admin_credentials[&credential];
        state.admin_credentials.insert(
            credential,
            CredentialRecord::new(
                old.state().clone(),
                CredentialMaterial::Totp {
                    envelope: enrollment.envelope,
                },
            )
            .unwrap(),
        );
    });
    let gate = PausedLoginUow {
        inner: &uow,
        begins: std::sync::atomic::AtomicUsize::new(0),
        ready: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    };
    let login = FinishTotp {
        uow: &gate,
        admins: &admins,
        sessions: &sessions,
        credentials: &credentials,
        audit: &audit,
        authenticator: &crypto,
        clock: &clock,
        ids: &ids,
        policy: &policy,
    };
    let recovery = RecoverAdmin {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        credentials: &credentials,
        otp: &otp,
        audit: &audit,
        clock: &clock,
        ids: &ids,
        policy: &policy,
    };
    let input = FinishTotpInput {
        proof: PendingCheck {
            reference: pending.reference().clone(),
            binding: pending.state().binding().clone(),
            now: fixture::now(),
        },
        credential,
        code,
    };
    let recover = async {
        gate.ready.notified().await;
        assert_eq!(gate.begins.load(std::sync::atomic::Ordering::SeqCst), 2);
        recovery
            .execute(RecoverAdminInput {
                actor: fixture::digest(7),
                target: fixture::target().id(),
                reason: Some(ReasonCode::AdminRecovery),
            })
            .await
            .unwrap();
        let recovered = store.try_read_committed().unwrap();
        gate.resume.notify_one();
        recovered
    };
    let (result, recovered) = tokio::join!(login.execute(input), recover);
    assert_eq!(
        result.unwrap_err().code(),
        kernel::error::ErrorCode::AuthenticationRequired
    );
    let after = store.try_read_committed().unwrap();
    unchanged(&recovered, &after);
    assert_eq!(after.audit_entries.len(), 1);
    assert!(
        after
            .audit_entries
            .values()
            .all(|entry| entry.action() == AuditAction::Recovered)
    );
    assert_eq!(after.admin_sessions.len(), initial.admin_sessions.len());
    let target = &after.admins[&fixture::target().id()];
    assert_eq!(target.epoch(), fixture::target().epoch().advance().unwrap());
    let authorization = target.enrollment().unwrap();
    assert_eq!(authorization.status(), EnrollmentStatus::Pending);
    assert_eq!(authorization.epoch(), target.epoch());
    assert_eq!(authorization.purpose(), EnrollmentPurpose::Recovery);
    assert_eq!(authorization.authorizer(), fixture::actor().id());
    assert!(authorization.expires_at() > fixture::now());
    for (id, old) in &initial.admins {
        if *id != target.id() {
            assert_eq!(&after.admins[id], old);
        }
    }
    for (key, old) in &initial.admin_sessions {
        if old.state.admin_id() != target.id() {
            assert_eq!(&after.admin_sessions[key], old);
        }
    }
    for (key, old) in &initial.admin_credentials {
        if old.state().admin_id() != target.id() {
            assert_eq!(&after.admin_credentials[key], old);
        }
    }
    for (key, old) in &initial.admin_pending {
        if old.state().binding().target != target.id() {
            assert_eq!(&after.admin_pending[key], old);
        }
    }
    for (key, old) in &initial.admin_otps {
        if old.binding().target != target.id() {
            assert_eq!(&after.admin_otps[key], old);
        }
    }
}
