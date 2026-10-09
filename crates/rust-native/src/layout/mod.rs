//! Transcript layout: exact row heights and display lists, computed in Rust.
//!
//! An adapter hands a transcript's rows (the `Transcript` element's children)
//! to a [`TranscriptLayout`] with a viewport width and a text scale. The
//! layout measures text through the adapter's [`Measurer`], keeps every row's
//! exact height and cumulative offset, and answers two questions: which rows
//! intersect a vertical range ([`TranscriptLayout::rows_in`]), and how to
//! paint one row ([`TranscriptLayout::display`]). A row is laid out again only
//! when its content version, the width, the text scale, or its expansion
//! changes, so a streamed token re-measures one row.
//!
//! Every row's display list is kept from layout, and each update publishes an
//! immutable [`Frame`]: keys, versions, offsets, and display lists. A frame
//! can be read on any thread while the next update runs on another, so an
//! adapter can lay out off its UI thread and swap frames when one is ready.
//!
//! The semantic contract does not change: applications still emit
//! `Transcript`, `Message`, `Markdown`, and `Tool` nodes. Layout is an adapter
//! implementation detail that Rust performs on the adapter's behalf.

pub mod display;
#[cfg(feature = "ffi")]
pub mod ffi;
mod measure;
mod rows;
#[cfg(feature = "shaping")]
pub mod shape;
pub mod source;
pub mod testing;

pub use display::{RowDisplay, Scroller};
pub use measure::{Line, MeasureCache, MeasureRun, Measured, Measurer};
pub use rows::{EARLIER_KEY, READING_WIDTH, content_band};

use crate::view::{Earlier, Element, Node, View, ViewError};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

/// Space above the first row and below the last.
pub const EDGE_INSET: f32 = 16.0;
/// Space between rows.
pub const ROW_GAP: f32 = 18.0;
/// The most rows one layout holds.
pub const MAX_ROWS: usize = 20_000;
/// The most points one text-size curve may name.
pub const MAX_CURVE_POINTS: usize = 16;

/// The older-rows control as the layout sees it: no intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EarlierRow {
    pub label: String,
    pub loading: bool,
}

impl<I> From<&Earlier<I>> for EarlierRow {
    fn from(earlier: &Earlier<I>) -> Self {
        Self {
            label: earlier.label.clone(),
            loading: earlier.loading,
        }
    }
}

/// One update. `order` names every row, oldest first; `None` keeps the
/// previous order, so a streamed token need not resend every key. `rows`
/// carries the nodes that are new or whose content changed; a row in the
/// order but not in `rows` keeps its previous content.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub width: f32,
    pub scale: f32,
    #[serde(default)]
    pub order: Option<Vec<String>>,
    #[serde(default)]
    pub rows: Vec<Node<()>>,
    /// Tool rows the reader expanded. Expansion is adapter state.
    #[serde(default)]
    pub expanded: Vec<String>,
    #[serde(default)]
    pub earlier: Option<EarlierRow>,
    /// The reader's text size for each nominal size, as `[nominal, scaled]`
    /// points, such as Dynamic Type's curve for each text style. Sizes
    /// between points interpolate; sizes outside keep the nearest point's
    /// ratio. Empty means every size scales by `scale`.
    #[serde(default)]
    pub curve: Vec<[f32; 2]>,
    /// Read the rows and the earlier control from this published source
    /// ([`source`]) instead of `order`, `rows`, and `earlier`, which must
    /// then be absent.
    #[serde(default)]
    pub source: Option<String>,
}

impl Update {
    /// An update that carries every row of a transcript node.
    pub fn from_transcript<I>(node: &Node<I>, width: f32, scale: f32) -> Option<Self> {
        let Element::Transcript {
            children, earlier, ..
        } = &node.element
        else {
            return None;
        };
        Some(Self {
            width,
            scale,
            order: Some(children.iter().map(|c| c.key.clone()).collect()),
            rows: children.iter().map(without_intents).collect(),
            expanded: vec![],
            earlier: earlier.as_ref().map(EarlierRow::from),
            curve: vec![],
            source: None,
        })
    }
}

/// Scoped transcript dimensions in logical points. Defaults preserve the
/// established reader; applications supply their own component metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Metrics {
    pub reading_width: u16,
    pub body_size: u16,
    /// Zero uses measured font metrics; otherwise each body line uses this height.
    pub body_line_height: u16,
    pub row_gap: u16,
    pub bubble_padding: u16,
    /// Zero preserves the default fixed leading gutter.
    pub bubble_max_percent: u8,
    pub bubble_radius: u16,
    pub bubble_tail_radius: u16,
    pub markdown: Option<MarkdownMetrics>,
}

/// Scoped Markdown dimensions. Each heading is [font size, line height].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MarkdownMetrics {
    pub headings: [[u16; 2]; 4],
    /// Half-point units allow fractional code sizes without unstable float hashing.
    pub code_size_half_points: u16,
    pub code_line_height: u16,
    pub code_header_height: u16,
    pub code_label_size: u16,
    pub code_padding_y: u16,
    pub copy_icon: bool,
    pub inline_code: Option<InlineCodeMetrics>,
    pub strong_weight: Option<display::Weight>,
}

/// Scoped inline-code typography, paint geometry, and sRGB text color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InlineCodeMetrics {
    pub size_percent: u16,
    pub inset_y: u16,
    pub radius_half_points: u16,
    pub color: [u8; 4],
}
impl MarkdownMetrics {
    fn valid(self) -> bool {
        self.headings
            .iter()
            .all(|[size, height]| (1..=400).contains(size) && (1..=800).contains(height))
            && (2..=800).contains(&self.code_size_half_points)
            && (1..=800).contains(&self.code_line_height)
            && (24..=128).contains(&self.code_header_height)
            && (1..=128).contains(&self.code_label_size)
            && self.code_padding_y <= 128
            && self.inline_code.is_none_or(|code| {
                (1..=400).contains(&code.size_percent)
                    && code.inset_y <= 128
                    && code.radius_half_points <= 256
            })
    }
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            reading_width: 720,
            body_size: 16,
            body_line_height: 0,
            row_gap: 18,
            bubble_padding: 14,
            bubble_max_percent: 0,
            bubble_radius: 18,
            bubble_tail_radius: 4,
            markdown: None,
        }
    }
}
impl Metrics {
    fn valid(self) -> bool {
        (1..=16384).contains(&self.reading_width)
            && (1..=400).contains(&self.body_size)
            && self.body_line_height <= 800
            && self.row_gap <= 400
            && self.bubble_padding <= 128
            && self.bubble_max_percent <= 100
            && self.bubble_radius <= 128
            && self.bubble_tail_radius <= 128
            && self.markdown.is_none_or(MarkdownMetrics::valid)
    }
}

/// How nominal font sizes become drawn sizes: one scale, or a curve.
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    scale: f32,
    family: display::FontFamily,
    metrics: Metrics,
    /// Sorted by nominal size, without repeats.
    curve: Vec<(f32, f32)>,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            scale: 1.0,
            family: Default::default(),
            metrics: Metrics::default(),
            curve: vec![],
        }
    }
}

impl Typography {
    /// Checks a scale and a curve: the scale is 0.5–4, and each point names
    /// a nominal size of 1–400 points whose scaled size is 0.5–4 times it.
    pub fn new(scale: f32, curve: &[[f32; 2]]) -> Result<Self, LayoutError> {
        if !scale.is_finite() || !(0.5..=4.0).contains(&scale) || curve.len() > MAX_CURVE_POINTS {
            return Err(LayoutError::Geometry);
        }
        let mut points = Vec::with_capacity(curve.len());
        for [nominal, scaled] in curve {
            if !nominal.is_finite()
                || !scaled.is_finite()
                || !(1.0..=400.0).contains(nominal)
                || !(0.5..=4.0).contains(&(scaled / nominal))
            {
                return Err(LayoutError::Geometry);
            }
            points.push((*nominal, *scaled));
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        points.dedup_by(|a, b| a.0 == b.0);
        Ok(Self {
            scale,
            family: Default::default(),
            metrics: Metrics::default(),
            curve: points,
        })
    }

    /// The drawn size for a nominal size.
    pub fn size(&self, nominal: f32) -> f32 {
        let (Some(first), Some(last)) = (self.curve.first(), self.curve.last()) else {
            return nominal * self.scale;
        };
        if nominal <= first.0 {
            return nominal * first.1 / first.0;
        }
        if nominal >= last.0 {
            return nominal * last.1 / last.0;
        }
        let upper = self.curve.partition_point(|p| p.0 < nominal);
        let (a, b) = (self.curve[upper - 1], self.curve[upper]);
        let t = (nominal - a.0) / (b.0 - a.0);
        a.1 + t * (b.1 - a.1)
    }

    fn fingerprint(&self) -> u64 {
        let bits: Vec<(u32, u32)> = self
            .curve
            .iter()
            .map(|(a, b)| (a.to_bits(), b.to_bits()))
            .collect();
        hash_of(&(self.scale.to_bits(), bits, self.family, self.metrics))
    }
}

/// What an update did.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Summary {
    /// Rows, including the earlier control.
    pub count: usize,
    /// The content height, including the edge insets.
    pub height: f32,
    /// Rows laid out again by this update.
    pub relaid: usize,
    /// Platform measurements this update requested.
    pub measured: u64,
    pub micros: u64,
}

/// A row's place in the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub index: usize,
    /// Changes whenever the row's painted content can change.
    pub version: u64,
    pub y: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutError {
    /// Width must be finite and 1–16,384 points; scale 0.5–4.
    Geometry,
    Limit,
    DuplicateRow(String),
    /// `order` names a row the layout has never received.
    UnknownRow(String),
    Row(ViewError),
    /// A source name is malformed, or an update names a source and also
    /// carries rows, an order, or an earlier control.
    Source,
    /// An update names a source nothing published.
    UnknownSource(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Geometry => {
                f.write_str("layout width, text scale, or text-size curve is out of range")
            }
            Self::Limit => f.write_str("transcript exceeds the layout's row bound"),
            Self::DuplicateRow(key) => write!(f, "duplicate transcript row: {key}"),
            Self::UnknownRow(key) => write!(f, "transcript row has no content: {key}"),
            Self::Row(error) => write!(f, "invalid transcript row: {error}"),
            Self::Source => f.write_str("malformed transcript source"),
            Self::UnknownSource(name) => write!(f, "no transcript source is published as {name}"),
        }
    }
}

impl std::error::Error for LayoutError {}

enum Source {
    Node(Arc<Node<()>>),
    Earlier(EarlierRow),
}

struct Row {
    key: String,
    source: Source,
    /// A hash of the row's content.
    content: u64,
    expanded: bool,
    height: f32,
    /// The inputs the display was computed for: content, width bits,
    /// typography, and expansion.
    laid: Option<(u64, u32, u64, bool)>,
    /// The display list from the last layout.
    display: Option<Arc<RowDisplay>>,
}

/// One published layout: every row's key, version, offset, and display
/// list. Frames are immutable and can be read on any thread.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    width: f32,
    keys: Arc<Vec<String>>,
    index: Arc<HashMap<String, usize>>,
    /// Each row's top, then the content height.
    tops: Vec<f32>,
    rows: Vec<Arc<RowDisplay>>,
}

impl Frame {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The content height, including the edge insets.
    pub fn height(&self) -> f32 {
        self.tops.last().copied().unwrap_or(2.0 * EDGE_INSET)
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn placement(&self, index: usize) -> Option<Placement> {
        let row = self.rows.get(index)?;
        Some(Placement {
            index,
            version: row.version,
            y: self.tops[index],
            height: row.height,
        })
    }

    /// The row index for a key; the earlier control is `EARLIER_KEY`.
    pub fn find(&self, key: &str) -> Option<usize> {
        self.index.get(key).copied()
    }

    pub fn key(&self, index: usize) -> Option<&str> {
        self.keys.get(index).map(String::as_str)
    }

    /// The rows that intersect `y0..y1`, by binary search.
    pub fn rows_in(&self, y0: f32, y1: f32) -> Range<usize> {
        rows_in(&self.tops, |i| self.rows[i].height, y0, y1)
    }

    pub fn display(&self, index: usize) -> Option<&RowDisplay> {
        self.rows.get(index).map(|row| &**row)
    }
}

/// The rows of `tops` (each row's top, then the content height) that
/// intersect `y0..y1`.
fn rows_in(tops: &[f32], height: impl Fn(usize) -> f32, y0: f32, y1: f32) -> Range<usize> {
    let count = tops.len().saturating_sub(1);
    let first = tops[..count]
        .partition_point(|top| *top <= y0)
        .saturating_sub(1);
    let first = if first < count && tops[first] + height(first) < y0 {
        first + 1
    } else {
        first
    };
    let last = tops[..count].partition_point(|top| *top < y1);
    first.min(last)..last
}

/// Exact heights and display lists for one transcript. Use one per mounted
/// transcript, on one thread.
pub struct TranscriptLayout {
    width: f32,
    family: display::FontFamily,
    metrics: Metrics,
    typography: Typography,
    rows: Vec<Row>,
    keys: Arc<Vec<String>>,
    index: Arc<HashMap<String, usize>>,
    /// Each row's top, then the content height.
    tops: Vec<f32>,
    expanded: HashSet<String>,
    cache: MeasureCache,
    /// The frame for the current state, built when first asked for.
    frame: Option<Arc<Frame>>,
    /// The source publication the rows came from, if they came from one.
    pulled: Option<(String, u64)>,
}

impl Default for TranscriptLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptLayout {
    pub fn new() -> Self {
        Self {
            width: 0.0,
            family: Default::default(),
            metrics: Metrics::default(),
            typography: Typography::default(),
            rows: vec![],
            keys: Arc::default(),
            index: Arc::default(),
            tops: vec![2.0 * EDGE_INSET],
            expanded: HashSet::new(),
            cache: MeasureCache::default(),
            frame: None,
            pulled: None,
        }
    }

    /// Select a bundled font pair. The next update invalidates measured rows.
    pub fn set_font_family(&mut self, family: display::FontFamily) {
        self.family = family;
    }

    /// Apply checked component dimensions on the next update.
    pub fn set_metrics(&mut self, metrics: Metrics) -> Result<(), LayoutError> {
        if !metrics.valid() {
            return Err(LayoutError::Geometry);
        }
        self.metrics = metrics;
        Ok(())
    }

    /// Applies an update and lays out the rows it invalidates.
    pub fn update(
        &mut self,
        update: Update,
        measurer: &mut dyn Measurer,
    ) -> Result<Summary, LayoutError> {
        let started = Instant::now();
        let misses = self.cache.misses;
        if !update.width.is_finite() || !(1.0..=16_384.0).contains(&update.width) {
            return Err(LayoutError::Geometry);
        }
        let mut typography = Typography::new(update.scale, &update.curve)?;
        typography.family = self.family;
        typography.metrics = self.metrics;
        let expanded: HashSet<String> = update.expanded.into_iter().collect();
        if let Some(name) = update.source {
            if update.order.is_some() || !update.rows.is_empty() || update.earlier.is_some() {
                return Err(LayoutError::Source);
            }
            let snapshot =
                source::get(&name).ok_or_else(|| LayoutError::UnknownSource(name.clone()))?;
            return self.pull(
                name,
                &snapshot,
                expanded,
                update.width,
                typography,
                measurer,
                started,
                misses,
            );
        }
        self.pulled = None;
        let has_earlier = self.rows.first().is_some_and(|row| row.key == EARLIER_KEY);
        if update.order.is_none() && update.earlier.is_some() == has_earlier {
            if update.rows.len() > MAX_ROWS {
                return Err(LayoutError::Limit);
            }
            let incoming = checked_rows(update.rows)?;
            return self.update_in_place(
                incoming,
                update.earlier,
                expanded,
                update.width,
                typography,
                measurer,
                started,
                misses,
            );
        }
        let order = match update.order {
            Some(order) => order,
            None => self
                .rows
                .iter()
                .filter(|row| row.key != EARLIER_KEY)
                .map(|row| row.key.clone())
                .collect(),
        };
        if order.len() > MAX_ROWS || update.rows.len() > MAX_ROWS {
            return Err(LayoutError::Limit);
        }
        let incoming = checked_rows(update.rows)?;
        self.reorder(order, incoming, update.earlier, expanded)?;
        Ok(self.relayout(update.width, typography, measurer, started, misses))
    }

    /// Takes a source's newest publication. Rows whose content is unchanged
    /// keep their layout; an unchanged publication only applies the width,
    /// the text size, and expansion.
    #[allow(clippy::too_many_arguments)]
    fn pull(
        &mut self,
        name: String,
        snapshot: &source::Snapshot,
        expanded: HashSet<String>,
        width: f32,
        typography: Typography,
        measurer: &mut dyn Measurer,
        started: Instant,
        misses: u64,
    ) -> Result<Summary, LayoutError> {
        let current = self.pulled.as_ref().is_some_and(|(pulled, generation)| {
            *pulled == name && *generation == snapshot.generation
        });
        if !current {
            let offset = usize::from(self.rows.first().is_some_and(|row| row.key == EARLIER_KEY));
            let same_order = snapshot.earlier.is_some() == (offset == 1)
                && self.rows.len() - offset == snapshot.rows.len()
                && self.rows[offset..]
                    .iter()
                    .zip(&snapshot.rows)
                    .all(|(row, published)| row.key == published.node.key);
            if same_order {
                for (row, published) in self.rows[offset..].iter_mut().zip(&snapshot.rows) {
                    if row.content != published.content {
                        row.source = Source::Node(published.node.clone());
                        row.content = published.content;
                    }
                }
                if let Some(earlier) = &snapshot.earlier {
                    self.set_earlier(earlier.clone());
                }
            } else {
                let order = snapshot.keys().map(str::to_owned).collect();
                let incoming = snapshot
                    .rows
                    .iter()
                    .map(|row| (row.node.key.clone(), (row.node.clone(), row.content)))
                    .collect();
                self.reorder(order, incoming, snapshot.earlier.clone(), expanded.clone())?;
            }
            self.pulled = Some((name, snapshot.generation));
        }
        self.expand(expanded);
        Ok(self.relayout(width, typography, measurer, started, misses))
    }

    /// Replaces the rows with `order`, taking content from `incoming` or,
    /// for rows it lacks, from the previous rows.
    fn reorder(
        &mut self,
        order: Vec<String>,
        mut incoming: Incoming,
        earlier: Option<EarlierRow>,
        expanded: HashSet<String>,
    ) -> Result<(), LayoutError> {
        let mut previous_earlier = None;
        let mut previous: HashMap<String, Row> = HashMap::with_capacity(self.rows.len());
        for row in self.rows.drain(..) {
            if row.key == EARLIER_KEY {
                previous_earlier = Some(row);
            } else {
                previous.insert(row.key.clone(), row);
            }
        }
        let mut rows = Vec::with_capacity(order.len() + 1);
        if let Some(earlier) = earlier {
            let content = hash_of(&(earlier.label.as_str(), earlier.loading));
            rows.push(match previous_earlier {
                Some(old) if old.content == content => old,
                _ => Row {
                    key: EARLIER_KEY.into(),
                    source: Source::Earlier(earlier),
                    content,
                    expanded: false,
                    height: 0.0,
                    laid: None,
                    display: None,
                },
            });
        }
        let mut seen = HashSet::with_capacity(order.len());
        for key in order {
            if !seen.insert(key.clone()) {
                return Err(LayoutError::DuplicateRow(key));
            }
            let open = expanded.contains(&key);
            let row = match (incoming.remove(&key), previous.remove(&key)) {
                (Some((_, content)), Some(old)) if old.content == content => Row {
                    expanded: open,
                    ..old
                },
                (Some((node, content)), _) => Row {
                    key,
                    source: Source::Node(node),
                    content,
                    expanded: open,
                    height: 0.0,
                    laid: None,
                    display: None,
                },
                (None, Some(old)) => Row {
                    expanded: open,
                    ..old
                },
                (None, None) => return Err(LayoutError::UnknownRow(key)),
            };
            rows.push(row);
        }
        self.rows = rows;
        let keys: Vec<String> = self.rows.iter().map(|row| row.key.clone()).collect();
        self.index = Arc::new(
            keys.iter()
                .enumerate()
                .map(|(i, key)| (key.clone(), i))
                .collect(),
        );
        self.keys = Arc::new(keys);
        self.expanded = expanded;
        Ok(())
    }

    /// Replaces the earlier control's content when it changed.
    fn set_earlier(&mut self, earlier: EarlierRow) {
        let content = hash_of(&(earlier.label.as_str(), earlier.loading));
        if let Some(row) = self.rows.first_mut().filter(|row| row.key == EARLIER_KEY)
            && row.content != content
        {
            row.source = Source::Earlier(earlier);
            row.content = content;
        }
    }

    /// Marks the expanded tool rows.
    fn expand(&mut self, expanded: HashSet<String>) {
        if expanded != self.expanded {
            for row in &mut self.rows {
                row.expanded = row.key != EARLIER_KEY && expanded.contains(&row.key);
            }
            self.expanded = expanded;
        }
    }

    /// The streaming path: the order is unchanged, so changed rows are
    /// replaced where they stand.
    #[allow(clippy::too_many_arguments)]
    fn update_in_place(
        &mut self,
        incoming: Incoming,
        earlier: Option<EarlierRow>,
        expanded: HashSet<String>,
        width: f32,
        typography: Typography,
        measurer: &mut dyn Measurer,
        started: Instant,
        misses: u64,
    ) -> Result<Summary, LayoutError> {
        for key in incoming.keys() {
            if !self.index.contains_key(key) {
                return Err(LayoutError::UnknownRow(key.clone()));
            }
        }
        for (key, (node, content)) in incoming {
            let row = &mut self.rows[self.index[&key]];
            if row.content != content {
                row.source = Source::Node(node);
                row.content = content;
            }
        }
        if let Some(earlier) = earlier {
            self.set_earlier(earlier);
        }
        self.expand(expanded);
        Ok(self.relayout(width, typography, measurer, started, misses))
    }

    /// Lays out every row whose inputs changed, then recomputes offsets.
    fn relayout(
        &mut self,
        width: f32,
        typography: Typography,
        measurer: &mut dyn Measurer,
        started: Instant,
        misses: u64,
    ) -> Summary {
        self.width = width;
        let fingerprint = typography.fingerprint();
        self.typography = typography;
        self.frame = None;
        let mut relaid = 0;
        for row in &mut self.rows {
            let inputs = (row.content, width.to_bits(), fingerprint, row.expanded);
            if row.laid != Some(inputs) {
                let mut display = lay(row, width, &self.typography, &mut self.cache, measurer);
                display.version = hash_of(&inputs);
                row.height = display.height;
                row.display = Some(Arc::new(display));
                row.laid = Some(inputs);
                relaid += 1;
            }
        }
        self.offsets();
        Summary {
            count: self.rows.len(),
            height: self.height(),
            relaid,
            measured: self.cache.misses - misses,
            micros: started.elapsed().as_micros() as u64,
        }
    }

    fn offsets(&mut self) {
        self.tops.clear();
        let mut y = EDGE_INSET;
        for row in &self.rows {
            self.tops.push(y);
            y += row.height + f32::from(self.metrics.row_gap);
        }
        let end = if self.rows.is_empty() {
            2.0 * EDGE_INSET
        } else {
            y - f32::from(self.metrics.row_gap) + EDGE_INSET
        };
        self.tops.push(end);
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The content height, including the edge insets.
    pub fn height(&self) -> f32 {
        *self.tops.last().unwrap_or(&0.0)
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    /// The measurement cache, for diagnostics.
    pub fn cache(&self) -> &MeasureCache {
        &self.cache
    }

    pub fn placement(&self, index: usize) -> Option<Placement> {
        let row = self.rows.get(index)?;
        Some(Placement {
            index,
            version: row.display.as_ref().map_or(0, |d| d.version),
            y: self.tops[index],
            height: row.height,
        })
    }

    /// The row index for a key; the earlier control is `EARLIER_KEY`.
    pub fn find(&self, key: &str) -> Option<usize> {
        self.index.get(key).copied()
    }

    pub fn key(&self, index: usize) -> Option<&str> {
        self.rows.get(index).map(|row| row.key.as_str())
    }

    /// The rows that intersect `y0..y1`, by binary search.
    pub fn rows_in(&self, y0: f32, y1: f32) -> Range<usize> {
        rows_in(&self.tops, |i| self.rows[i].height, y0, y1)
    }

    /// One row's display list, as laid out.
    pub fn display(&self, index: usize) -> Option<&RowDisplay> {
        self.rows.get(index)?.display.as_deref()
    }

    /// The current layout as an immutable frame. Building one after an
    /// update copies offsets and shares display lists; later calls reuse it.
    pub fn frame(&mut self) -> Arc<Frame> {
        if let Some(frame) = &self.frame {
            return frame.clone();
        }
        let frame = Arc::new(Frame {
            width: self.width,
            keys: self.keys.clone(),
            index: self.index.clone(),
            tops: self.tops.clone(),
            rows: self
                .rows
                .iter()
                .map(|row| row.display.clone().unwrap_or_default())
                .collect(),
        });
        self.frame = Some(frame.clone());
        frame
    }
}

/// Rows by key, each with a hash of its content.
type Incoming = HashMap<String, (Arc<Node<()>>, u64)>;

/// Validates each row as a one-node view and hashes its content.
fn checked_rows(rows: Vec<Node<()>>) -> Result<Incoming, LayoutError> {
    let mut incoming = HashMap::with_capacity(rows.len());
    for row in rows {
        let checked = View::new("layout", 1, row)
            .validate()
            .map_err(LayoutError::Row)?;
        let row = checked.view().root.clone();
        let content = content_hash(&row);
        incoming.insert(row.key.clone(), (Arc::new(row), content));
    }
    Ok(incoming)
}

fn lay(
    row: &Row,
    width: f32,
    typography: &Typography,
    cache: &mut MeasureCache,
    measurer: &mut dyn Measurer,
) -> RowDisplay {
    let mut ctx = rows::Ctx {
        measurer,
        cache,
        typography,
        out: RowDisplay {
            key: row.key.clone(),
            ..RowDisplay::default()
        },
    };
    let height = match &row.source {
        Source::Node(node) => rows::lay_row(&mut ctx, node, row.expanded, width),
        Source::Earlier(earlier) => {
            rows::lay_earlier(&mut ctx, &earlier.label, earlier.loading, width)
        }
    };
    ctx.out.height = height;
    link_targets(&mut ctx.out);
    ctx.out
}

/// A [`display::WidgetKind::Link`] over each link that opens, outside the
/// regions that scroll sideways (a widget does not scroll with them).
fn link_targets(out: &mut RowDisplay) {
    let targets: Vec<display::Widget> = out
        .links
        .iter()
        .enumerate()
        .filter(|(number, link)| {
            let number = *number as u32;
            crate::markdown::opens(&link.destination)
                && !out
                    .scrollers
                    .iter()
                    .any(|s| (s.links[0]..s.links[1]).contains(&number))
        })
        .map(|(_, link)| display::Widget {
            x: link.x,
            y: link.y,
            w: link.w,
            h: link.h,
            kind: display::WidgetKind::Link {
                url: link.destination.clone(),
            },
        })
        .collect();
    out.widgets.extend(targets);
}

fn hash_of(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// A hash of a row's serialized content, so an unchanged row resent by the
/// adapter keeps its version and is not measured again.
fn content_hash(node: &Node<()>) -> u64 {
    struct Hashing(DefaultHasher);
    impl Write for Hashing {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.write(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut hashing = Hashing(DefaultHasher::new());
    // Serializing a validated node cannot fail.
    let _ = serde_json::to_writer(&mut hashing, node);
    hashing.0.finish()
}

/// The same node without its application intents, which layout never reads.
pub fn without_intents<I>(node: &Node<I>) -> Node<()> {
    let children = |nodes: &[Node<I>]| nodes.iter().map(without_intents).collect();
    let element = match &node.element {
        Element::Surface { resource, label } => Element::Surface {
            resource: resource.clone(),
            label: label.clone(),
        },
        Element::Stack { axis, children: c } => Element::Stack {
            axis: *axis,
            children: children(c),
        },
        Element::List { label, children: c } => Element::List {
            label: label.clone(),
            children: children(c),
        },
        Element::Text { value, role } => Element::Text {
            value: value.clone(),
            role: *role,
        },
        Element::RichText { runs, role } => Element::RichText {
            runs: runs.clone(),
            role: *role,
        },
        Element::Field {
            label,
            value,
            placeholder,
            secret,
            multiline,
            enabled,
            max_bytes,
            ..
        } => Element::Field {
            label: label.clone(),
            value: value.clone(),
            placeholder: placeholder.clone(),
            secret: *secret,
            multiline: *multiline,
            enabled: *enabled,
            max_bytes: *max_bytes,
            on_change: (),
        },
        Element::Choice {
            label,
            selected,
            enabled,
            children: c,
            ..
        } => Element::Choice {
            label: label.clone(),
            selected: *selected,
            enabled: *enabled,
            intent: (),
            children: children(c),
        },
        Element::Dialog {
            label,
            open,
            children: c,
            ..
        } => Element::Dialog {
            label: label.clone(),
            open: *open,
            on_close: (),
            children: children(c),
        },
        Element::Button {
            label,
            enabled,
            icon,
            ..
        } => Element::Button {
            shortcut: None,
            label: label.clone(),
            enabled: *enabled,
            icon: *icon,
            intent: (),
        },
        Element::Transcript {
            label,
            children: c,
            earlier,
            source,
        } => Element::Transcript {
            source: source.clone(),
            label: label.clone(),
            children: children(c),
            earlier: earlier.as_ref().map(|e| Earlier {
                label: e.label.clone(),
                loading: e.loading,
                intent: (),
            }),
        },
        Element::Message {
            role,
            note,
            children: c,
        } => Element::Message {
            role: *role,
            note: note.clone(),
            children: children(c),
        },
        Element::Markdown { blocks } => Element::Markdown {
            blocks: blocks.clone(),
        },
        Element::Tool {
            name,
            detail,
            state,
            children: c,
        } => Element::Tool {
            name: name.clone(),
            detail: detail.clone(),
            state: *state,
            children: children(c),
        },
        Element::Working { label } => Element::Working {
            label: label.clone(),
        },
        Element::Composer {
            token,
            placeholder,
            max_bytes,
            enabled,
            busy,
            stop,
            choices,
            draft,
            focus,
        } => Element::Composer {
            token: token.clone(),
            placeholder: placeholder.clone(),
            max_bytes: *max_bytes,
            enabled: *enabled,
            busy: *busy,
            stop: stop.as_ref().map(|_| ()),
            choices: choices.clone(),
            draft: draft.clone(),
            focus: *focus,
        },
    };
    Node {
        key: node.key.clone(),
        style: node.style,
        element,
    }
}

#[cfg(test)]
mod tests;
