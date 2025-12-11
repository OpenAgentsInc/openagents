//! MechaCoder - Claude Code harness with GPUI interface.
//!
//! This crate provides a focused UI for interacting with Claude Code,
//! featuring chat, diffs, terminal output, and permission handling.

use std::sync::Arc;

use gpui::App;

pub mod actions;
pub mod app_menus;
pub mod screen;
pub mod ui;

// Re-export key types
pub use actions::*;
pub use screen::MechaCoderScreen;

/// Initialize the theme system for MechaCoder.
///
/// This sets up the minimal Zed theme globals required for markdown rendering.
/// Must be called early in app initialization before any UI is rendered.
pub fn init_theme(cx: &mut App) {
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
