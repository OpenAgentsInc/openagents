//! Text: lines broken by Rust Native's shaper, glyphs painted with `swash`.
//!
//! [`Fonts::paragraph`] breaks a paragraph with
//! [`rust_native::layout::shape::ShapingMeasurer`], the same line breaker
//! the iOS transcript is checked against, and [`Fonts::draw`] shapes and
//! rasterizes each line with the face and variations
//! [`FontSpec`](rust_native::layout::shape::FontSpec) names, so the adapter
//! paints exactly the outlines it measured, in the faces
//! [`faces`](rust_native::layout::shape::faces) names (Paper Mono unless
//! the app installed its own). A character those faces lack is
//! painted from one of this computer's fonts ([`crate::fallback`]), centered
//! in the width the shaper measured for it.

use crate::canvas::Frame;
use rust_native::layout::display::{Font, Weight};
use rust_native::layout::shape::{FontSpec, ShapingMeasurer, faces, fallback_em};
use rust_native::layout::{MeasureRun, Measurer};
use rust_native::style::{Color, TextAlign};
use std::collections::HashMap;
use std::rc::Rc;
use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source};
use swash::shape::ShapeContext;
use swash::zeno::{Format, Vector};

/// A line height, in ems.
pub const LINE_EM: f32 = 1.4;

/// A paragraph broken into lines at one width.
#[derive(Clone, Debug, PartialEq)]
pub struct Paragraph {
    pub text: String,
    pub font: Font,
    pub lines: Vec<TextLine>,
    /// The widest line, in points.
    pub width: f32,
    /// Every line's height together, in points.
    pub height: f32,
    /// Baseline-to-baseline line spacing.
    pub line_height: f32,
}

/// One line of a paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    /// The line's text as a byte range, excluding hard line breaks.
    /// Display paragraphs trim trailing spaces; editable paragraphs retain them.
    pub start: usize,
    pub end: usize,
    /// The line's width, in points.
    pub width: f32,
}

impl Paragraph {
    /// The height of one line, in points.
    pub fn line_height(&self) -> f32 {
        self.line_height
    }
}

/// A rasterized glyph.
struct Glyph {
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    coverage: Vec<u8>,
}

type GlyphKey = (usize, u32, u32, u16, u8);

/// The shaper, the rasterizer, and their caches. Keep one per thread.
pub struct Fonts {
    measurer: ShapingMeasurer,
    shape: ShapeContext,
    scale: ScaleContext,
    faces: Vec<FontRef<'static>>,
    paragraphs: HashMap<(String, u64, u32, u32), Rc<Paragraph>>,
    paragraph_bytes: usize,
    advances: HashMap<(String, u64), f32>,
    advance_bytes: usize,
    glyphs: HashMap<GlyphKey, Option<Glyph>>,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

/// A font's cache key.
fn font_bits(font: Font) -> u64 {
    u64::from(font.size.to_bits())
        | (font.weight as u64) << 32
        | u64::from(font.italic) << 40
        | u64::from(font.mono) << 41
        | (font.family as u64) << 42
}

/// A regular or bold font at `size`.
pub fn font(size: f32, weight: Weight, mono: bool) -> Font {
    Font {
        family: Default::default(),
        size,
        weight,
        italic: false,
        mono,
    }
}

impl Fonts {
    pub fn new() -> Fonts {
        Fonts {
            measurer: ShapingMeasurer::new(),
            shape: ShapeContext::new(),
            scale: ScaleContext::new(),
            faces: faces()
                .iter()
                .map(|data| FontRef::from_index(data, 0).expect("a checked face"))
                .collect(),
            paragraphs: HashMap::new(),
            paragraph_bytes: 0,
            advances: HashMap::new(),
            advance_bytes: 0,
            glyphs: HashMap::new(),
        }
    }

    /// `text` in `font`, broken to fit `width` points, or only at hard line
    /// breaks when `width` is `None`.
    pub fn paragraph(&mut self, text: &str, font: Font, width: Option<f32>) -> Rc<Paragraph> {
        self.paragraph_with_line_height(text, font, width, None)
    }

    /// Cache exact line boxes alongside text, face, and wrap width.
    pub fn paragraph_with_line_height(
        &mut self,
        text: &str,
        font: Font,
        width: Option<f32>,
        line_height: Option<f32>,
    ) -> Rc<Paragraph> {
        let key = (
            text.to_string(),
            font_bits(font),
            width.map_or(u32::MAX, f32::to_bits),
            line_height.map_or(0, f32::to_bits),
        );
        if let Some(paragraph) = self.paragraphs.get(&key) {
            return paragraph.clone();
        }
        if self.paragraphs.len() >= 4_096 || self.paragraph_bytes + text.len() > 512 * 1024 {
            self.paragraphs.clear();
            self.paragraph_bytes = 0;
        }
        self.paragraph_bytes += text.len();
        let mut paragraph = self.break_lines(text, font, width);
        if let Some(height) = line_height.filter(|height| height.is_finite() && *height > 0.0) {
            paragraph.line_height = height;
            paragraph.height = paragraph.lines.len() as f32 * height;
        }
        let paragraph = Rc::new(paragraph);
        self.paragraphs.insert(key, paragraph.clone());
        paragraph
    }

    /// The editing pen position, including trailing spaces and tab stops.
    pub fn advance(&mut self, text: &str, font: Font) -> f32 {
        let key = (text.to_owned(), font_bits(font));
        if let Some(width) = self.advances.get(&key) {
            return *width;
        }
        let spec = FontSpec::of(font);
        let mut variations = vec![("wght", spec.weight)];
        if spec.optical > 0.0 {
            variations.push(("opsz", spec.optical));
        }
        let mut width = 0.0;
        for (index, part) in text.split('\t').enumerate() {
            if index > 0 {
                width = ((width / 28.0f32).floor() + 1.0) * 28.0;
            }
            let mut shaper = self
                .shape
                .builder(self.faces[spec.face])
                .size(spec.size)
                .variations(variations.clone())
                .features([("calt", u16::from(spec.calt))])
                .build();
            shaper.add_str(part);
            shaper.shape_with(|cluster| {
                width += cluster
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.advance)
                    .sum::<f32>();
            });
        }
        if self.advances.len() >= 4096 || self.advance_bytes + text.len() > 512 * 1024 {
            self.advances.clear();
            self.advance_bytes = 0;
        }
        self.advance_bytes += text.len();
        self.advances.insert(key, width);
        width
    }

    /// Find the closest grapheme boundary without shaping every prefix.
    pub fn caret_byte(&mut self, text: &str, font: Font, x: f32) -> usize {
        let boundaries = rust_native::selection::grapheme_boundaries(text);
        let mut left = 0;
        let mut right = boundaries.len();
        while left < right {
            let middle = left + (right - left) / 2;
            if self.advance(&text[..boundaries[middle]], font) < x {
                left = middle + 1;
            } else {
                right = middle;
            }
        }
        let after = left.min(boundaries.len() - 1);
        let before = after.saturating_sub(1);
        let a = (self.advance(&text[..boundaries[before]], font) - x).abs();
        let b = (self.advance(&text[..boundaries[after]], font) - x).abs();
        boundaries[if a <= b { before } else { after }]
    }

    /// Fit one line within its display width, preserving character boundaries.
    pub fn ellipsized(&mut self, text: &str, font: Font, width: f32) -> String {
        if self.advance(text, font) <= width {
            return text.into();
        }
        let room = width - self.advance("…", font);
        if room < 0.0 {
            return String::new();
        }
        let mut end = self.caret_byte(text, font, room);
        while end > 0 && self.advance(&text[..end], font) > room {
            end = text[..end]
                .char_indices()
                .next_back()
                .map_or(0, |(byte, _)| byte);
        }
        format!("{}…", &text[..end])
    }

    /// Wrapped editable lines retain spaces and an empty final line after Enter.
    pub fn editable_paragraph(&mut self, text: &str, font: Font, width: f32) -> Rc<Paragraph> {
        let mut paragraph = (*self.paragraph(text, font, Some(width))).clone();
        for index in 0..paragraph.lines.len() {
            let start = paragraph.lines[index].start;
            let end = paragraph
                .lines
                .get(index + 1)
                .map_or(text.len(), |line| line.start);
            let end = start
                + text[start..end]
                    .trim_end_matches(['\r', '\n', '\u{85}', '\u{2028}', '\u{2029}'])
                    .len();
            paragraph.lines[index].end = end;
        }
        if paragraph.lines.is_empty()
            || (text.ends_with(['\r', '\n', '\u{85}', '\u{2028}', '\u{2029}'])
                && paragraph
                    .lines
                    .last()
                    .is_some_and(|line| line.start != text.len()))
        {
            paragraph.lines.push(TextLine {
                start: text.len(),
                end: text.len(),
                width: 0.0,
            });
        }
        paragraph.height = paragraph.line_height() * paragraph.lines.len() as f32;
        Rc::new(paragraph)
    }

    fn break_lines(&mut self, text: &str, font: Font, width: Option<f32>) -> Paragraph {
        let length16: u32 = text.chars().map(|c| c.len_utf16() as u32).sum();
        let runs = [MeasureRun {
            font,
            start16: 0,
            end16: length16,
        }];
        let measured = self
            .measurer
            .measure(text, &runs, width.map(|w| w.max(1.0)))
            .unwrap_or_default();
        // UTF-16 offset to byte offset.
        let mut bytes = Vec::with_capacity(length16 as usize + 1);
        for (at, ch) in text.char_indices() {
            for _ in 0..ch.len_utf16() {
                bytes.push(at);
            }
        }
        bytes.push(text.len());
        let byte = |at16: u32| bytes[(at16 as usize).min(bytes.len() - 1)];
        let lines: Vec<TextLine> = measured
            .lines
            .iter()
            .map(|line| {
                let start = byte(line.start16);
                let end = start + text[start..byte(line.end16)].trim_end().len();
                TextLine {
                    start,
                    end,
                    width: line.width,
                }
            })
            .collect();
        let line_height = (font.size * LINE_EM).round();
        Paragraph {
            text: text.to_string(),
            font,
            width: lines.iter().map(|line| line.width).fold(0.0, f32::max),
            height: line_height * lines.len() as f32,
            line_height,
            lines,
        }
    }

    /// Paint a pre-positioned transcript run at its measured baseline.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_run(
        &mut self,
        frame: &mut Frame,
        text: &str,
        font: Font,
        x: f32,
        baseline: f32,
        scale: f32,
        color: Color,
    ) {
        let paragraph = self.paragraph(text, font, None);
        let spec = FontSpec::of(font);
        let metrics = self.faces[spec.face].metrics(&[]).scale(spec.size * scale);
        let offset = (paragraph.line_height() * scale - (metrics.ascent + metrics.descent)) / 2.0
            + metrics.ascent;
        self.draw(
            frame,
            &paragraph,
            x,
            baseline - offset,
            paragraph.width,
            TextAlign::Start,
            scale,
            color,
        );
    }

    /// Paint a positioned run using only foreground syntax spans.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_highlighted_run(
        &mut self,
        frame: &mut Frame,
        text: &str,
        font: Font,
        x: f32,
        baseline: f32,
        scale: f32,
        color: Color,
        spans: &[rust_native::syntax::Span],
        byte_offset: usize,
    ) {
        let paragraph = self.paragraph(text, font, None);
        let spec = FontSpec::of(font);
        let metrics = self.faces[spec.face].metrics(&[]).scale(spec.size * scale);
        let offset = (paragraph.line_height() * scale - (metrics.ascent + metrics.descent)) / 2.0
            + metrics.ascent;
        self.draw_colored(
            frame,
            &paragraph,
            x,
            baseline - offset,
            paragraph.width,
            TextAlign::Start,
            scale,
            color,
            spans,
            byte_offset,
        );
    }

    /// Paints `paragraph` with its top-left corner at `x`, `y` pixels, lines
    /// aligned within `width` points, at `scale` pixels a point.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        frame: &mut Frame,
        paragraph: &Paragraph,
        x: f32,
        y: f32,
        width: f32,
        align: TextAlign,
        scale: f32,
        color: Color,
    ) {
        self.draw_colored(frame, paragraph, x, y, width, align, scale, color, &[], 0);
    }

    /// Colors shaped clusters without splitting runs or changing their metrics.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_colored(
        &mut self,
        frame: &mut Frame,
        paragraph: &Paragraph,
        x: f32,
        y: f32,
        width: f32,
        align: TextAlign,
        scale: f32,
        color: Color,
        spans: &[rust_native::syntax::Span],
        byte_offset: usize,
    ) {
        let spec = FontSpec::of(paragraph.font);
        let size = spec.size * scale;
        let face = self.faces[spec.face];
        let mut variations = vec![("wght", spec.weight)];
        if spec.optical > 0.0 {
            variations.push(("opsz", spec.optical));
        }
        let metrics = face.metrics(&[]).scale(size);
        let line_height = paragraph.line_height() * scale;
        for (index, line) in paragraph.lines.iter().enumerate() {
            let text = &paragraph.text[line.start..line.end];
            if text.is_empty() {
                continue;
            }
            let offset = match align {
                TextAlign::Start => 0.0,
                TextAlign::Center => ((width - line.width) / 2.0).max(0.0),
                TextAlign::End => (width - line.width).max(0.0),
            } * scale;
            let top = y + index as f32 * line_height;
            if !frame.visible(crate::PxRect {
                x: 0.0,
                y: top - 4.0 * scale,
                w: frame.width as f32,
                h: line_height + 8.0 * scale,
            }) {
                continue;
            }
            let baseline =
                (top + (line_height - (metrics.ascent + metrics.descent)) / 2.0 + metrics.ascent)
                    .round();
            let mut shaper = self
                .shape
                .builder(face)
                .size(size)
                .variations(variations.clone())
                .features([("calt", u16::from(spec.calt))])
                .build();
            shaper.add_str(text);
            let mut placed = Vec::new();
            let mut pen = x + offset;
            shaper.shape_with(|cluster| {
                let byte = byte_offset + line.start + cluster.source.start as usize;
                let at = spans.partition_point(|span| span.end <= byte);
                let color = spans
                    .get(at)
                    .filter(|span| span.start <= byte)
                    .map_or(color, |span| {
                        let [red, green, blue, alpha] = span.foreground;
                        Color {
                            red,
                            green,
                            blue,
                            alpha,
                        }
                    });
                if cluster.glyphs.iter().any(|glyph| glyph.id == 0) {
                    // Paper Mono lacks this character: the shaper measured
                    // it at an estimate, so center a fallback glyph there.
                    let start = line.start + cluster.source.start as usize;
                    let ch = paragraph.text[start..].chars().next().unwrap_or('?');
                    let width = fallback_em(ch) as f32 * size;
                    if let Some((index, fallback)) = crate::fallback::face_for(ch) {
                        let id = fallback.charmap().map(ch);
                        let advance = fallback.glyph_metrics(&[]).scale(size).advance_width(id);
                        let gx = pen + ((width - advance) / 2.0).max(0.0);
                        placed.push((Some(index), id, gx, 0.0, color));
                    } else if let Some(glyph) = cluster.glyphs.first() {
                        placed.push((None, glyph.id, pen + glyph.x, glyph.y, color));
                    }
                    pen += width;
                    return;
                }
                for glyph in cluster.glyphs {
                    placed.push((None, glyph.id, pen + glyph.x, glyph.y, color));
                    pen += glyph.advance;
                }
            });
            for (fallback, id, gx, gy, color) in placed {
                let whole = gx.floor();
                let quarter = ((gx - whole) * 4.0).round() as u8 % 4;
                let (key_face, face) = match fallback {
                    Some(index) => (
                        self.faces.len() + index,
                        crate::fallback::face_for_index(index).unwrap_or(face),
                    ),
                    None => (spec.face, face),
                };
                let key = (key_face, spec.weight.to_bits(), size.to_bits(), id, quarter);
                if !self.glyphs.contains_key(&key) {
                    let mut scaler = self
                        .scale
                        .builder(face)
                        .size(size)
                        .hint(false)
                        .variations(if fallback.is_some() {
                            Vec::new()
                        } else {
                            variations.clone()
                        })
                        .build();
                    let image = Render::new(&[Source::Outline])
                        .format(Format::Alpha)
                        .offset(Vector::new(f32::from(quarter) / 4.0, 0.0))
                        .render(&mut scaler, id)
                        .map(|image| Glyph {
                            left: image.placement.left,
                            top: image.placement.top,
                            width: image.placement.width as usize,
                            height: image.placement.height as usize,
                            coverage: image.data,
                        });
                    if self.glyphs.len() > 8_192 {
                        self.glyphs.clear();
                    }
                    self.glyphs.insert(key, image);
                }
                let Some(Some(glyph)) = self.glyphs.get(&key) else {
                    continue;
                };
                let left = whole as i64 + i64::from(glyph.left);
                let top = (baseline - gy) as i64 - i64::from(glyph.top);
                for row in 0..glyph.height {
                    for col in 0..glyph.width {
                        let coverage = glyph.coverage[row * glyph.width + col];
                        if coverage > 0 {
                            // Paper Mono has no italic, so italic text
                            // is drawn upright.
                            frame.blend(
                                left + col as i64,
                                top + row as i64,
                                color,
                                f32::from(coverage) / 255.0,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_detail_ellipsis_fits_without_splitting_unicode() {
        let mut fonts = Fonts::new();
        let body = font(15.0, Weight::Regular, false);
        let text = "run a tool — 候補 path";
        assert_eq!(fonts.ellipsized(text, body, 1000.0), text);
        for width in [0.0, 20.0, 80.0, 120.0] {
            let fitted = fonts.ellipsized(text, body, width);
            assert!(fonts.advance(&fitted, body) <= width + 0.01);
            assert!(fitted.is_empty() || fitted.ends_with('…'));
        }
    }

    #[test]
    fn a_long_paragraph_wraps_and_a_short_one_does_not() {
        let mut fonts = Fonts::new();
        let body = font(15.0, Weight::Regular, false);
        let text = "Scan with the OpenAgents app on your phone.";
        let one = fonts.paragraph(text, body, Some(1_000.0));
        assert_eq!(one.lines.len(), 1);
        let narrow = fonts.paragraph(text, body, Some(120.0));
        assert!(narrow.lines.len() > 1);
        assert!(narrow.width <= 120.0);
        // The lines cover the words without their trailing spaces.
        let words: Vec<&str> = narrow
            .lines
            .iter()
            .map(|line| &narrow.text[line.start..line.end])
            .collect();
        assert_eq!(words.join(" "), text);
    }

    #[test]
    fn drawing_lights_pixels_in_the_color() {
        let mut fonts = Fonts::new();
        let paragraph = fonts.paragraph("Hello", font(20.0, Weight::Bold, false), None);
        let mut frame = Frame::new(120, 40, Color::rgb(0, 0, 0));
        fonts.draw(
            &mut frame,
            &paragraph,
            2.0,
            2.0,
            100.0,
            TextAlign::Start,
            1.0,
            Color::rgb(255, 255, 255),
        );
        let lit = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|(x, y)| frame.pixel(*x, *y) == [255, 255, 255])
            .count();
        assert!(lit > 30, "{lit} pixels");
    }
}

#[cfg(test)]
mod coverage {
    #[test]
    fn the_body_face_has_the_marks_screens_use() {
        let face = swash::FontRef::from_index(rust_native::layout::shape::FACES[0], 0).unwrap();
        for ch in ['·', '…', '—', '’', '●', '○', '→', '⌘'] {
            assert_ne!(face.charmap().map(ch), 0, "Paper Mono lacks {ch}");
        }
    }

    #[test]
    fn a_mark_paper_mono_lacks_paints_from_a_fallback_in_its_measured_width() {
        let mut fonts = super::Fonts::new();
        let font = super::font(14.0, rust_native::layout::display::Weight::Regular, false);
        let paragraph = fonts.paragraph("✓ ₿ ok", font, None);
        // The missing marks are measured at the shaper's estimates, the
        // check mark as a symbol 1 em wide and the bitcoin sign at 0.6 em,
        // beside Paper Mono's 0.606 em.
        assert!((paragraph.width - (4.0 * 0.606 + 1.0 + 0.6) * 14.0).abs() < 0.1);
        let mut frame = crate::canvas::Frame::transparent(120, 40);
        fonts.draw(
            &mut frame,
            &paragraph,
            0.0,
            0.0,
            120.0,
            rust_native::style::TextAlign::Start,
            1.0,
            rust_native::style::Color::rgb(255, 255, 255),
        );
        if cfg!(target_os = "macos") {
            // The check mark's cell is painted from a system face.
            let lit = (0..40)
                .flat_map(|y| (0..8).map(move |x| (x, y)))
                .filter(|(x, y)| frame.pixel(*x, *y) != [0, 0, 0])
                .count();
            assert!(lit > 5, "{lit} pixels");
        }
    }
}

#[cfg(test)]
mod refit {
    use super::*;

    #[test]
    fn a_paragraph_refits_its_own_width() {
        let mut fonts = Fonts::new();
        for (text, weight) in [
            ("Connect another phone", Weight::Bold),
            (
                "Let this phone open a terminal on this Mac",
                Weight::Regular,
            ),
            ("Can't scan? Copy a code instead", Weight::Regular),
        ] {
            let body = font(15.0, weight, false);
            let one = fonts.paragraph(text, body, None);
            let again = fonts.paragraph(text, body, Some(one.width + 1.0));
            assert_eq!(
                again.lines.len(),
                1,
                "{text}: {} then {:?}",
                one.width,
                again.lines
            );
        }
    }
}
