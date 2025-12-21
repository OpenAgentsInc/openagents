//! Unified application state

use std::sync::Arc;
use tokio::sync::RwLock;

use super::ws::WsBroadcaster;

/// Tab identifiers for navigation
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Wallet,
    Marketplace,
    Autopilot,
    AgentGit,
    Daemon,
    Settings,
}

impl Tab {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tab::Wallet => "wallet",
            Tab::Marketplace => "marketplace",
            Tab::Autopilot => "autopilot",
            Tab::AgentGit => "agentgit",
            Tab::Daemon => "daemon",
            Tab::Settings => "settings",
        }
    }
}

/// Unified application state shared across all routes
pub struct AppState {
    /// WebSocket broadcaster for real-time updates
    pub broadcaster: Arc<WsBroadcaster>,

    /// Currently active tab
    pub active_tab: RwLock<Tab>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            broadcaster: Arc::new(WsBroadcaster::new(64)),
            active_tab: RwLock::new(Tab::default()),
        }
    }
}
