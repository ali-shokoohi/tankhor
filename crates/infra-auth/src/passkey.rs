//! Safe `WebAuthn` passkey ceremonies with serialized server-only state.

use std::fmt;

use admin::domain::secrets::SecretBytes;
use kernel::error::{DomainError, ErrorCode};
use uuid::Uuid;
use webauthn_rs::prelude::{
    Passkey, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential, Url, Webauthn, WebauthnBuilder,
};

pub struct PasskeyEngine {
    webauthn: Webauthn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyCredential {
    pub credential_id: SecretBytes,
    pub credential: SecretBytes,
}

impl PasskeyEngine {
    /// # Errors
    /// Rejects an origin that does not belong to the configured relying party.
    pub fn new(rp_id: &str, origin: &str) -> Result<Self, DomainError> {
        let origin = Url::parse(origin).map_err(|_| denied())?;
        let webauthn = WebauthnBuilder::new(rp_id, &origin)
            .map_err(|_| denied())?
            .rp_name("Tankhor")
            .build()
            .map_err(|_| denied())?;
        Ok(Self { webauthn })
    }

    /// # Errors
    /// Rejects a failure to construct a UV-required registration challenge.
    pub fn begin_registration(
        &self,
        admin: Uuid,
        account_name: &str,
    ) -> Result<(SecretBytes, SecretBytes), DomainError> {
        let (public, state) = self
            .webauthn
            .start_passkey_registration(admin, account_name, account_name, None)
            .map_err(|_| denied())?;
        Ok((json(&public)?, json(&state)?))
    }

    /// # Errors
    /// Rejects malformed, rebound, invalid or untrusted registration proofs.
    pub fn finish_registration(
        &self,
        state: &SecretBytes,
        response: &SecretBytes,
    ) -> Result<PasskeyCredential, DomainError> {
        let state: PasskeyRegistration = from_json(state)?;
        let response: RegisterPublicKeyCredential = from_json(response)?;
        let credential = self
            .webauthn
            .finish_passkey_registration(&response, &state)
            .map_err(|_| denied())?;
        credential_value(&credential)
    }

    /// # Errors
    /// Rejects malformed credentials or a failure to construct a UV-required challenge.
    pub fn begin_authentication(
        &self,
        credentials: &[SecretBytes],
    ) -> Result<(SecretBytes, SecretBytes), DomainError> {
        let credentials = credentials
            .iter()
            .map(from_json)
            .collect::<Result<Vec<Passkey>, _>>()?;
        let (public, state) = self
            .webauthn
            .start_passkey_authentication(&credentials)
            .map_err(|_| denied())?;
        Ok((json(&public)?, json(&state)?))
    }

    /// Verify against the credential selected by the signed assertion.
    ///
    /// # Errors
    /// Rejects malformed, rebound, replayed, wrong-origin or otherwise invalid assertions.
    pub fn finish_authentication_any(
        &self,
        state: &SecretBytes,
        response: &SecretBytes,
        credentials: &[SecretBytes],
    ) -> Result<PasskeyCredential, DomainError> {
        for credential in credentials {
            if let Ok(verified) = self.finish_authentication(state, response, credential) {
                return Ok(verified);
            }
        }
        Err(denied())
    }

    /// # Errors
    /// Rejects malformed, rebound, replayed, wrong-origin or otherwise invalid assertions.
    pub fn finish_authentication(
        &self,
        state: &SecretBytes,
        response: &SecretBytes,
        credential: &SecretBytes,
    ) -> Result<PasskeyCredential, DomainError> {
        let state: PasskeyAuthentication = from_json(state)?;
        let response: PublicKeyCredential = from_json(response)?;
        let mut credential: Passkey = from_json(credential)?;
        let result = self
            .webauthn
            .finish_passkey_authentication(&response, &state)
            .map_err(|_| denied())?;
        if credential.update_credential(&result).is_none() {
            return Err(denied());
        }
        credential_value(&credential)
    }
}

fn credential_value(credential: &Passkey) -> Result<PasskeyCredential, DomainError> {
    Ok(PasskeyCredential {
        credential_id: SecretBytes::new(credential.cred_id().clone()),
        credential: json(credential)?,
    })
}

fn json<T: serde::Serialize>(value: &T) -> Result<SecretBytes, DomainError> {
    serde_json::to_vec(value)
        .map(SecretBytes::new)
        .map_err(|_| denied())
}

fn from_json<T: serde::de::DeserializeOwned>(value: &SecretBytes) -> Result<T, DomainError> {
    serde_json::from_slice(value.expose()).map_err(|_| denied())
}

fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}

impl fmt::Debug for PasskeyEngine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PasskeyEngine([REDACTED])")
    }
}
