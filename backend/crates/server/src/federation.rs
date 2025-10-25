use std::collections::HashMap;

use base64::Engine;
use openguild_core::{CanonicalEvent, EventId};
use openguild_crypto::{verify_signature, verifying_key_from_base64, Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::warn;
use uuid::Uuid;

use crate::{config::FederationConfig, messaging};

#[derive(Debug, Error)]
pub enum FederationError {
    #[error("trusted server '{server}' has invalid verifying key: {reason}")]
    InvalidTrustedServer { server: String, reason: String },
    #[error("received event from untrusted origin '{origin}'")]
    UntrustedOrigin { origin: String },
    #[error("event origin '{event_origin}' mismatched transaction origin '{origin}'")]
    OriginMismatch {
        origin: String,
        event_origin: String,
    },
    #[error("missing signature '{key_id}' for origin '{origin}'")]
    MissingSignature { origin: String, key_id: String },
    #[error("event id mismatch (expected {expected}, got {actual})")]
    EventIdMismatch { expected: EventId, actual: EventId },
    #[error("signature encoding invalid")]
    InvalidSignatureEncoding,
    #[error("signature verification failed")]
    SignatureVerificationFailed,
}

#[derive(Clone)]
struct TrustedPeer {
    key_id: String,
    verifying_key: VerifyingKey,
}

#[derive(Clone)]
pub struct FederationService {
    peers: HashMap<String, TrustedPeer>,
}

impl FederationService {
    pub fn from_config(config: &FederationConfig) -> Result<Option<Self>, FederationError> {
        if config.trusted_servers.is_empty() {
            return Ok(None);
        }

        let mut peers = HashMap::new();
        for peer in &config.trusted_servers {
            let verifying_key = verifying_key_from_base64(&peer.verifying_key).map_err(|err| {
                FederationError::InvalidTrustedServer {
                    server: peer.server_name.clone(),
                    reason: err.to_string(),
                }
            })?;

            peers.insert(
                peer.server_name.clone(),
                TrustedPeer {
                    key_id: peer.key_id.clone(),
                    verifying_key,
                },
            );
        }

        Ok(Some(Self { peers }))
    }

    pub fn is_trusted(&self, origin: &str) -> bool {
        self.peers.contains_key(origin)
    }

    pub fn evaluate_transaction(
        &self,
        origin: &str,
        events: Vec<CanonicalEvent>,
    ) -> FederationEvaluation {
        let mut evaluation = FederationEvaluation::new(origin.to_string());

        for event in events {
            match self.verify_event(origin, &event) {
                Ok(()) => evaluation.accepted_events.push(event),
                Err(err) => {
                    warn!(
                        %origin,
                        event_id = %event.event_id,
                        error = %err,
                        "federation event rejected"
                    );
                    evaluation.rejected.push(RejectedEvent {
                        event_id: event.event_id.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }

        evaluation
    }

    fn verify_event(&self, origin: &str, event: &CanonicalEvent) -> Result<(), FederationError> {
        let peer = self
            .peers
            .get(origin)
            .ok_or_else(|| FederationError::UntrustedOrigin {
                origin: origin.to_string(),
            })?;

        if event.origin_server != origin {
            return Err(FederationError::OriginMismatch {
                origin: origin.to_string(),
                event_origin: event.origin_server.clone(),
            });
        }

        let hash = event.canonical_hash();
        let expected_id = CanonicalEvent::event_id_from_hash(&hash);
        if expected_id != event.event_id {
            return Err(FederationError::EventIdMismatch {
                expected: expected_id,
                actual: event.event_id.clone(),
            });
        }

        let signature_key = format!("ed25519:{}", peer.key_id);
        let signature_b64 = event
            .signatures
            .get(origin)
            .and_then(|map| map.get(&signature_key))
            .ok_or_else(|| FederationError::MissingSignature {
                origin: origin.to_string(),
                key_id: signature_key.clone(),
            })?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(signature_b64)
            .map_err(|_| FederationError::InvalidSignatureEncoding)?;
        let signature =
            Signature::from_slice(&bytes).map_err(|_| FederationError::InvalidSignatureEncoding)?;

        verify_signature(&peer.verifying_key, &hash, &signature)
            .map_err(|_| FederationError::SignatureVerificationFailed)
    }
}

#[derive(Debug, Deserialize)]
pub struct TransactionRequest {
    pub origin: String,
    #[serde(default)]
    pub pdus: Vec<CanonicalEvent>,
}

#[derive(Debug, Serialize)]
pub struct FederationEventsResponse {
    pub origin: String,
    pub channel_id: Uuid,
    pub events: Vec<messaging::TimelineEvent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub origin: String,
    pub accepted: Vec<EventId>,
    pub rejected: Vec<RejectedEvent>,
    pub disabled: bool,
}

impl TransactionResponse {
    pub fn disabled(origin: String) -> Self {
        Self {
            origin,
            accepted: Vec::new(),
            rejected: Vec::new(),
            disabled: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RejectedEvent {
    pub event_id: EventId,
    pub reason: String,
}

pub struct FederationEvaluation {
    pub origin: String,
    pub accepted_events: Vec<CanonicalEvent>,
    pub rejected: Vec<RejectedEvent>,
}

impl FederationEvaluation {
    pub fn new(origin: String) -> Self {
        Self {
            origin,
            accepted_events: Vec::new(),
            rejected: Vec::new(),
        }
    }

    pub fn into_response(self, disabled: bool) -> TransactionResponse {
        TransactionResponse {
            origin: self.origin,
            accepted: self
                .accepted_events
                .into_iter()
                .map(|event| event.event_id)
                .collect(),
            rejected: self.rejected,
            disabled,
        }
    }
}
