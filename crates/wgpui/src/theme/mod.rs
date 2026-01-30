//! Theme system for wgpui.
//!
//! Provides a global theme with colors accessible via `theme()`.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use wgpui::theme::theme;
//!
//! let bg = theme().colors.background;
//! let text = theme().colors.text;
//! ```

use crate::color::Hsla;
use std::sync::RwLock;

// Re-export submodules
pub mod builder;
pub mod color;
pub mod dynamic;
pub mod style;
pub mod themes;

// Re-export common types
pub use themes::MIDNIGHT;

/// Theme colors - all the colors a theme provides.
#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
    // Backgrounds
    /// Main app background
    pub background: Hsla,
    /// Surface/card background
    pub surface: Hsla,
    /// Elevated surface (modals, popovers)
    pub elevated: Hsla,
    /// Hover state background
    pub hover: Hsla,
    /// Selected/active state background
    pub selected: Hsla,

    // Text
    /// Primary text color
    pub text: Hsla,
    /// Muted/secondary text
    pub text_muted: Hsla,
    /// Disabled text
    pub text_disabled: Hsla,
    /// Accent text (links, highlights)
    pub text_accent: Hsla,

    // Borders
    /// Default border color
    pub border: Hsla,
    /// Focused border color
    pub border_focused: Hsla,
    /// Transparent/subtle border
    pub border_transparent: Hsla,

    // Status
    /// Success state
    pub success: Hsla,
    /// Warning state
    pub warning: Hsla,
    /// Error state
    pub error: Hsla,
    /// Info state
    pub info: Hsla,

    // Accents
    /// Primary accent color
    pub accent: Hsla,
    /// Accent hover state
    pub accent_hover: Hsla,
}

/// A complete theme definition.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// Theme name
    pub name: &'static str,
    /// Theme colors
    pub colors: ThemeColors,
}

// Global theme state
static CURRENT_THEME: RwLock<&'static Theme> = RwLock::new(&MIDNIGHT);

/// Get the current global theme.
///
/// # Example
///
/// ```rust,ignore
/// use wgpui::theme::theme;
///
/// let colors = theme().colors;
/// let bg = colors.background;
/// ```
pub fn theme() -> &'static Theme {
    *CURRENT_THEME.read().unwrap()
}

/// Set the current global theme.
///
/// # Example
///
/// ```rust,ignore
/// use wgpui::theme::{set_theme, themes::MIDNIGHT};
///
/// set_theme(&MIDNIGHT);
/// ```
pub fn set_theme(theme: &'static Theme) {
    *CURRENT_THEME.write().unwrap() = theme;
}

// ============================================================================
// Design tokens (non-color)
// ============================================================================

/// Font sizes in logical pixels.
pub mod font_size {
    /// Extra small (text-xs) - 12px
    pub const XS: f32 = 12.0;
    /// Small (text-sm) - 14px
    pub const SM: f32 = 14.0;
    /// Base/default (text-base) - 16px
    pub const BASE: f32 = 16.0;
    /// Large (text-lg) - 18px
    pub const LG: f32 = 18.0;
    /// Extra large (text-xl) - 20px
    pub const XL: f32 = 20.0;
    /// 2X large (text-2xl) - 24px
    pub const XXL: f32 = 24.0;
    /// 3X large (text-3xl) - 30px
    pub const XXXL: f32 = 30.0;
}

/// Line height multipliers.
pub mod line_height {
    /// Tight (leading-tight) - 1.25
    pub const TIGHT: f32 = 1.25;
    /// Snug (leading-snug) - 1.375
    pub const SNUG: f32 = 1.375;
    /// Normal (leading-normal) - 1.5
    pub const NORMAL: f32 = 1.5;
    /// Relaxed (leading-relaxed) - 1.625
    pub const RELAXED: f32 = 1.625;
    /// Loose (leading-loose) - 2.0
    pub const LOOSE: f32 = 2.0;
}

/// Spacing values in logical pixels (4px base unit).
pub mod spacing {
    /// 0.5 unit - 2px
    pub const HALF: f32 = 2.0;
    /// 1 unit - 4px
    pub const XS: f32 = 4.0;
    /// 2 units - 8px
    pub const SM: f32 = 8.0;
    /// 3 units - 12px
    pub const MD: f32 = 12.0;
    /// 4 units - 16px
    pub const LG: f32 = 16.0;
    /// 6 units - 24px
    pub const XL: f32 = 24.0;
    /// 8 units - 32px
    pub const XXL: f32 = 32.0;
    /// 12 units - 48px
    pub const XXXL: f32 = 48.0;
    /// 16 units - 64px
    pub const XXXXL: f32 = 64.0;
}

/// Shadow opacity values.
pub mod shadow {
    /// No shadow
    pub const NONE: f32 = 0.0;
    /// Subtle shadow (sm)
    pub const SM: f32 = 0.05;
    /// Default shadow
    pub const DEFAULT: f32 = 0.1;
    /// Medium shadow (md)
    pub const MD: f32 = 0.15;
    /// Large shadow (lg)
    pub const LG: f32 = 0.2;
    /// Extra large shadow (xl)
    pub const XL: f32 = 0.25;
}

/// Z-index layers for stacking.
pub mod z_index {
    /// Base layer
    pub const BASE: i32 = 0;
    /// Dropdown menus
    pub const DROPDOWN: i32 = 10;
    /// Sticky elements
    pub const STICKY: i32 = 20;
    /// Fixed elements
    pub const FIXED: i32 = 30;
    /// Modal backdrops
    pub const BACKDROP: i32 = 40;
    /// Modal dialogs
    pub const MODAL: i32 = 50;
    /// Popovers
    pub const POPOVER: i32 = 60;
    /// Tooltips
    pub const TOOLTIP: i32 = 70;
}

/// Animation durations in milliseconds.
pub mod duration {
    /// Instant (0ms)
    pub const INSTANT: u32 = 0;
    /// Fast (75ms)
    pub const FAST: u32 = 75;
    /// Normal (150ms)
    pub const NORMAL: u32 = 150;
    /// Slow (300ms)
    pub const SLOW: u32 = 300;
    /// Slower (500ms)
    pub const SLOWER: u32 = 500;
}

// ============================================================================
// Backwards-compatible color modules
// ============================================================================

/// Background colors aligned with Tailwind's dark theme.
pub mod bg {
    use crate::color::Hsla;

    /// Main app background (#0A0A0A)
    pub const APP: Hsla = Hsla::new(0.0, 0.0, 0.039, 1.0);

    /// Surface/card background - black with 95% opacity
    pub const SURFACE: Hsla = Hsla::new(0.0, 0.0, 0.0, 0.95);

    /// Muted background - matches `bg-muted` (#2A2A2A)
    pub const MUTED: Hsla = Hsla::new(0.0, 0.0, 0.165, 1.0);

    /// Code block background (#101010)
    pub const CODE: Hsla = Hsla::new(0.0, 0.0, 0.063, 1.0);

    /// Elevated surface (#0A0A0A)
    pub const ELEVATED: Hsla = Hsla::new(0.0, 0.0, 0.039, 1.0);

    /// Hover state background (#1A1A1A)
    pub const HOVER: Hsla = Hsla::new(0.0, 0.0, 0.102, 1.0);

    /// Selected/active state (#2A2A2A)
    pub const SELECTED: Hsla = Hsla::new(0.0, 0.0, 0.165, 1.0);
}

/// Text colors aligned with Tailwind's dark theme.
pub mod text {
    use crate::color::Hsla;

    /// Primary text - matches `text-foreground` (#CCCCCC)
    pub const PRIMARY: Hsla = Hsla::new(0.0, 0.0, 0.800, 1.0);

    /// Secondary text (#CCCCCC)
    pub const SECONDARY: Hsla = Hsla::new(0.0, 0.0, 0.800, 1.0);

    /// Muted text - matches `text-muted-foreground` (#888888)
    pub const MUTED: Hsla = Hsla::new(0.0, 0.0, 0.533, 1.0);

    /// Disabled text (#666666)
    pub const DISABLED: Hsla = Hsla::new(0.0, 0.0, 0.400, 1.0);
}

/// Accent colors for highlights and emphasis.
pub mod accent {
    use crate::color::Hsla;

    /// Primary accent - matches `--primary` (#CCCCCC)
    pub const PRIMARY: Hsla = Hsla::new(0.0, 0.0, 0.800, 1.0);

    /// Secondary accent - blue (#4A9EFF)
    pub const SECONDARY: Hsla = Hsla::new(0.592, 1.0, 0.65, 1.0);

    /// Blue alias for semantic use
    pub const BLUE: Hsla = Hsla::new(0.592, 1.0, 0.65, 1.0);

    /// Green accent (#00C853)
    pub const GREEN: Hsla = Hsla::new(0.403, 1.0, 0.39, 1.0);

    /// Red accent (#D32F2F)
    pub const RED: Hsla = Hsla::new(0.0, 0.76, 0.5, 1.0);

    /// Purple accent (#9C4DCC)
    pub const PURPLE: Hsla = Hsla::new(0.75, 0.65, 0.55, 1.0);
}

/// Border colors aligned with Tailwind's dark theme.
pub mod border {
    use crate::color::Hsla;

    /// Default border - matches `border` (#ffffff1a)
    pub const DEFAULT: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.1);

    /// Subtle border (#1A1A1A)
    pub const SUBTLE: Hsla = Hsla::new(0.0, 0.0, 0.102, 1.0);

    /// Focus border - matches `--ring` (#888888)
    pub const FOCUS: Hsla = Hsla::new(0.0, 0.0, 0.533, 1.0);

    /// Error border - semi-transparent red
    pub const ERROR: Hsla = Hsla::new(0.0, 0.76, 0.5, 0.5);
}

/// Status colors for states and feedback.
pub mod status {
    use crate::color::Hsla;

    /// Success/completed (#00C853)
    pub const SUCCESS: Hsla = Hsla::new(0.403, 1.0, 0.39, 1.0);

    /// Error/failed (#D32F2F)
    pub const ERROR: Hsla = Hsla::new(0.0, 0.76, 0.5, 1.0);

    /// Warning (#FFB400)
    pub const WARNING: Hsla = Hsla::new(0.117, 1.0, 0.5, 1.0);

    /// Running/in-progress (#FFB400)
    pub const RUNNING: Hsla = Hsla::new(0.117, 1.0, 0.5, 1.0);

    /// Info (#4A9EFF)
    pub const INFO: Hsla = Hsla::new(0.592, 1.0, 0.65, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_returns_midnight() {
        let t = theme();
        assert_eq!(t.name, "Midnight");
    }

    #[test]
    fn test_midnight_has_black_background() {
        let t = theme();
        assert!((t.colors.background.l - 0.039).abs() < 1e-3);
    }

    #[test]
    fn test_midnight_has_white_text() {
        let t = theme();
        assert!((t.colors.text.l - 0.8).abs() < 1e-3);
    }

    #[test]
    fn test_spacing_scale() {
        assert_eq!(spacing::XS, 4.0);
        assert_eq!(spacing::SM, 8.0);
        assert_eq!(spacing::MD, 12.0);
        assert_eq!(spacing::LG, 16.0);
        assert_eq!(spacing::XL, 24.0);
    }

    #[test]
    fn test_font_size_scale() {
        assert_eq!(font_size::XS, 12.0);
        assert_eq!(font_size::SM, 14.0);
        assert_eq!(font_size::BASE, 16.0);
        assert_eq!(font_size::LG, 18.0);
    }
}
