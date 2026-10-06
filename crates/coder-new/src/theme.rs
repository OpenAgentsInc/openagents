//! Exact Grok Night RGB slots from Grok Build. See the crate's NOTICE.

use ratatui::style::Color;

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const BG_BASE: Color = rgb(0x141414);
pub const BG_LIGHT: Color = rgb(0x242424);
pub const BG_DARK: Color = rgb(0x1c1c1c);
pub const TEXT_PRIMARY: Color = rgb(0xe1e1e1);
pub const TEXT_SECONDARY: Color = rgb(0xc8c8c8);
pub const GRAY_DIM: Color = rgb(0x585858);
pub const GRAY: Color = rgb(0x6c6c6c);
pub const GRAY_BRIGHT: Color = rgb(0x787878);
pub const PROMPT_BORDER_ACTIVE: Color = rgb(0x505058);
pub const ACCENT_MODEL: Color = rgb(0x1abc9c);
pub const ACCENT_SKILL: Color = rgb(0x7aa2f7);
pub const ACCENT_SUCCESS: Color = rgb(0x9ece6a);
pub const PATH: Color = rgb(0xff9e64);
pub const MD_CODE: Color = rgb(0x3a95ab);
pub const DIFF_DELETE_FG: Color = rgb(0xf7768e);
pub const DIFF_DELETE_BG: Color = rgb(0x420e14);
pub const DIFF_INSERT_FG: Color = rgb(0x9ece6a);
pub const DIFF_INSERT_BG: Color = rgb(0x063806);
