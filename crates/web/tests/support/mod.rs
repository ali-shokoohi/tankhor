#![allow(dead_code)]

use std::sync::Arc;

use admin::domain::security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput};
use admin::ports::otp_challenge_repository::SubmissionCategory;
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator,
    admin_credential_repository::FakeAdminCredentialRepository,
    admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
    change_repository::FakeChangeRepository, clock::FixedClock,
    flag_repository::FakeFlagRepository, id_generator::SequenceIdGenerator,
    otp_challenge_repository::FakeOtpChallengeRepository,
    rate_limit_repository::FakeRateLimitRepository, session_store::FakeSessionStore,
    settings_repository::FakeSettingsRepository, sms_sender::FakeSmsSender, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
use time::OffsetDateTime;
use web::console::ConsoleServices;
use web::i18n::ConsoleCatalog;
use web::security::CsrfProtectionState;

pub const ORIGIN: &str = "https://tankhor.test";
pub const CSRF: &str = "browser-bound-test-csrf";

pub struct Fixture {
    pub services: ConsoleServices,
    pub store: FakeStore,
    pub authenticator: Arc<FakeAdminAuthenticator>,
    pub clock: Arc<FixedClock>,
}

impl Fixture {
    pub fn new() -> Self {
        let store = FakeStore::new();
        let authenticator = Arc::new(FakeAdminAuthenticator::controlled_fixture());
        let clock = Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH));
        let sms = Arc::new(FakeSmsSender::new(vec![SubmissionCategory::Accepted; 20]));
        let policy = SecurityPolicy::try_from(SecurityPolicyInput {
            otp_ttl_seconds: Some(60),
            primary_proof_ttl_seconds: Some(60),
            ceremony_ttl_seconds: Some(60),
            enrollment_ttl_seconds: Some(300),
            session_idle_ttl_seconds: Some(60),
            session_absolute_ttl_seconds: Some(300),
            otp_max_attempts: Some(3),
            subject_rate_limit: Some(RateLimitInput {
                max_attempts: Some(30),
                window_seconds: Some(300),
            }),
            origin_rate_limit: Some(RateLimitInput {
                max_attempts: Some(50),
                window_seconds: Some(300),
            }),
            rp_id: Some("console.example.test".into()),
            origin: Some("https://console.example.test".into()),
        })
        .unwrap();
        let services = ConsoleServices {
            uow: Arc::new(FakeUnitOfWork::new(store.clone())),
            admins: Arc::new(FakeAdminRepository::new(store.clone())),
            credentials: Arc::new(FakeAdminCredentialRepository::new(store.clone())),
            sessions: Arc::new(FakeSessionStore::new(store.clone())),
            otps: Arc::new(FakeOtpChallengeRepository::new(store.clone())),
            rates: Arc::new(FakeRateLimitRepository::new(store.clone())),
            audit: Arc::new(FakeAuditRepository::new(store.clone())),
            settings: Arc::new(FakeSettingsRepository::new(store.clone())),
            flags: Arc::new(FakeFlagRepository::new(store.clone())),
            changes: Arc::new(FakeChangeRepository::new(store.clone())),
            authenticator: authenticator.clone(),
            primary_sms: sms.clone(),
            secondary_sms: sms,
            clock: clock.clone(),
            ids: Arc::new(SequenceIdGenerator::new(1)),
            policy: Arc::new(policy),
        };
        Self {
            services,
            store,
            authenticator,
            clock,
        }
    }

    pub fn catalog(&self) -> ConsoleCatalog {
        let _ = &self.store;
        ConsoleCatalog::load_authoritative().unwrap()
    }

    pub fn csrf(&self) -> CsrfProtectionState {
        CsrfProtectionState::new(ORIGIN, self.catalog(), kernel::locale::Locale::En).unwrap()
    }
}

pub async fn seed_console_session(
    fixture: &Fixture,
    admin: kernel::ids::AdminId,
    session: kernel::ids::SessionId,
) -> String {
    use admin::{
        domain::{
            admin_state::{AdminState, Eligibility, SecurityEpoch},
            audit::Factor,
            session_state::{SessionInput, SessionState},
        },
        ports::{
            admin_authenticator::{AdminAuthenticator, TokenPurpose},
            session_store::SessionRecord,
        },
    };
    use time::Duration;

    fixture.store.seed(|state| {
        state.admins.insert(
            admin,
            AdminState::new(admin, Eligibility::Active, SecurityEpoch::new(0)),
        );
    });
    let token = fixture
        .authenticator
        .issue_token(TokenPurpose::Session)
        .await
        .unwrap();
    let digest = fixture
        .authenticator
        .token_digest(TokenPurpose::Session, token.clone())
        .await
        .unwrap();
    let state = SessionState::try_from(SessionInput {
        id: session,
        admin_id: admin,
        epoch: SecurityEpoch::new(0),
        primary_verified: true,
        factor: Some(Factor::Totp),
        issued_at: OffsetDateTime::UNIX_EPOCH,
        expires_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(60),
        idle_timeout: Duration::seconds(60),
    })
    .unwrap();
    fixture.store.seed(|committed| {
        committed
            .admin_sessions
            .insert(*digest.as_bytes(), SessionRecord { digest, state });
    });
    String::from_utf8(token.expose().to_vec()).unwrap()
}

pub struct EnrollmentFixture {
    pub target: kernel::ids::AdminId,
    pub session: kernel::ids::SessionId,
    pub proof: String,
    pub browser: String,
}

pub async fn seed_enrollment(
    fixture: &Fixture,
    purpose: admin::domain::admin_state::EnrollmentPurpose,
) -> EnrollmentFixture {
    use admin::{
        domain::{
            admin_state::{AdminState, Eligibility, SecurityEpoch},
            auth_state::{AuthBinding, AuthInput, AuthPurpose, AuthStage, AuthState},
        },
        ports::{
            admin_authenticator::{AdminAuthenticator, TokenPurpose},
            session_store::PendingRecord,
        },
    };
    use kernel::ports::clock::Clock;
    use time::Duration;

    let now = fixture.clock.now().await.unwrap();
    let authorizer = fixture.services.ids.next_admin_id().await.unwrap();
    let target = fixture.services.ids.next_admin_id().await.unwrap();
    let actor = AdminState::new(authorizer, Eligibility::Active, SecurityEpoch::new(0));
    let mut admin = AdminState::new(target, Eligibility::Invited, SecurityEpoch::new(0));
    admin
        .authorize_enrollment(
            &actor,
            purpose,
            admin.epoch(),
            now,
            now + Duration::minutes(5),
        )
        .unwrap();
    let proof = fixture
        .authenticator
        .issue_token(TokenPurpose::PrimaryProof)
        .await
        .unwrap();
    let browser = fixture
        .authenticator
        .issue_token(TokenPurpose::Browser)
        .await
        .unwrap();
    let reference = fixture
        .authenticator
        .token_digest(TokenPurpose::PrimaryProof, proof.clone())
        .await
        .unwrap();
    let browser_digest = fixture
        .authenticator
        .token_digest(TokenPurpose::Browser, browser.clone())
        .await
        .unwrap();
    let session = fixture.services.ids.next_session_id().await.unwrap();
    let binding = AuthBinding {
        target,
        browser: browser_digest,
        session,
        purpose: AuthPurpose::Enrollment(purpose),
        stage: AuthStage::PrimaryVerified,
        epoch: SecurityEpoch::new(0),
        credential_revision: None,
    };
    let pending = PendingRecord::new(
        reference,
        AuthState::try_from(AuthInput {
            id: "019a53a0-0000-7000-8000-000000000006".parse().unwrap(),
            binding,
            primary_verified_at: Some(now),
            created_at: now,
            expires_at: now + Duration::minutes(1),
        })
        .unwrap(),
        None,
        None,
    )
    .unwrap();
    fixture.store.seed(|state| {
        state.admins.insert(target, admin);
        state
            .admin_pending
            .insert(*pending.reference().as_bytes(), pending);
    });
    EnrollmentFixture {
        target,
        session,
        proof: String::from_utf8(proof.expose().to_vec()).unwrap(),
        browser: String::from_utf8(browser.expose().to_vec()).unwrap(),
    }
}
