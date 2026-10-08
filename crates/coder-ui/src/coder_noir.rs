//! Coder Noir: Coder’s neutral accents over Superlogical’s “Static Noir” palette.

use rust_native::style::Color;

pub const CANVAS: u32 = 0x0a0a0a;
pub const SURFACE_SUBTLE: u32 = 0x101010;
pub const SURFACE_RAISED: u32 = 0x191919;
pub const SURFACE: u32 = 0x0e0e0e;
pub const TERMINAL_BACKGROUND: u32 = 0x0e0e0e;
pub const STROKE_SUBTLE: u32 = 0x2c2c2c;
pub const STROKE: u32 = 0x404040;
pub const SCROLLBAR_HOVER: u32 = 0x4e4e4e;
pub const CONTROL_HOVER: u32 = 0xededed;
pub const CONTROL: u32 = 0xededed;
pub const CONTROL_ON_OVERLAY: u32 = 0x1e1e1e;
pub const CONTROL_PRESSED: u32 = 0xededed;
pub const CONTENT: u32 = 0xededed;
pub const CONTENT_SECONDARY: u32 = 0x818181;
pub const CONTENT_TERTIARY: u32 = 0x656565;
pub const ACCENT: u32 = 0xededed;
pub const ACCENT_SOLID: u32 = 0xededed;
pub const ACCENT_ON_SOLID: u32 = 0x0a0a0a;
pub const ACCENT_CONTAINER: u32 = 0x191919;
pub const ACCENT_ON_CONTAINER: u32 = 0xededed;
pub const ACCENT_BORDER: u32 = 0xededed;
pub const ACCENT_RGB: u32 = 0xededed;
pub const ACCENT_DIM: u32 = 0xededed;
pub const ACCENT_DIM_SUBTLE: u32 = 0xededed;
pub const ACCENT_LINE: u32 = 0xededed;
pub const ACCENT_RING_OUTER: u32 = 0xededed;
pub const SELECTION: u32 = 0xededed;
pub const SELECTION_FOREGROUND: u32 = 0x0a0a0a;
pub const DANGER: u32 = 0xff4d42;
pub const DANGER_SOLID: u32 = 0xff4d42;
pub const DANGER_ON_SOLID: u32 = 0x0a0a0a;
pub const DANGER_CONTAINER: u32 = 0x2b1613;
pub const DANGER_ON_CONTAINER: u32 = 0xff4d42;
pub const DANGER_BORDER: u32 = 0xff4d42;
pub const DANGER_BG: u32 = 0x2b1613;
pub const SUCCESS: u32 = 0x9fd08a;
pub const SUCCESS_SOLID: u32 = 0x9fd08a;
pub const SUCCESS_ON_SOLID: u32 = 0x0a0a0a;
pub const SUCCESS_CONTAINER: u32 = 0x1e241c;
pub const SUCCESS_ON_CONTAINER: u32 = 0x9fd08a;
pub const SUCCESS_BORDER: u32 = 0x9fd08a;
pub const WARNING: u32 = 0xe6c15c;
pub const WARNING_SOLID: u32 = 0xe6c15c;
pub const WARNING_ON_SOLID: u32 = 0x0a0a0a;
pub const WARNING_CONTAINER: u32 = 0x262217;
pub const WARNING_ON_CONTAINER: u32 = 0xe6c15c;
pub const WARNING_BORDER: u32 = 0xe6c15c;
pub const INFO: u32 = 0x7fb2e8;
pub const INFO_SOLID: u32 = 0x7fb2e8;
pub const INFO_ON_SOLID: u32 = 0x0a0a0a;
pub const INFO_CONTAINER: u32 = 0x1a2027;
pub const INFO_ON_CONTAINER: u32 = 0x7fb2e8;
pub const INFO_BORDER: u32 = 0x7fb2e8;
pub const TERMINAL_FOREGROUND: u32 = 0xededed;
pub const TERMINAL_CURSOR: u32 = 0xededed;
pub const TERMINAL_SELECTION: u32 = 0x333333;
pub const TERMINAL_SELECTION_FOREGROUND: u32 = 0xffffff;
pub const TERMINAL_ANSI_0: u32 = 0x1a1a1a;
pub const TERMINAL_ANSI_1: u32 = 0xff4d42;
pub const TERMINAL_ANSI_2: u32 = 0x9fd08a;
pub const TERMINAL_ANSI_3: u32 = 0xe6c15c;
pub const TERMINAL_ANSI_4: u32 = 0x7fb2e8;
pub const TERMINAL_ANSI_5: u32 = 0xd093d0;
pub const TERMINAL_ANSI_6: u32 = 0x74cfd1;
pub const TERMINAL_ANSI_7: u32 = 0xc9c9c9;
pub const TERMINAL_ANSI_8: u32 = 0x666666;
pub const TERMINAL_ANSI_9: u32 = 0xff6e64;
pub const TERMINAL_ANSI_10: u32 = 0xb7e2a3;
pub const TERMINAL_ANSI_11: u32 = 0xf2d47c;
pub const TERMINAL_ANSI_12: u32 = 0x9dc7f2;
pub const TERMINAL_ANSI_13: u32 = 0xe0aede;
pub const TERMINAL_ANSI_14: u32 = 0x93e1e2;
pub const TERMINAL_ANSI_15: u32 = 0xffffff;
pub const CURSOR: u32 = TERMINAL_CURSOR;
pub const CURSOR_TEXT: u32 = CANVAS;
pub const ANSI: [u32; 16] = [
    TERMINAL_ANSI_0,
    TERMINAL_ANSI_1,
    TERMINAL_ANSI_2,
    TERMINAL_ANSI_3,
    TERMINAL_ANSI_4,
    TERMINAL_ANSI_5,
    TERMINAL_ANSI_6,
    TERMINAL_ANSI_7,
    TERMINAL_ANSI_8,
    TERMINAL_ANSI_9,
    TERMINAL_ANSI_10,
    TERMINAL_ANSI_11,
    TERMINAL_ANSI_12,
    TERMINAL_ANSI_13,
    TERMINAL_ANSI_14,
    TERMINAL_ANSI_15,
];

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
