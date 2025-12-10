//! Nostr chat state machine for OpenAgents.
//!
//! This crate provides a chat state machine that combines:
//! - NIP-28: Public chat channels
//! - NIP-90: DVM job requests/results
//!
//! # Example
//!
//! ```rust,no_run
//! use nostr_chat::{ChatState, ChatEvent};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create chat state
//!     let mut chat = ChatState::new();
//!
//!     // Set identity from mnemonic
//!     chat.set_identity_from_mnemonic(
//!         "leader monkey parrot ring guide accident before fence cannon height naive bean"
//!     ).unwrap();
//!
//!     // Subscribe to chat events
//!     let mut events = chat.subscribe();
//!
//!     // Connect to relays
//!     chat.connect().await.unwrap();
//!
//!     // Join a channel
//!     chat.join_channel("channel_event_id").await.unwrap();
//!
//!     // Process events
//!     while let Ok(event) = events.recv().await {
//!         match event {
//!             ChatEvent::MessageReceived { channel_id, message } => {
//!                 println!("New message in {}: {}", channel_id, message.content());
//!             }
//!             ChatEvent::JobResult { job_id, content } => {
//!                 println!("Job {} completed: {}", job_id, content);
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```

mod channel;
mod message;
mod state;

pub use channel::{Channel, ChannelListItem};
pub use message::{
    ChannelMessage, ChatMessage, JobRequestMessage, JobResultMessage, SystemMessage,
    SystemMessageType,
};
pub use state::{ChatError, ChatEvent, ChatState, DvmJob, DvmJobStatus};

// Re-export useful types from dependencies
pub use nostr::{Keypair, derive_keypair};
pub use nostr_relay::{Filter, RelayPool};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_chat_state() {
        let state = ChatState::new();
        assert!(!state.has_identity());
    }

    #[test]
    fn test_identity_from_mnemonic() {
        let mut state = ChatState::new();
        state
            .set_identity_from_mnemonic(
                "leader monkey parrot ring guide accident before fence cannon height naive bean",
            )
            .unwrap();
        assert!(state.has_identity());
        // Known npub for this mnemonic
        assert_eq!(
            state.npub().unwrap(),
            "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu"
        );
    }
}
