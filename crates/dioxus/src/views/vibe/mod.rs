mod screen;
mod types;
mod data;

mod projects;
mod editor;
mod database;
mod deploy;

pub use screen::VibeScreen;

// Shared theme constants for the Vibe surface
pub const BG: &str = "#030303";
pub const PANEL: &str = "#0a0a0a";
pub const BORDER: &str = "#1c1c1c";
pub const TEXT: &str = "#e6e6e6";
pub const MUTED: &str = "#9a9a9a";
pub const ACCENT: &str = "#ffb400";
