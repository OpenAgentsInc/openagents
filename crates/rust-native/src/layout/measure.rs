//! Text measurement through the platform, with a cache keyed by paragraph,
//! fonts, and width.
//!
//! Stage 1 breaks lines with the platform's text engine (CoreText on iOS), so
//! the adapter draws exactly the lines it measured. Rust decides everything
//! else: which text goes in which paragraph, fonts, spacing, and positions.

use super::display::Font;
use std::collections::HashMap;
use std::sync::Arc;

/// One styled range of a paragraph, in UTF-16 code units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureRun {
    pub font: Font,
    pub start16: u32,
    pub end16: u32,
}

/// One line the platform broke. `start16..end16` covers the line's text,
/// including trailing whitespace and a hard line break.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Line {
    pub start16: u32,
    pub end16: u32,
    /// The typographic width without trailing whitespace.
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
}

/// A measured paragraph.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measured {
    pub lines: Vec<Line>,
    /// For each line in order, the x offset of every run boundary strictly
    /// inside it (`start16 < boundary < end16`), in increasing boundary
    /// order. A run boundary is the `start16` of every run after the first.
    pub offsets: Vec<f32>,
}

/// Breaks and measures styled paragraphs. `width` is the wrap width in
/// points; `None` breaks only at hard line breaks. The adapter must draw a
/// line's text with the same fonts it measured with.
pub trait Measurer {
    fn measure(&mut self, text: &str, runs: &[MeasureRun], width: Option<f32>) -> Option<Measured>;
}

/// The run boundaries strictly inside `start..end`.
pub(crate) fn inner_boundaries(
    runs: &[MeasureRun],
    start: u32,
    end: u32,
) -> impl Iterator<Item = u32> + '_ {
    runs.iter()
        .skip(1)
        .map(|run| run.start16)
        .filter(move |b| *b > start && *b < end)
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: Box<str>,
    runs: Box<[(u64, u32, u32)]>,
    width: u32,
}

/// Measurements by (text, fonts, width). Rows that repeat a paragraph, and
/// rows laid out again at the same width, never call the platform.
#[derive(Default)]
pub struct MeasureCache {
    entries: HashMap<Key, Arc<Measured>>,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
}

/// The most measurements kept before the cache starts over.
const MAX_ENTRIES: usize = 60_000;

impl MeasureCache {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn measure(
        &mut self,
        measurer: &mut dyn Measurer,
        text: &str,
        runs: &[MeasureRun],
        width: Option<f32>,
    ) -> Arc<Measured> {
        let key = Key {
            text: text.into(),
            runs: runs
                .iter()
                .map(|r| (r.font.bits(), r.start16, r.end16))
                .collect(),
            width: width.map_or(u32::MAX, f32::to_bits),
        };
        if let Some(found) = self.entries.get(&key) {
            self.hits += 1;
            return found.clone();
        }
        self.misses += 1;
        let measured = measurer
            .measure(text, runs, width)
            .filter(|m| valid(m, text, runs))
            .unwrap_or_else(|| estimate(text, runs, width));
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.clear();
        }
        let measured = Arc::new(measured);
        self.entries.insert(key, measured.clone());
        measured
    }
}

/// Refuse a platform answer whose lines do not tile the paragraph in order,
/// or whose offsets do not match its run boundaries.
fn valid(measured: &Measured, text: &str, runs: &[MeasureRun]) -> bool {
    let length = text.encode_utf16().count() as u32;
    let mut at = 0;
    let mut offsets = 0;
    for line in &measured.lines {
        if line.start16 != at
            || line.end16 <= line.start16
            || line.end16 > length
            || ![line.width, line.ascent, line.descent, line.leading]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0 && *v < 1.0e6)
        {
            return false;
        }
        offsets += inner_boundaries(runs, line.start16, line.end16).count();
        at = line.end16;
    }
    at == length
        && offsets == measured.offsets.len()
        && measured.offsets.iter().all(|v| v.is_finite())
}

/// A deterministic estimate for when the platform refuses: each UTF-16 unit
/// advances by 0.55 of its font size, and lines break at spaces.
pub(crate) fn estimate(text: &str, runs: &[MeasureRun], width: Option<f32>) -> Measured {
    super::testing::FixedMeasurer::default()
        .measure(text, runs, width)
        .unwrap_or_default()
}
