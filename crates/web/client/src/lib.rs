mod app;
mod autopilot_chat;
mod claude_agent;
mod claude_chat;
mod fs_access;
mod hud;
mod intro_agent;
mod ml_viz;
mod gptoss_viz;
mod ml_gate;
mod nostr;
mod state;
mod telemetry;
mod utils;
mod views;
mod wallet;

pub use app::start_demo;
pub use telemetry::{TelemetryCollector, TelemetryEvent, set_panic_hook, track_cta_click};
