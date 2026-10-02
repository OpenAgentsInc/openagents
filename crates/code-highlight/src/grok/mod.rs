//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI).
//!
//! grok-build's syntax highlighting, for the terminal: syntect with
//! two-face's syntaxes and grok-build's patched Swift grammar
//! ([`syntax`]), grok-build's Grok Night and Grok Day `.tmTheme` files
//! ([`theme`]), its RGB-to-terminal mapping and quantization ([`color`]),
//! its light/dark choice ([`appearance`]), and its incremental highlighter
//! for a code block still streaming in ([`open_code`]).

pub mod appearance;
pub mod color;
pub mod open_code;
pub mod syntax;
pub mod theme;

use std::sync::OnceLock;

use ratatui::style::{Modifier, Style};

/// The syntect this module highlights with, for callers that drive it.
pub use syntect;

pub use appearance::Appearance;
pub use color::ColorLevel;
pub use open_code::{HlLine, OpenCodeHighlighter};
pub use syntax::Syntect;
pub use theme::{DiffColors, Palette};

/// One highlighted token's look, as syntect gave it: the theme's RGB and
/// font style. Quantized only when drawn, at the terminal's level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token {
    pub rgb: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Token {
    /// The token of a syntect style.
    pub fn from_syntect(style: syntect::highlighting::Style) -> Self {
        use syntect::highlighting::FontStyle;
        Token {
            rgb: (style.foreground.r, style.foreground.g, style.foreground.b),
            bold: style.font_style.contains(FontStyle::BOLD),
            italic: style.font_style.contains(FontStyle::ITALIC),
            underline: style.font_style.contains(FontStyle::UNDERLINE),
        }
    }

    /// grok-build's `syntect_to_ratatui_fg`: foreground only, at `level`.
    pub fn style(self, level: ColorLevel) -> Style {
        let (r, g, b) = self.rgb;
        let mut out = Style::default().fg(color::syntect_rgb_to_fg(r, g, b, level));
        if self.bold {
            out = out.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            out = out.add_modifier(Modifier::ITALIC);
        }
        if self.underline {
            out = out.add_modifier(Modifier::UNDERLINED);
        }
        out
    }
}

static PALETTE: OnceLock<Palette> = OnceLock::new();

/// Fixes the process's palette. A surface that paints its own background
/// calls this before its first frame with that field's polarity. Returns
/// `Err` when the palette was already chosen.
pub fn set_palette(palette: Palette) -> Result<(), Palette> {
    PALETTE.set(palette)
}

/// The process's palette: the one set, or the terminal's polarity from the
/// environment (Grok Night when nothing says).
pub fn palette() -> Palette {
    *PALETTE.get_or_init(|| Palette::for_appearance(Appearance::detect(None)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn token(palette: Palette, language: &str, line: &str, text: &str) -> Token {
        let syn = palette.syntect();
        let mut hl = syn.highlight_lines_for_token(language).expect("syntax");
        let ranges = hl.highlight_line(line, &syn.syntax_set).expect("highlight");
        let (style, _) = ranges
            .into_iter()
            .find(|(_, piece)| *piece == text)
            .unwrap_or_else(|| panic!("no {text:?} token"));
        Token::from_syntect(style)
    }

    #[test]
    fn a_rust_keyword_is_grok_night_in_truecolor_and_quantized_below() {
        let night = std::str::from_utf8(theme::GROK_NIGHT_TMTHEME).expect("utf-8");
        let keyword = token(Palette::Night, "rust", "fn main() {}\n", "fn");
        let (r, g, b) = keyword.rgb;
        let hex = format!("#{r:02x}{g:02x}{b:02x}");
        assert!(
            night.to_ascii_lowercase().contains(&hex),
            "{hex} is not a Grok Night color"
        );
        assert_ne!(
            keyword.rgb,
            (0xb2, 0xb2, 0xb2),
            "keyword drew as plain text"
        );
        assert_eq!(
            keyword.style(ColorLevel::TrueColor).fg,
            Some(Color::Rgb(r, g, b))
        );
        assert_eq!(
            keyword.style(ColorLevel::Ansi256).fg,
            Some(Color::Indexed(color::nearest_indexed(r, g, b)))
        );
        assert_eq!(
            keyword.style(ColorLevel::Basic).fg,
            Some(color::polarity_safe_syntax_fg(r, g, b))
        );
        assert_eq!(keyword.style(ColorLevel::None).fg, Some(Color::Reset));
        // Grok Day paints the same keyword its own, deeper color.
        assert_ne!(
            token(Palette::Day, "rust", "fn main() {}\n", "fn").rgb,
            keyword.rgb
        );
    }
}
