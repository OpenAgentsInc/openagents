//! HSLA color type matching GPUI/theme_oa format

use bytemuck::{Pod, Zeroable};

/// HSLA color represented as [hue, saturation, lightness, alpha]
/// - hue: 0.0-1.0 (maps to 0-360 degrees)
/// - saturation: 0.0-1.0
/// - lightness: 0.0-1.0
/// - alpha: 0.0-1.0
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Hsla {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl Hsla {
    /// Create a new HSLA color
    pub const fn new(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self { h, s, l, a }
    }

    /// Create an opaque HSLA color
    pub const fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l, a: 1.0 }
    }

    /// Create a grayscale color (hue=0, saturation=0)
    pub const fn gray(lightness: f32, alpha: f32) -> Self {
        Self {
            h: 0.0,
            s: 0.0,
            l: lightness,
            a: alpha,
        }
    }

    /// Transparent color
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// Black
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    /// White
    pub const WHITE: Self = Self::new(0.0, 0.0, 1.0, 1.0);

    /// Convert to array for GPU
    pub fn to_array(self) -> [f32; 4] {
        [self.h, self.s, self.l, self.a]
    }

    /// Create from array
    pub fn from_array(arr: [f32; 4]) -> Self {
        Self {
            h: arr[0],
            s: arr[1],
            l: arr[2],
            a: arr[3],
        }
    }

    /// Convert to RGBA for display
    pub fn to_rgba(self) -> [f32; 4] {
        let h = self.h * 6.0;
        let s = self.s;
        let l = self.l;
        let a = self.a;

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let m = l - c / 2.0;

        let (r, g, b) = if h < 1.0 {
            (c, x, 0.0)
        } else if h < 2.0 {
            (x, c, 0.0)
        } else if h < 3.0 {
            (0.0, c, x)
        } else if h < 4.0 {
            (0.0, x, c)
        } else if h < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        [r + m, g + m, b + m, a]
    }
}

impl From<[f32; 4]> for Hsla {
    fn from(arr: [f32; 4]) -> Self {
        Self::from_array(arr)
    }
}

impl From<Hsla> for [f32; 4] {
    fn from(color: Hsla) -> Self {
        color.to_array()
    }
}

/// Helper function to create HSLA colors (matches theme_oa pattern)
pub const fn hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla::new(h, s, l, a)
}

/// Helper function to create opaque HSL colors
pub const fn hsl(h: f32, s: f32, l: f32) -> Hsla {
    Hsla::hsl(h, s, l)
}
