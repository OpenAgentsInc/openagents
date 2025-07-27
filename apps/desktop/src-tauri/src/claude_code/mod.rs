pub mod discovery;
pub mod manager;
pub mod models;
pub mod convex_client;
pub mod database;
pub mod convex_impl;
pub mod auth;
pub mod commands;
pub mod token_storage;
pub mod auth_metrics;
pub mod cors_utils;
pub mod error_recovery;

pub use discovery::ClaudeDiscovery;
pub use manager::ClaudeManager;
pub use models::{Message, ClaudeConversation, UnifiedSession};
pub use database::{
    ConvexDatabase, SessionRepository, MessageRepository
};
pub use convex_impl::EnhancedConvexClient;
pub use token_storage::{TokenStorage, TokenEntry, TokenInfo};
// pub use auth::{AuthService, AuthContext}; // Commented out - will be used in auth integration (Issue #1215)