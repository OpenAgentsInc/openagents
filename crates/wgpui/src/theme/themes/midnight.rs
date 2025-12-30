//! Midnight theme - pure black background, pure white text.

use crate::color::Hsla;
use super::super::{Theme, ThemeColors};

/// Midnight theme: pure black background, pure white text, yellow accents.
pub static MIDNIGHT: Theme = Theme {
    name: "Midnight",
    colors: ThemeColors {
        // Pure black background
        background: Hsla::new(0.0, 0.0, 0.0, 1.0),      // #000000
        surface: Hsla::new(0.0, 0.0, 0.05, 1.0),        // #0d0d0d
        elevated: Hsla::new(0.0, 0.0, 0.08, 1.0),       // #141414
        hover: Hsla::new(0.0, 0.0, 0.12, 1.0),          // #1f1f1f
        selected: Hsla::new(0.0, 0.0, 0.15, 1.0),       // #262626

        // Pure white text
        text: Hsla::new(0.0, 0.0, 1.0, 1.0),            // #ffffff
        text_muted: Hsla::new(0.0, 0.0, 0.7, 1.0),      // #b3b3b3
        text_disabled: Hsla::new(0.0, 0.0, 0.4, 1.0),   // #666666
        text_accent: Hsla::new(0.125, 1.0, 0.5, 1.0),   // Yellow accent (45/360)

        // Subtle borders
        border: Hsla::new(0.0, 0.0, 0.2, 1.0),          // #333333
        border_focused: Hsla::new(0.125, 1.0, 0.5, 0.5), // Yellow 50%
        border_transparent: Hsla::new(0.0, 0.0, 0.1, 0.5),

        // Status colors
        success: Hsla::new(0.389, 0.7, 0.45, 1.0),      // Green (140/360)
        warning: Hsla::new(0.125, 1.0, 0.5, 1.0),       // Yellow (45/360)
        error: Hsla::new(0.0, 0.8, 0.5, 1.0),           // Red
        info: Hsla::new(0.583, 0.8, 0.55, 1.0),         // Blue (210/360)

        // Accents (Bloomberg yellow)
        accent: Hsla::new(0.125, 1.0, 0.5, 1.0),        // #FFB400 (45/360)
        accent_hover: Hsla::new(0.125, 1.0, 0.55, 1.0),
    },
};
