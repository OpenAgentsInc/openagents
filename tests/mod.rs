mod admin_middleware;
mod admin_routes;
mod agent;
mod chat;
mod deepseek;
mod emailoptin;
mod github;
mod health_check;
mod nostr;
mod openrouter;
mod repomap;
mod solver;

// Only re-export what's actually needed
pub use agent::*;
