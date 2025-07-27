use std::sync::Arc;
use tokio::sync::Mutex;
use crate::claude_code::{ClaudeDiscovery, ClaudeManager};

/// Application state managed by Tauri
pub struct AppState {
    /// Claude Code discovery service
    pub discovery: Arc<Mutex<ClaudeDiscovery>>,
    /// Claude Code manager instance
    pub manager: Arc<Mutex<Option<ClaudeManager>>>,
}

impl AppState {
    /// Create a new application state instance
    pub fn new() -> Self {
        Self {
            discovery: Arc::new(Mutex::new(ClaudeDiscovery::new())),
            manager: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_state_creation() {
        let state = AppState::new();
        
        // Verify we can lock the discovery mutex
        let discovery = state.discovery.lock().await;
        drop(discovery);
        
        // Verify we can lock the manager mutex and it's None initially
        let manager = state.manager.lock().await;
        assert!(manager.is_none());
    }

    #[test]
    fn test_app_state_default() {
        let _state = AppState::default();
        // Just verify it creates without panic
    }
}