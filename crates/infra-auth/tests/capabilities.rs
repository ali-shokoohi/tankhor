use std::str::FromStr;

use admin::{domain::secrets::SecretBytes, ports::admin_authenticator::TokenPurpose};
use infra_auth::capabilities::CapabilityGenerator;
use kernel::ids::ChallengeId;

fn challenge(number: u128) -> ChallengeId {
    ChallengeId::from_str(&format!("019a53a0-0000-7000-8000-{number:012}")).unwrap()
}

#[test]
fn otp_and_capabilities_are_secure_and_digest_only() {
    let capabilities = CapabilityGenerator::new([19; 32]);
    let id = challenge(1);
    let issued = capabilities.issue_otp(id).unwrap();
    assert_eq!(issued.code.expose().len(), 6);
    assert!(issued.code.expose().iter().all(u8::is_ascii_digit));
    assert!(
        capabilities
            .verify_otp(id, &issued.code, &issued.digest)
            .unwrap()
    );
    assert!(
        !capabilities
            .verify_otp(challenge(2), &issued.code, &issued.digest)
            .unwrap()
    );
    assert!(
        !capabilities
            .verify_otp(id, &SecretBytes::new(b"000000".to_vec()), &issued.digest)
            .unwrap()
    );
    assert_ne!(issued.digest.as_bytes().as_slice(), issued.code.expose());

    let session = capabilities.issue_token(TokenPurpose::Session).unwrap();
    let csrf = capabilities.issue_token(TokenPurpose::Csrf).unwrap();
    assert!(session.expose().len() >= 32);
    assert!(csrf.expose().len() >= 32);
    assert_ne!(session, csrf);
    assert_ne!(
        capabilities
            .token_digest(TokenPurpose::Session, &session)
            .unwrap(),
        capabilities
            .token_digest(TokenPurpose::Csrf, &session)
            .unwrap()
    );
    assert_eq!(
        capabilities
            .token_digest(TokenPurpose::Session, &session)
            .unwrap(),
        capabilities
            .token_digest(TokenPurpose::Session, &session)
            .unwrap()
    );
    assert_ne!(
        capabilities
            .identity_digest(&SecretBytes::new(b"+989120000001".to_vec()))
            .unwrap(),
        capabilities
            .identity_digest(&SecretBytes::new(b"+989120000002".to_vec()))
            .unwrap()
    );
    assert!(!format!("{capabilities:?}").contains("13131313"));
}
