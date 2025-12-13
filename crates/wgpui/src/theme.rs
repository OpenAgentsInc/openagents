//! Bloomberg-inspired dark theme for wgpui.

use crate::color::Hsla;

/// Background colors
pub mod bg {
    use super::*;

    /// Pure black - main app background
    pub const APP: Hsla = Hsla::new(0.0, 0.0, 0.0, 1.0); // #000000

    /// Near black - surface/panel backgrounds
    pub const SURFACE: Hsla = Hsla::new(0.0, 0.0, 0.04, 1.0); // #0A0A0A

    /// Code block background
    pub const CODE: Hsla = Hsla::new(0.0, 0.0, 0.063, 1.0); // #101010

    /// Card/elevated surface background
    pub const CARD: Hsla = Hsla::new(0.0, 0.0, 0.05, 1.0); // #0D0D0D

    /// Hover state background
    pub const HOVER: Hsla = Hsla::new(0.0, 0.0, 0.08, 1.0); // #141414

    /// Selected/active state background
    pub const SELECTED: Hsla = Hsla::new(0.0, 0.0, 0.12, 1.0); // #1F1F1F
}

/// Text colors
pub mod text {
    use super::*;

    /// Primary text color - main content
    pub const PRIMARY: Hsla = Hsla::new(0.0, 0.0, 0.9, 1.0); // #E6E6E6

    /// Secondary text color - less emphasis
    pub const SECONDARY: Hsla = Hsla::new(0.0, 0.0, 0.69, 1.0); // #B0B0B0

    /// Muted text color - labels, hints
    pub const MUTED: Hsla = Hsla::new(0.0, 0.0, 0.62, 1.0); // #9E9E9E

    /// Disabled text color
    pub const DISABLED: Hsla = Hsla::new(0.0, 0.0, 0.4, 1.0); // #666666
}

/// Accent colors
pub mod accent {
    use super::*;

    /// Primary accent - Bloomberg yellow
    /// Hue: 42/360 = 0.117
    pub const PRIMARY: Hsla = Hsla::new(0.117, 1.0, 0.5, 1.0); // #FFB400

    /// Blue accent
    pub const BLUE: Hsla = Hsla::new(0.592, 1.0, 0.65, 1.0); // #4A9EFF

    /// Green accent
    pub const GREEN: Hsla = Hsla::new(0.403, 1.0, 0.39, 1.0); // #00C853

    /// Red accent
    pub const RED: Hsla = Hsla::new(0.0, 0.76, 0.5, 1.0); // #D32F2F

    /// Purple accent
    pub const PURPLE: Hsla = Hsla::new(0.75, 0.65, 0.55, 1.0); // #9C4DCC
}

/// Border colors
pub mod border {
    use super::*;

    /// Default border color
    pub const DEFAULT: Hsla = Hsla::new(0.0, 0.0, 0.1, 1.0); // #1A1A1A

    /// Subtle border color
    pub const SUBTLE: Hsla = Hsla::new(0.0, 0.0, 0.08, 1.0); // #141414

    /// Focus border color
    pub const FOCUS: Hsla = Hsla::new(0.117, 1.0, 0.5, 0.5); // Semi-transparent yellow

    /// Error border color
    pub const ERROR: Hsla = Hsla::new(0.0, 0.76, 0.5, 0.5); // Semi-transparent red
}

/// Status colors
pub mod status {
    use super::*;

    /// Success/completed status
    pub const SUCCESS: Hsla = Hsla::new(0.403, 1.0, 0.39, 1.0); // #00C853

    /// Error/failed status
    pub const ERROR: Hsla = Hsla::new(0.0, 0.76, 0.5, 1.0); // #D32F2F

    /// Warning status
    pub const WARNING: Hsla = Hsla::new(0.117, 1.0, 0.5, 1.0); // #FFB400

    /// Running/in-progress status
    pub const RUNNING: Hsla = Hsla::new(0.117, 1.0, 0.5, 1.0); // #FFB400

    /// Info status
    pub const INFO: Hsla = Hsla::new(0.592, 1.0, 0.65, 1.0); // #4A9EFF
}

/// Font sizes in logical pixels
pub mod font_size {
    /// Extra small (labels, badges)
    pub const XS: f32 = 9.0;

    /// Small (secondary text)
    pub const SM: f32 = 10.0;

    /// Base/default size
    pub const BASE: f32 = 11.0;

    /// Large (section headers)
    pub const LG: f32 = 12.0;

    /// Extra large (page titles)
    pub const XL: f32 = 14.0;

    /// 2X large (hero text)
    pub const XXL: f32 = 16.0;
}

/// Line heights
pub mod line_height {
    /// Tight line height (compact lists)
    pub const TIGHT: f32 = 2.0;

    /// Normal line height
    pub const NORMAL: f32 = 2.2;

    /// Relaxed line height (body text)
    pub const RELAXED: f32 = 2.5;
}

/// Spacing values in logical pixels
pub mod spacing {
    /// Extra small spacing
    pub const XS: f32 = 4.0;

    /// Small spacing
    pub const SM: f32 = 8.0;

    /// Medium spacing
    pub const MD: f32 = 12.0;

    /// Large spacing
    pub const LG: f32 = 16.0;

    /// Extra large spacing
    pub const XL: f32 = 24.0;

    /// 2X large spacing
    pub const XXL: f32 = 32.0;
}

/// Border radius values
pub mod radius {
    /// No radius
    pub const NONE: f32 = 0.0;

    /// Small radius
    pub const SM: f32 = 2.0;

    /// Default radius
    pub const DEFAULT: f32 = 4.0;

    /// Medium radius
    pub const MD: f32 = 6.0;

    /// Large radius
    pub const LG: f32 = 8.0;

    /// Full/pill radius
    pub const FULL: f32 = 9999.0;
}
