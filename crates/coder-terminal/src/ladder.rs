//! The color ladder: how an [`Intensity`] reaches the terminal at hand.
//!
//! A terminal either knows truecolor, knows a 256-color palette, or honors
//! `NO_COLOR` and knows none. The [`Ladder`] detects which and translates the
//! four amber tones into concrete colors — exact RGB, the nearest cube
//! entries, or plain dim text.
//!
//! One ladder serves every terminal in this repository. Where a surface
//! needs a step to behave differently, it says so in a named option rather
//! than in a second copy of the ladder: [`Colorless`] is the only such
//! option so far.

use crate::intensity::{Intensity, NEAR_BLACK, NEAR_BLACK_TINT};
use ratatui::style::{Color, Modifier, Style};

/// The color depth the terminal supports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Colors {
    /// 24-bit RGB — the ambers render exactly.
    #[default]
    True,
    /// The 256-color palette — ambers render as their nearest cube entries.
    Indexed,
    /// Color off (`NO_COLOR`) — tone falls back to modifiers.
    None,
}

/// What tone becomes when the terminal has no color to say it with.
///
/// `NO_COLOR` takes the hue away and leaves the four steps to land on
/// whatever attributes remain. Which attributes are enough depends on what
/// the surface draws, so the caller chooses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Colorless {
    /// The faint half dims and the rest stays plain.
    ///
    /// The composer's choice: prose is most of what it draws, and bold
    /// prose behind a hairline frame reads as shouting.
    #[default]
    Dim,
    /// The faint half dims, and `Full` goes bold.
    ///
    /// A table's choice. Where the loudest step carries a verdict — a
    /// failed gate against an unverifiable one — leaving it plain draws the
    /// two alike on a colorless terminal, and telling them apart is what
    /// the column is for.
    DimAndBold,
}

/// How [`Intensity`] values become terminal colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ladder {
    colors: Colors,
    colorless: Colorless,
}

impl Ladder {
    /// A ladder for the given color depth, dimming the faint half when
    /// there is no color.
    pub const fn new(colors: Colors) -> Self {
        Self {
            colors,
            colorless: Colorless::Dim,
        }
    }

    /// The same ladder, with a different answer to `NO_COLOR`.
    pub const fn when_colorless(self, colorless: Colorless) -> Self {
        Self { colorless, ..self }
    }

    /// The color depth this ladder serves.
    pub const fn colors(self) -> Colors {
        self.colors
    }

    /// What this ladder does when the terminal has no color.
    pub const fn colorless(self) -> Colorless {
        self.colorless
    }

    /// Detects the terminal's color depth from the environment.
    ///
    /// `NO_COLOR` wins first — its spec asks for no color at all. Otherwise
    /// `COLORTERM` decides: `truecolor` or `24bit` means the terminal knows
    /// RGB, anything else means the 256-color palette.
    pub fn from_environment() -> Self {
        Self::detected(|name| std::env::var(name).ok())
    }

    /// Detects the color depth from a supplied environment lookup, so tests
    /// need not touch the process environment.
    pub fn detected(lookup: impl Fn(&str) -> Option<String>) -> Self {
        if lookup("NO_COLOR").is_some() {
            return Self::new(Colors::None);
        }
        match lookup("COLORTERM").as_deref() {
            Some("truecolor" | "24bit") => Self::new(Colors::True),
            _ => Self::new(Colors::Indexed),
        }
    }

    /// The style an [`Intensity`] produces at this terminal.
    ///
    /// Under `Colors::None` the fainter half of the ladder dims rather than
    /// colors: `Quarter` and `Half` carry `Modifier::DIM`. What `Full` does
    /// there is [`Colorless`]'s to say.
    pub fn style(self, intensity: Intensity) -> Style {
        match self.colors {
            Colors::True => Style::new().fg(rgb(intensity.color())),
            Colors::Indexed => Style::new().fg(indexed(intensity)),
            Colors::None => match (intensity, self.colorless) {
                (Intensity::Quarter | Intensity::Half, _) => {
                    Style::new().add_modifier(Modifier::DIM)
                }
                (Intensity::Full, Colorless::DimAndBold) => {
                    Style::new().add_modifier(Modifier::BOLD)
                }
                _ => Style::new(),
            },
        }
    }

    /// The near-black field the ambers sit on, at this terminal.
    pub fn background(self) -> Color {
        match self.colors {
            Colors::True => rgb(NEAR_BLACK),
            Colors::Indexed => Color::Indexed(INDEXED_BACKGROUND),
            Colors::None => Color::Reset,
        }
    }

    /// The near-black tint a selected cell brightens to, at this terminal.
    ///
    /// A colorless terminal has no tint to give, so the field stays as it
    /// is and the selection has to show some other way.
    pub fn selection(self) -> Color {
        match self.colors {
            Colors::True => rgb(NEAR_BLACK_TINT),
            Colors::Indexed => Color::Indexed(INDEXED_SELECTION),
            Colors::None => Color::Reset,
        }
    }
}

/// A packed RGB value as a terminal color.
pub const fn rgb(color: u32) -> Color {
    Color::Rgb((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

/// The indexed entry nearest to each amber on the 256-color cube.
const fn indexed(intensity: Intensity) -> Color {
    match intensity {
        Intensity::Quarter => Color::Indexed(58),
        Intensity::Half => Color::Indexed(94),
        Intensity::ThreeQuarters => Color::Indexed(136),
        Intensity::Full => Color::Indexed(214),
    }
}

/// The indexed entry nearest to [`NEAR_BLACK`].
const INDEXED_BACKGROUND: u8 = 232;
/// The indexed entry nearest to [`NEAR_BLACK_TINT`].
const INDEXED_SELECTION: u8 = 235;

/// Strips foreground and background color from a style, keeping every other
/// attribute — glyphs, modifiers, and the like.
pub fn drain_color(style: Style) -> Style {
    Style {
        fg: None,
        bg: None,
        ..style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
            // The lookup returns None for absent names; that is what makes a
            // bare environment read as "no overrides".
        }
    }

    #[test]
    fn no_color_wins_over_colorterm() {
        let ladder = Ladder::detected(env(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")]));
        assert_eq!(ladder.colors(), Colors::None);
    }

    #[test]
    fn colorterm_truecolor_means_rgb() {
        for term in ["truecolor", "24bit"] {
            assert_eq!(
                Ladder::detected(env(&[("COLORTERM", term)])).colors(),
                Colors::True
            );
        }
    }

    #[test]
    fn anything_else_is_indexed() {
        assert_eq!(Ladder::detected(env(&[])).colors(), Colors::Indexed);
        assert_eq!(
            Ladder::detected(env(&[("COLORTERM", "yes")])).colors(),
            Colors::Indexed
        );
    }

    #[test]
    fn truecolor_renders_the_exact_amber() {
        let style = Ladder::new(Colors::True).style(Intensity::Full);
        assert_eq!(style.fg, Some(Color::Rgb(0xff, 0xb0, 0x00)));
    }

    #[test]
    fn indexed_renders_the_nearest_cube_entry() {
        let ladder = Ladder::new(Colors::Indexed);
        assert_eq!(ladder.style(Intensity::Full).fg, Some(Color::Indexed(214)));
        assert_eq!(
            ladder.style(Intensity::Quarter).fg,
            Some(Color::Indexed(58))
        );
        assert_eq!(ladder.background(), Color::Indexed(232));
    }

    #[test]
    fn no_color_dims_the_faint_half() {
        let ladder = Ladder::new(Colors::None);
        for faint in [Intensity::Quarter, Intensity::Half] {
            assert!(ladder.style(faint).add_modifier.contains(Modifier::DIM));
        }
        for loud in [Intensity::ThreeQuarters, Intensity::Full] {
            let style = ladder.style(loud);
            assert!(style.fg.is_none());
            assert!(!style.add_modifier.contains(Modifier::DIM));
            assert!(!style.add_modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn a_colorless_table_can_ask_for_a_bold_top_step() {
        let ladder = Ladder::new(Colors::None).when_colorless(Colorless::DimAndBold);
        assert!(
            ladder
                .style(Intensity::Full)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        // The rest of the ladder is untouched: the option moves one step.
        assert_eq!(
            ladder.style(Intensity::ThreeQuarters),
            Style::new(),
            "the third step stays plain"
        );
        for faint in [Intensity::Quarter, Intensity::Half] {
            assert!(ladder.style(faint).add_modifier.contains(Modifier::DIM));
        }
        // The option is about `NO_COLOR` alone; a terminal with color reads
        // the same either way.
        for colors in [Colors::True, Colors::Indexed] {
            assert_eq!(
                Ladder::new(colors)
                    .when_colorless(Colorless::DimAndBold)
                    .style(Intensity::Full),
                Ladder::new(colors).style(Intensity::Full)
            );
        }
    }

    #[test]
    fn the_selection_tint_sits_just_above_the_field() {
        assert_eq!(
            Ladder::new(Colors::True).selection(),
            rgb(crate::intensity::NEAR_BLACK_TINT)
        );
        assert_eq!(
            Ladder::new(Colors::Indexed).selection(),
            Color::Indexed(235)
        );
        assert_eq!(Ladder::new(Colors::None).selection(), Color::Reset);
    }

    #[test]
    fn drain_color_keeps_modifiers() {
        let style = Style::new()
            .fg(Color::Indexed(214))
            .bg(Color::Indexed(232))
            .add_modifier(Modifier::BOLD);
        let drained = drain_color(style);
        assert_eq!(drained.fg, None);
        assert_eq!(drained.bg, None);
        assert!(drained.add_modifier.contains(Modifier::BOLD));
    }
}
