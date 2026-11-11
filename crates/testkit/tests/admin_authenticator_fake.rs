use admin::{
    domain::secrets::SecretBytes,
    ports::admin_authenticator::{PasskeyChallenge, PasskeyStart, VerifiedPasskey},
};
use testkit::{
    contracts::admin_authenticator::{
        AdminAuthenticatorContractHarness, admin_authenticator_contract,
    },
    fakes::admin_authenticator::FakeAdminAuthenticator,
};
use time::OffsetDateTime;

struct Harness(FakeAdminAuthenticator);
impl AdminAuthenticatorContractHarness for Harness {
    type Authenticator = FakeAdminAuthenticator;
    fn authenticator(&self) -> &Self::Authenticator {
        &self.0
    }
    fn passkey_response(
        &self,
        request: &PasskeyStart,
        challenge: &PasskeyChallenge,
    ) -> SecretBytes {
        self.0.passkey_fixture(request, challenge).unwrap()
    }
    fn passkey_credentials(&self) -> Vec<SecretBytes> {
        self.0.passkey_credentials()
    }
    fn verified_passkey(&self) -> VerifiedPasskey {
        self.0.verified_passkey()
    }
    fn totp_proof(&self, envelope: &SecretBytes, now: OffsetDateTime) -> (SecretBytes, u64) {
        self.0.totp_fixture(envelope, now).unwrap()
    }
}
#[tokio::test]
async fn admin_authenticator_fake_passes_contract() {
    admin_authenticator_contract(&Harness(FakeAdminAuthenticator::controlled_fixture())).await;
}
