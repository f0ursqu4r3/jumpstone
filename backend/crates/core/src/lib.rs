//! Core domain types, canonical JSON handling, and signatures.

pub mod event;
pub mod messaging;

pub use event::{CanonicalEvent, EventBuilder, EventId};
