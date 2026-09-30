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

impl PxRect {
    pub(crate) fn intersection(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Self {
            x,
            y,
            w: ((self.x + self.w).min(other.x + other.w) - x).max(0.0),
            h: ((self.y + self.h).min(other.y + other.h) - y).max(0.0),
        }
    }
}

/// An RGBA frame, eight bits a channel, rows top to bottom. A frame made
/// with [`Frame::new`] is opaque; one made with [`Frame::transparent`]
/// starts clear, and its channels are premultiplied by alpha, so a window
/// can lay it over a backdrop.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    clip: Option<PxRect>,
}

impl Frame {
    /// Paint within a pixel region, returning the previous clip to restore later.
    pub fn clip_to(&mut self, rect: PxRect) -> Option<PxRect> {
        let previous = self.clip;
        self.clip = Some(previous.map_or(rect, |old| old.intersection(rect)));
        previous
    }
    /// Restore a clip returned by `clip_to`.
    pub fn restore_clip(&mut self, clip: Option<PxRect>) {
        self.clip = clip;
    }

    /// A frame filled with `color`.
    pub fn new(width: usize, height: usize, color: Color) -> Frame {
        let mut pixels = vec![0; width * height * 4];
        repeat_pixel(&mut pixels, [color.red, color.green, color.blue, 255]);
        Frame {
            width,
            height,
            pixels,
            clip: None,
        }
    }

    /// A clear frame: every pixel transparent black.
    pub fn transparent(width: usize, height: usize) -> Frame {
        Frame {
            width,
            height,
            pixels: vec![0; width * height * 4],
            clip: None,
        }
    }

    /// The color at `x`, `y`.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let at = (y * self.width + x) * 4;
        [self.pixels[at], self.pixels[at + 1], self.pixels[at + 2]]
    }

    /// Blends `color` over the pixel at `x`, `y` with `coverage` from 0 to 1,
    /// times the color's own alpha. The alpha channel composes the same way,
    /// so an opaque frame stays opaque and a clear one gathers premultiplied
    /// color.
    pub fn blend(&mut self, x: i64, y: i64, color: Color, coverage: f32) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        if self.clip.is_some_and(|clip| {
            (x as f32) < clip.x
                || (y as f32) < clip.y
                || (x as f32) >= clip.x + clip.w
                || (y as f32) >= clip.y + clip.h
        }) {
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
        let under = u32::from(self.pixels[at + 3]);
        self.pixels[at + 3] = ((255 * a + under * (256 - a)) >> 8) as u8;
    }

    /// A usage ring. `used` is the clockwise share from the top, 0 to 1.
    /// The track is the rest of the ring. A zero share paints only the track.
    pub fn usage_ring(&mut self, rect: PxRect, used: f32, track: Color, fill: Color) {
        let used = if used.is_finite() {
            used.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let outer = rect.w.min(rect.h) / 2.0;
        if outer <= 0.0 {
            return;
        }
        let cx = rect.x + rect.w / 2.0;
        let cy = rect.y + rect.h / 2.0;
        let width = (outer * 0.22).max(1.5);
        let inner = (outer - width).max(0.0);
        let x0 = rect.x.floor() as i64;
        let y0 = rect.y.floor() as i64;
        let x1 = (rect.x + rect.w).ceil() as i64;
        let y1 = (rect.y + rect.h).ceil() as i64;
        for y in y0..y1 {
            for x in x0..x1 {
                let px = x as f32 + 0.5 - cx;
                let py = y as f32 + 0.5 - cy;
                let dist = px.hypot(py);
                let coverage = (outer - dist + 0.5)
                    .clamp(0.0, 1.0)
                    .min((dist - inner + 0.5).clamp(0.0, 1.0));
                if coverage <= 0.0 {
                    continue;
                }
                let mut turns = px.atan2(-py) / (2.0 * std::f32::consts::PI);
                if turns < 0.0 {
                    turns += 1.0;
                }
                let color = if used >= 1.0 || turns < used {
                    fill
                } else {
                    track
                };
                self.blend(x, y, color, coverage);
            }
        }
    }

    /// Restricts subsequent drawing to a rectangle, or removes the restriction.
    pub(crate) fn set_clip(&mut self, clip: Option<PxRect>) {
        self.clip = clip;
    }

    pub(crate) fn clip(&self) -> Option<PxRect> {
        self.clip
    }

    pub(crate) fn visible(&self, rect: PxRect) -> bool {
        let bounds = self.bounds(rect);
        bounds.0 < bounds.2 && bounds.1 < bounds.3
    }

    fn bounds(&self, rect: PxRect) -> (usize, usize, usize, usize) {
        let rect = self.clip.map_or(rect, |clip| rect.intersection(clip));
        let x0 = rect.x.ceil().clamp(0.0, self.width as f32) as usize;
        let y0 = rect.y.ceil().clamp(0.0, self.height as f32) as usize;
        let x1 = (rect.x + rect.w).ceil().clamp(0.0, self.width as f32) as usize;
        let y1 = (rect.y + rect.h).ceil().clamp(0.0, self.height as f32) as usize;
        (x0, y0, x1, y1)
    }

    /// Clears a pixel-aligned region before repainting its layers.
    pub(crate) fn clear(&mut self, rect: PxRect, background: Option<Color>) {
        let (x0, y0, x1, y1) = self.bounds(rect);
        for y in y0..y1 {
            let row = &mut self.pixels[(y * self.width + x0) * 4..(y * self.width + x1) * 4];
            if let Some(color) = background {
                let pixel = [color.red, color.green, color.blue, 255];
                repeat_pixel(row, pixel);
            } else {
                row.fill(0);
            }
        }
    }

    /// Blends a fully covered span without a distance calculation per pixel.
    fn span(&mut self, y: usize, x0: usize, x1: usize, color: Color) {
        if color.alpha == 0 || x0 >= x1 {
            return;
        }
        let row = &mut self.pixels[(y * self.width + x0) * 4..(y * self.width + x1) * 4];
        if color.alpha == 255 {
            let pixel = [color.red, color.green, color.blue, 255];
            repeat_pixel(row, pixel);
        } else {
            let alpha = u64::from(color.alpha) * 256 / 255;
            let inverse = 256 - alpha;
            let rgba = u64::from(u32::from_le_bytes([
                color.red,
                color.green,
                color.blue,
                255,
            ]));
            let rb = (rgba & 0x00ff00ff) * alpha;
            let ga = ((rgba >> 8) & 0x00ff00ff) * alpha;
            for dest in row.chunks_exact_mut(4) {
                let under = u64::from(u32::from_le_bytes(dest.try_into().expect("one pixel")));
                let rb = ((rb + (under & 0x00ff00ff) * inverse) >> 8) & 0x00ff00ff;
                let ga = ((ga + ((under >> 8) & 0x00ff00ff) * inverse) >> 8) & 0x00ff00ff;
                dest.copy_from_slice(&((rb | (ga << 8)) as u32).to_le_bytes());
            }
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

    /// Fills a rectangle whose corners may differ: `radii` are top left,
    /// top right, bottom right, bottom left, in pixels (the layout's top
    /// leading, top trailing, bottom trailing, bottom leading).
    pub fn fill_corners(&mut self, rect: PxRect, radii: [f32; 4], color: Color) {
        self.corners(rect, radii, None, color);
    }

    /// Strokes a rectangle whose corners may differ; see [`Frame::fill_corners`].
    pub fn stroke_corners(&mut self, rect: PxRect, radii: [f32; 4], width: f32, color: Color) {
        self.corners(rect, radii, Some(width), color);
    }

    /// Draws each quadrant with its own corner's radius, clipped to that
    /// quadrant, so no pixel is painted twice and a translucent color stays
    /// even. Equal radii take the single-shape path.
    fn corners(&mut self, rect: PxRect, radii: [f32; 4], stroke: Option<f32>, color: Color) {
        if radii.iter().all(|r| (r - radii[0]).abs() < f32::EPSILON) {
            self.shape(rect, radii[0], stroke, color);
            return;
        }
        let mid_x = (rect.x + rect.w / 2.0).round();
        let mid_y = (rect.y + rect.h / 2.0).round();
        let (right, bottom) = (rect.x + rect.w, rect.y + rect.h);
        let quadrants = [
            (
                PxRect {
                    x: rect.x,
                    y: rect.y,
                    w: mid_x - rect.x,
                    h: mid_y - rect.y,
                },
                radii[0],
            ),
            (
                PxRect {
                    x: mid_x,
                    y: rect.y,
                    w: right - mid_x,
                    h: mid_y - rect.y,
                },
                radii[1],
            ),
            (
                PxRect {
                    x: mid_x,
                    y: mid_y,
                    w: right - mid_x,
                    h: bottom - mid_y,
                },
                radii[2],
            ),
            (
                PxRect {
                    x: rect.x,
                    y: mid_y,
                    w: mid_x - rect.x,
                    h: bottom - mid_y,
                },
                radii[3],
            ),
        ];
        for (quadrant, radius) in quadrants {
            if quadrant.w <= 0.0 || quadrant.h <= 0.0 {
                continue;
            }
            let previous = self.clip_to(quadrant);
            self.shape(rect, radius, stroke, color);
            self.restore_clip(previous);
        }
    }

    fn shape(&mut self, rect: PxRect, radius: f32, stroke: Option<f32>, color: Color) {
        if rect.w <= 0.0 || rect.h <= 0.0 || color.alpha == 0 {
            return;
        }
        let radius = radius.clamp(0.0, rect.w.min(rect.h) / 2.0);
        let (x0, y0, x1, y1) = self.bounds(PxRect {
            x: rect.x.floor(),
            y: rect.y.floor(),
            w: (rect.x + rect.w).ceil() - rect.x.floor(),
            h: (rect.y + rect.h).ceil() - rect.y.floor(),
        });
        let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        let (hw, hh) = (rect.w / 2.0, rect.h / 2.0);
        for y in y0..y1 {
            let (span_start, span_end) = if stroke.is_none()
                && y as f32 >= rect.y.ceil()
                && (y as f32) < (rect.y + rect.h).floor()
            {
                let inset = if (y as f32 + 0.5 - cy).abs() <= hh - radius {
                    0.0
                } else {
                    radius
                };
                let start = (rect.x + inset).ceil().clamp(x0 as f32, x1 as f32) as usize;
                let end = (rect.x + rect.w - inset)
                    .floor()
                    .clamp(start as f32, x1 as f32) as usize;
                self.span(y, start, end, color);
                (start, end)
            } else if let Some(width) = stroke {
                let inner_radius = (radius - width).max(0.0);
                let inner_h = hh - width;
                if inner_h >= 0.5 && (y as f32 + 0.5 - cy).abs() <= inner_h - 0.5 {
                    let inset = if (y as f32 + 0.5 - cy).abs() <= inner_h - inner_radius {
                        width + 0.5
                    } else {
                        width + inner_radius + 0.5
                    };
                    let start = (rect.x + inset).ceil().clamp(x0 as f32, x1 as f32) as usize;
                    let end = (rect.x + rect.w - inset)
                        .floor()
                        .clamp(start as f32, x1 as f32) as usize;
                    (start, end)
                } else {
                    (x0, x0)
                }
            } else {
                (x0, x0)
            };
            for x in (x0..span_start).chain(span_end..x1) {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let outer = rounded_distance(px, py, hw, hh, radius);
                let mut coverage = (0.5 - outer).clamp(0.0, 1.0);
                if let Some(width) = stroke {
                    let inner =
                        rounded_distance(px, py, hw - width, hh - width, (radius - width).max(0.0));
                    coverage -= (0.5 - inner).clamp(0.0, 1.0);
                }
                if coverage > 0.0 {
                    self.blend(x as i64, y as i64, color, coverage);
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
    if qx <= 0.0 || qy <= 0.0 {
        qx.max(qy) - radius
    } else {
        (qx * qx + qy * qy).sqrt() - radius
    }
}

// Double an initialized RGBA span using bulk copies, including in debug builds.
fn repeat_pixel(row: &mut [u8], pixel: [u8; 4]) {
    if row.is_empty() {
        return;
    }
    row[..4].copy_from_slice(&pixel);
    let mut initialized = 4;
    while initialized < row.len() {
        let count = initialized.min(row.len() - initialized);
        row.copy_within(..count, initialized);
        initialized += count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table header rounds only its top corners: its bottom corners
    /// are square and filled to the edge, and each pixel is painted once.
    #[test]
    fn corners_round_only_where_asked() {
        let mut frame = Frame::new(20, 20, Color::rgb(0, 0, 0));
        let rect = PxRect {
            x: 2.0,
            y: 2.0,
            w: 16.0,
            h: 16.0,
        };
        frame.fill_corners(rect, [6.0, 6.0, 0.0, 0.0], Color::rgb(255, 255, 255));
        assert_eq!(frame.pixel(2, 2), [0, 0, 0], "top left is rounded");
        assert_eq!(frame.pixel(17, 2), [0, 0, 0], "top right is rounded");
        assert_eq!(frame.pixel(2, 17), [255, 255, 255], "bottom left is square");
        assert_eq!(
            frame.pixel(17, 17),
            [255, 255, 255],
            "bottom right is square"
        );
        assert_eq!(frame.pixel(10, 10), [255, 255, 255]);
        // Half-transparent: the seams between quadrants are painted once.
        let mut frame = Frame::new(20, 20, Color::rgb(0, 0, 0));
        let half = Color {
            alpha: 128,
            ..Color::rgb(200, 200, 200)
        };
        frame.fill_corners(rect, [6.0, 6.0, 0.0, 0.0], half);
        let middle = frame.pixel(10, 10);
        for (x, y) in [(9, 10), (10, 9), (9, 9), (10, 17), (17, 10)] {
            assert_eq!(frame.pixel(x, y), middle, "({x},{y}) painted once");
        }
    }

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
    fn an_opaque_frame_stays_opaque_and_a_clear_one_gathers_alpha() {
        let rect = PxRect {
            x: 0.0,
            y: 0.0,
            w: 4.0,
            h: 4.0,
        };
        let mut opaque = Frame::new(4, 4, Color::rgb(0, 0, 0));
        opaque.fill(
            rect,
            0.0,
            Color {
                alpha: 128,
                ..Color::rgb(255, 255, 255)
            },
        );
        assert!(opaque.pixels.chunks(4).all(|p| p[3] == 255));
        let mut clear = Frame::transparent(4, 4);
        clear.fill(rect, 0.0, Color::rgb(255, 255, 255));
        assert!(clear.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
        let mut half = Frame::transparent(4, 4);
        half.fill(
            rect,
            0.0,
            Color {
                alpha: 128,
                ..Color::rgb(200, 100, 0)
            },
        );
        // Premultiplied: color and alpha both halved.
        assert_eq!(&half.pixels[..4], &[100, 50, 0, 127]);
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

    #[test]
    fn clipped_spans_match_pixel_coverage_for_fractional_shapes_and_alpha() {
        for alpha in [0, 1, 42, 105, 128, 254, 255] {
            for radius in [0.0, 0.6, 4.0, 50.0] {
                for stroke in [None, Some(0.7), Some(3.0)] {
                    let rect = PxRect {
                        x: -2.3,
                        y: 3.7,
                        w: 27.4,
                        h: 19.6,
                    };
                    let clip = PxRect {
                        x: 4.2,
                        y: 2.1,
                        w: 12.3,
                        h: 24.8,
                    };
                    let color = Color {
                        alpha,
                        ..Color::rgb(201, 53, 159)
                    };
                    let mut actual = Frame::new(30, 30, Color::rgb(21, 43, 84));
                    let mut expected = actual.clone();
                    actual.set_clip(Some(clip));
                    expected.set_clip(Some(clip));
                    actual.shape(rect, radius, stroke, color);
                    let radius = radius.min(rect.w.min(rect.h) / 2.0);
                    let distance = |px: f32, py: f32, hw: f32, hh: f32, radius: f32| {
                        let qx = px.abs() - (hw - radius);
                        let qy = py.abs() - (hh - radius);
                        (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0)
                            - radius
                    };
                    for y in 0..30 {
                        for x in 0..30 {
                            let px = x as f32 + 0.5 - rect.x - rect.w / 2.0;
                            let py = y as f32 + 0.5 - rect.y - rect.h / 2.0;
                            let mut coverage = (0.5
                                - distance(px, py, rect.w / 2.0, rect.h / 2.0, radius))
                            .clamp(0.0, 1.0);
                            if let Some(width) = stroke {
                                coverage -= (0.5
                                    - distance(
                                        px,
                                        py,
                                        rect.w / 2.0 - width,
                                        rect.h / 2.0 - width,
                                        (radius - width).max(0.0),
                                    ))
                                .clamp(0.0, 1.0);
                            }
                            if coverage > 0.0 {
                                expected.blend(x, y, color, coverage);
                            }
                        }
                    }
                    assert_eq!(
                        actual.pixels, expected.pixels,
                        "alpha={alpha}, radius={radius}, stroke={stroke:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_usage_ring_starts_at_the_top_and_runs_clockwise() {
        let rect = PxRect {
            x: 0.0,
            y: 0.0,
            w: 24.0,
            h: 24.0,
        };
        let track = Color::rgb(58, 64, 73);
        let fill = Color::rgb(214, 168, 92);
        let mut empty = Frame::new(24, 24, Color::rgb(0, 0, 0));
        empty.usage_ring(rect, 0.0, track, fill);
        assert_eq!(empty.pixel(12, 1), [58, 64, 73]);
        assert_eq!(empty.pixel(22, 12), [58, 64, 73]);
        let mut full = Frame::new(24, 24, Color::rgb(0, 0, 0));
        full.usage_ring(rect, 1.0, track, fill);
        assert_eq!(full.pixel(12, 1), [214, 168, 92]);
        let mut half = Frame::new(24, 24, Color::rgb(0, 0, 0));
        half.usage_ring(rect, 0.5, track, fill);
        assert_eq!(half.pixel(22, 12), [214, 168, 92]);
    }
}
