//! Dashboard application state

/// Dashboard application state shared across routes
#[derive(Clone)]
pub struct DashboardState {
    pub db_path: String,
}

impl DashboardState {
    /// Create new dashboard state
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }
}
