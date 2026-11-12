//! AES-256-GCM envelopes for protected server-only state.

use std::fmt;

use admin::domain::secrets::SecretBytes;
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use rand::{RngCore, rng};

const VERSION: u8 = 1;
const NONCE_LENGTH: usize = 12;
const TAG_LENGTH: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeError {
    InvalidKey,
    InvalidEnvelope,
    AuthenticationFailed,
}

pub struct EnvelopeCipher {
    cipher: Aes256Gcm,
}

impl EnvelopeCipher {
    /// Construct from an externally managed 256-bit key.
    #[must_use]
    pub fn new(key: [u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(&key.into());
        Self { cipher }
    }

    /// Encrypt and authenticate secret bytes with the key ID and context as AAD.
    /// # Errors
    /// Rejects an empty/oversized key ID or an encryption failure.
    pub fn seal(
        &self,
        key_id: &str,
        aad: &[u8],
        plaintext: &SecretBytes,
    ) -> Result<SecretBytes, EnvelopeError> {
        let key_id = u8::try_from(key_id.len())
            .ok()
            .filter(|_| !key_id.is_empty())
            .map(|length| (length, key_id.as_bytes()))
            .ok_or(EnvelopeError::InvalidKey)?;
        let mut nonce = [0_u8; NONCE_LENGTH];
        rng().fill_bytes(&mut nonce);
        let nonce = Nonce::from(nonce);
        let ciphertext = self
            .cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.expose(),
                    aad,
                },
            )
            .map_err(|_| EnvelopeError::AuthenticationFailed)?;
        let mut envelope =
            Vec::with_capacity(2 + usize::from(key_id.0) + NONCE_LENGTH + ciphertext.len());
        envelope.push(VERSION);
        envelope.push(key_id.0);
        envelope.extend_from_slice(key_id.1);
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&ciphertext);
        Ok(SecretBytes::new(envelope))
    }

    /// Authenticate the complete envelope and its context before decrypting.
    /// # Errors
    /// Rejects malformed, rebound, wrongly keyed or tampered envelopes.
    pub fn open(
        &self,
        expected_key_id: &str,
        aad: &[u8],
        envelope: &SecretBytes,
    ) -> Result<SecretBytes, EnvelopeError> {
        let bytes = envelope.expose();
        if bytes.len() < 2 {
            return Err(EnvelopeError::InvalidEnvelope);
        }
        if bytes[0] != VERSION {
            return Err(EnvelopeError::InvalidEnvelope);
        }
        let key_length = usize::from(bytes[1]);
        let nonce_start = 2_usize
            .checked_add(key_length)
            .ok_or(EnvelopeError::InvalidEnvelope)?;
        let ciphertext_start = nonce_start
            .checked_add(NONCE_LENGTH)
            .ok_or(EnvelopeError::InvalidEnvelope)?;
        if bytes.len() < ciphertext_start + TAG_LENGTH
            || bytes.get(2..nonce_start) != Some(expected_key_id.as_bytes())
        {
            return Err(EnvelopeError::InvalidEnvelope);
        }
        let nonce = Nonce::try_from(&bytes[nonce_start..ciphertext_start])
            .map_err(|_| EnvelopeError::InvalidEnvelope)?;
        let plaintext = self
            .cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &bytes[ciphertext_start..],
                    aad,
                },
            )
            .map_err(|_| EnvelopeError::AuthenticationFailed)?;
        Ok(SecretBytes::new(plaintext))
    }
}

impl fmt::Debug for EnvelopeCipher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EnvelopeCipher([REDACTED])")
    }
}
