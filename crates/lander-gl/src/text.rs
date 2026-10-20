//! Two words, filled at the device size they are drawn at.
//!
//! The outlines are baked from the font at build time (`src/glyphs.rs`);
//! here they become one coverage bitmap per word, so the page downloads
//! no font and every pixel is computed for the screen it lands on.

use ab_glyph_rasterizer::{Rasterizer, point};
use alloc::vec;
use alloc::vec::Vec;
use libm::{ceilf, floorf};

/// One shaped word: its outline in font units, y up, from the pen at the
/// baseline. `path` is a flat list of commands: `0 x y` moves, `1 x y`
/// draws a line, `2 cx cy x y` a quadratic curve, `3 c1x c1y c2x c2y x y` a
/// cubic one, and `4` closes the contour.
pub struct Run {
    pub advance: f32,
    /// `[x_min, y_min, x_max, y_max]` of the ink, in font units.
    pub bounds: [f32; 4],
    pub path: &'static [i16],
}

/// A filled word: its coverage, one byte a pixel, row 0 at the top, and
/// where its top-left pixel sits from the pen at the baseline.
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    /// The bitmap's left edge from the pen, in whole pixels.
    pub left: i32,
    /// The bitmap's top edge from the baseline, in whole pixels (negative:
    /// above it).
    pub top: i32,
    pub coverage: Vec<u8>,
}

impl Run {
    /// Fills the word at `scale` pixels a font unit, with the pen on a
    /// whole pixel. One pixel of margin keeps the anti-aliased edge.
    pub fn fill(&self, scale: f32) -> Bitmap {
        let [x0, y0, x1, y1] = self.bounds;
        let left = floorf(x0 * scale) as i32 - 1;
        let right = ceilf(x1 * scale) as i32 + 1;
        let top = -(ceilf(y1 * scale) as i32) - 1;
        let bottom = -(floorf(y0 * scale) as i32) + 1;
        let width = (right - left).max(1) as usize;
        let height = (bottom - top).max(1) as usize;
        let mut raster = Rasterizer::new(width, height);
        let at = |x: i16, y: i16| {
            point(
                x as f32 * scale - left as f32,
                -(y as f32) * scale - top as f32,
            )
        };
        let path = self.path;
        let (mut start, mut last) = (point(0.0, 0.0), point(0.0, 0.0));
        let mut i = 0;
        while i < path.len() {
            match path[i] {
                0 => {
                    start = at(path[i + 1], path[i + 2]);
                    last = start;
                    i += 3;
                }
                1 => {
                    let p = at(path[i + 1], path[i + 2]);
                    raster.draw_line(last, p);
                    last = p;
                    i += 3;
                }
                2 => {
                    let (c, p) = (at(path[i + 1], path[i + 2]), at(path[i + 3], path[i + 4]));
                    raster.draw_quad(last, c, p);
                    last = p;
                    i += 5;
                }
                3 => {
                    let c1 = at(path[i + 1], path[i + 2]);
                    let c2 = at(path[i + 3], path[i + 4]);
                    let p = at(path[i + 5], path[i + 6]);
                    raster.draw_cubic(last, c1, c2, p);
                    last = p;
                    i += 7;
                }
                _ => {
                    if last != start {
                        raster.draw_line(last, start);
                    }
                    last = start;
                    i += 1;
                }
            }
        }
        let mut coverage = vec![0u8; width * height];
        raster.for_each_pixel(|index, alpha| {
            coverage[index] = (alpha.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        });
        Bitmap {
            width,
            height,
            left,
            top,
            coverage,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::glyphs;

    /// Both words fill with ink inside their margin, at phone and desktop
    /// sizes.
    #[test]
    fn the_words_fill_inside_their_bitmaps() {
        for run in [&glyphs::TITLE, &glyphs::LABEL] {
            for px in [17.0, 34.0, 96.0, 256.0] {
                let bitmap = run.fill(px / glyphs::UNITS_PER_EM);
                let ink = bitmap.coverage.iter().filter(|a| **a > 128).count();
                assert!(ink > bitmap.width * bitmap.height / 12, "{px}: {ink}");
                let edge = bitmap.coverage[..bitmap.width].iter().all(|a| *a == 0);
                assert!(edge, "the top row stays clear at {px}");
            }
        }
    }
}
