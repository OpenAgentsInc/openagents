pub mod discovery;
pub mod manager;
pub mod models;
pub mod convex_client;
pub mod database;
pub mod convex_impl;
pub mod auth;
pub mod commands;

pub use discovery::ClaudeDiscovery;
pub use manager::ClaudeManager;
pub use models::{Message, ClaudeConversation, UnifiedSession};
pub use database::{
    ConvexDatabase, SessionRepository, MessageRepository, ApmRepository, UserRepository, BatchOperations
};
pub use convex_impl::EnhancedConvexClient;
pub use auth::{AuthService, AuthContext, ConvexAuth};