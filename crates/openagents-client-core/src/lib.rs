//! Shared OpenAgents client-core building blocks reused across web/desktop/iOS surfaces.

#[cfg(target_os = "ios")]
pub use wgpui::IosBackgroundState;

pub mod auth;
pub mod codex_control;
pub mod codex_worker;
pub mod command;
pub mod compatibility;
pub mod ffi;
pub mod ios_codex_state;
pub mod ios_mission_control;
pub mod ios_khala_session;
pub mod ios_worker_selection;
pub mod khala_protocol;
pub mod sync_persistence;
pub mod web_sync_storage;
