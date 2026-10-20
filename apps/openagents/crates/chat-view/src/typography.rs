//! Typography sizes every surface shares. The desktop's font catalog,
//! registration, and saved choices stay in `openagents_ui::typography`, which
//! re-exports these.

use gpui::{Rems, rems};

/// Convert a size designed at the 16px baseline into a scalable interface rem.
/// Code, diffs, and terminal text deliberately keep absolute pixel sizes.
pub const fn ui_rems(pixels_at_default: f32) -> Rems {
    rems(pixels_at_default / 16.0)
}

pub const CODE_FONT_SIZE_DEFAULT: f32 = 12.5;
pub const TERMINAL_FONT_SIZE_DEFAULT: f32 = 13.0;
pub const FONT_SIZE_MIN: f32 = 8.0;
pub const FONT_SIZE_MAX: f32 = 32.0;

/// Clamp an absolute code/terminal pixel size into the supported range.
pub fn clamp_font_size(size: f32) -> f32 {
    size.clamp(FONT_SIZE_MIN, FONT_SIZE_MAX)
}
