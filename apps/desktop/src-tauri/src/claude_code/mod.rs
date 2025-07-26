pub mod discovery;
pub mod manager;
pub mod models;
pub mod convex_client;

pub use discovery::ClaudeDiscovery;
pub use manager::ClaudeManager;
pub use models::{Message, ClaudeConversation, UnifiedSession};