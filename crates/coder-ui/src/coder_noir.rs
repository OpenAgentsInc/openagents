//! Coder Noir: Coder’s neutral accents over Superlogical’s “Static Noir” palette.
//!
//! Terminal UIs paint with these values only; GUI surfaces choose between
//! this and [`crate::coder_light`] through [`crate::gui_theme`].

use rust_native::style::Color;

// The values live in `oa-tokens`, the token table the web themes are
// generated from, so native and web Noir cannot drift apart.
pub use oa_tokens::noir::*;

/// Coder Noir's GUI role palette (the dark side of the token table).
pub const PALETTE: oa_tokens::Palette = oa_tokens::Palette::NOIR;

pub const fn rgb(value: u32) -> Color {
    Color::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// Shared browser tokens, generated from the same roles as native rendering.
pub fn css_variables() -> String {
    let mut css = String::from(":root{");
    css.push_str(&format!("--noir-canvas:#{:06x};", CANVAS));
    css.push_str(&format!("--noir-surface-subtle:#{:06x};", SURFACE_SUBTLE));
    css.push_str(&format!("--noir-surface-raised:#{:06x};", SURFACE_RAISED));
    css.push_str(&format!("--noir-surface:#{:06x};", SURFACE));
    css.push_str(&format!(
        "--noir-terminal-background:#{:06x};",
        TERMINAL_BACKGROUND
    ));
    css.push_str(&format!("--noir-stroke-subtle:#{:06x};", STROKE_SUBTLE));
    css.push_str(&format!("--noir-stroke:#{:06x};", STROKE));
    css.push_str(&format!("--noir-scrollbar-hover:#{:06x};", SCROLLBAR_HOVER));
    css.push_str(&format!(
        "--noir-control-hover:rgb({} {} {} / 0.047);",
        CONTROL_HOVER >> 16,
        (CONTROL_HOVER >> 8) & 255,
        CONTROL_HOVER & 255
    ));
    css.push_str(&format!(
        "--noir-control:rgb({} {} {} / 0.023);",
        CONTROL >> 16,
        (CONTROL >> 8) & 255,
        CONTROL & 255
    ));
    css.push_str(&format!(
        "--noir-control-on-overlay:#{:06x};",
        CONTROL_ON_OVERLAY
    ));
    css.push_str(&format!(
        "--noir-control-pressed:rgb({} {} {} / 0.079);",
        CONTROL_PRESSED >> 16,
        (CONTROL_PRESSED >> 8) & 255,
        CONTROL_PRESSED & 255
    ));
    css.push_str(&format!("--noir-content:#{:06x};", CONTENT));
    css.push_str(&format!(
        "--noir-content-secondary:#{:06x};",
        CONTENT_SECONDARY
    ));
    css.push_str(&format!(
        "--noir-content-tertiary:#{:06x};",
        CONTENT_TERTIARY
    ));
    css.push_str(&format!("--noir-accent:#{:06x};", ACCENT));
    css.push_str(&format!("--noir-accent-solid:#{:06x};", ACCENT_SOLID));
    css.push_str(&format!("--noir-accent-on-solid:#{:06x};", ACCENT_ON_SOLID));
    css.push_str(&format!(
        "--noir-accent-container:#{:06x};",
        ACCENT_CONTAINER
    ));
    css.push_str(&format!(
        "--noir-accent-on-container:#{:06x};",
        ACCENT_ON_CONTAINER
    ));
    css.push_str(&format!("--noir-accent-border:#{:06x};", ACCENT_BORDER));
    css.push_str(&format!("--noir-accent-rgb:#{:06x};", ACCENT_RGB));
    css.push_str(&format!(
        "--noir-accent-dim:rgb({} {} {} / 0.177);",
        ACCENT_DIM >> 16,
        (ACCENT_DIM >> 8) & 255,
        ACCENT_DIM & 255
    ));
    css.push_str(&format!(
        "--noir-accent-dim-subtle:rgb({} {} {} / 0.102);",
        ACCENT_DIM_SUBTLE >> 16,
        (ACCENT_DIM_SUBTLE >> 8) & 255,
        ACCENT_DIM_SUBTLE & 255
    ));
    css.push_str(&format!("--noir-accent-line:#{:06x};", ACCENT_LINE));
    css.push_str(&format!(
        "--noir-accent-ring-outer:rgb({} {} {} / 0.232);",
        ACCENT_RING_OUTER >> 16,
        (ACCENT_RING_OUTER >> 8) & 255,
        ACCENT_RING_OUTER & 255
    ));
    css.push_str(&format!("--noir-selection:#{:06x};", SELECTION));
    css.push_str(&format!(
        "--noir-selection-foreground:#{:06x};",
        SELECTION_FOREGROUND
    ));
    css.push_str(&format!("--noir-danger:#{:06x};", DANGER));
    css.push_str(&format!("--noir-danger-solid:#{:06x};", DANGER_SOLID));
    css.push_str(&format!("--noir-danger-on-solid:#{:06x};", DANGER_ON_SOLID));
    css.push_str(&format!(
        "--noir-danger-container:#{:06x};",
        DANGER_CONTAINER
    ));
    css.push_str(&format!(
        "--noir-danger-on-container:#{:06x};",
        DANGER_ON_CONTAINER
    ));
    css.push_str(&format!("--noir-danger-border:#{:06x};", DANGER_BORDER));
    css.push_str(&format!("--noir-danger-bg:#{:06x};", DANGER_BG));
    css.push_str(&format!("--noir-success:#{:06x};", SUCCESS));
    css.push_str(&format!("--noir-success-solid:#{:06x};", SUCCESS_SOLID));
    css.push_str(&format!(
        "--noir-success-on-solid:#{:06x};",
        SUCCESS_ON_SOLID
    ));
    css.push_str(&format!(
        "--noir-success-container:#{:06x};",
        SUCCESS_CONTAINER
    ));
    css.push_str(&format!(
        "--noir-success-on-container:#{:06x};",
        SUCCESS_ON_CONTAINER
    ));
    css.push_str(&format!("--noir-success-border:#{:06x};", SUCCESS_BORDER));
    css.push_str(&format!("--noir-warning:#{:06x};", WARNING));
    css.push_str(&format!("--noir-warning-solid:#{:06x};", WARNING_SOLID));
    css.push_str(&format!(
        "--noir-warning-on-solid:#{:06x};",
        WARNING_ON_SOLID
    ));
    css.push_str(&format!(
        "--noir-warning-container:#{:06x};",
        WARNING_CONTAINER
    ));
    css.push_str(&format!(
        "--noir-warning-on-container:#{:06x};",
        WARNING_ON_CONTAINER
    ));
    css.push_str(&format!("--noir-warning-border:#{:06x};", WARNING_BORDER));
    css.push_str(&format!("--noir-info:#{:06x};", INFO));
    css.push_str(&format!("--noir-info-solid:#{:06x};", INFO_SOLID));
    css.push_str(&format!("--noir-info-on-solid:#{:06x};", INFO_ON_SOLID));
    css.push_str(&format!("--noir-info-container:#{:06x};", INFO_CONTAINER));
    css.push_str(&format!(
        "--noir-info-on-container:#{:06x};",
        INFO_ON_CONTAINER
    ));
    css.push_str(&format!("--noir-info-border:#{:06x};", INFO_BORDER));
    css.push_str(&format!(
        "--noir-terminal-foreground:#{:06x};",
        TERMINAL_FOREGROUND
    ));
    css.push_str(&format!("--noir-terminal-cursor:#{:06x};", TERMINAL_CURSOR));
    css.push_str(&format!(
        "--noir-terminal-selection:#{:06x};",
        TERMINAL_SELECTION
    ));
    css.push_str(&format!(
        "--noir-terminal-selection-foreground:#{:06x};",
        TERMINAL_SELECTION_FOREGROUND
    ));
    css.push_str(&format!("--noir-terminal-ansi-0:#{:06x};", TERMINAL_ANSI_0));
    css.push_str(&format!("--noir-terminal-ansi-1:#{:06x};", TERMINAL_ANSI_1));
    css.push_str(&format!("--noir-terminal-ansi-2:#{:06x};", TERMINAL_ANSI_2));
    css.push_str(&format!("--noir-terminal-ansi-3:#{:06x};", TERMINAL_ANSI_3));
    css.push_str(&format!("--noir-terminal-ansi-4:#{:06x};", TERMINAL_ANSI_4));
    css.push_str(&format!("--noir-terminal-ansi-5:#{:06x};", TERMINAL_ANSI_5));
    css.push_str(&format!("--noir-terminal-ansi-6:#{:06x};", TERMINAL_ANSI_6));
    css.push_str(&format!("--noir-terminal-ansi-7:#{:06x};", TERMINAL_ANSI_7));
    css.push_str(&format!("--noir-terminal-ansi-8:#{:06x};", TERMINAL_ANSI_8));
    css.push_str(&format!("--noir-terminal-ansi-9:#{:06x};", TERMINAL_ANSI_9));
    css.push_str(&format!(
        "--noir-terminal-ansi-10:#{:06x};",
        TERMINAL_ANSI_10
    ));
    css.push_str(&format!(
        "--noir-terminal-ansi-11:#{:06x};",
        TERMINAL_ANSI_11
    ));
    css.push_str(&format!(
        "--noir-terminal-ansi-12:#{:06x};",
        TERMINAL_ANSI_12
    ));
    css.push_str(&format!(
        "--noir-terminal-ansi-13:#{:06x};",
        TERMINAL_ANSI_13
    ));
    css.push_str(&format!(
        "--noir-terminal-ansi-14:#{:06x};",
        TERMINAL_ANSI_14
    ));
    css.push_str(&format!(
        "--noir-terminal-ansi-15:#{:06x};",
        TERMINAL_ANSI_15
    ));
    css.push_str(
        "--noir-cursor:var(--noir-terminal-cursor);--noir-cursor-text:var(--noir-canvas);}",
    );
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_accents_stay_neutral_and_distinct_from_errors() {
        for color in [ACCENT, ACCENT_SOLID, ACCENT_BORDER, SELECTION, CURSOR] {
            assert_eq!(color >> 16, (color >> 8) & 255);
            assert_eq!((color >> 8) & 255, color & 255);
            assert_ne!(color, DANGER);
        }
        assert_ne!(CURSOR, CURSOR_TEXT);
        assert_ne!(SELECTION, SELECTION_FOREGROUND);
    }

    #[test]
    fn browser_tokens_preserve_translucent_controls_and_neutral_cursor() {
        let css = css_variables();
        assert!(css.starts_with(":root{"));
        assert!(css.ends_with('}'));
        assert!(css.contains("--noir-control-hover:rgb(237 237 237 / 0.047)"));
        assert!(css.contains("--noir-terminal-cursor:#ededed"));
        assert!(!css.contains("#ff3b30"));
    }
}
