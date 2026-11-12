#![cfg(feature = "dev-fakes")]

use admin::domain::{
    secrets::SecretBytes,
    security_policy::{RateLimitInput, SecurityPolicy, SecurityPolicyInput},
};
use app::{
    composition::compose_fake_access,
    provision_admin::{ProvisionAdminCommand, run_provision_admin},
};
use kernel::ids::AdminId;

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
async fn initial_provisioning_is_operator_authorized_not_public_registration() {
    let composition = compose_fake_access(policy()).unwrap();
    let operator: AdminId = composition.services().ids.next_admin_id().await.unwrap();
    let command = ProvisionAdminCommand::parse([
        "--operator-id".to_owned(),
        operator.to_string(),
        "--owner-authorized".to_owned(),
    ])
    .unwrap();
    assert!(
        ProvisionAdminCommand::parse(["--operator-id".to_owned(), operator.to_string(),]).is_err()
    );
    let unsafe_argument = ProvisionAdminCommand::parse([
        "--operator-id".to_owned(),
        operator.to_string(),
        "--owner-authorized".to_owned(),
        "--phone".to_owned(),
        "+989120000001".to_owned(),
    ])
    .unwrap_err();
    assert!(!unsafe_argument.to_string().contains("+989120000001"));

    let target = run_provision_admin(
        command,
        composition.services(),
        SecretBytes::new(b"+989120000001".to_vec()),
    )
    .await
    .unwrap();
    assert_ne!(target, operator);
    let state = composition.store().try_read_committed().unwrap();
    assert_eq!(state.admins.len(), 1);
    assert_eq!(state.audit_entries.len(), 1);
    assert!(state.admin_sessions.is_empty());

    let repeated = run_provision_admin(
        ProvisionAdminCommand::parse([
            "--operator-id".to_owned(),
            operator.to_string(),
            "--owner-authorized".to_owned(),
        ])
        .unwrap(),
        composition.services(),
        SecretBytes::new(b"+989120000002".to_vec()),
    )
    .await
    .unwrap_err();
    assert!(!repeated.to_string().contains("+989120000002"));
    let state = composition.store().try_read_committed().unwrap();
    assert_eq!(state.admins.len(), 1);
    assert_eq!(state.audit_entries.len(), 1);
}
