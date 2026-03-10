//! Midnight theme - Arcade-style black/cyan palette with clear semantic states.

use super::super::{Theme, ThemeColors};
use crate::color::Hsla;

/// Midnight theme: Arcade-style black foundations and cyan interaction chrome.
pub static MIDNIGHT: Theme = Theme {
    name: "Midnight",
    colors: ThemeColors {
        // Backgrounds
        background: Hsla::new(0.0, 0.0, 0.012, 1.0), // #030303
        surface: Hsla::new(0.0, 0.0, 0.0, 1.0),      // #000000
        elevated: Hsla::new(0.547, 0.789, 0.149, 1.0), // #083344
        hover: Hsla::new(0.545, 0.636, 0.237, 1.0),  // #164e63
        selected: Hsla::new(0.540, 0.696, 0.271, 1.0), // #155e75
        code: Hsla::new(0.0, 0.0, 0.0, 1.0),         // #000000
        overlay: Hsla::new(0.540, 1.0, 0.635, 0.11), // Arcade overlay20
        overlay_subtle: Hsla::new(0.540, 1.0, 0.635, 0.05), // Arcade overlay50
        overlay_scrim: Hsla::new(0.0, 0.0, 0.0, 0.5), // Arcade overlay80
        glow: Hsla::new(0.533, 0.682, 0.618, 1.0),   // #5BC6E0

        // Text
        text: Hsla::new(0.0, 0.0, 1.0, 1.0), // #FFFFFF
        text_muted: Hsla::new(0.625, 0.016, 0.490, 1.0), // #7B7C7F
        text_disabled: Hsla::new(0.540, 0.696, 0.271, 1.0), // #155e75
        text_accent: Hsla::new(0.522, 0.857, 0.533, 1.0), // #22d3ee
        text_on_accent: Hsla::new(0.0, 0.0, 1.0, 1.0), // #FFFFFF

        // Borders
        border: Hsla::new(0.545, 0.636, 0.237, 1.0), // #164e63
        border_focused: Hsla::new(0.522, 0.857, 0.533, 1.0), // #22d3ee
        border_transparent: Hsla::new(0.540, 1.0, 0.635, 0.11), // overlay20
        border_active: Hsla::new(0.524, 0.945, 0.427, 1.0), // #06b6d4

        // Status colors
        success: Hsla::new(0.389, 0.7, 0.45, 1.0), // Green (140/360)
        warning: Hsla::new(0.125, 1.0, 0.5, 1.0),  // Yellow (45/360)
        error: Hsla::new(0.0, 0.8, 0.5, 1.0),      // Red
        info: Hsla::new(0.522, 0.857, 0.533, 1.0), // #22d3ee
        running: Hsla::new(0.524, 0.945, 0.427, 1.0), // #06b6d4

        // Accents
        accent: Hsla::new(0.522, 0.857, 0.533, 1.0), // #22d3ee
        accent_hover: Hsla::new(0.524, 0.945, 0.427, 1.0), // #06b6d4
        accent_secondary: Hsla::new(0.532, 0.914, 0.365, 1.0), // #0891b2
        accent_strong: Hsla::new(0.536, 0.823, 0.310, 1.0), // #0e7490
        accent_tertiary: Hsla::new(0.540, 0.696, 0.271, 1.0), // #155e75
    },
};
