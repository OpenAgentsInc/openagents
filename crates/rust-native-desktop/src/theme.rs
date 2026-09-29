//! The adapter's defaults: colors, type sizes, and spacing.
//!
//! Rust Native's core has no palette; the application supplies one. A
//! node's own `style.foreground` and `style.background` override these.

use rust_native::style::{Color, Space};

/// The colors and sizes a view is painted with, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The window's field.
    pub background: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text: a `status` text role, a disabled control.
    pub muted: Color,
    /// Rules between list rows and the outline of an empty checkbox.
    pub rule: Color,
    /// A button's fill, and a checked checkbox's.
    pub button: Color,
    /// A button's label, and a checked checkbox's mark.
    pub button_text: Color,
    /// A button drawn as a link (a transparent `style.background`).
    pub link: Color,
    /// The keyboard focus ring.
    pub focus: Color,
    /// A card: a stack with a `style.background`.
    pub card_radius: f32,
    /// A button's corner radius.
    pub button_radius: f32,
    /// Body text.
    pub body: f32,
    /// A heading.
    pub heading: f32,
    /// A status line.
    pub status: f32,
    /// Code and terminal text.
    pub code: f32,
    /// The widest the content column grows.
    pub column: f32,
    /// The space around the column.
    pub margin: f32,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            background: Color::rgb(0, 0, 0),
            text: Color::rgb(245, 245, 245),
            muted: Color::rgb(150, 150, 150),
            rule: Color::rgb(58, 58, 58),
            button: Color::rgb(245, 245, 245),
            button_text: Color::rgb(0, 0, 0),
            link: Color::rgb(170, 200, 255),
            focus: Color::rgb(120, 170, 255),
            card_radius: 12.0,
            button_radius: 8.0,
            body: 15.0,
            heading: 22.0,
            status: 13.0,
            code: 13.0,
            column: 520.0,
            margin: 28.0,
        }
    }
}

/// The points a semantic space stands for.
pub fn space(space: Option<Space>) -> f32 {
    match space {
        None | Some(Space::None) => 0.0,
        Some(Space::Xs) => 4.0,
        Some(Space::Sm) => 8.0,
        Some(Space::Md) => 16.0,
        Some(Space::Lg) => 28.0,
    }
}
