//! The Verse palette: the Coder terminal's amber ladder, in linear light.
//!
//! The world speaks in the terminal's one hue. Every line is one of the
//! four [`Intensity`] steps, every solid face is the terminal's near-black
//! field, and fog fades toward that same field. No second color exists, so
//! the palette is borrowed from `coder_terminal` rather than restated.

use coder_terminal::{Intensity, NEAR_BLACK};

/// A linear-light RGB color the renderer writes to an sRGB surface.
pub type Linear = [f32; 3];

/// The linear color of one ladder step.
#[must_use]
pub fn amber(step: Intensity) -> Linear {
    linear(step.color())
}

/// The near-black field: the clear color, the fog, and every solid face.
#[must_use]
pub fn field() -> Linear {
    linear(NEAR_BLACK)
}

/// Converts a packed `0xRRGGBB` sRGB value into linear light.
#[must_use]
pub fn linear(rgb: u32) -> Linear {
    let channel = |shift: u32| srgb_to_linear(((rgb >> shift) & 0xff) as f32 / 255.0);
    [channel(16), channel(8), channel(0)]
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_amber_is_the_terminal_amber() {
        let [r, g, b] = amber(Intensity::Full);
        assert!((r - 1.0).abs() < 1e-6);
        assert!(g > 0.4 && g < 0.45, "0xb0 in linear light, got {g}");
        assert_eq!(b, 0.0);
    }

    #[test]
    fn the_ladder_stays_ordered_in_linear_light() {
        let lum = |c: Linear| c[0] + c[1] + c[2];
        let steps = Intensity::ALL.map(|s| lum(amber(s)));
        assert!(steps.windows(2).all(|w| w[0] < w[1]));
        assert!(lum(field()) < steps[0]);
    }
}
