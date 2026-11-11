//! Deterministic fixture registry, deliberately not cryptography or arbitrary-proof success.

use admin::{
    domain::{
        auth_state::{AuthStage, BindingDigest},
        otp_state::OtpDigest,
        secrets::SecretBytes,
    },
    ports::{
        admin_authenticator::{
            AdminAuthenticator, CeremonyContext, IssuedOtp, PasskeyChallenge, PasskeyFinish,
            PasskeyStart, ProtectionContext, TokenPurpose, TotpCheck, TotpEnrollment,
            VerifiedPasskey,
        },
        admin_repository::IdentityDigest,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::ChallengeId,
};
use std::{fmt, sync::Mutex};
use time::OffsetDateTime;

#[derive(Default)]
struct Fixtures {
    next: u64,
    tokens: Vec<(TokenPurpose, SecretBytes, BindingDigest)>,
    envelopes: Vec<(ProtectionContext, SecretBytes, SecretBytes)>,
    otps: Vec<(ChallengeId, IssuedOtp)>,
    passkeys: Vec<(PasskeyStart, PasskeyChallenge, SecretBytes)>,
    totps: Vec<(CeremonyContext, TotpEnrollment)>,
}
impl Fixtures {
    fn opaque(&mut self, kind: &str) -> SecretBytes {
        self.next = self
            .next
            .checked_add(1)
            .expect("fixture sequence exhausted");
        bytes(&format!("synthetic-{kind}-{}", self.next))
    }
}

pub struct FakeAdminAuthenticator {
    fixtures: Mutex<Fixtures>,
}
impl fmt::Debug for FakeAdminAuthenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FakeAdminAuthenticator([REDACTED])")
    }
}
fn bytes(value: &str) -> SecretBytes {
    SecretBytes::new(value.as_bytes().to_vec())
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
fn exhausted() -> DomainError {
    DomainError::unavailable(ErrorCode::SourceExhausted)
}
fn valid_context(context: &CeremonyContext, now: OffsetDateTime) -> bool {
    context.primary_verified_at <= context.created_at
        && now >= context.created_at
        && now < context.expires_at
        && context.binding.credential_revision != Some(0)
}

impl FakeAdminAuthenticator {
    #[must_use]
    pub fn controlled_fixture() -> Self {
        Self {
            fixtures: Mutex::new(Fixtures::default()),
        }
    }
    #[must_use]
    pub fn passkey_credentials(&self) -> Vec<SecretBytes> {
        vec![bytes("synthetic-public-credential")]
    }
    #[must_use]
    pub fn verified_passkey(&self) -> VerifiedPasskey {
        VerifiedPasskey {
            credential_id: bytes("synthetic-credential-id"),
            credential: bytes("synthetic-updated-credential"),
        }
    }
    /// # Errors
    /// Only a previously issued matching local fixture has a controlled response.
    /// # Panics
    /// Panics if the fixture mutex is poisoned.
    pub fn passkey_fixture(
        &self,
        request: &PasskeyStart,
        challenge: &PasskeyChallenge,
    ) -> Result<SecretBytes, DomainError> {
        self.fixtures
            .lock()
            .expect("fixture mutex poisoned")
            .passkeys
            .iter()
            .find(|(r, c, _)| r == request && c == challenge)
            .map(|(_, _, proof)| proof.clone())
            .ok_or_else(denied)
    }
    /// # Errors
    /// No proof is invented for unknown envelopes or instants outside the fixed vector.
    /// # Panics
    /// Panics if the fixture mutex is poisoned.
    pub fn totp_fixture(
        &self,
        envelope: &SecretBytes,
        now: OffsetDateTime,
    ) -> Result<(SecretBytes, u64), DomainError> {
        let fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        if fixtures.totps.iter().any(|(context, enrollment)| {
            &enrollment.envelope == envelope && valid_context(context, now)
        }) && now == time::macros::datetime!(2026-01-01 00:00 UTC)
        {
            Ok((
                bytes("654321"),
                u64::try_from(now.unix_timestamp() / 30).map_err(|_| denied())?,
            ))
        } else {
            Err(denied())
        }
    }
}

#[async_trait::async_trait]
impl AdminAuthenticator for FakeAdminAuthenticator {
    async fn identity_digest(&self, phone: SecretBytes) -> Result<IdentityDigest, DomainError> {
        match phone.expose() {
            b"+989120000001" => Ok(IdentityDigest::new([1; 32])),
            b"+989120000002" => Ok(IdentityDigest::new([2; 32])),
            _ => Err(exhausted()),
        }
    }
    async fn issue_otp(&self, challenge: ChallengeId) -> Result<IssuedOtp, DomainError> {
        let mut fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        let sequence = u8::try_from(fixtures.otps.len() + 1).map_err(|_| exhausted())?;
        let otp = IssuedOtp {
            code: bytes(&format!("987{sequence:03}")),
            digest: OtpDigest::new([sequence; 32]),
        };
        fixtures.otps.push((challenge, otp.clone()));
        Ok(otp)
    }
    async fn verify_otp(
        &self,
        challenge: ChallengeId,
        code: SecretBytes,
        digest: OtpDigest,
    ) -> Result<bool, DomainError> {
        Ok(self
            .fixtures
            .lock()
            .expect("fixture mutex poisoned")
            .otps
            .iter()
            .any(|(id, otp)| *id == challenge && otp.code == code && otp.digest == digest))
    }
    async fn issue_token(&self, purpose: TokenPurpose) -> Result<SecretBytes, DomainError> {
        let mut fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        let token = fixtures.opaque("token");
        let mut digest = [0; 32];
        digest[..8].copy_from_slice(&fixtures.next.to_be_bytes());
        fixtures
            .tokens
            .push((purpose, token.clone(), BindingDigest::new(digest)));
        Ok(token)
    }
    async fn token_digest(
        &self,
        purpose: TokenPurpose,
        token: SecretBytes,
    ) -> Result<BindingDigest, DomainError> {
        let mut fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        if let Some((_, _, digest)) = fixtures
            .tokens
            .iter()
            .find(|(p, t, _)| *p == purpose && *t == token)
        {
            return Ok(digest.clone());
        }
        if !fixtures.tokens.iter().any(|(_, t, _)| *t == token) {
            return Err(denied());
        }
        let _ = fixtures.opaque("digest");
        let mut digest = [0; 32];
        digest[..8].copy_from_slice(&fixtures.next.to_be_bytes());
        let digest = BindingDigest::new(digest);
        fixtures.tokens.push((purpose, token, digest.clone()));
        Ok(digest)
    }
    async fn protect(
        &self,
        context: ProtectionContext,
        plaintext: SecretBytes,
    ) -> Result<SecretBytes, DomainError> {
        let mut fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        let envelope = fixtures.opaque("envelope");
        fixtures
            .envelopes
            .push((context, plaintext, envelope.clone()));
        Ok(envelope)
    }
    async fn unprotect(
        &self,
        context: ProtectionContext,
        envelope: SecretBytes,
    ) -> Result<SecretBytes, DomainError> {
        self.fixtures
            .lock()
            .expect("fixture mutex poisoned")
            .envelopes
            .iter()
            .find(|(c, _, e)| *c == context && *e == envelope)
            .map(|(_, p, _)| p.clone())
            .ok_or_else(denied)
    }
    async fn begin_passkey(&self, request: PasskeyStart) -> Result<PasskeyChallenge, DomainError> {
        if request.rp_id != "console.example.test"
            || request.origin != "https://console.example.test"
            || request.context.binding.stage != AuthStage::Passkey
            || !valid_context(&request.context, request.context.created_at)
            || (!request.credentials.is_empty()
                && request.credentials != self.passkey_credentials())
        {
            return Err(denied());
        }
        let mut fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        let result = PasskeyChallenge {
            public_options: fixtures.opaque("public-options"),
            protected_state: fixtures.opaque("server-state"),
        };
        let response = fixtures.opaque("signed-fixture");
        fixtures.passkeys.push((request, result.clone(), response));
        Ok(result)
    }
    async fn finish_passkey(&self, request: PasskeyFinish) -> Result<VerifiedPasskey, DomainError> {
        if !valid_context(&request.request.context, request.now) {
            return Err(denied());
        }
        if self
            .fixtures
            .lock()
            .expect("fixture mutex poisoned")
            .passkeys
            .iter()
            .any(|(r, c, p)| {
                *r == request.request
                    && c.protected_state == request.protected_state
                    && *p == request.response
            })
        {
            Ok(self.verified_passkey())
        } else {
            Err(denied())
        }
    }
    async fn begin_totp(&self, context: CeremonyContext) -> Result<TotpEnrollment, DomainError> {
        if context.binding.stage != AuthStage::Totp
            || context.credential.is_none()
            || !valid_context(&context, context.created_at)
        {
            return Err(denied());
        }
        let mut fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        let enrollment = TotpEnrollment {
            envelope: fixtures.opaque("totp-envelope"),
            protected_provisioning: fixtures.opaque("totp-provisioning"),
        };
        fixtures.totps.push((context, enrollment.clone()));
        Ok(enrollment)
    }
    async fn verify_totp(&self, request: TotpCheck) -> Result<u64, DomainError> {
        if !valid_context(&request.context, request.now)
            || request.context.binding.stage != AuthStage::Totp
        {
            return Err(denied());
        }
        let (code, step) = self.totp_fixture(&request.envelope, request.now)?;
        let fixtures = self.fixtures.lock().expect("fixture mutex poisoned");
        if request.code == code
            && fixtures
                .totps
                .iter()
                .any(|(c, e)| e.envelope == request.envelope && *c == request.context)
        {
            Ok(step)
        } else {
            Err(denied())
        }
    }
}
