//! UI components for the compute provider
//!
//! Uses wgpui and HUD components for a sci-fi styled interface.

pub mod backup;
pub mod dashboard;
pub mod root;

// Panel components
pub mod earnings_panel;
pub mod job_queue;
pub mod models_panel;
pub mod network_panel;
pub mod wallet_panel;

pub use backup::BackupScreen;
pub use dashboard::DashboardScreen;
pub use root::RootView;
