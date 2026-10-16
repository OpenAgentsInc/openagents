//! A shallow sand bed. Tools change its height; light across the slopes
//! reveals the grooves, raised edges, and individual grains.
use crate::{HEIGHT, Point, Tool, WIDTH};
const W: usize = WIDTH as usize;
const H: usize = HEIGHT as usize;
pub struct Sand {
    heights: Vec<f32>,
    grain: Vec<f32>,
    pub pixels: Vec<u8>,
    water: Vec<f32>,
    water_colors: Vec<[f32; 3]>,
    wet: usize,
}
impl Default for Sand {
    fn default() -> Self {
        Self::new()
    }
}
impl Sand {
    pub fn new() -> Self {
        let mut seed = 731u32;
        let grain = (0..W * H)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                ((seed & 255) as f32 / 255. - 0.5) * 18.
            })
            .collect();
        let mut s = Self {
            heights: vec![0.; W * H],
            grain,
            pixels: vec![255; W * H * 4],
            water: vec![0.; W * H],
            water_colors: vec![[84., 156., 170.]; W * H],
            wet: 0,
        };
        s.shade((0, 0, W, H));
        s
    }
    pub fn clear(&mut self) {
        self.heights.fill(0.);
        self.water.fill(0.);
        self.wet = 0;
    }
    pub fn water_ratio(&self) -> f64 {
        self.wet as f64 / (W * H) as f64
    }
    pub fn is_water(&self, p: Point) -> bool {
        let x = (p.x as usize).min(W - 1);
        let y = (p.y as usize).min(H - 1);
        self.water[y * W + x] > 0.4
    }
    pub fn stroke(
        &mut self,
        tool: Tool,
        size: f64,
        a: Point,
        b: Point,
        old: (f64, f64),
        n: (f64, f64),
        color: Option<&str>,
    ) -> (usize, usize, usize, usize) {
        let tint = color
            .filter(|v| crate::valid_color(v))
            .map(|c| {
                [
                    u8::from_str_radix(&c[1..3], 16).unwrap() as f32,
                    u8::from_str_radix(&c[3..5], 16).unwrap() as f32,
                    u8::from_str_radix(&c[5..7], 16).unwrap() as f32,
                ]
            })
            .unwrap_or([84., 156., 170.]);
        let count = if tool == Tool::Rake { 5 } else { 1 };
        let radius = if tool == Tool::Water {
            48. * size
        } else if tool == Tool::Smooth {
            30. * size
        } else if tool == Tool::Rake {
            24. * size
        } else {
            12. * size
        };
        let x0 = (a.x.min(b.x) - radius - 2.).max(0.) as usize;
        let y0 = (a.y.min(b.y) - radius - 2.).max(0.) as usize;
        let x1 = (a.x.max(b.x) + radius + 3.).min(WIDTH) as usize;
        let y1 = (a.y.max(b.y) + radius + 3.).min(HEIGHT) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = y * W + x;
                let was_wet = self.water[i] > 0.4;
                for k in 0..count {
                    let off = (k as f64 - (count - 1) as f64 / 2.) * 8. * size;
                    let p = Point {
                        x: a.x + old.0 * off,
                        y: a.y + old.1 * off,
                    };
                    let q = Point {
                        x: b.x + n.0 * off,
                        y: b.y + n.1 * off,
                    };
                    let dx = q.x - p.x;
                    let dy = q.y - p.y;
                    let d2 = dx * dx + dy * dy;
                    let t = if d2 < 0.001 {
                        0.
                    } else {
                        (((x as f64 - p.x) * dx + (y as f64 - p.y) * dy) / d2).clamp(0., 1.)
                    };
                    let dist = (x as f64 - p.x - t * dx).hypot(y as f64 - p.y - t * dy);
                    if tool == Tool::Water {
                        let level = ((radius - dist) / 12.).clamp(0., 1.) as f32;
                        if level > 0. {
                            self.water[i] = self.water[i].max(level);
                            self.water_colors[i] = tint;
                        }
                    } else if tool == Tool::Smooth {
                        let weight = ((radius - dist) / 8.).clamp(0., 1.) as f32;
                        self.heights[i] *= 1. - weight;
                        self.water[i] *= 1. - weight;
                    } else {
                        let width = if tool == Tool::Finger {
                            3.6 * size
                        } else {
                            1.6 * size
                        };
                        if dist > width * 3.5 {
                            continue;
                        }
                        let d = dist / width;
                        let trench = -1.5 * (-d * d * 0.9).exp();
                        let edge = 0.55 * (-(d - 1.9).powi(2) * 3.).exp();
                        let target = (trench + edge) as f32;
                        // Repeated samples carve the same depth, rather than darkening ink.
                        self.heights[i] = if target < 0. {
                            self.heights[i].min(target)
                        } else if self.heights[i] >= 0. {
                            self.heights[i].max(target)
                        } else {
                            self.heights[i]
                        };
                    }
                }
                let is_wet = self.water[i] > 0.4;
                if !was_wet && is_wet {
                    self.wet += 1;
                } else if was_wet && !is_wet {
                    self.wet -= 1;
                }
            }
        }
        (
            x0.saturating_sub(1),
            y0.saturating_sub(1),
            (x1 + 1).min(W),
            (y1 + 1).min(H),
        )
    }
    pub fn shade(&mut self, r: (usize, usize, usize, usize)) {
        for y in r.1..r.3 {
            for x in r.0..r.2 {
                let i = y * W + x;
                let dx = self.heights[y * W + (x + 1).min(W - 1)]
                    - self.heights[y * W + x.saturating_sub(1)];
                let dy = self.heights[(y + 1).min(H - 1) * W + x]
                    - self.heights[y.saturating_sub(1) * W + x];
                let relief = (-dx * 23. - dy * 29. + self.heights[i] * 3.).clamp(-42., 38.);
                let ripple = (x as f32 * 0.023 + (y as f32 * 0.009).sin() * 2.).sin() * 1.7;
                let glow = 4. * (1. - y as f32 / H as f32) - 2. * x as f32 / W as f32;
                let grain = self.grain[i] * (1. - self.heights[i] * 0.09);
                for (k, c) in [226., 204., 169.].iter().enumerate() {
                    let dry = c + relief + grain + ripple + glow;
                    let water = self.water[i];
                    let shine = (x as f32 * 0.085 + (y as f32 * 0.022).sin() * 3.).sin() * 2.5;
                    let wet = self.water_colors[i][k] + grain * 0.16 + shine + glow + relief * 0.08;
                    let shore = (1. - (water - 0.25).abs() * 5.).max(0.) * 6.;
                    self.pixels[i * 4 + k] =
                        (dry + (wet - dry) * water * 0.84 + shore).clamp(0., 255.) as u8;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_rake_carves_five_grooves_without_darkening_repeated_samples() {
        let mut s = Sand::new();
        let a = Point::new(100., 200.);
        let b = Point::new(300., 200.);
        s.stroke(Tool::Rake, 1., a, b, (0., 1.), (0., 1.), None);
        for y in [184, 192, 200, 208, 216] {
            assert!(s.heights[y * W + 200] < -1.);
        }
        let previous = s.heights.clone();
        s.stroke(Tool::Rake, 1., a, b, (0., 1.), (0., 1.), None);
        assert_eq!(previous, s.heights);
        s.stroke(Tool::Smooth, 1.8, a, b, (0., 1.), (0., 1.), None);
        assert_eq!(s.heights[200 * W + 200], 0.);
    }
    #[test]
    fn painting_and_smoothing_water_change_the_habitat() {
        let mut s = Sand::new();
        let a = Point::new(200., 250.);
        let b = Point::new(400., 250.);
        let r = s.stroke(Tool::Water, 1., a, b, (0., 1.), (0., 1.), Some("#52d6ca"));
        s.shade(r);
        assert!(s.water_ratio() > 0.025);
        assert!(s.is_water(Point::new(300., 250.)));
        assert!(!s.is_water(Point::new(700., 250.)));
        let water = s.water_ratio();
        s.stroke(Tool::Smooth, 1.8, a, b, (0., 1.), (0., 1.), None);
        assert!(s.water_ratio() < water);
        assert!(!s.is_water(Point::new(300., 250.)));
        s.clear();
        assert_eq!(s.water_ratio(), 0.);
    }
    #[test]
    fn grains_survive_carving_and_smoothing() {
        let mut s = Sand::new();
        let before = s.pixels.clone();
        let a = Point::new(5., 5.);
        let rect = s.stroke(Tool::Finger, 1., a, a, (0., 1.), (0., 1.), None);
        s.shade(rect);
        assert_ne!(s.pixels, before);
        let rect = s.stroke(Tool::Smooth, 1.8, a, a, (0., 1.), (0., 1.), None);
        s.shade(rect);
        assert_eq!(s.pixels, before);
    }
}
