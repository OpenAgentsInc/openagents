//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! the per-theme syntect instances of `xai-grok-pager-render/src/syntax.rs`
//! and the diff colors of `src/theme/groknight.rs` / `src/theme/grokday.rs`.
//!
//! Grok Night (`grok-night.tmTheme`) is the default; Grok Day
//! (`grok-day.tmTheme`, deepened colors for light backgrounds) is for a light
//! background. Both `.tmTheme` files are grok-build's, byte for byte.

use std::sync::OnceLock;

use ratatui::style::Color;

use super::appearance::Appearance;
use super::color::{ColorLevel, quantize_color};
use super::syntax::Syntect;

static SYNTECT_GROKNIGHT: OnceLock<Syntect> = OnceLock::new();
static SYNTECT_GROKDAY: OnceLock<Syntect> = OnceLock::new();

/// grok-build's Grok Night theme file, verbatim.
pub const GROK_NIGHT_TMTHEME: &[u8] = include_bytes!("../../assets/grok-build/grok-night.tmTheme");
/// grok-build's Grok Day theme file, verbatim.
pub const GROK_DAY_TMTHEME: &[u8] = include_bytes!("../../assets/grok-build/grok-day.tmTheme");

/// Which of grok-build's two code palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Palette {
    /// Grok Night: the default, and any dark background.
    #[default]
    Night,
    /// Grok Day: a light background.
    Day,
}

impl Palette {
    /// grok-build's `to_theme_kind`: light is Grok Day, dark is Grok Night,
    /// and no answer is Grok Night.
    pub fn for_appearance(appearance: Option<Appearance>) -> Self {
        match appearance {
            Some(Appearance::Light) => Palette::Day,
            Some(Appearance::Dark) | None => Palette::Night,
        }
    }

    /// The palette's lazily-initialized syntect instance.
    pub fn syntect(self) -> &'static Syntect {
        match self {
            Palette::Night => SYNTECT_GROKNIGHT.get_or_init(|| Syntect::new(GROK_NIGHT_TMTHEME)),
            Palette::Day => SYNTECT_GROKDAY.get_or_init(|| Syntect::new(GROK_DAY_TMTHEME)),
        }
    }

    /// The palette's diff colors, quantized for `level` as grok-build's
    /// `Theme::quantized` does at startup.
    pub fn diff(self, level: ColorLevel) -> DiffColors {
        let canonical = match self {
            Palette::Night => DiffColors {
                // RED_DARK #420e14, quantizes to 256-color red, not gray
                delete_bg: Color::Rgb(66, 14, 20),
                // RED #f7768e
                delete_fg: Color::Rgb(247, 118, 142),
                // GREEN_DARK #063806, quantizes to 256-color green, not gray
                insert_bg: Color::Rgb(6, 56, 6),
                // GREEN #9ece6a
                insert_fg: Color::Rgb(158, 206, 106),
                // COMMENT #6c6c6c
                equal_fg: Color::Rgb(108, 108, 108),
                gutter_fg: Color::Rgb(108, 108, 108),
                // FG #e1e1e1
                text_primary: Color::Rgb(225, 225, 225),
                // COMMENT, `theme.muted()`
                muted: Color::Rgb(108, 108, 108),
            },
            Palette::Day => DiffColors {
                // RED_LIGHT #F5DADE
                delete_bg: Color::Rgb(245, 218, 222),
                // RED #CD3048
                delete_fg: Color::Rgb(205, 48, 72),
                // GREEN_LIGHT #DAF2DC
                insert_bg: Color::Rgb(218, 242, 220),
                // GREEN #378E23
                insert_fg: Color::Rgb(55, 142, 35),
                // COMMENT #767676
                equal_fg: Color::Rgb(118, 118, 118),
                gutter_fg: Color::Rgb(118, 118, 118),
                // FG #262626
                text_primary: Color::Rgb(38, 38, 38),
                muted: Color::Rgb(118, 118, 118),
            },
        };
        canonical.quantized(level)
    }
}

/// The colors a diff draws with: grok-build's `diff_*` theme slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffColors {
    pub delete_bg: Color,
    pub delete_fg: Color,
    pub insert_bg: Color,
    pub insert_fg: Color,
    pub equal_fg: Color,
    pub gutter_fg: Color,
    pub text_primary: Color,
    pub muted: Color,
}

impl DiffColors {
    fn quantized(self, level: ColorLevel) -> Self {
        let q = |c| quantize_color(c, level);
        Self {
            delete_bg: q(self.delete_bg),
            delete_fg: q(self.delete_fg),
            insert_bg: q(self.insert_bg),
            insert_fg: q(self.insert_fg),
            equal_fg: q(self.equal_fg),
            gutter_fg: q(self.gutter_fg),
            text_primary: q(self.text_primary),
            muted: q(self.muted),
        }
    }

    /// Whether this theme paints no diff row bands (`diff_*_bg` is `Reset`).
    /// In that case changed diff lines carry a whole-line red/green *foreground* instead of syntax highlighting on a colored band.
    #[must_use]
    pub fn uses_line_fg(&self) -> bool {
        self.delete_bg == Color::Reset && self.insert_bg == Color::Reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_h2_takes_each_theme_color() {
        // grok-build's own check that the theme files are theirs: an ATX h2
        // paints #61bdf2 in Grok Night and #2f64d2 in Grok Day.
        for (palette, want) in [
            (Palette::Night, (0x61, 0xbd, 0xf2)),
            (Palette::Day, (0x2f, 0x64, 0xd2)),
        ] {
            let syn = palette.syntect();
            let mut hl = syn.highlight_lines_for_token("md").expect("markdown");
            let ranges = hl
                .highlight_line("## Heading\n", &syn.syntax_set)
                .expect("highlight");
            let (style, _) = ranges
                .iter()
                .find(|(_, text)| *text == "Heading")
                .expect("heading token");
            assert_eq!(
                (style.foreground.r, style.foreground.g, style.foreground.b),
                want
            );
        }
    }

    #[test]
    fn light_is_day_and_the_rest_is_night() {
        assert_eq!(
            Palette::for_appearance(Some(Appearance::Light)),
            Palette::Day
        );
        assert_eq!(
            Palette::for_appearance(Some(Appearance::Dark)),
            Palette::Night
        );
        assert_eq!(Palette::for_appearance(None), Palette::Night);
    }

    #[test]
    fn no_color_drops_the_bands_for_line_colors() {
        assert!(!Palette::Night.diff(ColorLevel::TrueColor).uses_line_fg());
        assert!(Palette::Night.diff(ColorLevel::None).uses_line_fg());
    }
}
