use admin::domain::secrets::SecretBytes;
use infra_auth::envelopes::EnvelopeCipher;

fn aad(admin: Uuid, epoch: u64) -> Vec<u8> {
    let mut value = admin.as_bytes().to_vec();
    value.extend_from_slice(&epoch.to_be_bytes());
    value
}

use uuid::Uuid;

#[test]
fn secret_envelope_binds_admin_and_epoch() {
    let cipher = EnvelopeCipher::new([7; 32]);
    let admin = Uuid::from_u128(0x019a53a0_0000_7000_8000_000000000001);
    let plaintext = SecretBytes::new(b"synthetic-server-state".to_vec());
    let envelope = cipher
        .seal("test-key-v1", &aad(admin, 4), &plaintext)
        .unwrap();
    assert_ne!(envelope, plaintext);
    assert!(
        !envelope
            .expose()
            .windows(22)
            .any(|v| v == b"synthetic-server-state")
    );
    assert_eq!(
        cipher
            .open("test-key-v1", &aad(admin, 4), &envelope)
            .unwrap(),
        plaintext
    );
    let second = cipher
        .seal("test-key-v1", &aad(admin, 4), &plaintext)
        .unwrap();
    assert_ne!(envelope, second, "every envelope needs a unique nonce");
    assert!(
        cipher
            .open("test-key-v1", &aad(admin, 5), &envelope)
            .is_err()
    );
    let other = Uuid::from_u128(0x019a53a0_0000_7000_8000_000000000002);
    assert!(
        cipher
            .open("test-key-v1", &aad(other, 4), &envelope)
            .is_err()
    );
    assert!(cipher.open("wrong-key", &aad(admin, 4), &envelope).is_err());
    let mut tampered = envelope.expose().to_vec();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(
        cipher
            .open("test-key-v1", &aad(admin, 4), &SecretBytes::new(tampered))
            .is_err()
    );
    assert!(!format!("{cipher:?}").contains("07070707"));
}

#[test]
fn totp_wrong_envelope_key_fails_closed() {
    let owner = EnvelopeCipher::new([11; 32]);
    let wrong = EnvelopeCipher::new([12; 32]);
    let admin = Uuid::from_u128(0x019a53a0_0000_7000_8000_000000000003);
    let envelope = owner
        .seal(
            "totp-key-v1",
            &aad(admin, 9),
            &SecretBytes::new([42; 20].to_vec()),
        )
        .unwrap();
    assert!(
        wrong
            .open("totp-key-v1", &aad(admin, 9), &envelope)
            .is_err()
    );
}
