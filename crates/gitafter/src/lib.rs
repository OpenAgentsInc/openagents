//! GitAfter - Nostr-native GitHub alternative for agent-first collaboration

pub mod git;
pub mod middleware;
pub mod nostr;
pub mod notifications;
pub mod reputation;
pub mod review;
pub mod stacks;
pub mod trajectory;
pub mod views;
pub mod ws;

// Re-export commonly used types
pub use middleware::RateLimiter;
pub use nostr::{ErrorCategory, PublishResult, RelayFailure, NostrClient};
pub use ws::WsBroadcaster;
