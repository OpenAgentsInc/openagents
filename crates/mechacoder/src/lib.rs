//! MechaCoder - Claude Code harness with GPUI interface.
//!
//! This crate provides a focused UI for interacting with Claude Code,
//! featuring chat, diffs, terminal output, and permission handling.

use std::sync::Arc;

use gpui::App;
use settings::{Settings, SettingsStore};

pub mod actions;
pub mod app_menus;
pub mod screen;
pub mod ui;

// Re-export key types
pub use actions::*;
pub use screen::MechaCoderScreen;

/// Minimal default settings JSON for MechaCoder.
/// This provides enough structure for ThemeSettings to work.
const MINIMAL_SETTINGS: &str = r#"{
    "ui_font_size": 14,
    "buffer_font_size": 14,
    "theme": "One Dark",
    "buffer_font_family": "Berkeley Mono",
    "ui_font_family": "Berkeley Mono"
}"#;

/// Initialize the settings and theme systems for MechaCoder.
///
/// This sets up the minimal Zed globals required for markdown rendering and UI components.
/// Must be called early in app initialization before any UI is rendered.
pub fn init_theme(cx: &mut App) {
    // Initialize settings store with minimal settings
    let store = SettingsStore::new(cx, MINIMAL_SETTINGS);
    cx.set_global(store);

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
