use std::sync::Arc;

use crate::color::Hsla;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeColorSpace {
    Hsl,
    Rgb,
    Hwb,
    Lch,
}

#[derive(Clone, Copy, Debug)]
pub struct ThemeColorTuple {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub a: f32,
}

impl ThemeColorTuple {
    pub fn new(x: f32, y: f32, z: f32, a: f32) -> Self {
        Self { x, y, z, a }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ThemeColorInput {
    Hsla(Hsla),
    Tuple {
        space: ThemeColorSpace,
        values: ThemeColorTuple,
    },
}

impl ThemeColorInput {
    pub fn hsla(color: Hsla) -> Self {
        ThemeColorInput::Hsla(color)
    }

    pub fn hsl(h: f32, s: f32, l: f32, a: f32) -> Self {
        ThemeColorInput::Tuple {
            space: ThemeColorSpace::Hsl,
            values: ThemeColorTuple::new(h, s, l, a),
        }
    }

    pub fn rgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        ThemeColorInput::Tuple {
            space: ThemeColorSpace::Rgb,
            values: ThemeColorTuple::new(r, g, b, a),
        }
    }

    pub fn hwb(h: f32, w: f32, b: f32, a: f32) -> Self {
        ThemeColorInput::Tuple {
            space: ThemeColorSpace::Hwb,
            values: ThemeColorTuple::new(h, w, b, a),
        }
    }

    pub fn lch(l: f32, c: f32, h: f32, a: f32) -> Self {
        ThemeColorInput::Tuple {
            space: ThemeColorSpace::Lch,
            values: ThemeColorTuple::new(l, c, h, a),
        }
    }

    pub fn from_tuple(space: ThemeColorSpace, values: ThemeColorTuple) -> Self {
        ThemeColorInput::Tuple { space, values }
    }

    fn to_hsla(self) -> Hsla {
        match self {
            ThemeColorInput::Hsla(color) => color,
            ThemeColorInput::Tuple { space, values } => match space {
                ThemeColorSpace::Hsl => hsl_to_hsla(values.x, values.y, values.z, values.a),
                ThemeColorSpace::Rgb => rgb_to_hsla(values.x, values.y, values.z, values.a),
                ThemeColorSpace::Hwb => hwb_to_hsla(values.x, values.y, values.z, values.a),
                ThemeColorSpace::Lch => lch_to_hsla(values.x, values.y, values.z, values.a),
            },
        }
    }
}

pub type ThemeColorFn = Arc<dyn Fn(usize) -> ThemeColorInput + Send + Sync>;

#[derive(Clone)]
pub enum ThemeColorSettings {
    Series(Vec<ThemeColorInput>),
    Function(ThemeColorFn),
}

impl From<Vec<ThemeColorInput>> for ThemeColorSettings {
    fn from(values: Vec<ThemeColorInput>) -> Self {
        ThemeColorSettings::Series(values)
    }
}

impl<F> From<F> for ThemeColorSettings
where
    F: Fn(usize) -> ThemeColorInput + Send + Sync + 'static,
{
    fn from(func: F) -> Self {
        ThemeColorSettings::Function(Arc::new(func))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ThemeColorOptions {
    pub alpha: Option<f32>,
}

#[derive(Clone)]
pub struct ThemeColor {
    settings: ThemeColorSettings,
}

impl ThemeColor {
    pub fn new(settings: impl Into<ThemeColorSettings>) -> Self {
        Self {
            settings: settings.into(),
        }
    }

    pub fn value(&self, index: f32, options: ThemeColorOptions) -> Hsla {
        let input = match &self.settings {
            ThemeColorSettings::Series(series) => {
                if series.is_empty() {
                    return Hsla::transparent();
                }
                let index = index.round().max(0.0) as usize;
                let clamped = index.min(series.len().saturating_sub(1));
                series[clamped]
            }
            ThemeColorSettings::Function(func) => func(index.round().max(0.0) as usize),
        };

        let mut color = input.to_hsla();
        if let Some(alpha) = options.alpha {
            color.a = (color.a * alpha.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        }
        color
    }
}

pub fn create_theme_color(settings: impl Into<ThemeColorSettings>) -> ThemeColor {
    ThemeColor::new(settings)
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn clamp_range(value: f32, min: f32, max: f32) -> f32 {
    value.min(max).max(min)
}

fn hsl_to_hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    let h = clamp_range(h, 0.0, 360.0) / 360.0;
    let s = clamp_range(s, 0.0, 100.0) / 100.0;
    let l = clamp_range(l, 0.0, 100.0) / 100.0;
    let a = clamp01(a);
    Hsla::new(h, s, l, a)
}

fn rgb_to_hsla(r: f32, g: f32, b: f32, a: f32) -> Hsla {
    let r = clamp_range(r, 0.0, 100.0) / 100.0;
    let g = clamp_range(g, 0.0, 100.0) / 100.0;
    let b = clamp_range(b, 0.0, 100.0) / 100.0;
    let mut color = Hsla::from_rgb(r, g, b);
    color.a = clamp01(a);
    color
}

fn hwb_to_hsla(h: f32, w: f32, b: f32, a: f32) -> Hsla {
    let h = clamp_range(h, 0.0, 360.0) / 360.0;
    let w = clamp_range(w, 0.0, 100.0) / 100.0;
    let b = clamp_range(b, 0.0, 100.0) / 100.0;

    let (r, g, b) = hwb_to_rgb(h, w, b);
    let mut color = Hsla::from_rgb(r, g, b);
    color.a = clamp01(a);
    color
}

fn lch_to_hsla(l: f32, c: f32, h: f32, a: f32) -> Hsla {
    let l = clamp_range(l, 0.0, 100.0);
    let c = clamp_range(c, 0.0, 230.0);
    let h = clamp_range(h, 0.0, 360.0);

    let (r, g, b) = lch_to_rgb(l, c, h);
    let mut color = Hsla::from_rgb(r, g, b);
    color.a = clamp01(a);
    color
}

fn hwb_to_rgb(h: f32, w: f32, b: f32) -> (f32, f32, f32) {
    if w + b >= 1.0 {
        let gray = if w + b == 0.0 { 0.0 } else { w / (w + b) };
        return (gray, gray, gray);
    }

    let (mut r, mut g, mut bl) = hsv_to_rgb(h, 1.0, 1.0);
    let factor = 1.0 - w - b;
    r = r * factor + w;
    g = g * factor + w;
    bl = bl * factor + w;
    (r, g, bl)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

fn lch_to_rgb(l: f32, c: f32, h: f32) -> (f32, f32, f32) {
    let hr = h.to_radians();
    let a = c * hr.cos();
    let b = c * hr.sin();
    let (x, y, z) = lab_to_xyz(l, a, b);
    xyz_to_rgb(x, y, z)
}

fn lab_to_xyz(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let y = (l + 16.0) / 116.0;
    let x = a / 500.0 + y;
    let z = y - b / 200.0;

    let x3 = x.powi(3);
    let y3 = y.powi(3);
    let z3 = z.powi(3);

    let x = if x3 > 0.008856 {
        x3
    } else {
        (x - 16.0 / 116.0) / 7.787
    };
    let y = if y3 > 0.008856 {
        y3
    } else {
        (y - 16.0 / 116.0) / 7.787
    };
    let z = if z3 > 0.008856 {
        z3
    } else {
        (z - 16.0 / 116.0) / 7.787
    };

    let x = x * 95.047;
    let y = y * 100.0;
    let z = z * 108.883;

    (x, y, z)
}

fn xyz_to_rgb(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let x = x / 100.0;
    let y = y / 100.0;
    let z = z / 100.0;

    let r = x * 3.2406 + y * -1.5372 + z * -0.4986;
    let g = x * -0.9689 + y * 1.8758 + z * 0.0415;
    let b = x * 0.0557 + y * -0.2040 + z * 1.0570;

    (linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b))
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        (12.92 * c).clamp(0.0, 1.0)
    } else {
        (1.055 * c.powf(1.0 / 2.4) - 0.055).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_color_hsl() {
        let color = create_theme_color(vec![ThemeColorInput::hsl(120.0, 100.0, 50.0, 1.0)]);
        let value = color.value(0.0, ThemeColorOptions::default());
        assert!((value.h - (120.0 / 360.0)).abs() < 0.01);
        assert!((value.s - 1.0).abs() < 0.01);
        assert!((value.l - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_theme_color_rgb() {
        let color = create_theme_color(vec![ThemeColorInput::rgb(100.0, 0.0, 0.0, 1.0)]);
        let value = color.value(0.0, ThemeColorOptions::default());
        assert!(value.h < 0.05 || value.h > 0.95);
        assert!(value.s > 0.9);
    }

    #[test]
    fn test_theme_color_hwb() {
        let color = create_theme_color(vec![ThemeColorInput::hwb(0.0, 0.0, 0.0, 1.0)]);
        let value = color.value(0.0, ThemeColorOptions::default());
        assert!(value.s > 0.9);
    }

    #[test]
    fn test_theme_color_lch_gray() {
        let color = create_theme_color(vec![ThemeColorInput::lch(50.0, 0.0, 0.0, 1.0)]);
        let value = color.value(0.0, ThemeColorOptions::default());
        assert!(value.s < 0.01);
    }

    #[test]
    fn test_theme_color_alpha() {
        let color = create_theme_color(vec![ThemeColorInput::hsl(0.0, 0.0, 50.0, 0.5)]);
        let value = color.value(0.0, ThemeColorOptions { alpha: Some(0.5) });
        assert!((value.a - 0.25).abs() < 0.01);
    }
}
