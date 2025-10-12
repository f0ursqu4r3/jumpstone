//! Cryptographic helpers shared across the OpenGuild backend.

use anyhow::Result;
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
