//! The app's theme (#11028): Coder Light or the phones' dark look, chosen in
//! Account > Appearance as System, Light, or Dark. System follows the
//! phone's appearance, which the host reports at launch and whenever it
//! changes (`system_appearance`).
//!
//! Rust resolves the scheme here, the one theme seam: it sets the shared
//! views' scheme (`openagents_chat_app::visual::set_scheme`), so every Rust
//! Native tree built afterwards paints in it, and hands the hosts the
//! resolved [`HostPalette`] for the chrome they draw themselves (tab bar,
//! navigation, lists, status bar). The hosts keep no colors of their own
//! beyond a dark fallback for the first frame.
//!
//! Terminal panes never follow the theme: they stay Coder Noir in both
//! looks, like every terminal UI.
//!
//! The choice is a plain file under the app's state directory, like the
//! amount format; it is not a secret.

use openagents_chat_app::visual::{self, Scheme, ThemeChoice};
use rust_native::style::Color;
use serde::Serialize;
use std::path::{Path, PathBuf};

const THEME_FILE: &str = "theme";

pub struct Appearance {
    dir: PathBuf,
    choice: ThemeChoice,
    /// The phone's appearance, once the host reported it.
    system: Option<Scheme>,
}

/// The colors a host paints its own chrome with, by role. The names are
/// the hosts' (`Palette` on Android).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HostPalette {
    /// The screen's field.
    pub background: Color,
    pub primary: Color,
    pub secondary: Color,
    pub tertiary: Color,
    /// A message bubble or a filled control.
    pub bubble: Color,
    /// A quiet panel.
    pub surface: Color,
    /// A card or button above the field.
    pub raised: Color,
    pub border: Color,
    pub inline_code: Color,
    pub link: Color,
    pub success: Color,
    pub failure: Color,
    /// Selected text's highlight, and its handles.
    pub selection: Color,
    pub selection_handle: Color,
}

const fn argb(value: u32) -> Color {
    Color {
        red: (value >> 16) as u8,
        green: (value >> 8) as u8,
        blue: value as u8,
        alpha: (value >> 24) as u8,
    }
}

const fn token(value: oa_tokens::Rgba8) -> Color {
    Color {
        red: value.r,
        green: value.g,
        blue: value.b,
        alpha: value.a,
    }
}

const LIGHT_TOKENS: oa_tokens::Palette = oa_tokens::Palette::LIGHT;

impl HostPalette {
    /// The phones' established dark look: white text on black, gray
    /// surfaces (the Android host's former `Palette` constants).
    pub const DARK: HostPalette = HostPalette {
        background: argb(0xFF00_0000),
        primary: argb(0xFFFF_FFFF),
        secondary: argb(0xFF9A_9AA0),
        tertiary: argb(0xFF5C_5C62),
        bubble: argb(0xFF29_2929),
        surface: argb(0xFF14_1414),
        raised: argb(0xFF21_2121),
        border: argb(0xFF38_3838),
        inline_code: argb(0x1AFF_FFFF),
        link: argb(0xFF0A_84FF),
        success: argb(0xFF30_D158),
        failure: argb(0xFFFF_453A),
        selection: argb(0x4D0A_84FF),
        selection_handle: argb(0xFF0A_84FF),
    };

    /// Coder Light, from the shared token table.
    pub const LIGHT: HostPalette = HostPalette {
        background: token(LIGHT_TOKENS.canvas),
        primary: token(LIGHT_TOKENS.content),
        secondary: token(LIGHT_TOKENS.content_secondary),
        tertiary: token(LIGHT_TOKENS.content_tertiary),
        bubble: token(LIGHT_TOKENS.surface),
        surface: token(LIGHT_TOKENS.surface_subtle),
        raised: token(LIGHT_TOKENS.surface),
        border: token(LIGHT_TOKENS.stroke_subtle),
        inline_code: token(LIGHT_TOKENS.content.with_alpha(0x1A)),
        link: token(LIGHT_TOKENS.accent),
        success: token(LIGHT_TOKENS.success),
        failure: token(LIGHT_TOKENS.danger),
        selection: token(LIGHT_TOKENS.accent.with_alpha(0x4D)),
        selection_handle: token(LIGHT_TOKENS.accent),
    };

    #[must_use]
    pub const fn of(scheme: Scheme) -> &'static HostPalette {
        match scheme {
            Scheme::Light => &HostPalette::LIGHT,
            Scheme::Dark => &HostPalette::DARK,
        }
    }
}

/// What the host needs to paint and to offer the choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    /// `system`, `light`, or `dark`; send one back with `theme`.
    pub choice: &'static str,
    /// The resolved scheme, `light` or `dark`: the host's color scheme,
    /// and so its status bar and system chrome.
    pub scheme: &'static str,
    pub choices: Vec<Choice>,
    pub palette: HostPalette,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    pub selected: bool,
}

/// The default before the person chooses: the shared preference default,
/// dark until every phone surface paints from the seam.
fn default_choice() -> ThemeChoice {
    openagents_chat_app::preferences::Preferences::default().theme
}

impl Appearance {
    /// Read the saved choice (anything unreadable is the default) and
    /// apply it.
    pub fn open(dir: &Path) -> Self {
        let choice = std::fs::read_to_string(dir.join(THEME_FILE))
            .ok()
            .and_then(|text| ThemeChoice::parse(text.trim()))
            .unwrap_or_else(default_choice);
        let appearance = Self {
            dir: dir.to_owned(),
            choice,
            system: None,
        };
        appearance.apply();
        appearance
    }

    pub fn choice(&self) -> ThemeChoice {
        self.choice
    }

    /// The scheme the app paints with.
    pub fn scheme(&self) -> Scheme {
        self.choice.resolve(self.system)
    }

    /// Choose by id (`system`, `light`, `dark`) and save it. An unknown id
    /// changes nothing.
    pub fn choose(&mut self, id: &str) -> Option<ThemeChoice> {
        let choice = ThemeChoice::parse(id)?;
        let _ = std::fs::create_dir_all(&self.dir);
        // A failed write only means the next launch shows the default.
        let _ = std::fs::write(self.dir.join(THEME_FILE), format!("{}\n", choice.as_str()));
        self.choice = choice;
        self.apply();
        Some(choice)
    }

    /// The phone's appearance, from the host. Answers whether the scheme
    /// the app paints with changed.
    pub fn set_system(&mut self, dark: bool) -> bool {
        let before = self.scheme();
        self.system = Some(if dark { Scheme::Dark } else { Scheme::Light });
        self.apply();
        self.scheme() != before
    }

    fn apply(&self) {
        visual::set_scheme(self.scheme());
    }

    pub fn view(&self) -> View {
        let scheme = self.scheme();
        View {
            choice: self.choice.as_str(),
            scheme: match scheme {
                Scheme::Light => "light",
                Scheme::Dark => "dark",
            },
            choices: ThemeChoice::ALL
                .iter()
                .map(|choice| Choice {
                    id: choice.as_str(),
                    label: choice.label(),
                    selected: *choice == self.choice,
                })
                .collect(),
            palette: *HostPalette::of(scheme),
        }
    }
}

/// Whether the shared views paint in the light look.
pub fn light() -> bool {
    visual::scheme() == Scheme::Light
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The choice persists, System follows the phone, and the person's
    /// choice overrides it. The scheme is process-wide, so the test leaves
    /// it dark for the other tests.
    #[test]
    fn the_choice_persists_and_system_follows_the_phone() {
        let dir = tempfile::tempdir().unwrap();
        let mut appearance = Appearance::open(dir.path());
        assert_eq!(appearance.choice(), default_choice());
        assert_eq!(appearance.choose("sepia"), None);
        assert_eq!(appearance.choose("system"), Some(ThemeChoice::System));
        // Before the phone says, System reads as dark.
        assert_eq!(appearance.scheme(), Scheme::Dark);
        assert!(appearance.set_system(false));
        assert_eq!(appearance.scheme(), Scheme::Light);
        let view = appearance.view();
        assert_eq!((view.choice, view.scheme), ("system", "light"));
        assert_eq!(view.palette, HostPalette::LIGHT);
        assert!(view.choices.iter().any(|c| c.id == "system" && c.selected));
        assert!(!appearance.set_system(false));

        assert_eq!(appearance.choose("dark"), Some(ThemeChoice::Dark));
        assert_eq!(appearance.scheme(), Scheme::Dark);
        assert_eq!(appearance.view().palette, HostPalette::DARK);
        let reopened = Appearance::open(dir.path());
        assert_eq!(reopened.choice(), ThemeChoice::Dark);
        assert_eq!(visual::scheme(), Scheme::Dark);
    }

    #[test]
    fn the_light_palette_is_coder_light() {
        let tokens = oa_tokens::Palette::LIGHT;
        assert_eq!(HostPalette::LIGHT.background, token(tokens.canvas));
        assert_eq!(HostPalette::LIGHT.primary, token(tokens.content));
        assert_eq!(HostPalette::LIGHT.secondary, token(tokens.content_secondary));
        assert_eq!(HostPalette::LIGHT.link, token(tokens.accent));
        assert_eq!(HostPalette::DARK.background, Color::rgb(0, 0, 0));
        assert_eq!(HostPalette::DARK.inline_code.alpha, 0x1A);
    }
}
