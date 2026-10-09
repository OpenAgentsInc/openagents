//! A row's display list: everything an adapter needs to paint one transcript
//! row, with every position already decided. Adapters never measure or wrap
//! text for a row; they draw runs at the positions given here.
//!
//! Coordinates are points relative to the row's top-left corner. The row spans
//! the full viewport width, so `x` already includes the side margins.

use serde::Serialize;

/// A font the adapter resolves to a platform face. Sizes are already scaled
/// by the reader's text size.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Font {
    pub size: f32,
    pub weight: Weight,
    /// The bundled family, which is always Paper Mono.
    #[serde(skip_serializing_if = "FontFamily::is_default")]
    pub family: FontFamily,
    /// Italic text. Paper Mono has no italic, so it is drawn upright.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// Code text. Paper Mono is fixed-pitch throughout; this only turns off
    /// contextual alternates.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mono: bool,
}

/// The bundled family. Every surface draws Paper Mono (#10904), so this has
/// one variant; it stays a type so a family remains part of a font's
/// identity in measurement and glyph caches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontFamily {
    #[default]
    PaperMono,
}
impl FontFamily {
    fn is_default(&self) -> bool {
        *self == Self::PaperMono
    }
}

impl Font {
    pub(crate) fn bits(self) -> u64 {
        u64::from(self.size.to_bits())
            | (self.weight as u64) << 32
            | u64::from(self.italic) << 40
            | u64::from(self.mono) << 41
            | (self.family as u64) << 42
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Weight {
    Regular = 0,
    Medium = 1,
    Semibold = 2,
    Bold = 3,
}

/// A color the adapter maps to its palette, so light and dark appearance stay
/// paint-only decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorRole {
    /// Primary text.
    Primary,
    /// Secondary text, such as a tool detail or a note.
    Secondary,
    /// Tertiary marks, such as a disclosure chevron.
    Tertiary,
    /// Inert link text.
    Link,
    /// A user's message bubble.
    Bubble,
    /// Code blocks, tables, and tool output.
    Surface,
    /// A table header row.
    Raised,
    /// Hairlines, quote bars, and rules.
    Border,
    /// The background behind inline code.
    InlineCode,
}

/// A paint color: a palette role, or the node's explicit sRGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ink {
    Role(ColorRole),
    Rgba([u8; 4]),
}

impl Ink {
    /// The quieter color for quoted text.
    pub(crate) fn quieter(self) -> Self {
        match self {
            Self::Role(_) => Self::Role(ColorRole::Secondary),
            Self::Rgba([r, g, b, a]) => Self::Rgba([r, g, b, (f32::from(a) * 0.7) as u8]),
        }
    }
}

/// How one run of text is drawn. Runs refer to a row's styles by index.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TextStyle {
    pub font: Font,
    pub ink: Ink,
    /// Multiplies the ink's alpha.
    pub opacity: f32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
}

impl TextStyle {
    pub(crate) fn new(font: Font, ink: Ink) -> Self {
        Self {
            font,
            ink,
            opacity: 1.0,
            underline: false,
            strike: false,
        }
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        self.font.bits() == other.font.bits()
            && self.ink == other.ink
            && self.opacity.to_bits() == other.opacity.to_bits()
            && self.underline == other.underline
            && self.strike == other.strike
    }
}

/// One positioned piece of a line: `texts[text]`, UTF-16 units
/// `start16..start16 + len16` (UTF-8 bytes `start8..start8 + len8`), drawn with
/// `styles[style]` so its origin sits at (`x`, `baseline`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Run {
    pub text: u32,
    pub start16: u32,
    pub len16: u32,
    pub start8: u32,
    pub len8: u32,
    pub x: f32,
    pub baseline: f32,
    /// The measured advance, for decorations and hit rectangles.
    pub width: f32,
    pub style: u32,
    /// When set, the adapter truncates the run to this width with an
    /// ellipsis, such as a one-line tool detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate: Option<f32>,
}

/// A filled or stroked rounded rectangle: a bubble, code block, quote bar,
/// table, rule, or inline-code background.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Corner radii: top leading, top trailing, bottom trailing, bottom
    /// leading.
    pub radii: [f32; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Ink>,
    /// A one-point stroke inside the rectangle's edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Ink>,
}

/// An inert link's hit rectangle. Nothing opens the destination unless the
/// application separately admits it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Link {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub destination: String,
}

/// A native control or glyph the adapter supplies at a decided rectangle.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WidgetKind {
    /// An application button; activation returns only its semantic node key.
    Button { key: String, enabled: bool },
    /// A code block's copy control.
    Copy {
        text: String,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        icon: bool,
    },
    /// A tap target that expands or collapses the tool row `key`.
    Toggle { key: String, expanded: bool },
    /// A disclosure chevron.
    Chevron { expanded: bool },
    /// A tool's state: a spinner while running, then a check or a cross.
    Status { state: crate::ToolState },
    /// A task-list checkbox.
    Checkbox { checked: bool },
    /// The animated working dots. The layout draws a working row with a
    /// [`WidgetKind::Spinner`]; hosts still paint this kind.
    Working,
    /// A small activity spinner.
    Spinner,
    /// The control that loads older rows.
    Earlier { loading: bool },
    /// A surface the adapter draws itself, such as a link's preview card,
    /// in the box the layout reserved for it: a [`crate::view::Element::Surface`]
    /// with a `min_height` in its style. Its label is the spoken text.
    Surface { resource: String, label: String },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Widget {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(flatten)]
    pub kind: WidgetKind,
}

/// A region that scrolls sideways, such as a code block or a table wider than
/// the row. The items in its ranges are laid out at their unscrolled
/// positions in row coordinates; the adapter paints them inside a horizontal
/// scroller clipped to `x..x + w`, shifted left by its scroll offset, and
/// leaves them out of the row's own painting.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Scroller {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// The width of the scrolled content, from `x`. Always more than `w`.
    pub content_w: f32,
    /// `runs[runs[0]..runs[1]]` scroll with this region.
    pub runs: [u32; 2],
    pub rects: [u32; 2],
    pub links: [u32; 2],
}

/// A row's accessibility element, from the semantic tree.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Accessibility {
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hint: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub button: bool,
}

/// A code paragraph eligible for paint-only highlighting.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CodeBlock {
    pub text: u32,
    pub language: String,
}

/// Everything needed to paint one row.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RowDisplay {
    /// The row's node key; the earlier control uses `EARLIER_KEY`.
    pub key: String,
    /// Changes whenever the painted content can change, including width,
    /// text scale, and expansion.
    pub version: u64,
    pub height: f32,
    pub styles: Vec<TextStyle>,
    /// Paragraph texts that runs slice.
    pub texts: Vec<String>,
    pub runs: Vec<Run>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub code_blocks: Vec<CodeBlock>,
    pub rects: Vec<Rect>,
    pub links: Vec<Link>,
    pub widgets: Vec<Widget>,
    /// Regions that scroll sideways. Their items are in the lists above.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scrollers: Vec<Scroller>,
    pub accessibility: Accessibility,
    /// The message's plain text for a Copy action, if the row is a message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
}

impl RowDisplay {
    pub(crate) fn style(&mut self, style: TextStyle) -> u32 {
        if let Some(index) = self.styles.iter().position(|s| s.same(&style)) {
            return index as u32;
        }
        self.styles.push(style);
        (self.styles.len() - 1) as u32
    }

    /// Counts of each item list, to shift items laid out after it.
    pub(crate) fn mark(&self) -> Mark {
        Mark {
            texts: self.texts.len(),
            code_blocks: self.code_blocks.len(),
            styles: self.styles.len(),
            runs: self.runs.len(),
            rects: self.rects.len(),
            links: self.links.len(),
            widgets: self.widgets.len(),
            scrollers: self.scrollers.len(),
        }
    }

    /// Inserts a rectangle behind the ones from `at` on, keeping scroller
    /// ranges on the rectangles they named.
    pub(crate) fn insert_rect(&mut self, at: usize, rect: Rect) {
        self.rects.insert(at, rect);
        let at = at as u32;
        for scroller in &mut self.scrollers {
            let [start, end] = &mut scroller.rects;
            if *start >= at {
                *start += 1;
                *end += 1;
            } else if *end > at {
                *end += 1;
            }
        }
    }

    /// Moves every item added since `mark`.
    pub(crate) fn shift(&mut self, mark: Mark, dx: f32, dy: f32) {
        for run in &mut self.runs[mark.runs..] {
            run.x += dx;
            run.baseline += dy;
        }
        for rect in &mut self.rects[mark.rects..] {
            rect.x += dx;
            rect.y += dy;
        }
        for link in &mut self.links[mark.links..] {
            link.x += dx;
            link.y += dy;
        }
        for widget in &mut self.widgets[mark.widgets..] {
            widget.x += dx;
            widget.y += dy;
        }
        for scroller in &mut self.scrollers[mark.scrollers..] {
            scroller.x += dx;
            scroller.y += dy;
        }
    }

    /// The rightmost edge of the text runs added since `mark`.
    pub(crate) fn text_extent(&self, mark: Mark) -> f32 {
        self.runs[mark.runs..]
            .iter()
            .map(|run| run.x + run.width)
            .fold(0.0, f32::max)
    }

    /// Drops everything added since `mark`.
    pub(crate) fn truncate_to(&mut self, mark: Mark) {
        self.texts.truncate(mark.texts);
        self.code_blocks.truncate(mark.code_blocks);
        self.styles.truncate(mark.styles);
        self.runs.truncate(mark.runs);
        self.rects.truncate(mark.rects);
        self.links.truncate(mark.links);
        self.widgets.truncate(mark.widgets);
        self.scrollers.truncate(mark.scrollers);
    }

    pub(crate) fn has_blocks_since(&self, mark: Mark) -> bool {
        self.rects.len() > mark.rects || self.widgets.len() > mark.widgets
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Mark {
    pub(crate) texts: usize,
    pub(crate) code_blocks: usize,
    pub(crate) styles: usize,
    pub(crate) runs: usize,
    pub(crate) rects: usize,
    pub(crate) links: usize,
    pub(crate) widgets: usize,
    pub(crate) scrollers: usize,
}
