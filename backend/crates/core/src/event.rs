use std::collections::BTreeMap;

use base64::Engine;
use blake3::Hasher;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
/// Public alias representing canonical event identifiers.
pub type EventId = String;
/// Current schema version for canonical events.
pub const EVENT_SCHEMA_VERSION: u8 = 1;

fn default_event_version() -> u8 {
    EVENT_SCHEMA_VERSION
}

#[derive(Debug, Error)]
pub enum EventError {
    #[error("canonicalization failed: {0}")]
    Canonicalization(String),
    #[error("signature verification failed")]
    SignatureVerification,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CanonicalEvent {
    #[serde(default = "default_event_version")]
    pub schema_version: u8,
    pub event_id: EventId,
    pub origin_server: String,
    pub room_id: String,
    pub event_type: String,
    pub sender: String,
    pub origin_ts: i64,
    pub content: Value,
    #[serde(default)]
    pub prev_events: Vec<EventId>,
    #[serde(default)]
    pub auth_events: Vec<EventId>,
    #[serde(default)]
    pub signatures: BTreeMap<String, BTreeMap<String, String>>,
}

impl CanonicalEvent {
    pub fn current_schema_version() -> u8 {
        EVENT_SCHEMA_VERSION
    }

    pub fn sign_with(&mut self, server_name: &str, key_id: &str, signing_key: &SigningKey) {
        let hash = self.canonical_hash();
        let signature = signing_key.sign(&hash);
        let encoded = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        self.signatures
            .entry(server_name.to_owned())
            .or_default()
            .insert(format!("ed25519:{key_id}"), encoded);
    }

    pub fn verify_with(
        &self,
        server_name: &str,
        key_id: &str,
        verifying_key: &VerifyingKey,
    ) -> Result<(), EventError> {
        let hash = self.canonical_hash();
        let sig = self
            .signatures
            .get(server_name)
            .and_then(|map| map.get(&format!("ed25519:{key_id}")))
            .ok_or(EventError::SignatureVerification)?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(sig)
            .map_err(|_| EventError::SignatureVerification)?;
        let signature =
            Signature::from_slice(&bytes).map_err(|_| EventError::SignatureVerification)?;

        verifying_key
            .verify_strict(&hash, &signature)
            .map_err(|_| EventError::SignatureVerification)
    }

    pub fn canonical_hash(&self) -> Vec<u8> {
        let mut hasher = Hasher::new();
        let canonical = self.canonical_bytes();
        hasher.update(&canonical);
        hasher.finalize().as_bytes().to_vec()
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut cloned = self.clone();
        cloned.event_id.clear();
        cloned.signatures.clear();
        serde_json::to_vec(&cloned).expect("serialization must succeed")
    }

    pub fn event_id_from_hash(hash: &[u8]) -> EventId {
        let encoded = bs58::encode(hash).into_string();
        format!("${encoded}")
    }
}

pub struct EventBuilder {
    event: CanonicalEvent,
}

impl EventBuilder {
    pub fn new(
        origin_server: impl Into<String>,
        room_id: impl Into<String>,
        event_type: impl Into<String>,
    ) -> Self {
        let event = CanonicalEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: String::new(),
            origin_server: origin_server.into(),
            room_id: room_id.into(),
            event_type: event_type.into(),
            sender: String::new(),
            origin_ts: chrono::Utc::now().timestamp_millis(),
            content: Value::Null,
            prev_events: Vec::new(),
            auth_events: Vec::new(),
            signatures: BTreeMap::new(),
        };

        Self { event }
    }

    pub fn sender(mut self, sender: impl Into<String>) -> Self {
        self.event.sender = sender.into();
        self
    }

    pub fn content(mut self, content: Value) -> Self {
        self.event.content = content;
        self
    }

    pub fn prev_events(mut self, prev_events: Vec<EventId>) -> Self {
        self.event.prev_events = prev_events;
        self
    }

    pub fn auth_events(mut self, auth_events: Vec<EventId>) -> Self {
        self.event.auth_events = auth_events;
        self
    }

    pub fn schema_version(mut self, version: u8) -> Self {
        self.event.schema_version = version;
        self
    }

    pub fn build(mut self) -> CanonicalEvent {
        let hash = self.event.canonical_hash();
        self.event.event_id = CanonicalEvent::event_id_from_hash(&hash);
        self.event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_id_matches_canonical_hash() {
        let event = EventBuilder::new("example.org", "!room:example.org", "message")
            .sender("@user:example.org")
            .content(serde_json::json!({ "content": "hello world" }))
            .build();

        let hash = event.canonical_hash();
        let expected_id = CanonicalEvent::event_id_from_hash(&hash);
        assert_eq!(event.event_id, expected_id);
        assert!(event.event_id.starts_with('$'));
    }

    #[test]
    fn schema_version_defaults_to_current() {
        let event = EventBuilder::new("example.org", "!room:example.org", "message")
            .sender("@user:example.org")
            .content(serde_json::json!({ "content": "hello world" }))
            .build();

        assert_eq!(event.schema_version, EVENT_SCHEMA_VERSION);
    }

    #[test]
    fn schema_version_can_be_overridden() {
        let event = EventBuilder::new("example.org", "!room:example.org", "message")
            .schema_version(2)
            .build();

        assert_eq!(event.schema_version, 2);
        assert_ne!(event.event_id, String::new());
    }
}
