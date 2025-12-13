//! Nostr Relay Server Implementation
//!
//! This crate provides the core logic for running a NIP-01 compliant Nostr relay:
//!
//! - **Protocol**: Message parsing for client→relay and relay→client messages
//! - **Subscription**: Filter matching and subscription management
//! - **Storage**: Storage trait for event persistence (implement for SQLite, etc.)
//! - **Verification**: Event signature verification
//!
//! # Architecture
//!
//! This crate is runtime-agnostic. It provides the protocol logic but does NOT include:
//! - WebSocket handling (use your runtime's WebSocket implementation)
//! - Async runtime (use tokio, async-std, or Cloudflare Workers)
//! - Specific database implementation (implement the Storage trait)
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use nostr_relay::{ClientMessage, RelayMessage, Subscription, Filter};
//!
//! // Parse incoming message from WebSocket
//! let msg = ClientMessage::from_json(websocket_text)?;
//!
//! match msg {
//!     ClientMessage::Event(event) => {
//!         // Verify and store event
//!         if nostr_relay::verify_event(&event)? {
//!             storage.store_event(&event).await?;
//!             // Send OK response
//!             let response = RelayMessage::ok_success(&event.id);
//!             websocket.send(response.to_json()).await?;
//!             // Broadcast to matching subscriptions
//!             for sub in subscriptions.matching(&event) {
//!                 // ...
//!             }
//!         }
//!     }
//!     ClientMessage::Req { subscription_id, filters } => {
//!         // Query stored events
//!         for filter in &filters {
//!             let events = storage.query(filter).await?;
//!             for event in events {
//!                 let msg = RelayMessage::event(&subscription_id, event);
//!                 websocket.send(msg.to_json()).await?;
//!             }
//!         }
//!         // Send EOSE
//!         let eose = RelayMessage::eose(&subscription_id);
//!         websocket.send(eose.to_json()).await?;
//!         // Store subscription for future events
//!         subscriptions.add(subscription_id, filters);
//!     }
//!     ClientMessage::Close { subscription_id } => {
//!         subscriptions.remove(&subscription_id);
//!     }
//! }
//! ```

mod filter;
mod message;
mod storage;
mod subscription;
mod verify;

pub use filter::Filter;
pub use message::{ClientMessage, RelayMessage};
pub use nostr::Event;
pub use storage::{EventQuery, SqlQueryBuilder, Storage, StorageError, StorageResult};
pub use subscription::{Subscription, SubscriptionManager};
pub use verify::{verify_event, verify_event_id, VerifyError};

/// NIP-90 job kinds - re-export from nostr crate
pub mod nip90 {
    pub use nostr::{
        InputType, JobFeedback, JobInput, JobParam, JobRequest, JobResult, JobStatus, Nip90Error,
        JOB_REQUEST_KIND_MAX, JOB_REQUEST_KIND_MIN, JOB_RESULT_KIND_MAX, JOB_RESULT_KIND_MIN,
        KIND_JOB_FEEDBACK, KIND_JOB_IMAGE_GENERATION, KIND_JOB_SPEECH_TO_TEXT,
        KIND_JOB_SUMMARIZATION, KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION,
        KIND_JOB_TRANSLATION, get_request_kind, get_result_kind, is_dvm_kind,
        is_job_feedback_kind, is_job_request_kind, is_job_result_kind,
    };
}
