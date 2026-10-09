//! Text shaping in Rust with bundled fonts, so a platform needs no text
//! engine callback to lay out a transcript.
//!
//! The one bundled face is Paper Mono's variable font (`crates/paper-mono`,
//! SIL Open Font License 1.1), the typeface every surface uses. [`FontSpec`]
//! binds measured and painted outlines to the same face and `wght` value.
//! Paper Mono has no italic, so italic text is drawn upright, and it is
//! fixed-pitch throughout, so code differs from prose only in turning off
//! contextual alternates (`calt`).
//!
//! Lines break at Unicode line-break opportunities (UAX #14, with CoreText's
//! break between a word's closing slash and a digit), greedily, as CoreText's
//! word wrapping does: trailing spaces hang past the width, a hard
//! line break ends a line, and a word longer than the line breaks between
//! grapheme clusters. Tabs advance to the next multiple of 28 points, as
//! CoreText's default tab stops do. A character the bundled faces lack is
//! measured as a platform fallback would roughly draw it: a full em for wide
//! characters and emoji, else 0.6 em. The ground-truth test in
//! `shape::tests` checks the breaks against CoreText's for the same fonts.
//!
//! An app may instead draw with faces of its own, such as the computer's
//! system fonts, by calling [`install_faces`] once at start: a face for text
//! and a face for code. Measuring ([`ShapingMeasurer`]) and painting then
//! use [`faces`], and [`FontSpec::of`] sends code to the second face and
//! sets the text face's optical size when it has an `opsz` axis.

use super::display::{Font, FontFamily, Weight};
use super::measure::{Line, MeasureRun, Measured, Measurer};
use swash::shape::ShapeContext;
use swash::text::cluster::Boundary;
use swash::{FontRef, Metrics};

/// The bundled faces, by [`FontSpec::face`]: Paper Mono's variable font.
pub const FACES: [&[u8]; 1] = [paper_mono::VARIABLE];

/// Faces an app installed with [`install_faces`].
struct Installed {
    faces: [&'static [u8]; 2],
    /// Whether each face has an `opsz` axis.
    optical: [bool; 2],
}

static INSTALLED: std::sync::OnceLock<Installed> = std::sync::OnceLock::new();

/// The faces this process measures and paints with, by [`FontSpec::face`]:
/// the bundled [`FACES`], or the text and code faces an app installed.
#[must_use]
pub fn faces() -> &'static [&'static [u8]] {
    INSTALLED
        .get()
        .map_or(&FACES[..], |installed| &installed.faces[..])
}

/// Draws every font in this process with `text` (prose) and `code`
/// (monospace) instead of the bundled faces. Call once at start, before
/// the first measurer or painter. Fails when a face is not a font this
/// shaper reads, or when faces were already installed.
pub fn install_faces(text: &'static [u8], code: &'static [u8]) -> Result<(), &'static str> {
    let mut optical = [false; 2];
    for (index, data) in [text, code].into_iter().enumerate() {
        let face = FontRef::from_index(data, 0).ok_or("not a font")?;
        optical[index] = face
            .variations()
            .any(|axis| axis.tag() == swash::tag_from_bytes(b"opsz"));
    }
    INSTALLED
        .set(Installed {
            faces: [text, code],
            optical,
        })
        .map_err(|_| "faces are already installed")
}

/// Distance between default tab stops, in points.
pub const TAB_INTERVAL: f64 = 28.0;

/// How to draw a display-list font with the bundled faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontSpec {
    /// Index into [`faces`]: 0 for text, 1 for code when an app installed
    /// its own faces.
    pub face: usize,
    pub size: f32,
    /// The `wght` axis value.
    pub weight: f32,
    /// The `opsz` axis value: the size, for an installed face with that
    /// axis; zero otherwise (Paper Mono has none).
    pub optical: f32,
    /// Whether contextual alternates (`calt`) are on.
    pub calt: bool,
}

impl FontSpec {
    pub fn of(font: Font) -> Self {
        let weight = match font.weight {
            Weight::Regular => 400.0,
            Weight::Medium => 500.0,
            Weight::Semibold => 600.0,
            Weight::Bold => 700.0,
        };
        let FontFamily::PaperMono = font.family;
        // Installed faces: code draws from the second; the bundled face
        // draws both.
        let face = usize::from(font.mono && faces().len() > 1);
        let optical = INSTALLED
            .get()
            .filter(|installed| installed.optical[face])
            .map_or(0.0, |_| font.size);
        Self {
            face,
            size: font.size,
            weight,
            optical,
            calt: !font.mono,
        }
    }
}

/// A [`Measurer`] that shapes with the bundled faces. Keep one per thread.
pub struct ShapingMeasurer {
    context: ShapeContext,
    fonts: Vec<FontRef<'static>>,
}

impl Default for ShapingMeasurer {
    fn default() -> Self {
        Self::new()
    }
}

/// One character of a paragraph.
struct Char {
    ch: char,
    /// UTF-16 offset.
    at16: u32,
    boundary: Boundary,
    /// Whether a grapheme cluster starts here.
    cluster: bool,
    /// Advance in points; a tab's is decided per line.
    advance: f64,
    /// Which run styles the character.
    run: usize,
}

impl ShapingMeasurer {
    pub fn new() -> Self {
        Self {
            context: ShapeContext::new(),
            fonts: faces()
                .iter()
                .map(|data| FontRef::from_index(data, 0).expect("a checked face"))
                .collect(),
        }
    }

    /// Font metrics in points: ascent, descent, and leading.
    fn metrics(&mut self, spec: &FontSpec) -> (f64, f64, f64) {
        let shaper = self.shaper(spec);
        // A shaper sized zero reports metrics per em.
        let metrics: Metrics = shaper.metrics();
        let scale = f64::from(spec.size);
        (
            f64::from(metrics.ascent) * scale,
            f64::from(metrics.descent) * scale,
            f64::from(metrics.leading) * scale,
        )
    }

    fn shaper(&mut self, spec: &FontSpec) -> swash::shape::Shaper<'_> {
        let mut variations = vec![("wght", spec.weight)];
        if spec.optical > 0.0 {
            variations.push(("opsz", spec.optical));
        }
        // Shape in font units; points are scaled in f64 below.
        self.context
            .builder(self.fonts[spec.face])
            .size(0.0)
            .variations(variations)
            .features([("calt", u16::from(spec.calt))])
            .build()
    }

    /// Shapes `chars[range]` (UTF-8 `text`) with `spec`, writing each
    /// cluster's advance to its first character.
    fn shape_run(&mut self, text: &str, chars: &mut [Char], spec: &FontSpec) {
        let upem = f64::from(self.fonts[spec.face].metrics(&[]).units_per_em.max(1));
        let scale = f64::from(spec.size) / upem;
        let size = f64::from(spec.size);
        // Byte offset of each character within `text`.
        let mut starts = Vec::with_capacity(chars.len() + 1);
        let mut at = 0;
        for c in chars.iter() {
            starts.push(at);
            at += c.ch.len_utf8();
        }
        let mut shaper = self.shaper(spec);
        shaper.add_str(text);
        let mut clusters: Vec<(usize, f64, bool)> = Vec::new();
        shaper.shape_with(|cluster| {
            let advance: f64 = cluster.glyphs.iter().map(|g| f64::from(g.advance)).sum();
            // A cluster with any glyph the face lacks is drawn by a fallback
            // face, as a whole: a precomposed letter the face lacks may
            // shape as a missing base and a combining mark the face has.
            let missing = cluster.glyphs.iter().any(|g| g.id == 0);
            clusters.push((cluster.source.start as usize, advance * scale, missing));
        });
        for (byte, advance, missing) in clusters {
            let Ok(index) = starts.binary_search(&byte) else {
                continue;
            };
            let c = &mut chars[index];
            c.cluster = true;
            c.advance = if missing {
                fallback_em(c.ch) * size
            } else {
                advance
            };
        }
    }
}

/// The width, in ems, a platform fallback face roughly gives a character
/// the bundled faces lack.
#[must_use]
pub fn fallback_em(ch: char) -> f64 {
    let wide = matches!(u32::from(ch),
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6
        | 0x1F000..=0x1FAFF | 0x20000..=0x3FFFD | 0x2600..=0x27BF);
    if wide { 1.0 } else { 0.6 }
}

/// Whitespace that hangs at the end of a line.
fn hangs(ch: char) -> bool {
    ch.is_whitespace() && ch != '\u{a0}'
}

fn hard_break(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r' | '\u{2028}' | '\u{2029}' | '\u{0b}' | '\u{0c}' | '\u{85}'
    )
}

impl Measurer for ShapingMeasurer {
    fn measure(&mut self, text: &str, runs: &[MeasureRun], width: Option<f32>) -> Option<Measured> {
        let mut chars: Vec<Char> = Vec::with_capacity(text.len());
        let mut at16 = 0u32;
        for ((_, boundary), ch) in swash::text::analyze(text.chars()).zip(text.chars()) {
            chars.push(Char {
                ch,
                at16,
                boundary,
                cluster: false,
                advance: 0.0,
                run: 0,
            });
            at16 += ch.len_utf16() as u32;
        }
        // CoreText's tailoring of UAX #14 breaks after a slash that ends a
        // word and before a digit, as in "models/2026-09-19".
        for index in 2..chars.len() {
            if matches!(chars[index].boundary, Boundary::None | Boundary::Word)
                && chars[index].ch.is_ascii_digit()
                && chars[index - 1].ch == '/'
                && chars[index - 2].ch.is_alphabetic()
            {
                chars[index].boundary = Boundary::Line;
            }
        }
        // CoreText also breaks before a slash that stands alone between
        // spaces, which UAX #14 (LB13) forbids, as in "probes / Jev".
        for index in 1..chars.len().saturating_sub(1) {
            if matches!(chars[index].boundary, Boundary::None | Boundary::Word)
                && chars[index].ch == '/'
                && chars[index - 1].ch == ' '
                && chars[index + 1].ch == ' '
            {
                chars[index].boundary = Boundary::Line;
            }
        }
        // UAX #14 LB15a: no break after an opening quotation mark and the
        // spaces that follow it, as in "« déjà vu »".
        for index in 1..chars.len() {
            if chars[index].boundary == Boundary::Line && chars[index - 1].ch == ' ' {
                let mut before = index - 1;
                while before > 0 && chars[before].ch == ' ' {
                    before -= 1;
                }
                if matches!(chars[before].ch, '«' | '‹' | '“' | '‘' | '‛' | '‟') {
                    chars[index].boundary = Boundary::None;
                }
            }
        }
        let length = at16;
        if runs.is_empty() || length == 0 {
            return (length == 0).then(Measured::default);
        }
        let specs: Vec<FontSpec> = runs.iter().map(|r| FontSpec::of(r.font)).collect();
        // Assign characters to runs; runs tile the paragraph in order.
        let mut first = 0;
        let mut spans = Vec::with_capacity(runs.len());
        for (index, run) in runs.iter().enumerate() {
            let begin = chars[first..].partition_point(|c| c.at16 < run.start16) + first;
            let end = chars[begin..].partition_point(|c| c.at16 < run.end16) + begin;
            for c in &mut chars[begin..end] {
                c.run = index;
            }
            spans.push((begin, end));
            first = end;
        }
        // Neighboring runs in one font shape together, as the platform's
        // engine shapes them, so kerning crosses a change of color.
        let mut index = 0;
        while index < runs.len() {
            let mut last = index;
            while last + 1 < runs.len()
                && specs[last + 1] == specs[index]
                && spans[last + 1].0 == spans[last].1
            {
                last += 1;
            }
            let (begin, end) = (spans[index].0, spans[last].1);
            let bytes: usize = chars[..begin].iter().map(|c| c.ch.len_utf8()).sum();
            let len: usize = chars[begin..end].iter().map(|c| c.ch.len_utf8()).sum();
            let slice = text.get(bytes..bytes + len)?;
            if !slice.is_empty() {
                self.shape_run(slice, &mut chars[begin..end], &specs[index]);
            }
            index = last + 1;
        }
        // Every character starts a cluster unless shaping joined it to one.
        if let Some(c) = chars.first_mut() {
            c.cluster = true;
        }
        let limit = width.map_or(f64::INFINITY, f64::from);
        let boundaries: Vec<u32> = runs.iter().skip(1).map(|r| r.start16).collect();
        let mut out = Measured::default();
        let mut start = 0;
        while start < chars.len() {
            let end = break_line(&chars, start, limit);
            // Metrics: the tallest font styling the line.
            let (mut ascent, mut descent, mut leading) = (0.0f64, 0.0f64, 0.0f64);
            let mut seen = vec![false; runs.len()];
            for c in &chars[start..end] {
                if !seen[c.run] {
                    seen[c.run] = true;
                    let (a, d, l) = self.metrics(&specs[c.run]);
                    ascent = ascent.max(a);
                    descent = descent.max(d);
                    leading = leading.max(l);
                }
            }
            // Width without trailing whitespace and the hard break.
            let mut x = 0.0;
            let mut visible = 0.0;
            let line16 = chars[start].at16;
            let end16 = chars.get(end).map_or(length, |c| c.at16);
            let mut inner = boundaries
                .iter()
                .copied()
                .filter(|b| *b > line16 && *b < end16)
                .peekable();
            for c in &chars[start..end] {
                while inner.peek().is_some_and(|b| *b <= c.at16) {
                    inner.next();
                    out.offsets.push(x as f32);
                }
                x += advance(c, x);
                if !hangs(c.ch) {
                    visible = x;
                }
            }
            for _ in inner {
                out.offsets.push(x as f32);
            }
            out.lines.push(Line {
                start16: line16,
                end16,
                width: visible as f32,
                ascent: ascent as f32,
                descent: descent as f32,
                leading: leading as f32,
            });
            start = end;
        }
        Some(out)
    }
}

/// A character's advance at pen position `x` from the line's start.
fn advance(c: &Char, x: f64) -> f64 {
    if c.ch == '\t' {
        (x / TAB_INTERVAL + 1e-9).floor() * TAB_INTERVAL + TAB_INTERVAL - x
    } else if hard_break(c.ch) {
        0.0
    } else {
        c.advance
    }
}

/// The end (exclusive character index) of the line starting at `start`.
fn break_line(chars: &[Char], start: usize, limit: f64) -> usize {
    let mut x = 0.0;
    let mut last_break = None;
    let mut index = start;
    while index < chars.len() {
        let c = &chars[index];
        if index > start {
            match c.boundary {
                Boundary::Mandatory => return index,
                Boundary::Line => last_break = Some(index),
                _ => {}
            }
        }
        let step = advance(c, x);
        if index > start && !hangs(c.ch) && !hard_break(c.ch) && x + step > limit + 1e-6 {
            if let Some(at) = last_break {
                return at;
            }
            // No break opportunity fits: break before this grapheme
            // cluster, keeping at least one on the line.
            let mut at = index;
            while at > start && !chars[at].cluster {
                at -= 1;
            }
            if at > start {
                return at;
            }
            let mut next = start + 1;
            while next < chars.len() && !chars[next].cluster {
                next += 1;
            }
            return next;
        }
        x += step;
        if hard_break(c.ch) {
            // "\r\n" stays on one line.
            if c.ch == '\r' && chars.get(index + 1).is_some_and(|n| n.ch == '\n') {
                return index + 2;
            }
            return index + 1;
        }
        index += 1;
    }
    chars.len()
}

#[cfg(test)]
mod tests;
