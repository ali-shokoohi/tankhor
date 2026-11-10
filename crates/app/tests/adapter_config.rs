use std::sync::atomic::{AtomicBool, Ordering};

use app::config::{AdapterMode, AppConfig, ConfigError, RuntimeEnvironment};
use app::transport_policy::{
    MAX_ALLOWED_BODY_BYTES, MAX_ALLOWED_INFLIGHT, TransportPolicy, TransportPolicyError,
};

#[test]
fn production_rejects_fake_application_mode() {
    let valid_dev_default = AppConfig::from_pairs([("TANKHOR_ENV", "development")])
        .expect("development with default real adapter mode must succeed");
    assert_eq!(
        valid_dev_default.environment(),
        RuntimeEnvironment::Development
    );
    assert_eq!(valid_dev_default.adapter_mode(), AdapterMode::Real);

    let valid_staging_fake =
        AppConfig::from_pairs([("TANKHOR_ENV", "staging"), ("TANKHOR_ADAPTERS", "fake")])
            .expect("staging fake mode is permitted outside production");
    assert_eq!(
        valid_staging_fake.environment(),
        RuntimeEnvironment::Staging
    );
    assert_eq!(valid_staging_fake.adapter_mode(), AdapterMode::Fake);

    let valid_prod_real =
        AppConfig::from_pairs([("TANKHOR_ENV", "production"), ("TANKHOR_ADAPTERS", "real")])
            .expect("production real mode must succeed");
    assert_eq!(
        valid_prod_real.environment(),
        RuntimeEnvironment::Production
    );
    assert_eq!(valid_prod_real.adapter_mode(), AdapterMode::Real);

    // Production fake mode MUST fail before any backend resource acquisition runs.
    let acquired_resource = AtomicBool::new(false);
    let prod_fake_err = AppConfig::load_and_acquire(
        [("TANKHOR_ENV", "production"), ("TANKHOR_ADAPTERS", "fake")],
        |_cfg| {
            acquired_resource.store(true, Ordering::SeqCst);
            Ok::<_, ConfigError>(())
        },
    )
    .expect_err("production must reject fake application mode before resource acquisition");
    assert_eq!(prod_fake_err, ConfigError::ProductionRejectsFakeAdapters);
    assert!(
        !acquired_resource.load(Ordering::SeqCst),
        "resource acquisition callback must never execute when config validation fails"
    );

    // Missing or unknown environment values must fail closed.
    assert_eq!(
        AppConfig::from_pairs(std::iter::empty::<(&str, &str)>())
            .expect_err("missing TANKHOR_ENV must fail"),
        ConfigError::MissingEnvironment
    );
    assert_eq!(
        AppConfig::from_pairs([("TANKHOR_ENV", "qa")]).expect_err("unknown TANKHOR_ENV must fail"),
        ConfigError::InvalidEnvironment
    );

    // Empty or unknown adapter mode values must fail closed.
    assert_eq!(
        AppConfig::from_pairs([("TANKHOR_ENV", "development"), ("TANKHOR_ADAPTERS", "")])
            .expect_err("empty TANKHOR_ADAPTERS must fail"),
        ConfigError::InvalidAdapterMode
    );
    assert_eq!(
        AppConfig::from_pairs([
            ("TANKHOR_ENV", "development"),
            ("TANKHOR_ADAPTERS", "mixed"),
        ])
        .expect_err("mixed/unknown TANKHOR_ADAPTERS must fail"),
        ConfigError::InvalidAdapterMode
    );

    // Per-group adapter overrides are strictly forbidden (one coherent app-wide mode only).
    for forbidden_key in [
        "TANKHOR_ADAPTERS_DB",
        "TANKHOR_ADAPTERS_SMS",
        "TANKHOR_SMS_ADAPTER",
        "TANKHOR_AUTH_ADAPTERS",
        "TANKHOR_DB_ADAPTER_MODE",
    ] {
        let err = AppConfig::from_pairs([
            ("TANKHOR_ENV", "development"),
            ("TANKHOR_ADAPTERS", "real"),
            (forbidden_key, "fake"),
        ])
        .expect_err("per-group adapter override env vars must be rejected");
        assert_eq!(
            err,
            ConfigError::PerGroupAdapterOverride {
                key: forbidden_key.to_owned(),
            }
        );
    }
}

#[test]
fn transport_policy_requires_finite_positive_limits() {
    let valid = TransportPolicy::from_pairs([
        ("TANKHOR_HTTP_MAX_BODY_BYTES", "65536"),
        ("TANKHOR_HTTP_MAX_INFLIGHT", "64"),
    ])
    .expect("explicit finite positive body and inflight limits must succeed");
    assert_eq!(valid.max_body_bytes().get(), 65_536);
    assert_eq!(valid.max_inflight().get(), 64);

    // Missing limits must fail (no invented defaults).
    assert_eq!(
        TransportPolicy::from_pairs([("TANKHOR_HTTP_MAX_INFLIGHT", "32")])
            .expect_err("missing body limit must fail"),
        TransportPolicyError::MissingMaxBodyBytes
    );
    assert_eq!(
        TransportPolicy::from_pairs([("TANKHOR_HTTP_MAX_BODY_BYTES", "4096")])
            .expect_err("missing inflight limit must fail"),
        TransportPolicyError::MissingMaxInflight
    );

    // Zero, negative, non-numeric, and unbounded/excessive limits must fail closed.
    for bad_body in [
        "",
        "0",
        "-1",
        "unlimited",
        "inf",
        &(MAX_ALLOWED_BODY_BYTES + 1).to_string(),
    ] {
        assert_eq!(
            TransportPolicy::from_pairs([
                ("TANKHOR_HTTP_MAX_BODY_BYTES", bad_body),
                ("TANKHOR_HTTP_MAX_INFLIGHT", "16"),
            ])
            .expect_err("invalid body limit must be rejected"),
            TransportPolicyError::InvalidMaxBodyBytes
        );
    }

    for bad_inflight in [
        "",
        "0",
        "-4",
        "unlimited",
        "NaN",
        &(MAX_ALLOWED_INFLIGHT + 1).to_string(),
    ] {
        assert_eq!(
            TransportPolicy::from_pairs([
                ("TANKHOR_HTTP_MAX_BODY_BYTES", "4096"),
                ("TANKHOR_HTTP_MAX_INFLIGHT", bad_inflight),
            ])
            .expect_err("invalid inflight limit must be rejected"),
            TransportPolicyError::InvalidMaxInflight
        );
    }

    // Offline OpenAPI mode remains independent of serve-time transport policy and env config.
    assert!(
        AppConfig::command_requires_runtime_config("serve"),
        "serve requires runtime config and transport policy"
    );
    assert!(
        !AppConfig::command_requires_runtime_config("openapi"),
        "offline openapi export requires neither runtime config nor transport policy"
    );
}
