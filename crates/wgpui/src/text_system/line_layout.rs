//! Line layout types for text rendering.
//!
//! This module provides the core data structures for laid out text:
//! - [`LineLayout`] - A single line of shaped text
//! - [`WrappedLineLayout`] - A line with wrap boundaries
//! - [`ShapedRun`] - A run of glyphs with a single font
//! - [`ShapedGlyph`] - A positioned glyph
//! - [`FontRun`] - Font specification for a text segment

use crate::geometry::Point;
use smallvec::SmallVec;
use std::sync::Arc;

use super::LineWrapper;

/// A unique identifier for a loaded font.
pub type FontId = u32;

/// A unique identifier for a glyph within a font.
pub type GlyphId = u16;

/// A run of text with a single font.
///
/// Used to specify which font applies to which portion of text
/// when laying out mixed-font text.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FontRun {
    /// The length of this run in UTF-8 bytes.
    pub len: usize,
    /// The font ID for this run.
    pub font_id: FontId,
}

/// A laid out line of text.
///
/// Contains the shaped glyphs organized into runs, along with
/// metrics for the line.
#[derive(Default, Debug, Clone)]
pub struct LineLayout {
    /// The font size used for this line.
    pub font_size: f32,
    /// The total width of the line in pixels.
    pub width: f32,
    /// The ascent (height above baseline) of the line.
    pub ascent: f32,
    /// The descent (depth below baseline) of the line.
    pub descent: f32,
    /// The shaped runs that make up this line.
    pub runs: Vec<ShapedRun>,
    /// The length of the line in UTF-8 bytes.
    pub len: usize,
}

/// A run of shaped glyphs with a single font.
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// The font ID for this run.
    pub font_id: FontId,
    /// The positioned glyphs in this run.
    pub glyphs: Vec<ShapedGlyph>,
}

/// A single shaped glyph, ready to paint.
#[derive(Clone, Debug)]
pub struct ShapedGlyph {
    /// The glyph ID within the font.
    pub id: GlyphId,
    /// The position of this glyph relative to the line origin.
    pub position: Point,
    /// The index of this glyph in the original text (UTF-8 byte offset).
    pub index: usize,
    /// Whether this glyph is an emoji.
    pub is_emoji: bool,
}

impl LineLayout {
    /// Create a new empty line layout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the character index for the given x coordinate.
    ///
    /// Returns `None` if x is past the end of the line.
    pub fn index_for_x(&self, x: f32) -> Option<usize> {
        if x >= self.width {
            None
        } else {
            for run in self.runs.iter().rev() {
                for glyph in run.glyphs.iter().rev() {
                    if glyph.position.x <= x {
                        return Some(glyph.index);
                    }
                }
            }
            Some(0)
        }
    }

    /// Get the closest character boundary to the given x coordinate.
    ///
    /// This is useful for cursor positioning when clicking or using
    /// up/down arrow keys.
    pub fn closest_index_for_x(&self, x: f32) -> usize {
        let mut prev_index = 0;
        let mut prev_x = 0.0;

        for run in self.runs.iter() {
            for glyph in run.glyphs.iter() {
                if glyph.position.x >= x {
                    // Return the closer of this glyph or the previous
                    if glyph.position.x - x < x - prev_x {
                        return glyph.index;
                    } else {
                        return prev_index;
                    }
                }
                prev_index = glyph.index;
                prev_x = glyph.position.x;
            }
        }

        // Handle single-character line specially
        if self.len == 1 {
            if x > self.width / 2.0 {
                return 1;
            } else {
                return 0;
            }
        }

        self.len
    }

    /// Get the x position for the character at the given index.
    pub fn x_for_index(&self, index: usize) -> f32 {
        for run in &self.runs {
            for glyph in &run.glyphs {
                if glyph.index >= index {
                    return glyph.position.x;
                }
            }
        }
        self.width
    }

    /// Get the font ID at the given character index.
    pub fn font_id_for_index(&self, index: usize) -> Option<FontId> {
        for run in &self.runs {
            for glyph in &run.glyphs {
                if glyph.index >= index {
                    return Some(run.font_id);
                }
            }
        }
        None
    }

    /// Compute wrap boundaries for this line at the given width.
    ///
    /// Returns the positions where the line should wrap.
    pub fn compute_wrap_boundaries(
        &self,
        text: &str,
        wrap_width: f32,
        max_lines: Option<usize>,
    ) -> SmallVec<[WrapBoundary; 1]> {
        let mut boundaries = SmallVec::new();
        let mut first_non_whitespace_ix = None;
        let mut last_candidate_ix = None;
        let mut last_candidate_x = 0.0;
        let mut last_boundary = WrapBoundary {
            run_ix: 0,
            glyph_ix: 0,
        };
        let mut last_boundary_x = 0.0;
        let mut prev_ch = '\0';
        let mut glyphs = self
            .runs
            .iter()
            .enumerate()
            .flat_map(move |(run_ix, run)| {
                run.glyphs.iter().enumerate().map(move |(glyph_ix, glyph)| {
                    let character = text[glyph.index..].chars().next().unwrap_or('\0');
                    (
                        WrapBoundary { run_ix, glyph_ix },
                        character,
                        glyph.position.x,
                    )
                })
            })
            .peekable();

        while let Some((boundary, ch, x)) = glyphs.next() {
            if ch == '\n' {
                continue;
            }

            // Word boundary detection (mirrors LineWrapper::is_word_char)
            if LineWrapper::is_word_char(ch) {
                if prev_ch == ' ' && ch != ' ' && first_non_whitespace_ix.is_some() {
                    last_candidate_ix = Some(boundary);
                    last_candidate_x = x;
                }
            } else if ch != ' ' && first_non_whitespace_ix.is_some() {
                // CJK may not be space separated
                last_candidate_ix = Some(boundary);
                last_candidate_x = x;
            }

            if ch != ' ' && first_non_whitespace_ix.is_none() {
                first_non_whitespace_ix = Some(boundary);
            }

            let next_x = glyphs.peek().map_or(self.width, |(_, _, x)| *x);
            let width = next_x - last_boundary_x;

            if width > wrap_width && boundary > last_boundary {
                // Respect max_lines limit
                if let Some(max) = max_lines {
                    if boundaries.len() >= max - 1 {
                        break;
                    }
                }

                if let Some(candidate) = last_candidate_ix.take() {
                    last_boundary = candidate;
                    last_boundary_x = last_candidate_x;
                } else {
                    last_boundary = boundary;
                    last_boundary_x = x;
                }
                boundaries.push(last_boundary);
            }
            prev_ch = ch;
        }

        boundaries
    }
}

/// A line layout with wrap boundaries.
#[derive(Default, Debug, Clone)]
pub struct WrappedLineLayout {
    /// The underlying unwrapped line layout.
    pub unwrapped_layout: Arc<LineLayout>,
    /// The boundaries where the line wraps.
    pub wrap_boundaries: SmallVec<[WrapBoundary; 1]>,
    /// The width at which wrapping occurred.
    pub wrap_width: Option<f32>,
}

/// A position where a line wraps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct WrapBoundary {
    /// The index of the run just before the wrap.
    pub run_ix: usize,
    /// The index of the glyph just before the wrap.
    pub glyph_ix: usize,
}

impl WrappedLineLayout {
    /// Create a new wrapped line layout.
    pub fn new(
        unwrapped_layout: Arc<LineLayout>,
        text: &str,
        wrap_width: Option<f32>,
        max_lines: Option<usize>,
    ) -> Self {
        let wrap_boundaries = if let Some(width) = wrap_width {
            unwrapped_layout.compute_wrap_boundaries(text, width, max_lines)
        } else {
            SmallVec::new()
        };

        Self {
            unwrapped_layout,
            wrap_boundaries,
            wrap_width,
        }
    }

    /// The length of the underlying text in UTF-8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.unwrapped_layout.len
    }

    /// The width of this layout (wrapped width or line width).
    pub fn width(&self) -> f32 {
        self.wrap_width
            .unwrap_or(f32::MAX)
            .min(self.unwrapped_layout.width)
    }

    /// The total size of the wrapped text.
    pub fn size(&self, line_height: f32) -> (f32, f32) {
        (
            self.width(),
            line_height * (self.wrap_boundaries.len() + 1) as f32,
        )
    }

    /// The ascent of a line in this layout.
    pub fn ascent(&self) -> f32 {
        self.unwrapped_layout.ascent
    }

    /// The descent of a line in this layout.
    pub fn descent(&self) -> f32 {
        self.unwrapped_layout.descent
    }

    /// The wrap boundaries in this layout.
    pub fn wrap_boundaries(&self) -> &[WrapBoundary] {
        &self.wrap_boundaries
    }

    /// The font size of this layout.
    pub fn font_size(&self) -> f32 {
        self.unwrapped_layout.font_size
    }

    /// The runs in the underlying layout.
    pub fn runs(&self) -> &[ShapedRun] {
        &self.unwrapped_layout.runs
    }

    /// Get the character index for a position in wrapped text.
    ///
    /// Returns `Ok(index)` if within the text, `Err(index)` if outside.
    pub fn index_for_position(&self, position: Point, line_height: f32) -> Result<usize, usize> {
        self._index_for_position(position, line_height, false)
    }

    /// Get the closest character index for a position in wrapped text.
    pub fn closest_index_for_position(
        &self,
        position: Point,
        line_height: f32,
    ) -> Result<usize, usize> {
        self._index_for_position(position, line_height, true)
    }

    fn _index_for_position(
        &self,
        mut position: Point,
        line_height: f32,
        closest: bool,
    ) -> Result<usize, usize> {
        let wrapped_line_ix = (position.y / line_height) as usize;

        let (wrapped_line_start_index, wrapped_line_start_x) = if wrapped_line_ix > 0 {
            let Some(boundary) = self.wrap_boundaries.get(wrapped_line_ix - 1) else {
                return Err(0);
            };
            let run = &self.unwrapped_layout.runs[boundary.run_ix];
            let glyph = &run.glyphs[boundary.glyph_ix];
            (glyph.index, glyph.position.x)
        } else {
            (0, 0.0)
        };

        let (wrapped_line_end_index, wrapped_line_end_x) =
            if wrapped_line_ix < self.wrap_boundaries.len() {
                let boundary = self.wrap_boundaries[wrapped_line_ix];
                let run = &self.unwrapped_layout.runs[boundary.run_ix];
                let glyph = &run.glyphs[boundary.glyph_ix];
                (glyph.index, glyph.position.x)
            } else {
                (self.unwrapped_layout.len, self.unwrapped_layout.width)
            };

        position.x += wrapped_line_start_x;

        if position.x < wrapped_line_start_x {
            Err(wrapped_line_start_index)
        } else if position.x >= wrapped_line_end_x {
            Err(wrapped_line_end_index)
        } else if closest {
            Ok(self.unwrapped_layout.closest_index_for_x(position.x))
        } else {
            Ok(self.unwrapped_layout.index_for_x(position.x).unwrap_or(0))
        }
    }

    /// Get the pixel position for a character index.
    pub fn position_for_index(&self, index: usize, line_height: f32) -> Option<Point> {
        let mut line_start_ix = 0;
        let line_end_indices = self
            .wrap_boundaries
            .iter()
            .map(|boundary| {
                let run = &self.unwrapped_layout.runs[boundary.run_ix];
                let glyph = &run.glyphs[boundary.glyph_ix];
                glyph.index
            })
            .chain(std::iter::once(self.len()))
            .enumerate();

        for (ix, line_end_ix) in line_end_indices {
            let line_y = ix as f32 * line_height;
            if index < line_start_ix {
                break;
            } else if index > line_end_ix {
                line_start_ix = line_end_ix;
                continue;
            } else {
                let line_start_x = self.unwrapped_layout.x_for_index(line_start_ix);
                let x = self.unwrapped_layout.x_for_index(index) - line_start_x;
                return Some(Point::new(x, line_y));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_layout() -> LineLayout {
        LineLayout {
            font_size: 14.0,
            width: 100.0,
            ascent: 12.0,
            descent: 4.0,
            runs: vec![ShapedRun {
                font_id: 0,
                glyphs: vec![
                    ShapedGlyph {
                        id: 0,
                        position: Point::new(0.0, 0.0),
                        index: 0,
                        is_emoji: false,
                    },
                    ShapedGlyph {
                        id: 1,
                        position: Point::new(10.0, 0.0),
                        index: 1,
                        is_emoji: false,
                    },
                    ShapedGlyph {
                        id: 2,
                        position: Point::new(20.0, 0.0),
                        index: 2,
                        is_emoji: false,
                    },
                    ShapedGlyph {
                        id: 3,
                        position: Point::new(30.0, 0.0),
                        index: 3,
                        is_emoji: false,
                    },
                    ShapedGlyph {
                        id: 4,
                        position: Point::new(40.0, 0.0),
                        index: 4,
                        is_emoji: false,
                    },
                ],
            }],
            len: 5,
        }
    }

    #[test]
    fn test_index_for_x() {
        let layout = make_test_layout();

        assert_eq!(layout.index_for_x(0.0), Some(0));
        assert_eq!(layout.index_for_x(5.0), Some(0));
        assert_eq!(layout.index_for_x(10.0), Some(1));
        assert_eq!(layout.index_for_x(15.0), Some(1));
        assert_eq!(layout.index_for_x(25.0), Some(2));
        assert_eq!(layout.index_for_x(200.0), None);
    }

    #[test]
    fn test_closest_index_for_x() {
        let layout = make_test_layout();

        assert_eq!(layout.closest_index_for_x(0.0), 0);
        assert_eq!(layout.closest_index_for_x(4.0), 0);
        assert_eq!(layout.closest_index_for_x(6.0), 1);
        assert_eq!(layout.closest_index_for_x(10.0), 1);
    }

    #[test]
    fn test_x_for_index() {
        let layout = make_test_layout();

        assert_eq!(layout.x_for_index(0), 0.0);
        assert_eq!(layout.x_for_index(1), 10.0);
        assert_eq!(layout.x_for_index(4), 40.0);
        assert_eq!(layout.x_for_index(10), 100.0); // Past end returns width
    }

    #[test]
    fn test_font_run() {
        let run = FontRun {
            len: 10,
            font_id: 1,
        };
        assert_eq!(run.len, 10);
        assert_eq!(run.font_id, 1);
    }

    #[test]
    fn test_wrap_boundary_ordering() {
        let a = WrapBoundary {
            run_ix: 0,
            glyph_ix: 5,
        };
        let b = WrapBoundary {
            run_ix: 0,
            glyph_ix: 10,
        };
        let c = WrapBoundary {
            run_ix: 1,
            glyph_ix: 0,
        };

        assert!(a < b);
        assert!(b < c);
    }
}
