//! Theme colors ported from theme_oa
//!
//! Bloomberg Terminal inspired - dark, high density, semantic colors only.
//! All colors are HSLA format: [h, s, l, a] where h is 0-1 (not degrees).

/// HSLA color type: [hue, saturation, lightness, alpha]
pub type Color = [f32; 4];

/// Helper to create const colors
const fn c(h: f32, s: f32, l: f32, a: f32) -> Color {
    [h, s, l, a]
}

// =============================================================================
// TYPOGRAPHY
// =============================================================================

pub const FONT_FAMILY: &str = "Berkeley Mono";
pub const FONT_SIZE: f32 = 11.0;
pub const FONT_SIZE_SM: f32 = 10.0;
pub const FONT_SIZE_LG: f32 = 12.0;
pub const FONT_SIZE_XS: f32 = 9.0;
pub const LINE_HEIGHT: f32 = 1.15;
pub const LINE_HEIGHT_RELAXED: f32 = 1.3;

// =============================================================================
// BACKGROUND COLORS
// =============================================================================

pub mod bg {
    use super::*;

    /// Pure black - main app background (#000000)
    pub const APP: Color = c(0.0, 0.0, 0.0, 1.0);

    /// Near black - surface background (#0A0A0A)
    pub const SURFACE: Color = c(0.0, 0.0, 0.04, 1.0);

    /// Elevated surface (#1A1A1A)
    pub const ELEVATED: Color = c(0.0, 0.0, 0.10, 1.0);

    /// Card/panel background (#1C1C1C)
    pub const CARD: Color = c(0.0, 0.0, 0.11, 1.0);

    /// Hover state background (#1C1C1C)
    pub const HOVER: Color = c(0.0, 0.0, 0.11, 1.0);

    /// Selected item background (#1C1C1C)
    pub const SELECTED: Color = c(0.0, 0.0, 0.11, 1.0);

    /// Header background (#101010)
    pub const HEADER: Color = c(0.0, 0.0, 0.06, 1.0);

    /// Code block background (#101010)
    pub const CODE: Color = c(0.0, 0.0, 0.06, 1.0);
}

// =============================================================================
// BORDER COLORS
// =============================================================================

pub mod border {
    use super::*;

    /// Default border (#1A1A1A)
    pub const DEFAULT: Color = c(0.0, 0.0, 0.10, 1.0);

    /// Subtle border
    pub const SUBTLE: Color = c(0.0, 0.0, 0.08, 1.0);

    /// Strong border (#2A2A2A)
    pub const STRONG: Color = c(0.0, 0.0, 0.165, 1.0);

    /// Selected item border
    pub const SELECTED: Color = c(0.0, 0.0, 0.30, 1.0);

    /// Focus ring - Bloomberg yellow
    pub const FOCUS: Color = c(0.117, 1.0, 0.50, 1.0);
}

// =============================================================================
// TEXT COLORS
// =============================================================================

pub mod text {
    use super::*;

    /// Primary text - main content (#E6E6E6)
    pub const PRIMARY: Color = c(0.0, 0.0, 0.90, 1.0);

    /// Bright text - values, data (#FFFFFF)
    pub const BRIGHT: Color = c(0.0, 0.0, 1.0, 1.0);

    /// Secondary text - less emphasis (#B0B0B0)
    pub const SECONDARY: Color = c(0.0, 0.0, 0.69, 1.0);

    /// Muted text - labels, hints (#9E9E9E)
    pub const MUTED: Color = c(0.0, 0.0, 0.62, 1.0);

    /// Disabled text (#505050)
    pub const DISABLED: Color = c(0.0, 0.0, 0.31, 1.0);

    /// Placeholder text (#9E9E9E)
    pub const PLACEHOLDER: Color = c(0.0, 0.0, 0.62, 1.0);

    /// Link text color - Bloomberg blue (#2979FF)
    pub const LINK: Color = c(0.606, 1.0, 0.58, 1.0);

    /// Bloomberg yellow - highlights (#FFB400)
    pub const HIGHLIGHT: Color = c(0.117, 1.0, 0.50, 1.0);
}

// =============================================================================
// ACCENT COLORS
// =============================================================================

pub mod accent {
    use super::*;

    /// Bloomberg yellow (#FFB400) - primary highlight
    pub const PRIMARY: Color = c(0.117, 1.0, 0.50, 1.0);

    /// Secondary violet (#7E57C2)
    pub const SECONDARY: Color = c(0.72, 0.46, 0.55, 1.0);

    /// Blue (#2979FF)
    pub const BLUE: Color = c(0.606, 1.0, 0.58, 1.0);

    /// Green (#00C853)
    pub const GREEN: Color = c(0.403, 1.0, 0.39, 1.0);

    /// Red (#D32F2F)
    pub const RED: Color = c(0.0, 0.64, 0.51, 1.0);

    /// Orange (#FF6F00)
    pub const ORANGE: Color = c(0.072, 1.0, 0.50, 1.0);
}

// =============================================================================
// STATUS COLORS
// =============================================================================

pub mod status {
    use super::*;

    pub const SUCCESS: Color = c(0.403, 1.0, 0.39, 1.0);
    pub const SUCCESS_BG: Color = c(0.403, 0.50, 0.15, 0.4);

    pub const ERROR: Color = c(0.0, 0.64, 0.51, 1.0);
    pub const ERROR_BG: Color = c(0.0, 0.50, 0.20, 0.4);

    pub const WARNING: Color = c(0.072, 1.0, 0.50, 1.0);
    pub const WARNING_BG: Color = c(0.072, 0.50, 0.20, 0.4);

    pub const INFO: Color = c(0.606, 1.0, 0.58, 1.0);
    pub const INFO_BG: Color = c(0.606, 0.50, 0.20, 0.4);
}

// =============================================================================
// INPUT COLORS
// =============================================================================

pub mod input {
    use super::*;

    pub const PLACEHOLDER: Color = c(0.0, 0.0, 0.62, 1.0);
    pub const TEXT: Color = c(0.0, 0.0, 0.90, 1.0);
    pub const CURSOR: Color = c(0.117, 1.0, 0.50, 1.0);
    pub const SELECTION: Color = c(0.0, 0.0, 0.20, 0.60);
    pub const BG: Color = c(0.0, 0.0, 0.06, 1.0);
    pub const BORDER: Color = c(0.0, 0.0, 0.10, 1.0);
}

// =============================================================================
// UI COMPONENT COLORS
// =============================================================================

pub mod ui {
    pub mod button {
        use super::super::*;

        // Default - white/light for primary action
        pub const DEFAULT_BG: Color = c(0.0, 0.0, 0.90, 1.0);
        pub const DEFAULT_TEXT: Color = c(0.0, 0.0, 0.0, 1.0);
        pub const DEFAULT_HOVER_BG: Color = c(0.0, 0.0, 0.80, 1.0);

        // Secondary
        pub const SECONDARY_BG: Color = c(0.0, 0.0, 0.10, 1.0);
        pub const SECONDARY_TEXT: Color = c(0.0, 0.0, 0.90, 1.0);
        pub const SECONDARY_HOVER_BG: Color = c(0.0, 0.0, 0.15, 1.0);

        // Ghost
        pub const GHOST_BG: Color = c(0.0, 0.0, 0.0, 0.0);
        pub const GHOST_TEXT: Color = c(0.0, 0.0, 0.90, 1.0);
        pub const GHOST_HOVER_BG: Color = c(0.0, 0.0, 0.10, 1.0);

        // Outline
        pub const OUTLINE_BG: Color = c(0.0, 0.0, 0.0, 0.0);
        pub const OUTLINE_TEXT: Color = c(0.0, 0.0, 0.90, 1.0);
        pub const OUTLINE_BORDER: Color = c(0.0, 0.0, 0.20, 1.0);
        pub const OUTLINE_HOVER_BG: Color = c(0.0, 0.0, 0.10, 1.0);

        // Destructive - red
        pub const DESTRUCTIVE_BG: Color = c(0.0, 0.64, 0.51, 1.0);
        pub const DESTRUCTIVE_TEXT: Color = c(0.0, 0.0, 1.0, 1.0);
        pub const DESTRUCTIVE_HOVER_BG: Color = c(0.0, 0.64, 0.45, 1.0);

        // Link
        pub const LINK_TEXT: Color = c(0.606, 1.0, 0.58, 1.0);
        pub const LINK_HOVER_TEXT: Color = c(0.606, 1.0, 0.70, 1.0);
    }
}
