use admin::domain::secrets::SecretBytes;
use infra_auth::totp::TotpEngine;
use time::macros::datetime;

#[test]
fn totp_reports_matched_step_and_rejects_invalid_code() {
    let engine = TotpEngine::new();
    let generated = engine.generate_secret();
    assert_eq!(generated.expose().len(), 20);
    let provisioning = engine
        .provisioning(&generated, "admin@example.test")
        .unwrap();
    assert!(
        String::from_utf8(provisioning.expose().to_vec())
            .unwrap()
            .starts_with("otpauth://totp/")
    );

    let secret = SecretBytes::new(b"12345678901234567890".to_vec());
    let at = datetime!(1970-01-01 00:00:59 UTC);
    let current = engine.current_code(&secret, at).unwrap();
    assert_eq!(current.expose(), b"287082");
    assert_eq!(engine.verify(&secret, &current, at).unwrap(), 1);
    assert_eq!(
        engine
            .verify(&secret, &current, datetime!(1970-01-01 00:00:29 UTC))
            .unwrap(),
        1
    );
    assert!(
        engine
            .verify(&secret, &current, datetime!(1970-01-01 00:02:00 UTC))
            .is_err()
    );
    assert!(
        engine
            .verify(&secret, &SecretBytes::new(b"000000".to_vec()), at)
            .is_err()
    );
    assert!(
        engine
            .verify(&secret, &SecretBytes::new(b"12x456".to_vec()), at)
            .is_err()
    );
}
