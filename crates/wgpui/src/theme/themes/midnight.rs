//! Midnight theme - neutral dark background with light gray text.

use super::super::{Theme, ThemeColors};
use crate::color::Hsla;

/// Midnight theme: #0A0A0A background, #CCCCCC text, neutral accents.
pub static MIDNIGHT: Theme = Theme {
    name: "Midnight",
    colors: ThemeColors {
        // Backgrounds
        background: Hsla::new(0.0, 0.0, 0.039, 1.0), // #0A0A0A
        surface: Hsla::new(0.0, 0.0, 0.0, 0.95),     // #000000f2
        elevated: Hsla::new(0.0, 0.0, 0.039, 1.0),   // #0A0A0A
        hover: Hsla::new(0.0, 0.0, 0.102, 1.0),      // #1A1A1A
        selected: Hsla::new(0.0, 0.0, 0.165, 1.0),   // #2A2A2A

        // Text
        text: Hsla::new(0.0, 0.0, 0.8, 1.0),          // #CCCCCC
        text_muted: Hsla::new(0.0, 0.0, 0.533, 1.0),  // #888888
        text_disabled: Hsla::new(0.0, 0.0, 0.4, 1.0), // #666666
        text_accent: Hsla::new(0.0, 0.0, 1.0, 1.0),   // #FFFFFF

        // Borders
        border: Hsla::new(0.0, 0.0, 1.0, 0.1), // #ffffff1a
        border_focused: Hsla::new(0.0, 0.0, 0.533, 1.0), // #888888
        border_transparent: Hsla::new(0.0, 0.0, 1.0, 0.05),

        // Status colors
        success: Hsla::new(0.389, 0.7, 0.45, 1.0), // Green (140/360)
        warning: Hsla::new(0.125, 1.0, 0.5, 1.0),  // Yellow (45/360)
        error: Hsla::new(0.0, 0.8, 0.5, 1.0),      // Red
        info: Hsla::new(0.583, 0.8, 0.55, 1.0),    // Blue (210/360)

        // Accents
        accent: Hsla::new(0.0, 0.0, 0.8, 1.0),        // #CCCCCC
        accent_hover: Hsla::new(0.0, 0.0, 0.75, 1.0), // #BFBFBF
    },
};
