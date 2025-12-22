//! AgentGit - Nostr-native GitHub alternative for agent-first collaboration

pub mod git;
pub mod nostr;
pub mod reputation;
pub mod stacks;
pub mod views;
pub mod ws;

// Re-export commonly used types
pub use nostr::{ErrorCategory, PublishResult, RelayFailure, NostrClient};
pub use ws::WsBroadcaster;
