//! Historical USGC accents with Coder's existing neutral colors. See NOTICE.

use ratatui::{
    style::{Color, Style},
    text::Line,
};

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const BG_BASE: Color = rgb(coder_ui::theme::NEAR_BLACK);
pub const BG_LIGHT: Color = rgb(0x242424);
pub const BG_DARK: Color = rgb(0x1c1c1c);
pub const TEXT_PRIMARY: Color = rgb(0xe1e1e1);
pub const TEXT_SECONDARY: Color = rgb(0xc8c8c8);
pub const GRAY_DIM: Color = rgb(0x585858);
pub const GRAY: Color = rgb(0x6c6c6c);
pub const GRAY_BRIGHT: Color = rgb(0x787878);
pub const PROMPT_BORDER_ACTIVE: Color = rgb(0x505058);

// USGC foreground roles from the retained frontend palette: info, accent,
// amber, success, warning, and error. Cyan is its running-text color.
pub const ACCENT_MODEL: Color = rgb(0x00ffff);
pub const ACCENT_DELEGATE: Color = rgb(0xff00ff);
pub const COMMAND: Color = rgb(0xffbf00);
pub const ACCENT_SKILL: Color = rgb(0x00ffff);
pub const ACCENT_SUCCESS: Color = rgb(0x00a645);
pub const PATH: Color = rgb(0xff6600);
pub const MD_CODE: Color = rgb(0x00ffff);
pub const DIFF_DELETE_FG: Color = rgb(0xff0000);
pub const DIFF_INSERT_FG: Color = rgb(0x00a645);
pub const DIFF_DELETE_BG: Color = dim(0xff0000);
pub const DIFF_INSERT_BG: Color = dim(0x00a645);

// Terminal backgrounds have no alpha channel. Quarter-intensity USGC colors
// retain quiet change bands beneath the syntax-highlighted text.
const fn dim(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) as u8) / 4,
        ((hex >> 8) as u8) / 4,
        (hex as u8) / 4,
    )
}

/// Apply Coder's accents to imported Markdown and diff styles.
pub(crate) fn usgc_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    for line in &mut lines {
        line.style = usgc_style(line.style);
        for span in &mut line.spans {
            span.style = usgc_style(span.style);
        }
    }
    lines
}

fn usgc_style(style: Style) -> Style {
    Style {
        fg: style.fg.map(usgc_color),
        bg: style.bg.map(usgc_color),
        ..style
    }
}

fn usgc_color(color: Color) -> Color {
    let Color::Rgb(red, green, blue) = color else {
        return color;
    };
    let hex = u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue);
    match hex {
        0x420e14 => DIFF_DELETE_BG,
        0x063806 => DIFF_INSERT_BG,
        0x914c54 | 0xdb4b4b | 0xde5971 | 0xf7768e | 0xfc7b7b | 0xff5370 => DIFF_DELETE_FG,
        0x9ece6a => ACCENT_SUCCESS,
        0xff9e64 => PATH,
        0x9abdf5 | 0xc0cefc | 0xe0af68 | 0xffdb69 => COMMAND,
        0x9d7cd8 | 0xb267e6 | 0xba3c97 | 0xbb9af7 => ACCENT_DELEGATE,
        0x0db9d7 | 0x1abc9c | 0x3a95ab | 0x41a6b5 | 0x449dab | 0x6183bb | 0x61bdf2 | 0x6d91de
        | 0x73daca | 0x7aa2f7 | 0x7aa6da | 0x7dcfff | 0x89ddff | 0xb4f9f8 => ACCENT_SKILL,
        0x4e5579 | 0x51597d | 0x5a638c => GRAY_DIM,
        0x646e9c | 0x747ca1 | 0x9aa5ce => GRAY_BRIGHT,
        _ => color,
    }
}
