//! HSLA color type for GPU-friendly color representation.

use bytemuck::{Pod, Zeroable};

/// HSLA color with components in 0.0-1.0 range.
/// Hue is normalized (0.0-1.0 maps to 0-360 degrees).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Hsla {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl Hsla {
    pub const fn new(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self { h, s, l, a }
    }

    pub const fn transparent() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    pub const fn white() -> Self {
        Self::new(0.0, 0.0, 1.0, 1.0)
    }

    pub const fn black() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    /// Create from RGB hex value (e.g., 0xFFB400)
    pub fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let b = (hex & 0xFF) as f32 / 255.0;
        Self::from_rgb(r, g, b)
    }

    /// Create from RGB components (0.0-1.0)
    pub fn from_rgb(r: f32, g: f32, b: f32) -> Self {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        if max == min {
            return Self::new(0.0, 0.0, l, 1.0);
        }

        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        let h = if max == r {
            ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
        } else if max == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };

        Self::new(h, s, l, 1.0)
    }

    /// Convert to RGBA (0.0-1.0)
    pub fn to_rgba(&self) -> [f32; 4] {
        if self.s == 0.0 {
            return [self.l, self.l, self.l, self.a];
        }

        let q = if self.l < 0.5 {
            self.l * (1.0 + self.s)
        } else {
            self.l + self.s - self.l * self.s
        };
        let p = 2.0 * self.l - q;

        let hue_to_rgb = |t: f32| {
            let t = if t < 0.0 {
                t + 1.0
            } else if t > 1.0 {
                t - 1.0
            } else {
                t
            };

            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };

        [
            hue_to_rgb(self.h + 1.0 / 3.0),
            hue_to_rgb(self.h),
            hue_to_rgb(self.h - 1.0 / 3.0),
            self.a,
        ]
    }

    /// Create with modified alpha
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Lighten the color by a factor (0.0-1.0)
    pub fn lighten(self, factor: f32) -> Self {
        Self {
            l: (self.l + (1.0 - self.l) * factor).min(1.0),
            ..self
        }
    }

    /// Darken the color by a factor (0.0-1.0)
    pub fn darken(self, factor: f32) -> Self {
        Self {
            l: (self.l * (1.0 - factor)).max(0.0),
            ..self
        }
    }
}

impl From<u32> for Hsla {
    fn from(hex: u32) -> Self {
        Self::from_hex(hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_hsla() {
        let yellow = Hsla::from_hex(0xFFB400);
        assert!(yellow.h > 0.1 && yellow.h < 0.15); // ~42 degrees / 360
        assert!(yellow.s > 0.9); // High saturation
    }

    #[test]
    fn test_roundtrip() {
        let original = Hsla::new(0.5, 0.5, 0.5, 1.0);
        let rgba = original.to_rgba();
        let back = Hsla::from_rgb(rgba[0], rgba[1], rgba[2]);
        assert!((original.h - back.h).abs() < 0.01);
        assert!((original.s - back.s).abs() < 0.01);
        assert!((original.l - back.l).abs() < 0.01);
    }
}
