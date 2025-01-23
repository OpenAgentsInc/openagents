mod admin_middleware;
mod admin_routes;
mod agent;
mod deepseek;
mod emailoptin;
mod github;
mod health_check;
mod openrouter;
mod repomap;
mod solver;
mod chat;
mod nostr;

// Only re-export what's actually needed
pub use agent::*;