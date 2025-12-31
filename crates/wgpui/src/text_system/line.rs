//! Paintable line types with decoration support.
//!
//! This module provides:
//! - [`DecorationRun`] - Text decorations (color, underline, strikethrough, background)
//! - [`ShapedLine`] - A single line ready to paint
//! - [`WrappedLine`] - A wrapped line ready to paint
//! - [`TextAlign`] - Text alignment options

use crate::color::Hsla;
use crate::geometry::Point;
use smallvec::SmallVec;
use std::sync::Arc;

use super::{LineLayout, WrapBoundary, WrappedLineLayout};

/// Text alignment options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Align text to the left (default).
    #[default]
    Left,
    /// Center text horizontally.
    Center,
    /// Align text to the right.
    Right,
}

/// Style for underline decorations.
#[derive(Debug, Clone, PartialEq)]
pub struct UnderlineStyle {
    /// The color of the underline (uses text color if None).
    pub color: Option<Hsla>,
    /// The thickness of the underline in pixels.
    pub thickness: f32,
    /// Whether to use a wavy underline (for spell check, etc.).
    pub wavy: bool,
}

impl Default for UnderlineStyle {
    fn default() -> Self {
        Self {
            color: None,
            thickness: 1.0,
            wavy: false,
        }
    }
}

impl UnderlineStyle {
    /// Create a simple underline with the given color.
    pub fn new(color: Hsla) -> Self {
        Self {
            color: Some(color),
            thickness: 1.0,
            wavy: false,
        }
    }

    /// Create a wavy underline (for spell check indicators).
    pub fn wavy(color: Hsla) -> Self {
        Self {
            color: Some(color),
            thickness: 1.0,
            wavy: true,
        }
    }
}

/// Style for strikethrough decorations.
#[derive(Debug, Clone, PartialEq)]
pub struct StrikethroughStyle {
    /// The color of the strikethrough (uses text color if None).
    pub color: Option<Hsla>,
    /// The thickness of the strikethrough in pixels.
    pub thickness: f32,
}

impl Default for StrikethroughStyle {
    fn default() -> Self {
        Self {
            color: None,
            thickness: 1.0,
        }
    }
}

impl StrikethroughStyle {
    /// Create a strikethrough with the given color.
    pub fn new(color: Hsla) -> Self {
        Self {
            color: Some(color),
            thickness: 1.0,
        }
    }
}

/// A run of text with consistent decoration.
///
/// Decoration runs specify styling that applies to a contiguous
/// range of text within a line.
#[derive(Debug, Clone)]
pub struct DecorationRun {
    /// The length of this run in UTF-8 bytes.
    pub len: u32,
    /// The text color for this run.
    pub color: Hsla,
    /// Optional background color for this run.
    pub background_color: Option<Hsla>,
    /// Optional underline style.
    pub underline: Option<UnderlineStyle>,
    /// Optional strikethrough style.
    pub strikethrough: Option<StrikethroughStyle>,
}

impl DecorationRun {
    /// Create a new decoration run with just a color.
    pub fn new(len: u32, color: Hsla) -> Self {
        Self {
            len,
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    /// Add a background color to this run.
    pub fn with_background(mut self, color: Hsla) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Add an underline to this run.
    pub fn with_underline(mut self, style: UnderlineStyle) -> Self {
        self.underline = Some(style);
        self
    }

    /// Add a strikethrough to this run.
    pub fn with_strikethrough(mut self, style: StrikethroughStyle) -> Self {
        self.strikethrough = Some(style);
        self
    }
}

/// A shaped line of text ready to paint.
///
/// Contains the layout and decoration information for a single
/// line of text without wrapping.
#[derive(Clone, Debug)]
pub struct ShapedLine {
    /// The underlying line layout.
    pub layout: Arc<LineLayout>,
    /// The original text.
    pub text: String,
    /// Decoration runs for styling.
    pub decoration_runs: SmallVec<[DecorationRun; 32]>,
}

impl Default for ShapedLine {
    fn default() -> Self {
        Self {
            layout: Arc::new(LineLayout::default()),
            text: String::new(),
            decoration_runs: SmallVec::new(),
        }
    }
}

impl ShapedLine {
    /// Create a new shaped line.
    pub fn new(
        layout: Arc<LineLayout>,
        text: impl Into<String>,
        decoration_runs: impl Into<SmallVec<[DecorationRun; 32]>>,
    ) -> Self {
        Self {
            layout,
            text: text.into(),
            decoration_runs: decoration_runs.into(),
        }
    }

    /// Create a shaped line with a single color.
    pub fn with_color(layout: Arc<LineLayout>, text: impl Into<String>, color: Hsla) -> Self {
        let text = text.into();
        let len = text.len() as u32;
        Self {
            layout,
            text,
            decoration_runs: smallvec::smallvec![DecorationRun::new(len, color)],
        }
    }

    /// The length of the line in UTF-8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.layout.len
    }

    /// The width of the line.
    pub fn width(&self) -> f32 {
        self.layout.width
    }

    /// The ascent of the line.
    pub fn ascent(&self) -> f32 {
        self.layout.ascent
    }

    /// The descent of the line.
    pub fn descent(&self) -> f32 {
        self.layout.descent
    }

    /// Get paint information for this line.
    ///
    /// Returns an iterator of glyphs with their positions, colors, and decorations.
    pub fn paint_info(
        &self,
        origin: Point,
        line_height: f32,
        align: TextAlign,
        align_width: Option<f32>,
    ) -> LinePaintInfo {
        LinePaintInfo::new(
            origin,
            &self.layout,
            line_height,
            align,
            align_width,
            &self.decoration_runs,
            &[],
        )
    }
}

/// A wrapped line of text ready to paint.
///
/// Contains the layout, wrap boundaries, and decoration information
/// for a line of text that may span multiple visual lines.
#[derive(Clone, Debug)]
pub struct WrappedLine {
    /// The underlying wrapped layout.
    pub layout: Arc<WrappedLineLayout>,
    /// The original text.
    pub text: String,
    /// Decoration runs for styling.
    pub decoration_runs: SmallVec<[DecorationRun; 32]>,
}

impl Default for WrappedLine {
    fn default() -> Self {
        Self {
            layout: Arc::new(WrappedLineLayout::default()),
            text: String::new(),
            decoration_runs: SmallVec::new(),
        }
    }
}

impl WrappedLine {
    /// Create a new wrapped line.
    pub fn new(
        layout: Arc<WrappedLineLayout>,
        text: impl Into<String>,
        decoration_runs: impl Into<SmallVec<[DecorationRun; 32]>>,
    ) -> Self {
        Self {
            layout,
            text: text.into(),
            decoration_runs: decoration_runs.into(),
        }
    }

    /// Create a wrapped line with a single color.
    pub fn with_color(
        layout: Arc<WrappedLineLayout>,
        text: impl Into<String>,
        color: Hsla,
    ) -> Self {
        let text = text.into();
        let len = text.len() as u32;
        Self {
            layout,
            text,
            decoration_runs: smallvec::smallvec![DecorationRun::new(len, color)],
        }
    }

    /// The length of the underlying text in UTF-8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.layout.len()
    }

    /// The number of visual lines after wrapping.
    pub fn line_count(&self) -> usize {
        self.layout.wrap_boundaries.len() + 1
    }

    /// Get paint information for this line.
    pub fn paint_info(
        &self,
        origin: Point,
        line_height: f32,
        align: TextAlign,
        align_width: Option<f32>,
    ) -> LinePaintInfo {
        LinePaintInfo::new(
            origin,
            &self.layout.unwrapped_layout,
            line_height,
            align,
            align_width.or(self.layout.wrap_width),
            &self.decoration_runs,
            &self.layout.wrap_boundaries,
        )
    }
}

/// Information needed to paint a line of text.
///
/// This struct collects all the data needed to render glyphs
/// and decorations for a line.
#[derive(Debug)]
pub struct LinePaintInfo {
    /// The origin point for the line.
    pub origin: Point,
    /// The line height.
    pub line_height: f32,
    /// Text alignment.
    pub align: TextAlign,
    /// Width for alignment (if different from layout width).
    pub align_width: f32,
    /// Baseline offset from origin.
    pub baseline_offset: Point,
    /// Wrap boundaries (empty for unwrapped lines).
    pub wrap_boundaries: Vec<WrapBoundary>,
    /// Glyph paint entries.
    pub glyphs: Vec<GlyphPaintEntry>,
    /// Underline segments to paint.
    pub underlines: Vec<DecorationSegment>,
    /// Strikethrough segments to paint.
    pub strikethroughs: Vec<DecorationSegment>,
    /// Background segments to paint.
    pub backgrounds: Vec<BackgroundSegment>,
}

/// A single glyph ready to paint.
#[derive(Debug, Clone)]
pub struct GlyphPaintEntry {
    /// Position to paint the glyph.
    pub position: Point,
    /// Font ID for the glyph.
    pub font_id: super::FontId,
    /// Glyph ID within the font.
    pub glyph_id: super::GlyphId,
    /// Font size.
    pub font_size: f32,
    /// Color to paint the glyph.
    pub color: Hsla,
    /// Whether this is an emoji glyph.
    pub is_emoji: bool,
}

/// A decoration segment (underline or strikethrough).
#[derive(Debug, Clone)]
pub struct DecorationSegment {
    /// Start position.
    pub start: Point,
    /// Width of the segment.
    pub width: f32,
    /// Color of the decoration.
    pub color: Hsla,
    /// Thickness of the decoration.
    pub thickness: f32,
    /// Whether wavy (for underlines).
    pub wavy: bool,
}

/// A background segment.
#[derive(Debug, Clone)]
pub struct BackgroundSegment {
    /// Position of the background.
    pub position: Point,
    /// Width of the background.
    pub width: f32,
    /// Height of the background.
    pub height: f32,
    /// Color of the background.
    pub color: Hsla,
}

impl LinePaintInfo {
    /// Create paint info for a line.
    fn new(
        origin: Point,
        layout: &LineLayout,
        line_height: f32,
        align: TextAlign,
        align_width: Option<f32>,
        decoration_runs: &[DecorationRun],
        wrap_boundaries: &[WrapBoundary],
    ) -> Self {
        let align_width = align_width.unwrap_or(layout.width);
        let padding_top = (line_height - layout.ascent - layout.descent) / 2.0;
        let baseline_offset = Point::new(0.0, padding_top + layout.ascent);

        let mut info = Self {
            origin,
            line_height,
            align,
            align_width,
            baseline_offset,
            wrap_boundaries: wrap_boundaries.to_vec(),
            glyphs: Vec::new(),
            underlines: Vec::new(),
            strikethroughs: Vec::new(),
            backgrounds: Vec::new(),
        };

        info.collect_paint_entries(layout, decoration_runs, wrap_boundaries);
        info
    }

    fn collect_paint_entries(
        &mut self,
        layout: &LineLayout,
        decoration_runs: &[DecorationRun],
        wrap_boundaries: &[WrapBoundary],
    ) {
        let mut decoration_runs_iter = decoration_runs.iter();
        let mut wraps = wrap_boundaries.iter().peekable();
        let mut run_end = 0;
        let mut color = Hsla::white();
        let mut current_underline: Option<(Point, UnderlineStyle)> = None;
        let mut current_strikethrough: Option<(Point, StrikethroughStyle)> = None;
        let mut current_background: Option<(Point, Hsla)> = None;

        let mut glyph_origin = Point::new(
            self.aligned_origin_x(0.0, layout, wraps.peek()),
            self.origin.y,
        );
        let mut prev_glyph_position = Point::default();

        for (run_ix, run) in layout.runs.iter().enumerate() {
            for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
                glyph_origin.x += glyph.position.x - prev_glyph_position.x;

                // Handle wrap boundary
                if wraps.peek() == Some(&&WrapBoundary { run_ix, glyph_ix }) {
                    wraps.next();

                    // Finish current decorations at wrap
                    self.finish_underline(&mut current_underline, glyph_origin.x, layout);
                    self.finish_strikethrough(&mut current_strikethrough, glyph_origin.x, layout);
                    self.finish_background(&mut current_background, glyph_origin.x);

                    // Move to next line
                    glyph_origin.x = self.aligned_origin_x(glyph.position.x, layout, wraps.peek());
                    glyph_origin.y += self.line_height;

                    // Restart decorations on new line
                    if current_underline.is_some() {
                        if let Some((ref mut pos, _)) = current_underline {
                            pos.x = glyph_origin.x;
                            pos.y += self.line_height;
                        }
                    }
                    if current_strikethrough.is_some() {
                        if let Some((ref mut pos, _)) = current_strikethrough {
                            pos.x = glyph_origin.x;
                            pos.y += self.line_height;
                        }
                    }
                    if current_background.is_some() {
                        if let Some((ref mut pos, _)) = current_background {
                            pos.x = glyph_origin.x;
                            pos.y += self.line_height;
                        }
                    }
                }

                prev_glyph_position = glyph.position;

                // Update decoration state
                if glyph.index >= run_end {
                    if let Some(style_run) = decoration_runs_iter.next() {
                        // Finish decorations that changed
                        if let Some((_, ref underline_style)) = current_underline {
                            if style_run.underline.as_ref() != Some(underline_style) {
                                self.finish_underline(
                                    &mut current_underline,
                                    glyph_origin.x,
                                    layout,
                                );
                            }
                        }
                        if let Some((_, ref strike_style)) = current_strikethrough {
                            if style_run.strikethrough.as_ref() != Some(strike_style) {
                                self.finish_strikethrough(
                                    &mut current_strikethrough,
                                    glyph_origin.x,
                                    layout,
                                );
                            }
                        }
                        if let Some((_, ref bg_color)) = current_background {
                            if style_run.background_color.as_ref() != Some(bg_color) {
                                self.finish_background(&mut current_background, glyph_origin.x);
                            }
                        }

                        // Start new decorations
                        if let Some(ref underline) = style_run.underline {
                            if current_underline.is_none() {
                                let underline_y = glyph_origin.y
                                    + self.baseline_offset.y
                                    + (layout.descent * 0.618);
                                current_underline = Some((
                                    Point::new(glyph_origin.x, underline_y),
                                    underline.clone(),
                                ));
                            }
                        }
                        if let Some(ref strike) = style_run.strikethrough {
                            if current_strikethrough.is_none() {
                                let strike_y = glyph_origin.y
                                    + ((layout.ascent * 0.5 + self.baseline_offset.y) * 0.5);
                                current_strikethrough =
                                    Some((Point::new(glyph_origin.x, strike_y), strike.clone()));
                            }
                        }
                        if let Some(bg_color) = style_run.background_color {
                            if current_background.is_none() {
                                current_background =
                                    Some((Point::new(glyph_origin.x, glyph_origin.y), bg_color));
                            }
                        }

                        run_end += style_run.len as usize;
                        color = style_run.color;
                    }
                }

                // Add glyph to paint list
                self.glyphs.push(GlyphPaintEntry {
                    position: Point::new(
                        glyph_origin.x + self.baseline_offset.x,
                        glyph_origin.y + self.baseline_offset.y + glyph.position.y,
                    ),
                    font_id: run.font_id,
                    glyph_id: glyph.id,
                    font_size: layout.font_size,
                    color,
                    is_emoji: glyph.is_emoji,
                });
            }
        }

        // Finish any remaining decorations
        let end_x = glyph_origin.x + (layout.width - prev_glyph_position.x);
        self.finish_underline(&mut current_underline, end_x, layout);
        self.finish_strikethrough(&mut current_strikethrough, end_x, layout);
        self.finish_background(&mut current_background, end_x);
    }

    fn aligned_origin_x(
        &self,
        last_glyph_x: f32,
        layout: &LineLayout,
        wrap_boundary: Option<&&WrapBoundary>,
    ) -> f32 {
        let end_of_line = if let Some(WrapBoundary { run_ix, glyph_ix }) = wrap_boundary {
            layout.runs[*run_ix].glyphs[*glyph_ix].position.x
        } else {
            layout.width
        };

        let line_width = end_of_line - last_glyph_x;

        match self.align {
            TextAlign::Left => self.origin.x,
            TextAlign::Center => (self.origin.x * 2.0 + self.align_width - line_width) / 2.0,
            TextAlign::Right => self.origin.x + self.align_width - line_width,
        }
    }

    fn finish_underline(
        &mut self,
        current: &mut Option<(Point, UnderlineStyle)>,
        end_x: f32,
        _layout: &LineLayout,
    ) {
        if let Some((start, style)) = current.take() {
            let width = end_x - start.x;
            if width > 0.0 {
                self.underlines.push(DecorationSegment {
                    start,
                    width,
                    color: style.color.unwrap_or(Hsla::white()),
                    thickness: style.thickness,
                    wavy: style.wavy,
                });
            }
        }
    }

    fn finish_strikethrough(
        &mut self,
        current: &mut Option<(Point, StrikethroughStyle)>,
        end_x: f32,
        _layout: &LineLayout,
    ) {
        if let Some((start, style)) = current.take() {
            let width = end_x - start.x;
            if width > 0.0 {
                self.strikethroughs.push(DecorationSegment {
                    start,
                    width,
                    color: style.color.unwrap_or(Hsla::white()),
                    thickness: style.thickness,
                    wavy: false,
                });
            }
        }
    }

    fn finish_background(&mut self, current: &mut Option<(Point, Hsla)>, end_x: f32) {
        if let Some((start, color)) = current.take() {
            let width = end_x - start.x;
            if width > 0.0 {
                self.backgrounds.push(BackgroundSegment {
                    position: start,
                    width,
                    height: self.line_height,
                    color,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper colors for testing
    fn red() -> Hsla {
        Hsla::new(0.0, 1.0, 0.5, 1.0)
    }

    fn green() -> Hsla {
        Hsla::new(120.0 / 360.0, 1.0, 0.5, 1.0)
    }

    fn blue() -> Hsla {
        Hsla::new(240.0 / 360.0, 1.0, 0.5, 1.0)
    }

    #[test]
    fn test_text_align() {
        assert_eq!(TextAlign::default(), TextAlign::Left);
    }

    #[test]
    fn test_underline_style() {
        let style = UnderlineStyle::new(red());
        assert!(style.color.is_some());
        assert_eq!(style.thickness, 1.0);
        assert!(!style.wavy);

        let wavy = UnderlineStyle::wavy(green());
        assert!(wavy.wavy);
    }

    #[test]
    fn test_strikethrough_style() {
        let style = StrikethroughStyle::new(blue());
        assert!(style.color.is_some());
        assert_eq!(style.thickness, 1.0);
    }

    #[test]
    fn test_decoration_run() {
        let run = DecorationRun::new(10, Hsla::white())
            .with_background(Hsla::black())
            .with_underline(UnderlineStyle::default())
            .with_strikethrough(StrikethroughStyle::default());

        assert_eq!(run.len, 10);
        assert!(run.background_color.is_some());
        assert!(run.underline.is_some());
        assert!(run.strikethrough.is_some());
    }

    #[test]
    fn test_shaped_line_default() {
        let line = ShapedLine::default();
        assert_eq!(line.len(), 0);
        assert!(line.text.is_empty());
    }

    #[test]
    fn test_wrapped_line_default() {
        let line = WrappedLine::default();
        assert_eq!(line.len(), 0);
        assert_eq!(line.line_count(), 1);
    }
}
