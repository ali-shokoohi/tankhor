use std::process::Command;

use admin::domain::secrets::SecretBytes;
use infra_auth::passkey::PasskeyEngine;
use uuid::Uuid;

fn fixture(options: &SecretBytes, origin: &str, counter: u32) -> SecretBytes {
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/webauthn_fixture.py"
        ))
        .arg("authenticate")
        .arg(String::from_utf8(options.expose().to_vec()).unwrap())
        .arg(origin)
        .arg(counter.to_string())
        .output()
        .unwrap();
    assert!(output.status.success());
    SecretBytes::new(output.stdout)
}

fn register(engine: &PasskeyEngine) -> SecretBytes {
    let (options, state) = engine
        .begin_registration(
            Uuid::from_u128(0x019a53a0_0000_7000_8000_000000000001),
            "admin@example.test",
        )
        .unwrap();
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/webauthn_fixture.py"
        ))
        .arg("register")
        .arg(String::from_utf8(options.expose().to_vec()).unwrap())
        .arg("https://localhost:8443")
        .arg("0")
        .output()
        .unwrap();
    assert!(output.status.success());
    engine
        .finish_registration(&state, &SecretBytes::new(output.stdout))
        .unwrap()
        .credential
}

#[test]
fn webauthn_rejects_wrong_origin_and_replayed_state() {
    let engine = PasskeyEngine::new("localhost", "https://localhost:8443").unwrap();
    let credential = register(&engine);
    let (options, state) = engine
        .begin_authentication(std::slice::from_ref(&credential))
        .unwrap();
    let wrong_origin = fixture(&options, "https://attacker.example", 1);
    assert!(
        engine
            .finish_authentication(&state, &wrong_origin, &credential)
            .is_err()
    );
    let response = fixture(&options, "https://localhost:8443", 1);
    let verified = engine
        .finish_authentication(&state, &response, &credential)
        .unwrap();
    assert_eq!(
        verified.credential_id.expose(),
        b"tankhor-local-test-credential-0001"
    );
    let (_, updated_state) = engine
        .begin_authentication(std::slice::from_ref(&verified.credential))
        .unwrap();
    assert!(
        engine
            .finish_authentication(&updated_state, &response, &verified.credential)
            .is_err()
    );
}

#[test]
fn valid_zero_counter_passkey_is_supported() {
    let engine = PasskeyEngine::new("localhost", "https://localhost:8443").unwrap();
    let credential = register(&engine);
    let (options, state) = engine
        .begin_authentication(std::slice::from_ref(&credential))
        .unwrap();
    let response = fixture(&options, "https://localhost:8443", 0);
    assert!(
        engine
            .finish_authentication(&state, &response, &credential)
            .is_ok()
    );
}
