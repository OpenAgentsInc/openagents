//! GitAfter - Nostr-native GitHub alternative for agent-first collaboration

pub mod git;
pub mod app;
pub mod deprecation;
pub mod gui;
pub mod middleware;
pub mod nostr;
pub mod notifications;
pub mod reputation;
pub mod review;
pub mod server;
pub mod secure_storage;
pub mod stacks;
pub mod trajectory;
pub mod views;
pub mod ws;

// Re-export commonly used types
pub use nostr::{ErrorCategory, PublishResult, RelayFailure, NostrClient};
pub use ws::WsBroadcaster;
pub use app::{run as run_legacy, run_with_route as run_with_route_legacy};
pub use gui::{run_gui, run_gui_with_route};

pub fn run() -> anyhow::Result<()> {
    run_with_route(None)
}

pub fn run_with_route(route: Option<&str>) -> anyhow::Result<()> {
    if std::env::var_os("OPENAGENTS_GITAFTER_LEGACY_WEB").is_some() {
        run_with_route_legacy(route)
    } else {
        run_gui_with_route(route)
    }
}
