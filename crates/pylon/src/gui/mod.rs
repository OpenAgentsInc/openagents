//! Pylon GUI (placeholder)
//!
//! TODO: Implement WGPUI-based GUI following autopilot pattern
//! - Dashboard showing earnings, active jobs, backend status
//! - Settings for relays, pricing, backend preferences
//! - Real-time job visualization

/// Placeholder for GUI app
#[cfg(feature = "gui")]
pub struct PylonApp {
    // TODO: Implement WGPUI app
}

#[cfg(feature = "gui")]
impl PylonApp {
    pub fn new() -> Self {
        Self {}
    }
}
