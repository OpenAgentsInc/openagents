//! OpenAgents Marketplace
//!
//! Three sub-marketplaces in one:
//! - **Agents**: Publish and discover AI agents with benchmark scores
//! - **Compute**: Sell spare compute for Bitcoin via "Go Online"
//! - **Services**: Browse DVMs (NIP-90) and MCP servers with pricing
//!
//! # Usage
//!
//! ```rust
//! use marketplace::MarketplaceScreen;
//!
//! let screen = cx.new(|cx| MarketplaceScreen::new(cx));
//! ```

mod types;
mod resource_bar;
mod tab_bar;
mod activity_feed;
mod screen;
mod nostr_bridge;

pub mod agents;
pub mod compute;
pub mod services;

// Re-export main types
pub use types::*;
pub use screen::MarketplaceScreen;
pub use nostr_bridge::NostrBridge;

// Re-export TextInput from ui crate for convenience
pub use ui::TextInput;

// Re-export nostr-chat types for convenience
pub use nostr_chat::{ChatEvent, ChatState, DvmJob, DvmJobStatus};
