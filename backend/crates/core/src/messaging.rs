use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{CanonicalEvent, EventBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePayload {
    pub content: String,
}

impl MessagePayload {
    pub fn to_event(
        &self,
        origin_server: &str,
        room_id: &str,
        sender: &str,
        prev_events: Vec<String>,
    ) -> CanonicalEvent {
        EventBuilder::new(origin_server.to_owned(), room_id.to_owned(), "message")
            .sender(sender.to_owned())
            .content(json!({
                "content": self.content,
            }))
            .prev_events(prev_events)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;
    use uuid::Uuid;

    proptest! {
        #[test]
        fn event_builder_produces_unique_event_ids(count in 1usize..64) {
            let payload = MessagePayload { content: "hello world".into() };
            let mut seen = HashSet::new();

            for _ in 0..count {
                let event = payload.to_event("example.org", &Uuid::new_v4().to_string(), "@user:example.org", Vec::new());
                prop_assert!(seen.insert(event.event_id.clone()), "duplicate event id generated");
            }
        }
    }
}
