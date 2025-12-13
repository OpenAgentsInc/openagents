//! Theme colors - Bloomberg Terminal inspired dark theme
//!
//! All colors use HSLA format matching theme_oa.

use crate::color::Hsla;

/// Helper to create const colors
const fn c(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla::new(h, s, l, a)
}

// Typography
pub const FONT_SIZE: f32 = 11.0;
pub const FONT_SIZE_SM: f32 = 10.0;
pub const FONT_SIZE_LG: f32 = 12.0;
pub const LINE_HEIGHT: f32 = 1.15;

/// Background colors
pub mod bg {
    use super::*;

    pub const APP: Hsla = c(0.0, 0.0, 0.0, 1.0);
    pub const SURFACE: Hsla = c(0.0, 0.0, 0.04, 1.0);
    pub const ELEVATED: Hsla = c(0.0, 0.0, 0.10, 1.0);
    pub const CARD: Hsla = c(0.0, 0.0, 0.11, 1.0);
    pub const HOVER: Hsla = c(0.0, 0.0, 0.11, 1.0);
}

/// Border colors
pub mod border {
    use super::*;

    pub const DEFAULT: Hsla = c(0.0, 0.0, 0.10, 1.0);
    pub const SUBTLE: Hsla = c(0.0, 0.0, 0.08, 1.0);
    pub const STRONG: Hsla = c(0.0, 0.0, 0.165, 1.0);
    pub const FOCUS: Hsla = c(0.117, 1.0, 0.50, 1.0);
}

/// Text colors
pub mod text {
    use super::*;

    pub const PRIMARY: Hsla = c(0.0, 0.0, 0.93, 1.0);
    pub const SECONDARY: Hsla = c(0.0, 0.0, 0.62, 1.0);
    pub const MUTED: Hsla = c(0.0, 0.0, 0.50, 1.0);
    pub const PLACEHOLDER: Hsla = c(0.0, 0.0, 0.40, 1.0);
}

/// Accent colors
pub mod accent {
    use super::*;

    pub const PRIMARY: Hsla = c(0.117, 1.0, 0.50, 1.0);  // Yellow
    pub const BLUE: Hsla = c(0.606, 1.0, 0.58, 1.0);
    pub const GREEN: Hsla = c(0.403, 0.70, 0.50, 1.0);
    pub const RED: Hsla = c(0.0, 0.64, 0.51, 1.0);
}

/// Theme configuration
pub struct Theme {
    pub font_size: f32,
    pub line_height: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
        }
    }
}
