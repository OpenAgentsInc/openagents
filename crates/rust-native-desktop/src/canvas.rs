//! An RGBA frame and the few shapes the painter draws into it.
//!
//! Everything is antialiased by coverage: a rounded rectangle's edge pixels
//! blend by how much of each pixel the shape covers.

use rust_native::style::Color;

/// A rectangle in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PxRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// An RGBA frame, eight bits a channel, rows top to bottom, opaque.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Frame {
    /// A frame filled with `color`.
    pub fn new(width: usize, height: usize, color: Color) -> Frame {
        let mut pixels = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            pixels.extend_from_slice(&[color.red, color.green, color.blue, 255]);
        }
        Frame {
            width,
            height,
            pixels,
        }
    }

    /// The color at `x`, `y`.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let at = (y * self.width + x) * 4;
        [self.pixels[at], self.pixels[at + 1], self.pixels[at + 2]]
    }

    /// Blends `color` over the pixel at `x`, `y` with `coverage` from 0 to 1,
    /// times the color's own alpha.
    pub fn blend(&mut self, x: i64, y: i64, color: Color, coverage: f32) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let a = (coverage.clamp(0.0, 1.0) * f32::from(color.alpha) / 255.0 * 256.0) as u32;
        if a == 0 {
            return;
        }
        let at = (y as usize * self.width + x as usize) * 4;
        for (channel, value) in [color.red, color.green, color.blue].into_iter().enumerate() {
            let under = u32::from(self.pixels[at + channel]);
            self.pixels[at + channel] = ((u32::from(value) * a + under * (256 - a)) >> 8) as u8;
        }
    }

    /// Fills a rectangle with corners of `radius` pixels.
    pub fn fill(&mut self, rect: PxRect, radius: f32, color: Color) {
        self.shape(rect, radius, None, color);
    }

    /// Strokes the inside edge of a rectangle `width` pixels wide.
    pub fn stroke(&mut self, rect: PxRect, radius: f32, width: f32, color: Color) {
        self.shape(rect, radius, Some(width), color);
    }

    fn shape(&mut self, rect: PxRect, radius: f32, stroke: Option<f32>, color: Color) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let radius = radius.clamp(0.0, rect.w.min(rect.h) / 2.0);
        let (x0, y0) = (rect.x.floor() as i64, rect.y.floor() as i64);
        let (x1, y1) = (
            (rect.x + rect.w).ceil() as i64,
            (rect.y + rect.h).ceil() as i64,
        );
        let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        let (hw, hh) = (rect.w / 2.0, rect.h / 2.0);
        for y in y0.max(0)..y1.min(self.height as i64) {
            for x in x0.max(0)..x1.min(self.width as i64) {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let outer = rounded_distance(px, py, hw, hh, radius);
                let mut coverage = (0.5 - outer).clamp(0.0, 1.0);
                if let Some(width) = stroke {
                    let inner =
                        rounded_distance(px, py, hw - width, hh - width, (radius - width).max(0.0));
                    coverage -= (0.5 - inner).clamp(0.0, 1.0);
                }
                if coverage > 0.0 {
                    self.blend(x, y, color, coverage);
                }
            }
        }
    }

    /// Draws a line from `a` to `b` `width` pixels wide, with round ends.
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: Color) {
        let x0 = (a.0.min(b.0) - width).floor() as i64;
        let x1 = (a.0.max(b.0) + width).ceil() as i64;
        let y0 = (a.1.min(b.1) - width).floor() as i64;
        let y1 = (a.1.max(b.1) + width).ceil() as i64;
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = (dx * dx + dy * dy).max(1e-6);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = (((px - a.0) * dx + (py - a.1) * dy) / length).clamp(0.0, 1.0);
                let (qx, qy) = (a.0 + t * dx - px, a.1 + t * dy - py);
                let distance = (qx * qx + qy * qy).sqrt() - width / 2.0;
                let coverage = (0.5 - distance).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend(x, y, color, coverage);
                }
            }
        }
    }

    /// The frame as a PNG file's bytes.
    pub fn png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, self.width as u32, self.height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|error| format!("cannot write the PNG header: {error}"))?;
            writer
                .write_image_data(&self.pixels)
                .map_err(|error| format!("cannot write the PNG data: {error}"))?;
        }
        Ok(out)
    }
}

/// The signed distance from a point to a rounded rectangle centered on the
/// origin with half extents `hw`, `hh`.
fn rounded_distance(px: f32, py: f32, hw: f32, hh: f32, radius: f32) -> f32 {
    let qx = px.abs() - (hw - radius);
    let qy = py.abs() - (hh - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fill_covers_its_inside_and_softens_its_corner() {
        let mut frame = Frame::new(20, 20, Color::rgb(0, 0, 0));
        let rect = PxRect {
            x: 2.0,
            y: 2.0,
            w: 16.0,
            h: 16.0,
        };
        frame.fill(rect, 6.0, Color::rgb(255, 255, 255));
        assert_eq!(frame.pixel(10, 10), [255, 255, 255]);
        assert_eq!(frame.pixel(0, 0), [0, 0, 0]);
        // The corner pixel is outside the rounded corner.
        assert_eq!(frame.pixel(2, 2), [0, 0, 0]);
    }

    #[test]
    fn a_stroke_leaves_the_middle_alone() {
        let mut frame = Frame::new(20, 20, Color::rgb(0, 0, 0));
        let rect = PxRect {
            x: 0.0,
            y: 0.0,
            w: 20.0,
            h: 20.0,
        };
        frame.stroke(rect, 0.0, 2.0, Color::rgb(255, 255, 255));
        assert_eq!(frame.pixel(0, 10), [255, 255, 255]);
        assert_eq!(frame.pixel(10, 10), [0, 0, 0]);
    }
}
