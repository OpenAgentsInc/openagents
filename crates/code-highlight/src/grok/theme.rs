//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! the per-theme syntect instances of `xai-grok-pager-render/src/syntax.rs`,
//! and the diff and transcript colors of `src/theme/groknight.rs` /
//! `src/theme/grokday.rs` (the transcript's as `src/theme/md_style.rs` and
//! `xai-grok-pager/src/scrollback/blocks/user.rs` use them).
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

impl Palette {
    /// The palette's transcript colors, quantized for `level` as
    /// grok-build's `Theme::quantized` does at startup.
    pub fn transcript(self, level: ColorLevel) -> TranscriptColors {
        const fn rgb(r: u8, g: u8, b: u8) -> Color {
            Color::Rgb(r, g, b)
        }
        let canonical = match self {
            Palette::Night => TranscriptColors {
                // BG_HIGHLIGHT #242424: the band behind a prompt
                bg_light: rgb(36, 36, 36),
                // FG_DARK #c8c8c8: the prompt's ❯
                accent_user: rgb(200, 200, 200),
                // FG #e1e1e1: the prompt's text
                text_primary: rgb(225, 225, 225),
                // FG_DARK: a reply's body text
                md_text: rgb(200, 200, 200),
                // BLUE1 #3A95AB: inline code
                md_code: rgb(58, 149, 171),
                // #1c1c1c: a code block's band
                md_code_bg: rgb(28, 28, 28),
                // COMMENT #6c6c6c: list markers, rules, quote bars, URLs
                md_muted: rgb(108, 108, 108),
                // #7aa6da: link text
                link_fg: rgb(122, 166, 218),
                // TEAL, BLUE, PURPLE, DARK5, COMMENT, DARK3
                headings: [
                    rgb(26, 188, 156),
                    rgb(122, 162, 247),
                    rgb(157, 124, 216),
                    rgb(120, 120, 120),
                    rgb(108, 108, 108),
                    rgb(90, 90, 90),
                ],
                // GREEN #9ece6a / FG_DARK
                task_checked: rgb(158, 206, 106),
                task_unchecked: rgb(200, 200, 200),
                // #585858 / COMMENT / DARK5
                gray_dim: rgb(88, 88, 88),
                gray: rgb(108, 108, 108),
                gray_bright: rgb(120, 120, 120),
            },
            Palette::Day => TranscriptColors {
                bg_light: rgb(222, 222, 222),
                accent_user: rgb(68, 68, 68),
                text_primary: rgb(38, 38, 38),
                md_text: rgb(68, 68, 68),
                md_code: rgb(15, 135, 162),
                md_code_bg: rgb(228, 228, 228),
                md_muted: rgb(118, 118, 118),
                // BLUE #2F64D2: deep blue for a light field
                link_fg: rgb(47, 100, 210),
                headings: [
                    rgb(10, 142, 112),
                    rgb(47, 100, 210),
                    rgb(108, 62, 178),
                    rgb(98, 98, 98),
                    rgb(118, 118, 118),
                    rgb(142, 142, 142),
                ],
                task_checked: rgb(55, 142, 35),
                task_unchecked: rgb(68, 68, 68),
                gray_dim: rgb(165, 165, 165),
                gray: rgb(118, 118, 118),
                gray_bright: rgb(98, 98, 98),
            },
        };
        canonical.quantized(level)
    }
}

/// The colors a transcript draws with: grok-build's prompt band and
/// `md_*` theme slots. Headings 1 to 4 and 5 are bold; heading 6 is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptColors {
    pub bg_light: Color,
    pub accent_user: Color,
    pub text_primary: Color,
    pub md_text: Color,
    pub md_code: Color,
    pub md_code_bg: Color,
    pub md_muted: Color,
    pub link_fg: Color,
    pub headings: [Color; 6],
    pub task_checked: Color,
    pub task_unchecked: Color,
    pub gray_dim: Color,
    pub gray: Color,
    pub gray_bright: Color,
}

impl TranscriptColors {
    fn quantized(self, level: ColorLevel) -> Self {
        let q = |c| quantize_color(c, level);
        Self {
            bg_light: q(self.bg_light),
            accent_user: q(self.accent_user),
            text_primary: q(self.text_primary),
            md_text: q(self.md_text),
            md_code: q(self.md_code),
            md_code_bg: q(self.md_code_bg),
            md_muted: q(self.md_muted),
            link_fg: q(self.link_fg),
            headings: self.headings.map(q),
            task_checked: q(self.task_checked),
            task_unchecked: q(self.task_unchecked),
            gray_dim: q(self.gray_dim),
            gray: q(self.gray),
            gray_bright: q(self.gray_bright),
        }
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
