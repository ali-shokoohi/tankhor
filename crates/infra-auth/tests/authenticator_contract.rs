use std::process::Command;

use admin::{
    domain::secrets::SecretBytes,
    ports::admin_authenticator::{
        AdminAuthenticator, PasskeyChallenge, PasskeyFinish, PasskeyStart, TotpCredentialBinding,
        VerifiedPasskey,
    },
};
use infra_auth::{
    admin_authenticator::{RealAdminAuthenticator, totp_credential_aad},
    envelopes::EnvelopeCipher,
    totp::TotpEngine,
};
use testkit::contracts::admin_authenticator::{
    AdminAuthenticatorContractHarness, admin_authenticator_contract, context,
};
use time::OffsetDateTime;

const KEY: [u8; 32] = [41; 32];
const KEY_ID: &str = "local-contract";

struct RealHarness {
    authenticator: RealAdminAuthenticator,
    credential: VerifiedPasskey,
}

impl RealHarness {
    async fn new() -> Self {
        let authenticator = RealAdminAuthenticator::new(
            KEY,
            KEY_ID,
            "console.example.test",
            "https://console.example.test",
        )
        .unwrap();
        let context = context();
        let request = PasskeyStart {
            context: context.clone(),
            rp_id: "console.example.test".into(),
            origin: "https://console.example.test".into(),
            credentials: Vec::new(),
        };
        let challenge = authenticator.begin_passkey(request.clone()).await.unwrap();
        let response = fixture("register", &challenge.public_options, &request.origin);
        let credential = authenticator
            .finish_passkey(PasskeyFinish {
                request,
                protected_state: challenge.protected_state,
                response,
                now: context.created_at,
            })
            .await
            .unwrap();
        Self {
            authenticator,
            credential,
        }
    }
}

impl AdminAuthenticatorContractHarness for RealHarness {
    type Authenticator = RealAdminAuthenticator;

    fn authenticator(&self) -> &Self::Authenticator {
        &self.authenticator
    }

    fn passkey_response(
        &self,
        request: &PasskeyStart,
        challenge: &PasskeyChallenge,
    ) -> SecretBytes {
        fixture("assert", &challenge.public_options, &request.origin)
    }

    fn passkey_credentials(&self) -> Vec<SecretBytes> {
        vec![self.credential.credential.clone()]
    }

    fn verified_passkey(&self) -> VerifiedPasskey {
        self.credential.clone()
    }

    fn totp_proof(&self, envelope: &SecretBytes, now: OffsetDateTime) -> (SecretBytes, u64) {
        let binding = TotpCredentialBinding::try_from(&context()).unwrap();
        let secret = EnvelopeCipher::new(KEY)
            .open(KEY_ID, &totp_credential_aad(&binding), envelope)
            .unwrap();
        let totp = TotpEngine::new();
        let code = totp.current_code(&secret, now).unwrap();
        let step = totp.verify(&secret, &code, now).unwrap();
        (code, step)
    }
}

fn fixture(operation: &str, options: &SecretBytes, origin: &str) -> SecretBytes {
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/webauthn_fixture.py"
        ))
        .arg(operation)
        .arg(String::from_utf8(options.expose().to_vec()).unwrap())
        .arg(origin)
        .arg("0")
        .output()
        .unwrap();
    assert!(output.status.success());
    SecretBytes::new(output.stdout)
}

#[tokio::test]
async fn admin_authenticator_contract_real_crypto() {
    let harness = RealHarness::new().await;
    admin_authenticator_contract(&harness).await;
}
