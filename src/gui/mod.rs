//! Unified GUI module for OpenAgents desktop application
//!
//! Provides a tabbed interface combining:
//! - Wallet (identity, payments)
//! - Marketplace (compute, skills, data)
//! - Autopilot (agent sessions)
//! - GitAfter (Nostr-native git)
//! - Daemon status

mod app;
mod middleware;
mod routes;
mod server;
mod state;
mod views;
mod ws;

pub use app::run;
