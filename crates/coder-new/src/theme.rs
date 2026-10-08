//! Coder Noir roles for the native Coder renderer.

use ratatui::{
    style::{Color, Style},
    text::Line,
};

const fn rgb(value: u32) -> Color {
    Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

pub const BG_BASE: Color = rgb(coder_ui::coder_noir::TERMINAL_BACKGROUND);
pub const BG_LIGHT: Color = rgb(coder_ui::coder_noir::SURFACE_RAISED);
pub const BG_DARK: Color = rgb(coder_ui::coder_noir::SURFACE_SUBTLE);
pub const TEXT_PRIMARY: Color = rgb(coder_ui::coder_noir::ANSI[15]);
pub const TEXT_SECONDARY: Color = rgb(coder_ui::coder_noir::CONTENT);
pub const GRAY_DIM: Color = rgb(coder_ui::coder_noir::CONTENT_TERTIARY);
pub const GRAY: Color = rgb(coder_ui::coder_noir::CONTENT_TERTIARY);
pub const GRAY_BRIGHT: Color = rgb(coder_ui::coder_noir::CONTENT_SECONDARY);
pub const PROMPT_BORDER_ACTIVE: Color = rgb(coder_ui::coder_noir::CONTENT_SECONDARY);
pub const ACCENT_MODEL: Color = rgb(coder_ui::coder_noir::ANSI[6]);
pub const ACCENT_DELEGATE: Color = rgb(coder_ui::coder_noir::ANSI[5]);
pub const COMMAND: Color = rgb(coder_ui::coder_noir::WARNING);
pub const ACCENT_SKILL: Color = rgb(coder_ui::coder_noir::INFO);
pub const ACCENT_SUCCESS: Color = rgb(coder_ui::coder_noir::SUCCESS);
pub const PATH: Color = rgb(coder_ui::coder_noir::WARNING);
pub const MD_CODE: Color = rgb(coder_ui::coder_noir::ANSI[6]);
pub const DIFF_DELETE_FG: Color = rgb(coder_ui::coder_noir::DANGER);
pub const DIFF_INSERT_FG: Color = rgb(coder_ui::coder_noir::SUCCESS);
pub const DIFF_DELETE_BG: Color = rgb(coder_ui::coder_noir::DANGER_CONTAINER);
pub const DIFF_INSERT_BG: Color = rgb(coder_ui::coder_noir::SUCCESS_CONTAINER);
pub const CURSOR: Color = rgb(coder_ui::coder_noir::CURSOR);
pub const CURSOR_TEXT: Color = rgb(coder_ui::coder_noir::CURSOR_TEXT);

/// Apply the shared appearance to imported Markdown and diff styles.
pub(crate) fn noir_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    for line in &mut lines {
        line.style = noir_style(line.style);
        for span in &mut line.spans {
            span.style = noir_style(span.style);
        }
    }
    lines
}

fn noir_style(style: Style) -> Style {
    Style {
        fg: style.fg.map(noir_color),
        bg: style.bg.map(noir_color),
        ..style
    }
}

fn noir_color(value: Color) -> Color {
    let Color::Rgb(red, green, blue) = value else {
        return value;
    };
    let value = coder_ui::source_theme::remap(coder_ui::coder_noir::rgb(
        u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue),
    ));
    Color::Rgb(value.red, value.green, value.blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_roles_match_the_shared_coder_noir_profile() {
        use coder_ui::source_theme as shared;
        for (native, shared) in [
            (BG_BASE, shared::BG_BASE),
            (BG_LIGHT, shared::BG_LIGHT),
            (BG_DARK, shared::BG_DARK),
            (TEXT_PRIMARY, shared::TEXT_PRIMARY),
            (TEXT_SECONDARY, shared::TEXT_SECONDARY),
            (GRAY_DIM, shared::GRAY_DIM),
            (GRAY, shared::GRAY),
            (GRAY_BRIGHT, shared::GRAY_BRIGHT),
            (PROMPT_BORDER_ACTIVE, shared::PROMPT_BORDER_ACTIVE),
            (ACCENT_MODEL, shared::ACCENT_MODEL),
            (ACCENT_SKILL, shared::ACCENT_SKILL),
            (ACCENT_DELEGATE, shared::ACCENT_DELEGATE),
            (COMMAND, shared::COMMAND),
            (PATH, shared::PATH),
            (MD_CODE, shared::MD_CODE),
            (DIFF_DELETE_FG, shared::DIFF_DELETE_FG),
            (DIFF_INSERT_FG, shared::DIFF_INSERT_FG),
            (DIFF_DELETE_BG, shared::DIFF_DELETE_BG),
            (DIFF_INSERT_BG, shared::DIFF_INSERT_BG),
        ] {
            assert_eq!(native, Color::Rgb(shared.red, shared.green, shared.blue));
        }
        assert_eq!(CURSOR, TEXT_SECONDARY);
        assert_ne!(PROMPT_BORDER_ACTIVE, DIFF_DELETE_FG);
    }
}
