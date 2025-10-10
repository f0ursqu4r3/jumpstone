//! Voice SFU signaling helpers (placeholder).

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalingOffer {
    pub sdp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalingAnswer {
    pub sdp: String,
}
