//! Shared application state

/// Application state shared across handlers
pub struct AppState {
    // Future: session storage, permission rules, etc.
}

impl AppState {
    /// Create new application state
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
