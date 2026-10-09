//! The light/dark theme toggle (adoption plan decision 6).
//!
//! The page starts on the system setting (no `data-theme`), and one click
//! switches to the opposite of what is showing. The theme script (UI-00)
//! finds the button by [`THEME_TOGGLE_ATTR`], sets `data-theme` on `<html>`,
//! and stores the choice in the [`THEME_COOKIE`] cookie so the server can
//! paint the next page in the same theme through [`super::Document::theme`].

use maud::{Markup, Render, html};

use super::glyph;

/// The data attribute the theme script binds to. Its value is empty.
pub const THEME_TOGGLE_ATTR: &str = "data-oa-theme-toggle";

/// The first-party cookie holding the explicit choice: `light` or `dark`.
/// No cookie means "follow the system setting".
pub const THEME_COOKIE: &str = "oa_theme";

/// An explicit theme choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Coder Light.
    Light,
    /// Coder Noir.
    Dark,
}

impl Theme {
    /// The `data-theme` and cookie value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Reads a cookie value. Anything other than `light` or `dark` means no
    /// explicit choice.
    #[must_use]
    pub fn from_cookie(value: &str) -> Option<Self> {
        match value.trim() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

/// The theme toggle button.
///
/// Without JavaScript the button does nothing by default; give it
/// [`ThemeToggle::fallback_action`] to wrap it in a `POST` form the server
/// answers by setting the cookie and redirecting back. The theme script
/// prevents that submit and switches in place.
#[derive(Clone, Debug)]
pub struct ThemeToggle {
    label: String,
    fallback_action: Option<String>,
    return_to: Option<String>,
    light_icon: Option<Markup>,
    dark_icon: Option<Markup>,
}

impl Default for ThemeToggle {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeToggle {
    /// A toggle labelled "Toggle light and dark theme".
    #[must_use]
    pub fn new() -> Self {
        Self {
            label: "Toggle light and dark theme".to_owned(),
            fallback_action: None,
            return_to: None,
            light_icon: None,
            dark_icon: None,
        }
    }

    /// The accessible name.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// A no-JavaScript fallback: the button submits `POST action` with
    /// `theme=toggle` and, if set, `return_to`.
    #[must_use]
    pub fn fallback_action(mut self, action: impl Into<String>) -> Self {
        self.fallback_action = Some(action.into());
        self
    }

    /// The path the fallback form returns to.
    #[must_use]
    pub fn return_to(mut self, path: impl Into<String>) -> Self {
        self.return_to = Some(path.into());
        self
    }

    /// The icon shown while the light theme is showing (it offers dark).
    #[must_use]
    pub fn light_icon(mut self, icon: impl Render) -> Self {
        self.light_icon = Some(icon.render());
        self
    }

    /// The icon shown while the dark theme is showing (it offers light).
    #[must_use]
    pub fn dark_icon(mut self, icon: impl Render) -> Self {
        self.dark_icon = Some(icon.render());
        self
    }

    fn button(&self, submit: bool) -> Markup {
        let moon = self.light_icon.clone().unwrap_or_else(glyph::moon);
        let sun = self.dark_icon.clone().unwrap_or_else(glyph::sun);
        html! {
            button type=(if submit { "submit" } else { "button" })
                class="oa-theme-toggle" data-oa-theme-toggle=""
                name=[submit.then_some("theme")] value=[submit.then_some("toggle")]
                aria-label=(self.label) title=(self.label) {
                span class="oa-theme-toggle-icon" data-shows="light" { (moon) }
                span class="oa-theme-toggle-icon" data-shows="dark" { (sun) }
            }
        }
    }
}

impl Render for ThemeToggle {
    fn render(&self) -> Markup {
        match &self.fallback_action {
            None => self.button(false),
            Some(action) => html! {
                form class="oa-theme-toggle-form" method="post" action=(action) {
                    @if let Some(path) = &self.return_to {
                        input type="hidden" name="return_to" value=(path);
                    }
                    (self.button(true))
                }
            },
        }
    }
}
