//! Theme selection for native GUI surfaces (desktop and mobile): Coder Light
//! or Coder Noir from the same token values, following the system's
//! appearance unless the person overrides it.
//!
//! An app resolves the scheme once, at its theme seam
//! ([`ThemeChoice::resolve`] with the system's appearance), and paints from
//! [`palette`]; views read roles from the palette instead of naming Noir
//! constants. Terminal UIs never come here: they paint with
//! [`crate::coder_noir`] only.

use rust_native::style::Color;

pub use oa_tokens::{Palette, Rgba8, Scheme, ThemeChoice};

/// The role palette for `scheme`: [`crate::coder_light::PALETTE`] or
/// [`crate::coder_noir::PALETTE`].
#[must_use]
pub const fn palette(scheme: Scheme) -> &'static Palette {
    Palette::of(scheme)
}

/// A token color as a Rust Native color.
#[must_use]
pub const fn color(value: Rgba8) -> Color {
    Color {
        red: value.r,
        green: value.g,
        blue: value.b,
        alpha: value.a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coder_light, coder_noir};

    #[test]
    fn each_scheme_has_its_palette() {
        assert_eq!(*palette(Scheme::Light), coder_light::PALETTE);
        assert_eq!(*palette(Scheme::Dark), coder_noir::PALETTE);
        assert_eq!(
            color(coder_noir::PALETTE.canvas),
            coder_noir::rgb(coder_noir::CANVAS)
        );
    }

    #[test]
    fn the_person_overrides_the_system() {
        assert_eq!(
            palette(ThemeChoice::System.resolve(Some(Scheme::Light))).canvas,
            coder_light::PALETTE.canvas
        );
        assert_eq!(
            palette(ThemeChoice::Dark.resolve(Some(Scheme::Light))).canvas,
            coder_noir::PALETTE.canvas
        );
    }
}
