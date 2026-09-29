//! Paints a grid into pixels, in software.
//!
//! The window and the PNG capture both go through [`Painter`], so a
//! screenshot the deck writes is the frame the window shows. Text is
//! JetBrains Mono, the monospace face `rust-native` bundles, rasterized
//! with `swash` at the size the view picks; bold is the variable font's
//! `wght` axis at 700 and italics are the italic face. The block glyph
//! `█` fills its cell, and rule glyphs draw as hairlines from the arms
//! [`Cell::arms`] returns, so frames and rules join across cells whatever
//! the font's box-drawing metrics.
//!
//! Colors are the amber ladder over the near-black field from
//! `coder_ui::theme`, blended in sRGB.

use crate::grid::{Cell, Grid};
use coder_ui::theme::{Intensity, NEAR_BLACK};
use std::collections::HashMap;
use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;

/// JetBrains Mono, upright, as `rust-native` bundles it (OFL 1.1).
const MONO: &[u8] = include_bytes!("../../rust-native/fonts/JetBrainsMono-Variable.ttf");
/// JetBrains Mono, italic.
const MONO_ITALIC: &[u8] =
    include_bytes!("../../rust-native/fonts/JetBrainsMono-Italic-Variable.ttf");

/// A cell's width, in ems. JetBrains Mono advances 0.6 em.
pub const CELL_EM: f32 = 0.6;
/// A row's height, in ems.
pub const LINE_EM: f32 = 1.3;

/// An RGBA frame, eight bits a channel, rows top to bottom.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Frame {
    /// A frame filled with `color`.
    pub fn new(width: usize, height: usize, color: u32) -> Frame {
        let [r, g, b] = rgb(color);
        let mut pixels = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
        Frame {
            width,
            height,
            pixels,
        }
    }

    /// Fills the rectangle from `x0`, `y0` up to `x1`, `y1` with `color`,
    /// clipped to the frame.
    pub fn fill(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: u32) {
        let [r, g, b] = rgb(color);
        let (x0, x1) = (x0.max(0) as usize, (x1.max(0) as usize).min(self.width));
        let (y0, y1) = (y0.max(0) as usize, (y1.max(0) as usize).min(self.height));
        for y in y0..y1 {
            for x in x0..x1 {
                let at = (y * self.width + x) * 4;
                self.pixels[at..at + 3].copy_from_slice(&[r, g, b]);
            }
        }
    }

    /// Blends `color` over the pixel at `x`, `y` with `coverage` from 0 to
    /// 255.
    fn blend(&mut self, x: i64, y: i64, color: u32, coverage: u8) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let at = (y as usize * self.width + x as usize) * 4;
        let a = u32::from(coverage);
        for (channel, value) in rgb(color).into_iter().enumerate() {
            let under = u32::from(self.pixels[at + channel]);
            self.pixels[at + channel] = ((u32::from(value) * a + under * (255 - a)) / 255) as u8;
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

/// The three channels of a packed RGB value.
fn rgb(color: u32) -> [u8; 3] {
    [(color >> 16) as u8, (color >> 8) as u8, color as u8]
}

/// A rasterized glyph: its coverage bitmap and where it sits against the
/// pen and the baseline.
struct Glyph {
    width: usize,
    height: usize,
    left: i32,
    top: i32,
    coverage: Vec<u8>,
}

/// Paints grids at one type size.
pub struct Painter {
    size: f32,
    context: ScaleContext,
    glyphs: HashMap<(char, bool, bool), Option<Glyph>>,
    ascent: f32,
    descent: f32,
}

impl Painter {
    /// A painter at `size` pixels to the em.
    pub fn new(size: f32) -> Painter {
        let size = size.max(1.0);
        let font = FontRef::from_index(MONO, 0).expect("the bundled font parses");
        let metrics = font.metrics(&[]).scale(size);
        Painter {
            size,
            context: ScaleContext::new(),
            glyphs: HashMap::new(),
            ascent: metrics.ascent,
            descent: metrics.descent,
        }
    }

    /// The type size, in pixels to the em.
    pub fn size(&self) -> f32 {
        self.size
    }

    /// A cell's width, in pixels.
    pub fn cell_width(&self) -> f32 {
        self.size * CELL_EM
    }

    /// A row's height, in pixels.
    pub fn cell_height(&self) -> f32 {
        self.size * LINE_EM
    }

    /// The pixel size of `cells` by `rows` at this painter's size.
    pub fn extent(&self, cells: usize, rows: usize) -> (f32, f32) {
        (
            cells as f32 * self.cell_width(),
            rows as f32 * self.cell_height(),
        )
    }

    /// Paints `grid` onto `frame` with its top-left corner at `x`, `y`.
    pub fn paint(&mut self, frame: &mut Frame, grid: &Grid, x: f32, y: f32) {
        let (cw, ch) = (self.cell_width(), self.cell_height());
        for (row, cells) in grid.rows().enumerate() {
            for (col, cell) in cells.iter().enumerate() {
                if cell.is_blank() {
                    continue;
                }
                let x0 = (x + col as f32 * cw).round() as i64;
                let x1 = (x + (col + 1) as f32 * cw).round() as i64;
                let y0 = (y + row as f32 * ch).round() as i64;
                let y1 = (y + (row + 1) as f32 * ch).round() as i64;
                self.cell(frame, cell, [x0, y0, x1, y1]);
            }
        }
    }

    /// Paints one cell into its pixel box.
    fn cell(&mut self, frame: &mut Frame, cell: &Cell, [x0, y0, x1, y1]: [i64; 4]) {
        let color = cell.style.intensity.color();
        if cell.style.caret {
            // The caret spans the font's ascent to its descent.
            let baseline = self.baseline(y0, y1);
            frame.fill(
                x0,
                baseline - self.ascent.round() as i64,
                x1,
                baseline + self.descent.round() as i64,
                color,
            );
            return;
        }
        if cell.glyph == '█' {
            frame.fill(x0, y0, x1, y1, color);
            return;
        }
        if let Some(arms) = cell.arms() {
            let weight = (self.size / 16.0).round().max(1.0) as i64;
            let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
            let (hx, hy) = (mx - weight / 2, my - weight / 2);
            let dashes = |frame: &mut Frame, a: i64, b: i64, horizontal: bool| {
                if !arms.dashed {
                    if horizontal {
                        frame.fill(a, hy, b, hy + weight, color);
                    } else {
                        frame.fill(hx, a, hx + weight, b, color);
                    }
                    return;
                }
                let step = ((b - a) / 2).max(1);
                let mut at = a;
                while at < b {
                    let end = (at + step).min(b);
                    if horizontal {
                        frame.fill(at, hy, end, hy + weight, color);
                    } else {
                        frame.fill(hx, at, hx + weight, end, color);
                    }
                    at += step * 2;
                }
            };
            if arms.left {
                dashes(frame, x0, hx + weight, true);
            }
            if arms.right {
                dashes(frame, hx, x1, true);
            }
            if arms.up {
                dashes(frame, y0, hy + weight, false);
            }
            if arms.down {
                dashes(frame, hy, y1, false);
            }
            return;
        }
        let baseline = self.baseline(y0, y1);
        let key = (cell.glyph, cell.style.bold, cell.style.italic);
        if !self.glyphs.contains_key(&key) {
            let glyph = self.rasterize(key);
            self.glyphs.insert(key, glyph);
        }
        let Some(Some(glyph)) = self.glyphs.get(&key) else {
            return;
        };
        let left = x0 + i64::from(glyph.left);
        let top = baseline - i64::from(glyph.top);
        for gy in 0..glyph.height {
            for gx in 0..glyph.width {
                let coverage = glyph.coverage[gy * glyph.width + gx];
                if coverage > 0 {
                    frame.blend(left + gx as i64, top + gy as i64, color, coverage);
                }
            }
        }
        if cell.style.underline {
            let weight = (self.size / 16.0).round().max(1.0) as i64;
            frame.fill(x0, baseline + 2, x1, baseline + 2 + weight, color);
        }
    }

    /// The baseline of a row whose box runs from `y0` to `y1`: the font's
    /// line centered in the row.
    fn baseline(&self, y0: i64, y1: i64) -> i64 {
        let line = self.ascent + self.descent;
        let top = y0 as f32 + ((y1 - y0) as f32 - line) / 2.0;
        (top + self.ascent).round() as i64
    }

    /// The coverage bitmap of one glyph in one weight and slant.
    fn rasterize(&mut self, (glyph, bold, italic): (char, bool, bool)) -> Option<Glyph> {
        let data = if italic { MONO_ITALIC } else { MONO };
        let font = FontRef::from_index(data, 0)?;
        let id = font.charmap().map(glyph);
        let weight = if bold { 700.0 } else { 400.0 };
        let mut scaler = self
            .context
            .builder(font)
            .size(self.size)
            .hint(true)
            .variations(&[("wght", weight)][..])
            .build();
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, id)?;
        Some(Glyph {
            width: image.placement.width as usize,
            height: image.placement.height as usize,
            left: image.placement.left,
            top: image.placement.top,
            coverage: image.data,
        })
    }
}

/// The type size that fits `cells` by `rows` into `width` by `height`
/// pixels.
pub fn fitting_size(width: f32, height: f32, cells: usize, rows: usize) -> f32 {
    let by_width = width / (cells as f32 * CELL_EM);
    let by_height = height / (rows as f32 * LINE_EM);
    by_width.min(by_height).max(1.0)
}

/// The field color every slide sits on.
pub const FIELD: u32 = NEAR_BLACK;

/// The brightest color a cell paints, for tests.
pub const BRIGHTEST: u32 = Intensity::Full.color();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Style;

    /// A painted block glyph fills its cell with full amber, and text
    /// leaves some of its cell on the field.
    #[test]
    fn a_block_fills_its_cell_and_text_draws_amber() {
        let mut painter = Painter::new(20.0);
        let mut grid = Grid::new(3, 1);
        grid.put(0, 0, Cell::new('█', Style::at(Intensity::Full)));
        grid.put_str(1, 0, "A", Style::at(Intensity::Full));
        let mut frame = Frame::new(60, 40, FIELD);
        painter.paint(&mut frame, &grid, 0.0, 0.0);
        let pixel = |x: usize, y: usize| {
            let at = (y * frame.width + x) * 4;
            u32::from(frame.pixels[at]) << 16
                | u32::from(frame.pixels[at + 1]) << 8
                | u32::from(frame.pixels[at + 2])
        };
        assert_eq!(pixel(5, 10), BRIGHTEST);
        let amber = (13..24)
            .flat_map(|x| (0..26).map(move |y| (x, y)))
            .filter(|(x, y)| pixel(*x, *y) != FIELD)
            .count();
        assert!(amber > 10, "the letter drew {amber} pixels");
    }

    /// The fitting size keeps the canvas inside the frame on both axes.
    #[test]
    fn the_fitting_size_keeps_the_canvas_inside() {
        let size = fitting_size(1920.0, 1080.0, 108, 28);
        let painter = Painter::new(size);
        let (w, h) = painter.extent(108, 28);
        assert!(w <= 1920.5 && h <= 1080.5, "{w}x{h}");
    }

    /// A frame encodes as a PNG.
    #[test]
    fn a_frame_encodes_as_png() {
        let bytes = Frame::new(4, 4, FIELD).png().expect("the PNG");
        assert_eq!(&bytes[1..4], b"PNG");
    }
}
