//! Cryptographic helpers shared across the OpenGuild backend.

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64_URL;
use base64::Engine;
use ed25519_dalek::Signer;
use rand::rngs::OsRng;

pub use ed25519_dalek::{Signature, SigningKey, VerifyingKey};

pub fn generate_signing_key() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

pub fn verifying_key_from(signing_key: &SigningKey) -> VerifyingKey {
    signing_key.verifying_key()
}

pub fn sign_message(signing_key: &SigningKey, message: &[u8]) -> Signature {
    signing_key.sign(message)
}

pub fn verify_signature(
    verifying_key: &VerifyingKey,
    message: &[u8],
    signature: &Signature,
) -> Result<()> {
    verifying_key
        .verify_strict(message, signature)
        .map_err(|err| anyhow::anyhow!(err))
}

/// Decode an ed25519 signing key from a URL-safe base64 string.
pub fn signing_key_from_base64(raw: &str) -> Result<SigningKey> {
    let bytes = decode_base64_key(raw)?;
    Ok(SigningKey::from_bytes(&bytes))
}

/// Decode an ed25519 verifying key from a URL-safe base64 string.
pub fn verifying_key_from_base64(raw: &str) -> Result<VerifyingKey> {
    let bytes = decode_base64_key(raw)?;
    VerifyingKey::from_bytes(&bytes).map_err(|err| anyhow!(err))
}

fn decode_base64_key(raw: &str) -> Result<[u8; 32]> {
    let decoded = BASE64_URL
        .decode(raw.trim())
        .with_context(|| "failed to decode ed25519 key from url-safe base64")?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| anyhow!("ed25519 keys must be exactly 32 bytes"))?;
    Ok(bytes)
}

#[derive(Clone)]
pub struct SigningKeyRing {
    primary: SigningKey,
    fallback_verifiers: Vec<VerifyingKey>,
}

impl SigningKeyRing {
    pub fn new(primary: SigningKey, fallback_verifiers: Vec<VerifyingKey>) -> Self {
        Self {
            primary,
            fallback_verifiers,
        }
    }

    pub fn primary(&self) -> &SigningKey {
        &self.primary
    }

    pub fn active_verifying_key(&self) -> VerifyingKey {
        verifying_key_from(&self.primary)
    }

    pub fn fallback_verifying_keys(&self) -> &[VerifyingKey] {
        &self.fallback_verifiers
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        sign_message(&self.primary, message)
    }

    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<()> {
        let active = self.active_verifying_key();
        if verify_signature(&active, message, signature).is_ok() {
            return Ok(());
        }

        for verifier in &self.fallback_verifiers {
            if verify_signature(verifier, message, signature).is_ok() {
                return Ok(());
            }
        }

        Err(anyhow!("signature verification failed for all known keys"))
    }

    pub fn into_parts(self) -> (SigningKey, Vec<VerifyingKey>) {
        (self.primary, self.fallback_verifiers)
    }
}

impl Default for SigningKeyRing {
    fn default() -> Self {
        Self {
            primary: generate_signing_key(),
            fallback_verifiers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_round_trip_via_base64() {
        let signing = generate_signing_key();
        let encoded = BASE64_URL.encode(signing.to_bytes());
        let decoded = signing_key_from_base64(&encoded).expect("signing key decodes");
        assert_eq!(signing.to_bytes(), decoded.to_bytes());

        let verifying = verifying_key_from(&signing);
        let encoded_verifier = BASE64_URL.encode(verifying.to_bytes());
        let decoded_verifier =
            verifying_key_from_base64(&encoded_verifier).expect("verifier decodes");
        assert_eq!(verifying.to_bytes(), decoded_verifier.to_bytes());
    }

    #[test]
    fn signing_key_ring_verifies_with_fallback() {
        let primary = generate_signing_key();
        let fallback_signer = generate_signing_key();
        let fallback_verifier = verifying_key_from(&fallback_signer);

        let ring = SigningKeyRing::new(primary.clone(), vec![fallback_verifier]);
        let payload = b"hello world";

        let sig = sign_message(&primary, payload);
        ring.verify(payload, &sig)
            .expect("primary signature verifies");

        let fallback_sig = sign_message(&fallback_signer, payload);
        ring.verify(payload, &fallback_sig)
            .expect("fallback signature verifies");
    }

    #[test]
    fn signing_key_ring_rejects_unknown_signature() {
        let ring = SigningKeyRing::default();
        let payload = b"hello world";
        let other_signer = generate_signing_key();
        let sig = sign_message(&other_signer, payload);

        let err = ring.verify(payload, &sig).expect_err("verification fails");
        assert!(err.to_string().contains("failed"));
    }
}
