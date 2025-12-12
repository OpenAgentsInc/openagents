//! MechaCoder - Claude Code harness with GPUI interface.
//!
//! This crate provides a focused UI for interacting with Claude Code,
//! featuring chat, diffs, terminal output, and permission handling.

use std::sync::Arc;

use gpui::App;
use settings::Settings;

pub mod actions;
pub mod app_menus;
pub mod panels;
pub mod pi_thread;
pub mod screen;
pub mod sdk_thread;
pub mod ui;

// Re-export key types
pub use actions::*;
pub use pi_thread::{PiThread, PiThreadEvent};
pub use screen::MechaCoderScreen;
pub use sdk_thread::{SdkThread, SdkThreadEvent, ThreadEntry, ThreadStatus};

/// Initialize the settings and theme systems for MechaCoder.
///
/// This sets up SettingsStore and GlobalTheme which are required for Zed's
/// markdown rendering and UI components.
pub fn init_theme(cx: &mut App) {
    // Initialize settings from assets/settings/default.json
    settings::init(cx);

    // Register ThemeSettings so ThemeSettings::get_global works
    theme::ThemeSettings::register(cx);

    // Get default theme from Zed's theme family
    let theme_family = theme::zed_default_themes();
    let theme = theme_family
        .themes
        .into_iter()
        .next()
        .expect("zed_default_themes should have at least one theme");

    // Get default icon theme
    let icon_theme = theme::default_icon_theme();

    // Set the global theme
    cx.set_global(theme::GlobalTheme::new(Arc::new(theme), icon_theme));
}
