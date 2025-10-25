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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn signaling_offer_round_trips_through_json() {
        let offer = SignalingOffer {
            sdp: "v=0\no=- 4611739550431830983 2 IN IP4 127.0.0.1".into(),
        };

        let value = serde_json::to_value(&offer).expect("offer serializes");
        assert_eq!(
            value,
            json!({ "sdp": offer.sdp }),
            "serialized offer should expose SDP field"
        );

        let decoded: SignalingOffer =
            serde_json::from_value(value).expect("offer deserializes from json");
        assert_eq!(decoded.sdp, offer.sdp);
    }

    #[test]
    fn signaling_answer_round_trips_through_json() {
        let answer = SignalingAnswer {
            sdp: "v=0\no=- 4611739550431830983 2 IN IP4 127.0.0.1".into(),
        };

        let encoded = serde_json::to_string(&answer).expect("answer serializes to string");
        let decoded: SignalingAnswer =
            serde_json::from_str(&encoded).expect("answer deserializes from string");
        assert_eq!(decoded.sdp, answer.sdp);
    }
}
