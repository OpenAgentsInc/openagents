//! Chat Service Layer for Coder
//!
//! This crate provides the main API for chat operations, bridging the AI infrastructure
//! (providers, tools, permissions, sessions) with the UI layer.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        ChatService                           │
//! │                Simple API: send_message() -> Stream          │
//! ├─────────────────────────────────────────────────────────────┤
//! │                     Internal Bridge                          │
//! │         SessionEvent/PermissionEvent → ChatUpdate            │
//! ├─────────────────────────────────────────────────────────────┤
//! │                   AI Infrastructure                          │
//! │      ProviderRegistry + ToolRegistry + PermissionManager     │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use coder_service::{ChatService, ServiceConfig};
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = ServiceConfig::from_env();
//!     let service = ChatService::new(config).await.unwrap();
//!
//!     let session = service.create_session(None).await.unwrap();
//!     let stream = service.send_message(session.id, "Hello!".into());
//!
//!     futures::pin_mut!(stream);
//!     while let Some(update) = stream.next().await {
//!         println!("{:?}", update);
//!     }
//! }
//! ```

mod bridge;
mod service;
mod update;

pub use service::{ChatService, ChatStream, ServiceConfig, ServiceError};
pub use update::{ChatUpdate, MessageRole, SessionStatus};

// Re-export commonly used types from dependencies
pub use coder_domain::PermissionId;
pub use coder_domain::ids::{MessageId, SessionId, ThreadId};
pub use coder_permission::{PermissionRequest, Response as PermissionResponse};
pub use coder_session::{AgentConfig, Session};
pub use coder_storage::Storage;
