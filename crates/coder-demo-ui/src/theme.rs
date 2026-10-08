//! Coder colors from the shared source appearance profile.

use ratatui::style::Color;

const fn color(value: rust_native::style::Color) -> Color {
    Color::Rgb(value.red, value.green, value.blue)
}

pub const BG_BASE: Color = color(coder_ui::source_theme::BG_BASE);
pub const BG_LIGHT: Color = color(coder_ui::source_theme::BG_LIGHT);
pub const BG_DARK: Color = color(coder_ui::source_theme::BG_DARK);
pub const TEXT_PRIMARY: Color = color(coder_ui::source_theme::TEXT_PRIMARY);
pub const TEXT_SECONDARY: Color = color(coder_ui::source_theme::TEXT_SECONDARY);
pub const GRAY_DIM: Color = color(coder_ui::source_theme::GRAY_DIM);
pub const GRAY: Color = color(coder_ui::source_theme::GRAY);
pub const GRAY_BRIGHT: Color = color(coder_ui::source_theme::GRAY_BRIGHT);
pub const PROMPT_BORDER_ACTIVE: Color = color(coder_ui::source_theme::PROMPT_BORDER_ACTIVE);
pub const ACCENT_MODEL: Color = color(coder_ui::source_theme::ACCENT_MODEL);
pub const ACCENT_DELEGATE: Color = color(coder_ui::source_theme::ACCENT_DELEGATE);
pub const COMMAND: Color = color(coder_ui::source_theme::COMMAND);
pub const ACCENT_SKILL: Color = color(coder_ui::source_theme::ACCENT_SKILL);
pub const ACCENT_SUCCESS: Color = color(coder_ui::source_theme::ACCENT_SUCCESS);
pub const DIFF_DELETE_FG: Color = color(coder_ui::source_theme::DIFF_DELETE_FG);
pub const DIFF_INSERT_FG: Color = color(coder_ui::source_theme::DIFF_INSERT_FG);

pub const CURSOR: Color = color(coder_ui::coder_noir::rgb(coder_ui::coder_noir::CURSOR));
pub const CURSOR_TEXT: Color = color(coder_ui::coder_noir::rgb(coder_ui::coder_noir::CURSOR_TEXT));
