//! A slide as Rust Native nodes, and where each sits on the canvas.
//!
//! Every part of a slide is a [`rust_native::Node`]: a `Text` with a role
//! (a kicker is `status`, a title `heading`, a large number `heading` set
//! larger), `Markdown` for prose, lists, and tables, and a `Stack` with a
//! background for a card. Each part is laid out by
//! [`rust_native_desktop::rich::Rich`], the desktop and mobile chat's row
//! layout, and painted by the chat's transcript painter, so a slide has the
//! desktop's fonts, weights, colors, and spacing.
//!
//! This module decides only what those parts are and where they go: the
//! slide's composition. It draws nothing itself.

use crate::slide::{Deck, Layout, Metric, Slide};
use rust_native::layout::display::ColorRole;
use rust_native::markdown::{self, Block as MarkdownBlock};
use rust_native::style::{Color, Space, Style, TextAlign};
use rust_native::{Axis, Element, Node, TextRole};
use rust_native_desktop::Theme;
use rust_native_desktop::image::Image;
use rust_native_desktop::rich::{self, Rich};
use std::sync::Arc;

/// The canvas, in points: 16:9. The body's widest column is the row
/// layout's reading width, so prose on a slide breaks the way it does in a
/// chat.
pub const WIDTH: f32 = 800.0;
pub const HEIGHT: f32 = 450.0;
/// The space left and right of the content.
pub const MARGIN_X: f32 = 40.0;
/// The space above the content, and below it over the foot.
pub const MARGIN_Y: f32 = 26.0;
/// The content column's width.
pub const COLUMN: f32 = WIDTH - 2.0 * MARGIN_X;
/// Where the foot (the slide's place in the deck) sits, from the top.
pub const FOOT_Y: f32 = HEIGHT - 26.0;
/// The lowest the content reaches.
pub const CONTENT_BOTTOM: f32 = FOOT_Y - 8.0;

/// How much larger than its nominal size a part is painted.
const TITLE: f32 = 1.5;
/// The opening title and a banner's wordmark try this first.
const DISPLAY: f32 = 3.0;
/// A metric's number.
const NUMBER: f32 = 2.5;
/// A statement's sentence.
const STATEMENT: f32 = 1.5;

/// The space between a kicker and its title, a title and the body, and the
/// body and the note, in points.
const KICKER_GAP: f32 = 6.0;
const BODY_GAP: f32 = 22.0;
const NOTE_GAP: f32 = 18.0;
/// The space between two cells of a row of metrics or steps.
const CELL_GAP: f32 = 24.0;

/// One laid-out part of a slide.
pub struct Part {
    /// What kind of part it is: `kicker`, `title`, `body`, and so on.
    pub kind: &'static str,
    /// Its text, for the outline and the snapshots; an image's is its
    /// alternative text.
    pub text: String,
    pub content: Content,
    /// How much larger than nominal it paints.
    pub magnification: f32,
    /// Its top-left corner on the canvas, in points.
    pub x: f32,
    pub y: f32,
}

/// What a part draws.
pub enum Content {
    /// Nodes laid out by the shared row layout.
    Rich(Box<Rich>),
    /// An image, fitted into a box `width` by `height` points when it is
    /// painted (see [`rust_native_desktop::image::fit`]).
    Image {
        image: Arc<Image>,
        width: f32,
        height: f32,
    },
}

impl Part {
    /// Its width on the canvas, in points.
    pub fn width(&self) -> f32 {
        match &self.content {
            Content::Rich(rich) => rich.width() * self.magnification,
            Content::Image { width, .. } => *width,
        }
    }

    /// Its height on the canvas, in points.
    pub fn height(&self) -> f32 {
        match &self.content {
            Content::Rich(rich) => rich.height() * self.magnification,
            Content::Image { height, .. } => *height,
        }
    }

    /// Where what it draws ends, in points from its left edge.
    pub fn natural_width(&self) -> f32 {
        match &self.content {
            Content::Rich(rich) => rich.natural_width() * self.magnification,
            Content::Image { width, .. } => *width,
        }
    }

    /// Its laid-out nodes, unless it is an image.
    pub fn rich(&self) -> Option<&Rich> {
        match &self.content {
            Content::Rich(rich) => Some(rich),
            Content::Image { .. } => None,
        }
    }

    /// Whether a table or code in it scrolls sideways for want of room.
    pub fn scrolls(&self) -> bool {
        self.rich().is_some_and(Rich::scrolls)
    }
}

/// A whole slide, laid out on the canvas.
pub struct Composed {
    pub parts: Vec<Part>,
}

impl Composed {
    /// The slide's outline: each part's kind, place, size, and text. The
    /// text export prints it and the snapshots hold it.
    pub fn outline(&self) -> String {
        let mut out = String::new();
        for part in &self.parts {
            let magnification = if part.magnification == 1.0 {
                String::new()
            } else {
                format!(" ×{}", part.magnification)
            };
            out.push_str(&format!(
                "{} at {:.0},{:.0} {:.0}x{:.0}{magnification}\n",
                part.kind,
                part.x,
                part.y,
                part.width(),
                part.height()
            ));
            if let Content::Image { image, .. } = &part.content {
                out.push_str(&format!(
                    "  image {} x {} pixels\n",
                    image.width, image.height
                ));
            }
            for line in part.text.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
        out
    }

    /// The lowest point any part reaches, in points.
    pub fn bottom(&self) -> f32 {
        self.parts
            .iter()
            .map(|part| part.y + part.height())
            .fold(0.0, f32::max)
    }
}

/// The colors a slide's parts take, from the shared theme.
struct Tones {
    /// A kicker, a label, a note, an attribution.
    muted: Color,
    /// A card's fill.
    card: Color,
}

fn tones() -> Tones {
    Tones {
        muted: Theme::openagents().muted,
        card: rich::color(ColorRole::Surface),
    }
}

fn node(key: &str, style: Style, element: Element<()>) -> Node<()> {
    Node {
        key: key.to_owned(),
        style,
        element,
    }
}

fn text(key: &str, value: &str, role: TextRole, color: Option<Color>) -> Node<()> {
    node(
        key,
        Style {
            foreground: color,
            ..Style::default()
        },
        Element::Text {
            value: value.to_owned(),
            role,
        },
    )
}

fn prose(key: &str, blocks: Vec<MarkdownBlock>) -> Node<()> {
    node(key, Style::default(), Element::Markdown { blocks })
}

/// A card: a stack with the transcript's surface color, padded.
fn card(key: &str, padding: Space, children: Vec<Node<()>>) -> Node<()> {
    let tones = tones();
    node(
        key,
        Style {
            background: Some(tones.card),
            padding_top: Some(padding),
            padding_end: Some(padding),
            padding_bottom: Some(padding),
            padding_start: Some(padding),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

/// Lays `node` out `width` canvas points wide at `magnification`.
fn lay(node: Node<()>, width: f32, magnification: f32) -> Rich {
    let family = Theme::openagents().font_family;
    Rich::in_family(node, (width / magnification).max(1.0), family)
        .or_else(|_| {
            Rich::in_family(
                text(
                    "error",
                    "This part could not be laid out.",
                    TextRole::Body,
                    None,
                ),
                width,
                family,
            )
        })
        .expect("a plain line lays out")
}

/// The largest magnification, at most `most`, at which `node` fits `width`
/// canvas points on as few lines as it can.
fn fitting(node: &Node<()>, width: f32, most: f32) -> f32 {
    let natural = lay(node.clone(), rust_native::layout::READING_WIDTH, 1.0).natural_width();
    if natural <= 0.0 {
        return most;
    }
    (width / natural).clamp(1.0, most)
}

/// A column of parts built top to bottom, then set in place.
struct Column {
    parts: Vec<Part>,
    height: f32,
}

impl Column {
    fn new() -> Column {
        Column {
            parts: vec![],
            height: 0.0,
        }
    }

    /// Adds `part` under what is there, `gap` points below it.
    fn push(&mut self, mut part: Part, gap: f32) {
        let gap = if self.parts.is_empty() { 0.0 } else { gap };
        part.y = self.height + gap;
        self.height = part.y + part.height();
        self.parts.push(part);
    }

    /// Adds a row of parts side by side, each already placed across the
    /// column, `gap` points below what is there.
    fn push_row(&mut self, row: Vec<Part>, gap: f32) {
        let gap = if self.parts.is_empty() { 0.0 } else { gap };
        let top = self.height + gap;
        let mut bottom = top;
        for mut part in row {
            part.y += top;
            bottom = bottom.max(part.y + part.height());
            self.parts.push(part);
        }
        self.height = bottom;
    }

    /// The parts, centered as a group between the top margin and the
    /// content's bottom.
    fn centered(self) -> Vec<Part> {
        let room = CONTENT_BOTTOM - MARGIN_Y;
        let top = MARGIN_Y + ((room - self.height) / 2.0).max(0.0).round();
        self.parts
            .into_iter()
            .map(|mut part| {
                part.y += top;
                part
            })
            .collect()
    }
}

fn part(kind: &'static str, text: String, rich: Rich, magnification: f32, x: f32) -> Part {
    Part {
        kind,
        text,
        content: Content::Rich(Box::new(rich)),
        magnification,
        x,
        y: 0.0,
    }
}

/// A part laid out across the column at `magnification`, left-aligned.
fn left(kind: &'static str, node: Node<()>, magnification: f32) -> Part {
    let label = plain(&node);
    let rich = lay(node, COLUMN, magnification);
    part(kind, label, rich, magnification, MARGIN_X)
}

/// A part centered in the span `x` to `x + width`, broken to fit it. A
/// text is laid out across the span with its lines centered; anything
/// else (prose, a card) keeps its own width and sits in the middle.
fn centered_in(
    kind: &'static str,
    mut node: Node<()>,
    magnification: f32,
    x: f32,
    width: f32,
) -> Part {
    let label = plain(&node);
    if matches!(node.element, Element::Text { .. }) {
        node.style.align = Some(TextAlign::Center);
        return part(
            kind,
            label,
            lay(node, width, magnification),
            magnification,
            x,
        );
    }
    let rich = lay(node, width, magnification);
    let drawn = rich.natural_width() * magnification;
    let at = x + ((width - drawn) / 2.0).max(0.0).round();
    part(kind, label, rich, magnification, at)
}

fn centered(kind: &'static str, node: Node<()>, magnification: f32) -> Part {
    centered_in(kind, node, magnification, MARGIN_X, COLUMN)
}

/// The plain text of a node, for the outline.
pub fn plain(node: &Node<()>) -> String {
    match &node.element {
        Element::Text { value, .. } => value.clone(),
        Element::Markdown { blocks } => markdown::plain(blocks),
        Element::Stack { children, .. } => {
            children.iter().map(plain).collect::<Vec<_>>().join("\n")
        }
        _ => String::new(),
    }
}

/// The slide at `index` of `deck`, laid out on the canvas. Slides carry no
/// page number.
pub fn compose(deck: &Deck, index: usize) -> Composed {
    let Some(slide) = deck.slide(index) else {
        return Composed { parts: vec![] };
    };
    Composed { parts: body(slide) }
}

/// The slide's parts.
fn body(slide: &Slide) -> Vec<Part> {
    let tones = tones();
    let mut column = Column::new();
    let muted = Some(tones.muted);
    let kicker = |column: &mut Column, center: bool| {
        if let Some(kicker) = &slide.kicker {
            let node = text("kicker", kicker, TextRole::Status, muted);
            column.push(
                if center {
                    centered("kicker", node, 1.0)
                } else {
                    left("kicker", node, 1.0)
                },
                0.0,
            );
        }
    };
    let note = |column: &mut Column, center: bool| {
        if let Some(note) = &slide.note {
            let node = text("note", note, TextRole::Status, muted);
            column.push(
                if center {
                    centered("note", node, 1.0)
                } else {
                    left("note", node, 1.0)
                },
                NOTE_GAP,
            );
        }
    };
    // Every layout that takes a title draws it the same way.
    let title = |column: &mut Column| {
        if let Some(title) = &slide.title {
            let node = text("title", title, TextRole::Heading, None);
            column.push(left("title", node, TITLE), KICKER_GAP);
        }
    };
    let prose_blocks = || markdown::parse(&slide.body);
    match slide.layout() {
        Layout::Title | Layout::Banner => {
            kicker(&mut column, true);
            let heading = text(
                "title",
                slide.title.as_deref().unwrap_or(&slide.id),
                TextRole::Heading,
                None,
            );
            let magnification = fitting(&heading, COLUMN, DISPLAY);
            column.push(centered("title", heading, magnification), KICKER_GAP);
            if let Some(lead) = &slide.lead {
                let node = text("lead", lead, TextRole::Body, muted);
                column.push(centered("lead", node, 1.0), BODY_GAP);
            }
            if !slide.body.is_empty() {
                column.push(
                    centered("body", prose("body", prose_blocks()), 1.0),
                    BODY_GAP,
                );
            }
            note(&mut column, true);
        }
        Layout::Statement => {
            kicker(&mut column, true);
            column.push(
                centered("body", prose("body", prose_blocks()), STATEMENT),
                BODY_GAP,
            );
            note(&mut column, true);
        }
        Layout::Points => {
            kicker(&mut column, false);
            title(&mut column);
            column.push(left("body", prose("body", prose_blocks()), 1.0), BODY_GAP);
            note(&mut column, false);
        }
        Layout::Compare => {
            kicker(&mut column, false);
            title(&mut column);
            let table = compare_table(&slide.columns, &slide.rows);
            column.push(left("table", prose("table", table), 1.0), BODY_GAP);
            if !slide.body.is_empty() {
                column.push(left("body", prose("body", prose_blocks()), 1.0), NOTE_GAP);
            }
            note(&mut column, false);
        }
        Layout::Metrics => {
            kicker(&mut column, false);
            title(&mut column);
            column.push_row(metrics(&slide.metrics), BODY_GAP);
            note(&mut column, false);
        }
        Layout::Flow => {
            kicker(&mut column, false);
            title(&mut column);
            column.push_row(flow(&slide.steps), BODY_GAP);
            note(&mut column, false);
        }
        Layout::Quote => {
            kicker(&mut column, true);
            let inner = COLUMN * 0.84;
            let passage = card("quote", Space::Lg, vec![prose("passage", prose_blocks())]);
            column.push(
                centered_in(
                    "quote",
                    passage,
                    1.0,
                    MARGIN_X + (COLUMN - inner) / 2.0,
                    inner,
                ),
                BODY_GAP,
            );
            if let Some(lead) = &slide.lead {
                let node = text("lead", lead, TextRole::Body, muted);
                column.push(centered("lead", node, 1.0), 14.0);
            }
            note(&mut column, false);
        }
        Layout::Image => {
            kicker(&mut column, false);
            title(&mut column);
            let gap = if column.parts.is_empty() {
                0.0
            } else {
                BODY_GAP
            };
            // Room for a note under the image, when there is one.
            let note_room = if slide.note.is_some() { 48.0 } else { 0.0 };
            let room = CONTENT_BOTTOM - MARGIN_Y - column.height - gap - note_room;
            column.push(image_part(slide, room), gap);
            note(&mut column, false);
        }
        Layout::Ask => {
            kicker(&mut column, false);
            title(&mut column);
            let facts_width = COLUMN * 0.34;
            let facts_node = prose("facts", markdown::parse(&plain_facts(&slide.metrics)));
            let facts = part(
                "facts",
                plain(&facts_node),
                lay(facts_node, facts_width, 1.0),
                1.0,
                MARGIN_X,
            );
            let prose_x = MARGIN_X + facts_width + CELL_GAP;
            let body_width = COLUMN - facts_width - CELL_GAP;
            let body = part(
                "body",
                slide.body.clone(),
                lay(prose("body", prose_blocks()), body_width, 1.0),
                1.0,
                prose_x,
            );
            column.push_row(vec![facts, body], BODY_GAP);
            note(&mut column, false);
        }
    }
    column.centered()
}

/// An image slide's image across the column, `height` points tall, or a
/// line saying what is missing when the script names an image the deck
/// doesn't carry.
fn image_part(slide: &Slide, height: f32) -> Part {
    let Some(shown) = &slide.image else {
        return left(
            "body",
            text("body", "This slide names no image.", TextRole::Body, None),
            1.0,
        );
    };
    match crate::slide::asset(&shown.path)
        .ok_or_else(|| format!("{} is not in the deck", shown.path))
        .and_then(Image::png)
    {
        Ok(image) => Part {
            kind: "image",
            text: shown.alt.clone(),
            content: Content::Image {
                image: Arc::new(image),
                width: COLUMN,
                height: height.max(1.0),
            },
            magnification: 1.0,
            x: MARGIN_X,
            y: 0.0,
        },
        Err(complaint) => left(
            "body",
            text(
                "body",
                &format!("Missing image: {complaint}"),
                TextRole::Body,
                None,
            ),
            1.0,
        ),
    }
}

/// An ask's facts: each value in bold before its label, a paragraph each.
fn plain_facts(metrics: &[Metric]) -> String {
    metrics
        .iter()
        .map(|metric| format!("**{}** {}\n\n", metric.shown(), metric.label))
        .collect()
}

/// A comparison as a Markdown table: an empty corner, then the columns;
/// each row's label in bold.
fn compare_table(columns: &[String], rows: &[crate::slide::Row]) -> Vec<MarkdownBlock> {
    let cell = |value: &str| value.replace('|', "\\|");
    let width = columns.len();
    let mut source = format!(
        "| |{}|\n|---|{}|\n",
        columns
            .iter()
            .map(|c| cell(c))
            .collect::<Vec<_>>()
            .join("|"),
        vec!["---"; width].join("|")
    );
    for row in rows {
        let mut cells: Vec<String> = row.cells.iter().map(|c| cell(c)).collect();
        cells.resize(width, String::new());
        source.push_str(&format!("|**{}**|{}|\n", cell(&row.label), cells.join("|")));
    }
    markdown::parse(&source)
}

/// Numbers side by side, each large over its label, each centered in an
/// equal share of the column.
fn metrics(metrics: &[Metric]) -> Vec<Part> {
    let count = metrics.len().max(1) as f32;
    let share = (COLUMN - CELL_GAP * (count - 1.0)) / count;
    let muted = Some(tones().muted);
    // One size for every number, the largest at which the widest fits.
    let magnification = metrics
        .iter()
        .map(|metric| {
            let node = text("value", metric.shown(), TextRole::Heading, None);
            fitting(&node, share, NUMBER)
        })
        .fold(NUMBER, f32::min);
    let mut parts = vec![];
    for (index, metric) in metrics.iter().enumerate() {
        let x = MARGIN_X + index as f32 * (share + CELL_GAP);
        let value = text(
            &format!("value-{index}"),
            metric.shown(),
            TextRole::Heading,
            metric.is_unfilled().then_some(muted).flatten(),
        );
        let number = centered_in("metric", value, magnification, x, share);
        let top = number.height() + 6.0;
        let mut label = centered_in(
            "label",
            text(
                &format!("label-{index}"),
                &metric.label,
                TextRole::Body,
                muted,
            ),
            1.0,
            x,
            share,
        );
        label.y = top;
        parts.push(number);
        parts.push(label);
    }
    parts
}

/// The stages of a run: a card a stage, each hugging its words and
/// centered in an equal share of the column, an arrow between two.
fn flow(steps: &[String]) -> Vec<Part> {
    let count = steps.len().max(1) as f32;
    let arrow = 18.0;
    let share = (COLUMN - arrow * (count - 1.0)) / count;
    let muted = Some(tones().muted);
    let pad = 16.0;
    let mut parts: Vec<Part> = vec![];
    let mut height: f32 = 0.0;
    for (index, step) in steps.iter().enumerate() {
        let x = MARGIN_X + index as f32 * (share + arrow);
        let words = text(&format!("step-{index}"), step, TextRole::Body, None);
        // A little more than the words measure, so a card never breaks
        // the words it was fitted to.
        let hug = lay(words.clone(), share - 2.0 * pad, 1.0).natural_width() + 2.0 * pad + 4.0;
        let boxed = card(&format!("card-{index}"), Space::Md, vec![words]);
        let part = centered_in(
            "step",
            boxed,
            1.0,
            x + ((share - hug) / 2.0).max(0.0),
            hug.min(share),
        );
        height = height.max(part.height());
        parts.push(part);
    }
    let mut arrows = vec![];
    for index in 1..parts.len() {
        // Midway between the two cards' edges.
        let (before, after) = (&parts[index - 1], &parts[index]);
        let middle = (before.x + before.natural_width() + after.x) / 2.0;
        let mut part = centered_in(
            "arrow",
            text(&format!("arrow-{index}"), "→", TextRole::Body, muted),
            1.0,
            (middle - arrow / 2.0).round(),
            arrow,
        );
        part.y = ((height - part.height()) / 2.0).round();
        arrows.push(part);
    }
    parts.extend(arrows);
    parts
}

/// The presenter's notes for `slide`, one paragraph a line, laid out
/// `width` points wide.
pub fn notes(slide: Option<&Slide>, width: f32) -> Rich {
    let tones = tones();
    let lines: Vec<String> = slide.map(|slide| slide.notes.clone()).unwrap_or_default();
    // A rule between the slide and its notes.
    let mut children = vec![prose("rule", vec![MarkdownBlock::Rule])];
    if lines.is_empty() {
        children.push(text(
            "none",
            "No presenter note.",
            TextRole::Body,
            Some(tones.muted),
        ));
    }
    children.extend(
        lines
            .iter()
            .enumerate()
            .map(|(index, line)| text(&format!("note-{index}"), line, TextRole::Body, None)),
    );
    let stack = node(
        "notes",
        Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    );
    lay(stack, width, 1.0)
}

/// A line of muted status text, such as "Go to 12", `width` points wide.
pub fn status(value: &str, width: f32) -> Rich {
    lay(
        text("status", value, TextRole::Status, Some(tones().muted)),
        width,
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script;

    fn deck(source: &str) -> Deck {
        script::parse(source).expect("the script parses")
    }

    #[test]
    fn a_title_is_large_and_centered() {
        let deck = deck("layout: title\nid: t\ntitle: Test-Time Capabilities\n");
        let slide = compose(&deck, 0);
        let title = slide.parts.iter().find(|p| p.kind == "title").unwrap();
        assert!(title.magnification >= 2.5);
        let rich = title.rich().unwrap();
        let drawn = (rich.inset() + rich.natural_width()) / 2.0;
        let middle = title.x + drawn * title.magnification;
        assert!(
            (middle - WIDTH / 2.0).abs() < 2.0,
            "centered at {middle}: x {} inset {} natural {} width {} mag {}",
            title.x,
            rich.inset(),
            rich.natural_width(),
            rich.width(),
            title.magnification
        );
        assert!(
            slide.parts.iter().all(|p| p.kind != "foot"),
            "slides carry no page number"
        );
    }

    #[test]
    fn a_comparison_is_a_table_with_a_corner() {
        let blocks = compare_table(
            &["A".into(), "B".into()],
            &[crate::slide::Row {
                label: "Row".into(),
                cells: vec!["1".into()],
            }],
        );
        let MarkdownBlock::Table { header, rows, .. } = &blocks[0] else {
            panic!("a table: {blocks:?}");
        };
        assert_eq!(header.len(), 3);
        assert_eq!(rows[0].len(), 3);
    }

    #[test]
    fn metrics_sit_side_by_side_over_their_labels() {
        let deck = deck("layout: metrics\nid: m\ntitle: T\nmetric: 170 | ms\nmetric: 700 | ms\n");
        let slide = compose(&deck, 0);
        let numbers: Vec<&Part> = slide.parts.iter().filter(|p| p.kind == "metric").collect();
        assert_eq!(numbers.len(), 2);
        assert_eq!(numbers[0].y, numbers[1].y);
        assert!(numbers[0].x < numbers[1].x);
        let label = slide.parts.iter().find(|p| p.kind == "label").unwrap();
        assert!(label.y >= numbers[0].y + numbers[0].height());
    }
}
