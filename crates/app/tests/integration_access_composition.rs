use std::time::Duration;

use admin::{
    application::bootstrap_admin::{BootstrapAdmin, BootstrapAdminInput, OperatorContext},
    domain::{
        secrets::SecretBytes,
        security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput},
    },
};
use app::{
    composition::{BackendGroup, RealAccessConfig, compose_fake_access, compose_real_access},
    config::AppConfig,
};

fn policy() -> SecurityPolicy {
    SecurityPolicy::try_from(SecurityPolicyInput {
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
    .unwrap()
}

#[tokio::test]
async fn access_composition_selects_all_ports_from_one_backend_group() {
    let real = compose_real_access(RealAccessConfig {
        database_url: "postgresql://fixture_admin@127.0.0.1/tankhor".to_owned(),
        encryption_key: [7; 32],
        key_id: "test-key".to_owned(),
        kavenegar_url: "https://kavenegar.invalid".to_owned(),
        kavenegar_key: "secret".to_owned(),
        kavenegar_template: "admin".to_owned(),
        sms_ir_url: "https://sms-ir.invalid".to_owned(),
        sms_ir_key: "secret".to_owned(),
        sms_ir_template: 1,
        sms_ir_parameter: "code".to_owned(),
        sms_timeout: Duration::from_secs(2),
        policy: policy(),
    })
    .unwrap();
    assert_eq!(real.backend(), BackendGroup::Real);
    assert_eq!(real.catalog().len(), 13);
    assert!(
        real.catalog()
            .iter()
            .all(|entry| entry.starts_with("real:"))
    );

    let fake = compose_fake_access(policy()).unwrap();
    assert_eq!(fake.backend(), BackendGroup::Fake);
    assert_eq!(fake.catalog().len(), 13);
    assert!(
        fake.catalog()
            .iter()
            .all(|entry| entry.starts_with("fake:"))
    );
    let services = fake.services();
    let operator = services.ids.next_admin_id().await.unwrap();
    let target = (BootstrapAdmin {
        uow: services.uow.as_ref(),
        admins: services.admins.as_ref(),
        audit: services.audit.as_ref(),
        authenticator: services.authenticator.as_ref(),
        clock: services.clock.as_ref(),
        ids: services.ids.as_ref(),
        policy: &services.policy,
    })
    .execute(BootstrapAdminInput {
        operator: OperatorContext::new(operator, true),
        phone: SecretBytes::new(b"+989120000001".to_vec()),
    })
    .await
    .unwrap();
    assert_ne!(target, operator);
    let state = fake.store().try_read_committed().unwrap();
    assert_eq!(state.admins.len(), 1);
    assert_eq!(state.audit_entries.len(), 1);

    let production_fake =
        AppConfig::from_pairs([("TANKHOR_ENV", "production"), ("TANKHOR_ADAPTERS", "fake")]);
    assert!(production_fake.is_err());
}
