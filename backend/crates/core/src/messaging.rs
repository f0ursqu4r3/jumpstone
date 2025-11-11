use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CanonicalEvent, EventBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAuthorSnapshot {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePayload {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<MessageAuthorSnapshot>,
}

impl MessagePayload {
    pub fn to_event(
        &self,
        origin_server: &str,
        room_id: &str,
        sender: &str,
        prev_events: Vec<String>,
    ) -> CanonicalEvent {
        let mut content = serde_json::Map::new();
        content.insert("content".into(), Value::String(self.content.clone()));
        if let Some(author) = &self.author {
            if let Ok(author_value) = serde_json::to_value(author) {
                content.insert("author".into(), author_value);
            }
        }

        EventBuilder::new(origin_server.to_owned(), room_id.to_owned(), "message")
            .sender(sender.to_owned())
            .content(Value::Object(content))
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
            let payload = MessagePayload {
                content: "hello world".into(),
                author: None,
            };
            let mut seen = HashSet::new();

            for _ in 0..count {
                let event = payload.to_event("example.org", &Uuid::new_v4().to_string(), "@user:example.org", Vec::new());
                prop_assert!(seen.insert(event.event_id.clone()), "duplicate event id generated");
            }
        }
    }
}
