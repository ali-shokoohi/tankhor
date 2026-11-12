//! Application-wide adapter composition for delivered foundational services.
//!
//! Resolves [`AdapterMode`] (`real` or `fake`, never mixed) strictly after
//! [`AppConfig`](crate::config::AppConfig) has rejected `TANKHOR_ADAPTERS=fake` in `production`.

use std::sync::Arc;

use admin::ports::health_probe::HealthProbe;

use crate::config::{AdapterMode, AppConfig};

/// Errors returned when constructing runtime service adapters for `serve`.
#[derive(Debug, thiserror::Error)]
pub enum CompositionError {
    /// `TANKHOR_ADAPTERS=fake` was requested in a build compiled without `dev-fakes`.
    #[error(
        "TANKHOR_ADAPTERS=fake requires debug feature `dev-fakes` and is unavailable in default/release builds"
    )]
    FakeAdaptersNotCompiled,
    /// `TANKHOR_ADAPTERS=real` was requested before real `HealthProbe` adapter delivery (T356).
    #[error(
        "TANKHOR_ADAPTERS=real health probe and database seeding are not yet delivered in Phase 2"
    )]
    RealHealthProbeNotDelivered,
    /// Real access configuration was missing or invalid; sensitive values are never included.
    #[error("invalid real administrator access configuration")]
    InvalidAccessConfiguration,
}

/// Assembled foundational runtime services ready for HTTP serving.
pub struct FoundationComposition {
    health_probe: Arc<dyn HealthProbe>,
}

impl FoundationComposition {
    /// Shared [`HealthProbe`] wired for `/api/v1/health`.
    #[must_use]
    pub fn health_probe(&self) -> Arc<dyn HealthProbe> {
        Arc::clone(&self.health_probe)
    }
}

/// Constructs foundational adapters according to the single application-wide [`AdapterMode`].
///
/// In `dev-fakes` builds with `TANKHOR_ADAPTERS=fake`, wires an unseeded in-memory
/// `FakeHealthProbe` (which honestly reports `ProbeReadiness::Unseeded` -> HTTP 503 until
/// seeded, never fabricating readiness from an absent backend).
///
/// # Errors
///
/// Returns [`CompositionError`] if fake adapters were not compiled in or if real storage
/// readiness is requested before its Phase 4 adapter task.
pub fn compose_foundation(config: &AppConfig) -> Result<FoundationComposition, CompositionError> {
    match config.adapter_mode() {
        AdapterMode::Fake => compose_fake_foundation(),
        AdapterMode::Real => Err(CompositionError::RealHealthProbeNotDelivered),
    }
}

#[cfg(feature = "dev-fakes")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Fake-enabled and fake-disabled builds share the same fallible composition signature"
)]
fn compose_fake_foundation() -> Result<FoundationComposition, CompositionError> {
    use testkit::fakes::health_probe::FakeHealthProbe;
    use testkit::fakes::store::FakeStore;

    let store = FakeStore::new();
    let health_probe: Arc<dyn HealthProbe> = Arc::new(FakeHealthProbe::new(store));
    Ok(FoundationComposition { health_probe })
}

#[cfg(not(feature = "dev-fakes"))]
fn compose_fake_foundation() -> Result<FoundationComposition, CompositionError> {
    Err(CompositionError::FakeAdaptersNotCompiled)
}

/// Complete backend group selected for the administrator access slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendGroup {
    Real,
    Fake,
}

/// Sensitive real-adapter configuration. Deliberately has no diagnostic representation.
pub struct RealAccessConfig {
    pub database_url: String,
    pub encryption_key: [u8; 32],
    pub key_id: String,
    pub kavenegar_url: String,
    pub kavenegar_key: String,
    pub kavenegar_template: String,
    pub sms_ir_url: String,
    pub sms_ir_key: String,
    pub sms_ir_template: u64,
    pub sms_ir_parameter: String,
    pub sms_timeout: std::time::Duration,
    pub policy: admin::domain::security_policy::SecurityPolicy,
}

impl RealAccessConfig {
    /// Load protected real-adapter configuration without logging or reflecting secret values.
    ///
    /// # Errors
    /// Returns a sanitized configuration error when any required value is missing or invalid.
    pub fn from_env() -> Result<Self, CompositionError> {
        use admin::domain::security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput};

        fn required(key: &str) -> Result<String, CompositionError> {
            std::env::var(key)
                .ok()
                .filter(|value| !value.is_empty())
                .ok_or(CompositionError::InvalidAccessConfiguration)
        }
        fn number(key: &str) -> Result<i64, CompositionError> {
            required(key)?
                .parse()
                .map_err(|_| CompositionError::InvalidAccessConfiguration)
        }
        fn rate(prefix: &str) -> Result<RateLimitInput, CompositionError> {
            Ok(RateLimitInput {
                max_attempts: Some(number(&format!("{prefix}_MAX_ATTEMPTS"))?),
                window_seconds: Some(number(&format!("{prefix}_WINDOW_SECONDS"))?),
            })
        }
        let key = required("TANKHOR_AUTH_ENCRYPTION_KEY")?;
        if key.len() != 64 {
            return Err(CompositionError::InvalidAccessConfiguration);
        }
        let mut encryption_key = [0_u8; 32];
        for (index, output) in encryption_key.iter_mut().enumerate() {
            *output = u8::from_str_radix(&key[index * 2..index * 2 + 2], 16)
                .map_err(|_| CompositionError::InvalidAccessConfiguration)?;
        }
        let policy = SecurityPolicy::try_from(SecurityPolicyInput {
            otp_ttl_seconds: Some(number("TANKHOR_OTP_TTL_SECONDS")?),
            primary_proof_ttl_seconds: Some(number("TANKHOR_PRIMARY_PROOF_TTL_SECONDS")?),
            ceremony_ttl_seconds: Some(number("TANKHOR_CEREMONY_TTL_SECONDS")?),
            enrollment_ttl_seconds: Some(number("TANKHOR_ENROLLMENT_TTL_SECONDS")?),
            session_idle_ttl_seconds: Some(number("TANKHOR_SESSION_IDLE_TTL_SECONDS")?),
            session_absolute_ttl_seconds: Some(number("TANKHOR_SESSION_ABSOLUTE_TTL_SECONDS")?),
            otp_max_attempts: Some(number("TANKHOR_OTP_MAX_ATTEMPTS")?),
            subject_rate_limit: Some(rate("TANKHOR_SUBJECT_RATE")?),
            origin_rate_limit: Some(rate("TANKHOR_ORIGIN_RATE")?),
            rp_id: Some(required("TANKHOR_RP_ID")?),
            origin: Some(required("TANKHOR_AUTH_ORIGIN")?),
        })
        .map_err(|_| CompositionError::InvalidAccessConfiguration)?;
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            encryption_key,
            key_id: required("TANKHOR_AUTH_KEY_ID")?,
            kavenegar_url: required("TANKHOR_KAVENEGAR_URL")?,
            kavenegar_key: required("TANKHOR_KAVENEGAR_KEY")?,
            kavenegar_template: required("TANKHOR_KAVENEGAR_TEMPLATE")?,
            sms_ir_url: required("TANKHOR_SMS_IR_URL")?,
            sms_ir_key: required("TANKHOR_SMS_IR_KEY")?,
            sms_ir_template: number("TANKHOR_SMS_IR_TEMPLATE")?
                .try_into()
                .map_err(|_| CompositionError::InvalidAccessConfiguration)?,
            sms_ir_parameter: required("TANKHOR_SMS_IR_PARAMETER")?,
            sms_timeout: std::time::Duration::from_secs(
                number("TANKHOR_SMS_TIMEOUT_SECONDS")?
                    .try_into()
                    .map_err(|_| CompositionError::InvalidAccessConfiguration)?,
            ),
            policy,
        })
    }
}

/// Select one complete access backend group after application configuration validation.
///
/// # Errors
/// Returns sanitized real configuration errors or rejects unavailable fake compilation.
pub fn compose_access(config: &AppConfig) -> Result<AccessComposition, CompositionError> {
    match config.adapter_mode() {
        AdapterMode::Real => compose_real_access(RealAccessConfig::from_env()?),
        AdapterMode::Fake => compose_fake_access(debug_fake_policy()?),
    }
}

#[cfg(feature = "dev-fakes")]
fn debug_fake_policy() -> Result<admin::domain::security_policy::SecurityPolicy, CompositionError> {
    use admin::domain::security_policy::{RateLimitInput, SecurityPolicyInput};
    admin::domain::security_policy::SecurityPolicy::try_from(SecurityPolicyInput {
        otp_ttl_seconds: Some(300),
        primary_proof_ttl_seconds: Some(300),
        ceremony_ttl_seconds: Some(300),
        enrollment_ttl_seconds: Some(900),
        session_idle_ttl_seconds: Some(900),
        session_absolute_ttl_seconds: Some(3600),
        otp_max_attempts: Some(5),
        subject_rate_limit: Some(RateLimitInput {
            max_attempts: Some(5),
            window_seconds: Some(300),
        }),
        origin_rate_limit: Some(RateLimitInput {
            max_attempts: Some(5),
            window_seconds: Some(300),
        }),
        rp_id: Some("console.example.test".to_owned()),
        origin: Some("https://console.example.test".to_owned()),
    })
    .map_err(|_| CompositionError::InvalidAccessConfiguration)
}

#[cfg(not(feature = "dev-fakes"))]
fn debug_fake_policy() -> Result<admin::domain::security_policy::SecurityPolicy, CompositionError> {
    Err(CompositionError::FakeAdaptersNotCompiled)
}

/// One coherent set of administrator access ports.
pub struct AccessComposition {
    services: web::console::ConsoleServices,
    backend: BackendGroup,
    catalog: &'static [&'static str; 13],
    #[cfg(feature = "dev-fakes")]
    fake_store: Option<testkit::fakes::store::FakeStore>,
}

impl AccessComposition {
    #[must_use]
    pub fn services(&self) -> web::console::ConsoleServices {
        self.services.clone()
    }

    #[must_use]
    pub const fn backend(&self) -> BackendGroup {
        self.backend
    }

    #[must_use]
    pub const fn catalog(&self) -> &'static [&'static str; 13] {
        self.catalog
    }

    /// The shared store is exposed only in debug fake builds for integration assertions.
    ///
    /// # Panics
    /// Panics if called on a real composition.
    #[cfg(feature = "dev-fakes")]
    #[must_use]
    pub fn store(&self) -> &testkit::fakes::store::FakeStore {
        self.fake_store
            .as_ref()
            .expect("fake store requested from real composition")
    }
}

const REAL_ACCESS_CATALOG: [&str; 13] = [
    "real:unit-of-work",
    "real:admins",
    "real:credentials",
    "real:sessions",
    "real:otp-challenges",
    "real:rate-limits",
    "real:audit",
    "real:authenticator",
    "real:primary-sms",
    "real:secondary-sms",
    "real:clock",
    "real:ids",
    "real:policy",
];

#[cfg(feature = "dev-fakes")]
const FAKE_ACCESS_CATALOG: [&str; 13] = [
    "fake:unit-of-work",
    "fake:admins",
    "fake:credentials",
    "fake:sessions",
    "fake:otp-challenges",
    "fake:rate-limits",
    "fake:audit",
    "fake:authenticator",
    "fake:primary-sms",
    "fake:secondary-sms",
    "fake:clock",
    "fake:ids",
    "fake:policy",
];

/// Compose only concrete real adapters; lazy pool construction performs no database connection.
///
/// # Errors
/// Returns a sanitized error for invalid database, cryptographic or SMS configuration.
pub fn compose_real_access(
    config: RealAccessConfig,
) -> Result<AccessComposition, CompositionError> {
    use infra_postgres::{
        admin_credential_repository::PostgresAdminCredentialRepository,
        admin_repository::PostgresAdminRepository, audit_repository::PostgresAuditRepository,
        otp_challenge_repository::PostgresOtpChallengeRepository,
        rate_limit_repository::PostgresRateLimitRepository, session_store::PostgresSessionStore,
        unit_of_work::PostgresUnitOfWork,
    };

    let uow = PostgresUnitOfWork::connect_lazy(&config.database_url)
        .map_err(|_| CompositionError::InvalidAccessConfiguration)?;
    let authenticator = infra_auth::admin_authenticator::RealAdminAuthenticator::new(
        config.encryption_key,
        &config.key_id,
        config.policy.rp_id(),
        config.policy.origin(),
    )
    .map_err(|_| CompositionError::InvalidAccessConfiguration)?;
    let primary_sms = infra_sms::kavenegar::KavenegarSender::new(
        &config.kavenegar_url,
        &config.kavenegar_key,
        &config.kavenegar_template,
        config.sms_timeout,
    )
    .map_err(|_| CompositionError::InvalidAccessConfiguration)?;
    let secondary_sms = infra_sms::sms_ir::SmsIrSender::new(
        &config.sms_ir_url,
        &config.sms_ir_key,
        config.sms_ir_template,
        &config.sms_ir_parameter,
        config.sms_timeout,
    )
    .map_err(|_| CompositionError::InvalidAccessConfiguration)?;
    Ok(AccessComposition {
        services: web::console::ConsoleServices {
            uow: Arc::new(uow.clone()),
            admins: Arc::new(PostgresAdminRepository::new(uow.clone())),
            credentials: Arc::new(PostgresAdminCredentialRepository::new(uow.clone())),
            sessions: Arc::new(PostgresSessionStore::new(uow.clone())),
            otps: Arc::new(PostgresOtpChallengeRepository::new(uow.clone())),
            rates: Arc::new(PostgresRateLimitRepository::new(uow.clone())),
            audit: Arc::new(PostgresAuditRepository::new(uow)),
            authenticator: Arc::new(authenticator),
            primary_sms: Arc::new(primary_sms),
            secondary_sms: Arc::new(secondary_sms),
            clock: Arc::new(infra_system::clock::SystemClock::new()),
            ids: Arc::new(infra_system::id_generator::SystemIdGenerator::new()),
            policy: Arc::new(config.policy),
        },
        backend: BackendGroup::Real,
        catalog: &REAL_ACCESS_CATALOG,
        #[cfg(feature = "dev-fakes")]
        fake_store: None,
    })
}

/// Compose the complete deterministic fake catalog over one shared store without network or DB I/O.
///
/// # Errors
/// Returns [`CompositionError::FakeAdaptersNotCompiled`] when debug fakes are unavailable.
#[cfg(feature = "dev-fakes")]
pub fn compose_fake_access(
    policy: admin::domain::security_policy::SecurityPolicy,
) -> Result<AccessComposition, CompositionError> {
    use testkit::fakes::{
        admin_authenticator::FakeAdminAuthenticator,
        admin_credential_repository::FakeAdminCredentialRepository,
        admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
        clock::FixedClock, id_generator::SequenceIdGenerator,
        otp_challenge_repository::FakeOtpChallengeRepository,
        rate_limit_repository::FakeRateLimitRepository, session_store::FakeSessionStore,
        sms_sender::FakeSmsSender, store::FakeStore, unit_of_work::FakeUnitOfWork,
    };

    let store = FakeStore::new();
    Ok(AccessComposition {
        services: web::console::ConsoleServices {
            uow: Arc::new(FakeUnitOfWork::new(store.clone())),
            admins: Arc::new(FakeAdminRepository::new(store.clone())),
            credentials: Arc::new(FakeAdminCredentialRepository::new(store.clone())),
            sessions: Arc::new(FakeSessionStore::new(store.clone())),
            otps: Arc::new(FakeOtpChallengeRepository::new(store.clone())),
            rates: Arc::new(FakeRateLimitRepository::new(store.clone())),
            audit: Arc::new(FakeAuditRepository::new(store.clone())),
            authenticator: Arc::new(FakeAdminAuthenticator::controlled_fixture()),
            primary_sms: Arc::new(FakeSmsSender::new(Vec::new())),
            secondary_sms: Arc::new(FakeSmsSender::new(Vec::new())),
            clock: Arc::new(FixedClock::new(time::OffsetDateTime::UNIX_EPOCH)),
            ids: Arc::new(SequenceIdGenerator::new(1)),
            policy: Arc::new(policy),
        },
        backend: BackendGroup::Fake,
        catalog: &FAKE_ACCESS_CATALOG,
        fake_store: Some(store),
    })
}

/// Fake access composition is unavailable in normal production/release builds.
///
/// # Errors
/// Always returns [`CompositionError::FakeAdaptersNotCompiled`].
#[cfg(not(feature = "dev-fakes"))]
pub fn compose_fake_access(
    _policy: admin::domain::security_policy::SecurityPolicy,
) -> Result<AccessComposition, CompositionError> {
    Err(CompositionError::FakeAdaptersNotCompiled)
}
