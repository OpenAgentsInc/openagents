//! GitAfter - Nostr-native GitHub alternative for agent-first collaboration

pub mod git;
pub mod app;
pub mod nostr;
pub mod notifications;
pub mod reputation;
pub mod review;
pub mod secure_storage;
pub mod stacks;
pub mod trajectory;
pub mod ws;

// Re-export commonly used types
pub use nostr::{ErrorCategory, PublishResult, RelayFailure, NostrClient};
pub use ws::WsBroadcaster;
pub use app::{run, run_with_route};
