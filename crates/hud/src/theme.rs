//! HUD theme - white/opacity color palette on black background.

use wgpui::Hsla;

/// HUD color palette - all white with varying opacities.
pub mod hud {
    use super::*;

    /// Background - pure black.
    pub const BG: Hsla = Hsla::new(0.0, 0.0, 0.0, 1.0);

    /// Frame lines - bright white (full opacity).
    pub const FRAME_BRIGHT: Hsla = Hsla::new(0.0, 0.0, 1.0, 1.0);

    /// Frame lines - standard opacity.
    pub const FRAME_NORMAL: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.7);

    /// Frame lines - dim.
    pub const FRAME_DIM: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.4);

    /// Dot grid dots - subtle.
    pub const DOT_GRID: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.15);

    /// Primary text color.
    pub const TEXT: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.9);

    /// Muted text color.
    pub const TEXT_MUTED: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.5);

    /// Hover highlight color.
    pub const HOVER: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.2);

    /// Pressed/active color.
    pub const ACTIVE: Hsla = Hsla::new(0.0, 0.0, 1.0, 0.3);
}

/// Animation timing constants.
pub mod timing {
    /// Default enter animation duration in frames (at 60fps).
    /// ~250ms
    pub const ENTER_FRAMES: u32 = 15;

    /// Default exit animation duration in frames.
    /// ~167ms
    pub const EXIT_FRAMES: u32 = 10;

    /// Stagger offset between children in frames.
    /// ~50ms between each child starting
    pub const STAGGER_OFFSET: u32 = 3;
}
