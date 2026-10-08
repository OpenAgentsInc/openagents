//! A reply's Markdown as styled display lines.
//!
//! The layout of tables, links, quote bars, and rules is ported from
//! grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI),
//! `xai-grok-markdown/src/parse.rs`: a boxed table with a bold header and
//! dim borders, `[text](url)` as "text (url)", the bar through a quote's
//! blank rows, a three-cell rule.
//!
//! `pulldown-cmark` events project onto a tree of blocks — paragraphs,
//! headings, fenced code, quotes, lists, tables, rules — each holding
//! flat inline runs with their marks. [`render`] lays the tree out as
//! [`Marked`] lines: marked text the draw loop wraps and styles. Raw
//! HTML in the source is text. Fenced code in a language syntect knows
//! carries grok-build's highlighting per run: its Grok Night / Grok Day
//! token colors, quantized for the terminal (`code_highlight::grok`). A
//! fence still streaming in is highlighted incrementally, each committed
//! line once, as grok-build does.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::LazyLock;

use code_highlight::grok::{self, ColorLevel, HlLine, OpenCodeHighlighter, Token};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};

use crate::{Colors, Intensity, Ladder, wrap_rows};

/// The marks on one run of text. Marks nest, so a run inside `**_a_**`
/// carries both `bold` and `italic`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    /// The destination of the innermost enclosing link, when there is one.
    pub link: Option<String>,
    /// The source of an image. The run's text is the image's alt text.
    pub image: Option<String>,
    /// In fenced code, the token's look from grok-build's theme.
    pub syntax: Option<Token>,
    /// The heading level, 1 to 6, of a heading's text.
    pub heading: Option<u8>,
    /// Layout the Markdown drew rather than wrote: a list's bullet or
    /// number, a quote's bar, a rule, a table's borders. grok-build draws
    /// it muted.
    pub muted: bool,
    /// Muted and dimmed as well: a quote's bar, a table's borders.
    pub dim: bool,
}

/// The color level syntax colors draw at: none when the ladder has no
/// color (`NO_COLOR`), truecolor when it draws RGB, and otherwise what
/// grok-build's detection finds (256, 16, or a truecolor terminal that
/// did not say so in `COLORTERM`).
pub fn syntax_level(ladder: Ladder) -> ColorLevel {
    match ladder.colors() {
        Colors::None => ColorLevel::None,
        Colors::True => ColorLevel::TrueColor,
        Colors::Indexed => grok::color::get(),
    }
}

/// grok-build's choice of palette for the field code is drawn on: Grok Day
/// on a light field, Grok Night on a dark one. The ladder paints its own
/// near-black field, so that is the polarity; only a colorless ladder
/// leaves the terminal's own background showing, and then the terminal's
/// polarity decides (`OPENAGENTS_APPEARANCE`, `COLORFGBG`; Grok Night when
/// nothing says).
pub fn palette_for(ladder: Ladder) -> grok::Palette {
    match grok::color::resolve_to_rgb(ladder.background()) {
        Some((r, g, b)) => grok::Palette::for_appearance(Some(grok::Appearance::of_field(r, g, b))),
        None => grok::Palette::for_appearance(grok::Appearance::detect(None)),
    }
}

/// This process's palette, chosen once from the environment's ladder.
pub fn palette() -> grok::Palette {
    static CHOSEN: LazyLock<grok::Palette> = LazyLock::new(|| {
        let _ = grok::set_palette(palette_for(Ladder::from_environment()));
        grok::palette()
    });
    *CHOSEN
}

/// The style a code run draws in: its token's grok-build color at the
/// ladder's level, or the top of the ladder when the fence's language is
/// unknown.
pub fn code_style(syntax: Option<Token>, ladder: Ladder) -> Style {
    match syntax {
        Some(token) => {
            let mut style = token.style(ColorLevel::TrueColor);
            style.fg = style
                .fg
                .map(|color| crate::ladder::appearance(color, syntax_level(ladder)));
            style
        }
        None => ladder.style(Intensity::Full),
    }
}

impl Marks {
    /// Applies the terminal's Markdown marks without emitting terminal escapes.
    pub fn style(&self, base: Style, ladder: Ladder) -> Style {
        let mut style = if self.code {
            code_style(self.syntax, ladder).bg(ladder.background())
        } else {
            base
        };
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if self.strike {
            style = style.add_modifier(Modifier::CROSSED_OUT);
        }
        if self.link.is_some() || self.image.is_some() {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        style
    }
}

/// The marks of layout the Markdown drew: bullets, bars, rules, borders.
const MUTED: Marks = Marks {
    bold: false,
    italic: false,
    code: false,
    strike: false,
    link: None,
    image: None,
    syntax: None,
    heading: None,
    muted: true,
    dim: false,
};

/// The marks of a quote's bar and a table's borders: muted and dim.
const BORDER: Marks = Marks {
    bold: false,
    italic: false,
    code: false,
    strike: false,
    link: None,
    image: None,
    syntax: None,
    heading: None,
    muted: true,
    dim: true,
};

/// Pushes a line's lead: list bullets and numbers muted, quote bars
/// muted and dim, as grok-build draws them.
fn push_prefix(marked: &mut Marked, prefix: &str) {
    for ch in prefix.chars() {
        let marks = if ch == '│' { &BORDER } else { &MUTED };
        marked.push(ch.encode_utf8(&mut [0; 4]), marks);
    }
}

/// Text plus the marks it carries: non-overlapping runs, sorted, covering
/// the whole string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marked {
    pub text: String,
    /// `(byte range, marks)` pairs covering `text` in order.
    pub runs: Vec<(Range<usize>, Marks)>,
}

impl Marked {
    /// Text with no marks at all.
    pub fn plain(text: impl Into<String>) -> Marked {
        let text = text.into();
        Marked {
            runs: vec![(0..text.len(), Marks::default())],
            text,
        }
    }

    /// The runs covering `bytes`, clipped to it — what one wrapped row of
    /// this text draws.
    pub fn runs_in(&self, bytes: Range<usize>) -> Vec<(String, Marks)> {
        let mut out = Vec::new();
        for (range, marks) in &self.runs {
            let start = range.start.max(bytes.start);
            let end = range.end.min(bytes.end);
            if start < end {
                out.push((self.text[start..end].to_string(), marks.clone()));
            }
        }
        out
    }

    /// Appends `text` carrying `marks`, joining the last run when the
    /// marks match.
    fn push(&mut self, text: &str, marks: &Marks) {
        if text.is_empty() {
            return;
        }
        if let Some((range, held)) = self.runs.last_mut()
            && *held == *marks
        {
            range.end += text.len();
            self.text.push_str(text);
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        self.runs.push((start..self.text.len(), marks.clone()));
    }
}

/// One logical display line of a rendered reply: marked text, the white
/// it draws at, and `hang` — extra cells continuation rows indent by so
/// a list item's wraps sit under its text, not its marker.
pub struct Rendered {
    pub marked: Marked,
    pub intensity: Intensity,
    pub hang: usize,
    /// A line of a code block, which draws on grok-build's code band.
    pub code: bool,
}

/// Lays `source` out as display lines. Blocks separate with a blank
/// line; a paragraph is one logical line (the draw loop wraps it), a
/// code block is one line per source line.
pub fn render(source: &str) -> Vec<Rendered> {
    let mut lines = Vec::new();
    blocks_lines(&parse(source), "", 0, None, &mut lines);
    lines
}

/// The source of each code block in `source`, in order: what a copy of
/// one puts on the clipboard.
pub fn code_blocks(source: &str) -> Vec<String> {
    fn collect(blocks: &[Block], out: &mut Vec<String>) {
        for block in blocks {
            match block {
                Block::Code { source, .. } => out.push(source.clone()),
                Block::Quote(blocks) => collect(blocks, out),
                Block::List { items, .. } => {
                    for item in items {
                        collect(&item.blocks, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    collect(&parse(source), &mut out);
    out
}

/// Renders physical rows, preserving inline marks and hanging list indentation.
/// Code wraps by row; tables wrap within each cell before their borders are drawn.
pub fn wrapped(source: &str, width: usize) -> Vec<Rendered> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut lines = Vec::new();
    blocks_lines(&parse(source), "", 0, Some(width), &mut lines);
    for rendered in lines {
        let hang = rendered.hang.min(width.saturating_sub(1));
        for (index, range) in wrap_rows(&rendered.marked.text, width.saturating_sub(hang))
            .into_iter()
            .enumerate()
        {
            let mut marked = Marked::default();
            if index > 0 {
                marked.push(&" ".repeat(hang), &Marks::default());
            }
            for (text, marks) in rendered.marked.runs_in(range) {
                marked.push(&text, &marks);
            }
            rows.push(Rendered {
                marked,
                intensity: rendered.intensity,
                hang: 0,
                code: rendered.code,
            });
        }
    }
    rows
}

fn blocks_lines(
    blocks: &[Block],
    prefix: &str,
    hang: usize,
    viewport: Option<usize>,
    out: &mut Vec<Rendered>,
) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            // Inside a quote the bar runs through the blank row, as
            // grok-build draws it ("│ Foo", "│", "│ Bar").
            let mut marked = Marked::default();
            push_prefix(&mut marked, prefix.trim_end());
            out.push(Rendered {
                marked,
                intensity: Intensity::Half,
                hang: 0,
                code: false,
            });
        }
        block_lines(block, prefix, hang, viewport, out);
    }
}

fn block_lines(
    block: &Block,
    prefix: &str,
    hang: usize,
    viewport: Option<usize>,
    out: &mut Vec<Rendered>,
) {
    match block {
        Block::Paragraph(inlines) => {
            out.push(marked_line(inlines, prefix, hang, Intensity::ThreeQuarters));
        }
        Block::Heading { level, inlines } => {
            let inlines: Vec<Inline> = inlines
                .iter()
                .map(|inline| Inline {
                    text: inline.text.clone(),
                    marks: Marks {
                        bold: true,
                        heading: Some(*level),
                        ..inline.marks.clone()
                    },
                })
                .collect();
            let intensity = if *level <= 2 {
                Intensity::Full
            } else {
                Intensity::ThreeQuarters
            };
            out.push(marked_line(&inlines, prefix, hang, intensity));
        }
        Block::Code {
            language,
            source,
            body,
            start,
            open,
        } => {
            let highlighted = language
                .as_deref()
                .and_then(|language| highlighted(language, body, *start, *open));
            let plain = Marks {
                code: true,
                ..Marks::default()
            };
            for (index, line) in source
                .split('\n')
                .filter(|_| !source.is_empty())
                .enumerate()
            {
                let line = line.strip_suffix('\r').unwrap_or(line);
                let mut marked = Marked::default();
                push_prefix(&mut marked, prefix);
                match highlighted.as_ref().and_then(|lines| lines.get(index)) {
                    Some(segments) => code_runs(&mut marked, segments),
                    None => marked.push(line, &plain),
                }
                out.push(Rendered {
                    marked,
                    intensity: Intensity::ThreeQuarters,
                    hang,
                    code: true,
                });
            }
        }
        Block::Quote(blocks) => {
            blocks_lines(blocks, &format!("{prefix}│ "), hang + 2, viewport, out);
        }
        Block::List { start, items } => {
            for (index, item) in items.iter().enumerate() {
                let marker = match (start, item.task) {
                    (_, Some(true)) => "[x] ".to_string(),
                    (_, Some(false)) => "[ ] ".to_string(),
                    (Some(first), None) => format!("{}. ", first + index as u64),
                    (None, None) => "• ".to_string(),
                };
                item_lines(item, &marker, prefix, hang, viewport, out);
            }
        }
        Block::Table { header, rows, .. } => {
            table_lines(header, rows, prefix, hang, viewport, out);
        }
        Block::Rule => out.push(Rendered {
            marked: {
                let mut marked = Marked::default();
                marked.push(&format!("{prefix}───"), &MUTED);
                marked
            },
            intensity: Intensity::Half,
            hang,
            code: false,
        }),
    }
}

/// Pushes one highlighted line's segments as code runs, each carrying its
/// token, with the line ending dropped.
fn code_runs(marked: &mut Marked, segments: &HlLine) {
    for (style, text) in segments {
        let text = text.trim_end_matches(['\n', '\r']);
        let marks = Marks {
            code: true,
            syntax: Some(Token::from_syntect(*style)),
            ..Marks::default()
        };
        marked.push(text, &marks);
    }
}

/// A code block's highlighted lines, through grok-build's streaming
/// highlighter: a fence still open at the end of the source resumes from
/// its last committed line, a closed one is memoized on its body. The draw
/// loop lays a streaming reply out every frame, so this is what keeps a
/// long fence from being re-highlighted whole on every chunk. syntect's
/// parse state is not `Send`, so each drawing thread keeps its own.
fn highlighted(language: &str, body: &str, start: usize, open: bool) -> Option<Vec<HlLine>> {
    thread_local! {
        static HIGHLIGHTER: RefCell<Option<OpenCodeHighlighter>> = const { RefCell::new(None) };
    }
    let syntect = palette().syntect();
    HIGHLIGHTER.with(|held| {
        held.borrow_mut()
            .get_or_insert_with(|| OpenCodeHighlighter::new(syntect))
            .highlight_block(syntect, language, start, open, body)
    })
}

/// Boxes a table with grok-build's muted borders and bold header. With a
/// viewport, column widths share its budget and each marked cell wraps before
/// physical rows are assembled. A narrow viewport uses labeled cells instead.
fn table_lines(
    header: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    prefix: &str,
    hang: usize,
    viewport: Option<usize>,
    out: &mut Vec<Rendered>,
) {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    let columns = std::iter::once(header.len())
        .chain(rows.iter().map(Vec::len))
        .max()
        .unwrap_or(0);
    if columns == 0 {
        return;
    }
    let mut widths = vec![0usize; columns];
    let mut word_widths = vec![0usize; columns];
    let mut floors = vec![0usize; columns];
    for row in std::iter::once(header).chain(rows.iter().map(Vec::as_slice)) {
        for (index, cell) in row.iter().enumerate() {
            let text = text_of(cell);
            widths[index] = widths[index].max(text.split('\n').map(str::width).max().unwrap_or(0));
            word_widths[index] =
                word_widths[index].max(text.split_whitespace().map(str::width).max().unwrap_or(0));
            floors[index] = floors[index].max(
                text.graphemes(true)
                    .filter(|text| *text != "\n")
                    .map(|text| text.width().max(1))
                    .max()
                    .unwrap_or(0),
            );
            word_widths[index] = word_widths[index].max(floors[index]);
            widths[index] = widths[index].max(word_widths[index]);
        }
    }
    if let Some(viewport) = viewport {
        let overhead = columns.saturating_mul(3).saturating_add(1);
        let available = viewport.saturating_sub(prefix.width());
        let budget = available.saturating_sub(overhead);
        if available < overhead || floors.iter().sum::<usize>() > budget {
            stacked_table(header, rows, prefix, viewport, out);
            return;
        }
        if widths.iter().sum::<usize>() > budget {
            // Preserve whole words when they fit, otherwise grow from the
            // widest grapheme in each column toward its word width.
            let (base, target) = if word_widths.iter().sum::<usize>() <= budget {
                (word_widths, widths)
            } else {
                (floors, word_widths)
            };
            widths = table_widths(base, &target, budget);
        }
    }
    // These are already physical table rows. A list's hanging indent must
    // not reduce their width again in the outer wrapping pass.
    let hang = if viewport.is_some() { 0 } else { hang };
    let border = |left: char, mid: char, right: char| {
        let mut line = String::new();
        line.push(left);
        for (index, width) in widths.iter().enumerate() {
            line.push_str(&"─".repeat(width + 2));
            if index + 1 < columns {
                line.push(mid);
            }
        }
        line.push(right);
        let mut marked = Marked::default();
        push_prefix(&mut marked, prefix);
        marked.push(&line, &BORDER);
        Rendered {
            marked,
            intensity: Intensity::Half,
            hang,
            code: false,
        }
    };
    let row_lines = |cells: &[Vec<Inline>], bold: bool| {
        let wrapped: Vec<Vec<Marked>> = widths
            .iter()
            .enumerate()
            .map(|(index, width)| {
                let cell = cells.get(index).map(Vec::as_slice).unwrap_or(&[]);
                wrap_marked(&cell_marked(cell, bold), *width)
            })
            .collect();
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
        (0..height)
            .map(|line| {
                let mut marked = Marked::default();
                push_prefix(&mut marked, prefix);
                marked.push("│", &BORDER);
                for (index, width) in widths.iter().enumerate() {
                    marked.push(" ", &Marks::default());
                    let mut used = 0;
                    if let Some(cell) = wrapped[index].get(line) {
                        for (text, marks) in cell.runs_in(0..cell.text.len()) {
                            marked.push(&text, &marks);
                        }
                        used = cell.text.width();
                    }
                    marked.push(
                        &" ".repeat(width.saturating_sub(used) + 1),
                        &Marks::default(),
                    );
                    marked.push("│", &BORDER);
                }
                Rendered {
                    marked,
                    intensity: if bold {
                        Intensity::Full
                    } else {
                        Intensity::ThreeQuarters
                    },
                    hang,
                    code: false,
                }
            })
            .collect::<Vec<_>>()
    };
    out.push(border('┌', '┬', '┐'));
    out.extend(row_lines(header, true));
    out.push(border('├', '┼', '┤'));
    for (index, row) in rows.iter().enumerate() {
        out.extend(row_lines(row, false));
        if index + 1 < rows.len() {
            out.push(border('├', '┼', '┤'));
        }
    }
    out.push(border('└', '┴', '┘'));
}

/// Shares spare cells in proportion to each column's unmet width.
fn table_widths(mut widths: Vec<usize>, targets: &[usize], budget: usize) -> Vec<usize> {
    let extra = budget.saturating_sub(widths.iter().sum());
    let wants: Vec<_> = targets
        .iter()
        .zip(&widths)
        .map(|(target, base)| target.saturating_sub(*base))
        .collect();
    let total: usize = wants.iter().sum();
    if total == 0 {
        return widths;
    }
    for (width, want) in widths.iter_mut().zip(&wants) {
        *width += ((*want as u128 * extra as u128) / total as u128) as usize;
    }
    let mut remaining = budget.saturating_sub(widths.iter().sum());
    let mut indices: Vec<_> = (0..widths.len()).collect();
    indices.sort_by_key(|&index| std::cmp::Reverse(targets[index].saturating_sub(widths[index])));
    for index in indices {
        if remaining == 0 {
            break;
        }
        if widths[index] < targets[index] {
            widths[index] += 1;
            remaining -= 1;
        }
    }
    widths
}

fn cell_marked(inlines: &[Inline], bold: bool) -> Marked {
    let mut marked = Marked::default();
    for inline in inlines {
        marked.push(
            &inline.text,
            &Marks {
                bold: bold || inline.marks.bold,
                ..inline.marks.clone()
            },
        );
    }
    marked
}

fn wrap_marked(marked: &Marked, width: usize) -> Vec<Marked> {
    wrap_rows(&marked.text, width)
        .into_iter()
        .map(|range| {
            let mut row = Marked::default();
            for (text, marks) in marked.runs_in(range) {
                row.push(&text, &marks);
            }
            row
        })
        .collect()
}

/// Retains all cells when borders and grapheme floors cannot fit together.
fn stacked_table(
    header: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    prefix: &str,
    viewport: usize,
    out: &mut Vec<Rendered>,
) {
    use unicode_width::UnicodeWidthStr;
    let width = viewport.saturating_sub(prefix.width()).max(1);
    for (row_index, cells) in rows.iter().enumerate() {
        if row_index > 0 {
            out.push(Rendered {
                marked: Marked::default(),
                intensity: Intensity::Half,
                hang: 0,
                code: false,
            });
        }
        for index in 0..header.len().max(cells.len()) {
            let mut cell = cell_marked(header.get(index).map(Vec::as_slice).unwrap_or(&[]), true);
            if !cell.text.is_empty() {
                cell.push(": ", &MUTED);
            }
            for (text, marks) in
                cell_marked(cells.get(index).map(Vec::as_slice).unwrap_or(&[]), false)
                    .runs_in(0..usize::MAX)
            {
                cell.push(&text, &marks);
            }
            for row in wrap_marked(&cell, width) {
                let mut marked = Marked::default();
                push_prefix(&mut marked, prefix);
                for (text, marks) in row.runs_in(0..row.text.len()) {
                    marked.push(&text, &marks);
                }
                out.push(Rendered {
                    marked,
                    intensity: Intensity::ThreeQuarters,
                    hang: 0,
                    code: false,
                });
            }
        }
    }
    if rows.is_empty() {
        for cell in header {
            for row in wrap_marked(&cell_marked(cell, true), width) {
                let mut marked = Marked::default();
                push_prefix(&mut marked, prefix);
                for (text, marks) in row.runs_in(0..row.text.len()) {
                    marked.push(&text, &marks);
                }
                out.push(Rendered {
                    marked,
                    intensity: Intensity::Full,
                    hang: 0,
                    code: false,
                });
            }
        }
    }
}

/// A list item's blocks: the marker leads the first line, padding the
/// rest, and the item's wraps hang under the marker.
fn item_lines(
    item: &Item,
    marker: &str,
    prefix: &str,
    hang: usize,
    viewport: Option<usize>,
    out: &mut Vec<Rendered>,
) {
    let width = marker.chars().count();
    for (index, block) in item.blocks.iter().enumerate() {
        // A nested list sits right under its item's text, as grok-build
        // draws a tight list.
        if index > 0 && !matches!(block, Block::List { .. }) {
            out.push(Rendered {
                marked: Marked::default(),
                intensity: Intensity::Half,
                hang: 0,
                code: false,
            });
        }
        let lead = if index == 0 {
            marker.to_string()
        } else {
            " ".repeat(width)
        };
        block_lines(
            block,
            &format!("{prefix}{lead}"),
            hang + width,
            viewport,
            out,
        );
    }
}

fn marked_line(inlines: &[Inline], prefix: &str, hang: usize, intensity: Intensity) -> Rendered {
    let mut marked = Marked::default();
    push_prefix(&mut marked, prefix);
    for inline in inlines {
        marked.push(&inline.text, &inline.marks);
    }
    Rendered {
        marked,
        intensity,
        hang,
        code: false,
    }
}

// --- The parse: pulldown-cmark events projected onto a block tree. ---

/// One block of a document. Quotes and list items hold blocks of their own.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Block {
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    /// A fenced or indented code block. `language` is the first word of
    /// the fence's info string, when it has one.
    ///
    /// `body` is the text as the fence holds it, line endings and all —
    /// what the highlighter reads; `source` drops its last newline. `start`
    /// is the block's byte offset in the document, and `open` says its text
    /// runs to the end of the document: a fence still streaming in.
    Code {
        language: Option<String>,
        source: String,
        body: String,
        start: usize,
        open: bool,
    },
    Quote(Vec<Block>),
    /// `start` is `None` for a bullet list and the first number for an
    /// ordered one.
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Table {
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
}

/// One list item: its task marker, when it has one, and its blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Item {
    /// `Some(true)` for `[x]`, `Some(false)` for `[ ]`.
    task: Option<bool>,
    blocks: Vec<Block>,
}

/// One run of inline text and the marks it carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Inline {
    text: String,
    marks: Marks,
}

/// The extensions the parser turns on.
fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Parses `source` into its top-level blocks.
fn parse(source: &str) -> Vec<Block> {
    let (events, ranges) = Parser::new_ext(source, options())
        .into_offset_iter()
        .unzip();
    let mut cursor = Cursor {
        events,
        ranges,
        len: source.len(),
        at: 0,
    };
    blocks(&mut cursor, None)
}

/// A position in the event stream, with each event's source range.
struct Cursor<'a> {
    events: Vec<Event<'a>>,
    ranges: Vec<Range<usize>>,
    /// The length of the source.
    len: usize,
    at: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&Event<'a>> {
        self.events.get(self.at)
    }

    fn advance(&mut self) {
        self.at += 1;
    }

    fn take(&mut self) -> Option<Event<'a>> {
        let event = self.events.get(self.at).cloned();
        self.at += 1;
        event
    }
}

/// Whether `end` closes the container `open` opened, by variant.
fn closes(end: &TagEnd, open: &TagEnd) -> bool {
    std::mem::discriminant(end) == std::mem::discriminant(open)
}

/// Whether a tag opens a block, as opposed to an inline mark.
fn is_block(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::MetadataBlock(_)
    )
}

/// The blocks up to the end that closes `until`, which is consumed, or to
/// the end of the stream.
fn blocks(cursor: &mut Cursor<'_>, until: Option<TagEnd>) -> Vec<Block> {
    let mut blocks = Vec::new();
    while let Some(event) = cursor.peek() {
        match event {
            Event::End(end) => {
                let done = until.as_ref().is_some_and(|open| closes(end, open));
                cursor.advance();
                if done {
                    break;
                }
            }
            Event::Rule => {
                cursor.advance();
                blocks.push(Block::Rule);
            }
            Event::Start(tag) if is_block(tag) => {
                let tag = tag.clone();
                cursor.advance();
                blocks.extend(block(cursor, tag));
            }
            _ => {
                let inlines = inlines(cursor, None);
                if !text_of(&inlines).trim().is_empty() {
                    blocks.push(Block::Paragraph(inlines));
                }
            }
        }
    }
    blocks
}

/// The blocks the container `tag` contributes, with its end consumed.
fn block(cursor: &mut Cursor<'_>, tag: Tag<'_>) -> Vec<Block> {
    match tag {
        Tag::Paragraph => vec![Block::Paragraph(inlines(cursor, Some(TagEnd::Paragraph)))],
        Tag::Heading { level, .. } => vec![Block::Heading {
            level: heading_level(level),
            inlines: inlines(cursor, Some(TagEnd::Heading(level))),
        }],
        Tag::CodeBlock(kind) => {
            let start = cursor
                .ranges
                .get(cursor.at.wrapping_sub(1))
                .map_or(0, |range| range.start);
            vec![code(cursor, kind, start)]
        }
        Tag::BlockQuote(_) => vec![Block::Quote(blocks(cursor, Some(TagEnd::BlockQuote(None))))],
        Tag::List(start) => vec![list(cursor, start)],
        Tag::Table(_) => vec![table(cursor)],
        Tag::HtmlBlock => {
            let inlines = inlines(cursor, Some(TagEnd::HtmlBlock));
            if text_of(&inlines).trim().is_empty() {
                Vec::new()
            } else {
                vec![Block::Paragraph(inlines)]
            }
        }
        Tag::Item => blocks(cursor, Some(TagEnd::Item)),
        Tag::MetadataBlock(kind) => {
            blocks(cursor, Some(TagEnd::MetadataBlock(kind)));
            Vec::new()
        }
        other => blocks(cursor, Some(other.to_end())),
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn code(cursor: &mut Cursor<'_>, kind: CodeBlockKind<'_>, start: usize) -> Block {
    let language = match kind {
        CodeBlockKind::Fenced(info) => info
            .split_whitespace()
            .next()
            .map(str::to_string)
            .filter(|language| !language.is_empty()),
        CodeBlockKind::Indented => None,
    };
    let mut source = String::new();
    let mut open = false;
    while let Some(event) = cursor.take() {
        let reaches_end = cursor
            .ranges
            .get(cursor.at.wrapping_sub(1))
            .is_some_and(|range| range.end == cursor.len);
        match event {
            Event::End(TagEnd::CodeBlock) => break,
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                source.push_str(&text);
                open = reaches_end;
            }
            Event::SoftBreak | Event::HardBreak => source.push('\n'),
            _ => {}
        }
    }
    let body = source.clone();
    if source.ends_with('\n') {
        source.pop();
    }
    Block::Code {
        language,
        source,
        body,
        start,
        open,
    }
}

fn list(cursor: &mut Cursor<'_>, start: Option<u64>) -> Block {
    let mut items = Vec::new();
    while let Some(event) = cursor.peek() {
        match event {
            Event::Start(Tag::Item) => {
                cursor.advance();
                items.push(item(cursor));
            }
            Event::End(TagEnd::List(_)) => {
                cursor.advance();
                break;
            }
            _ => cursor.advance(),
        }
    }
    Block::List { start, items }
}

fn item(cursor: &mut Cursor<'_>) -> Item {
    let task = match cursor.peek() {
        Some(Event::TaskListMarker(done)) => {
            let done = *done;
            cursor.advance();
            Some(done)
        }
        _ => None,
    };
    Item {
        task,
        blocks: blocks(cursor, Some(TagEnd::Item)),
    }
}

fn table(cursor: &mut Cursor<'_>) -> Block {
    let mut header = Vec::new();
    let mut rows = Vec::new();
    while let Some(event) = cursor.peek() {
        match event {
            Event::Start(Tag::TableHead) => {
                cursor.advance();
                header = row(cursor, TagEnd::TableHead);
            }
            Event::Start(Tag::TableRow) => {
                cursor.advance();
                rows.push(row(cursor, TagEnd::TableRow));
            }
            Event::End(TagEnd::Table) => {
                cursor.advance();
                break;
            }
            _ => cursor.advance(),
        }
    }
    Block::Table { header, rows }
}

fn row(cursor: &mut Cursor<'_>, until: TagEnd) -> Vec<Vec<Inline>> {
    let mut cells = Vec::new();
    while let Some(event) = cursor.peek() {
        match event {
            Event::Start(Tag::TableCell) => {
                cursor.advance();
                cells.push(inlines(cursor, Some(TagEnd::TableCell)));
            }
            Event::End(end) if closes(end, &until) => {
                cursor.advance();
                break;
            }
            _ => cursor.advance(),
        }
    }
    cells
}

/// The inline runs up to the end that closes `until`, which is consumed.
/// With no `until`, the runs up to the next block event, which is left.
fn inlines(cursor: &mut Cursor<'_>, until: Option<TagEnd>) -> Vec<Inline> {
    let mut runs: Vec<Inline> = Vec::new();
    let mut stack: Vec<Marks> = vec![Marks::default()];
    // Each open link's destination and the run its text starts at.
    let mut links: Vec<(String, usize)> = Vec::new();
    while let Some(event) = cursor.peek() {
        let marks = stack.last().cloned().unwrap_or_default();
        match event {
            Event::End(end) => {
                if let Some(open) = &until
                    && closes(end, open)
                {
                    cursor.advance();
                    break;
                }
                if matches!(end, TagEnd::Link)
                    && let Some((url, start)) = links.pop()
                {
                    // grok-build writes `[text](url)` as "text (url)", the
                    // parentheses and the URL muted; an autolink once.
                    let text = text_of(runs.get(start..).unwrap_or(&[]));
                    if !url.is_empty() && text != url && !url.starts_with('#') {
                        let muted = Marks {
                            muted: true,
                            ..stack.first().cloned().unwrap_or_default()
                        };
                        push(&mut runs, &format!(" ({url})"), muted);
                    }
                }
                match end {
                    TagEnd::Emphasis
                    | TagEnd::Strong
                    | TagEnd::Strikethrough
                    | TagEnd::Link
                    | TagEnd::Superscript
                    | TagEnd::Subscript => {
                        if stack.len() > 1 {
                            stack.pop();
                        }
                        cursor.advance();
                    }
                    _ if until.is_none() => break,
                    _ => cursor.advance(),
                }
            }
            Event::Start(tag) if is_block(tag) => {
                // A block inside a span is a parse the stream does not
                // produce; leave it to the block reader above.
                break;
            }
            Event::Rule => {
                if until.is_none() {
                    break;
                }
                cursor.advance();
            }
            Event::Start(Tag::Emphasis) => {
                stack.push(Marks {
                    italic: true,
                    ..marks
                });
                cursor.advance();
            }
            Event::Start(Tag::Strong) => {
                stack.push(Marks {
                    bold: true,
                    ..marks
                });
                cursor.advance();
            }
            Event::Start(Tag::Strikethrough) => {
                stack.push(Marks {
                    strike: true,
                    ..marks
                });
                cursor.advance();
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                links.push((dest_url.to_string(), runs.len()));
                stack.push(Marks {
                    link: Some(dest_url.to_string()),
                    ..marks
                });
                cursor.advance();
            }
            Event::Start(Tag::Superscript) | Event::Start(Tag::Subscript) => {
                stack.push(marks);
                cursor.advance();
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let url = dest_url.to_string();
                cursor.advance();
                let mut alt = String::new();
                while let Some(event) = cursor.take() {
                    match event {
                        Event::End(TagEnd::Image) => break,
                        Event::Text(text) | Event::Code(text) => alt.push_str(&text),
                        Event::SoftBreak | Event::HardBreak => alt.push(' '),
                        _ => {}
                    }
                }
                runs.push(Inline {
                    text: alt,
                    marks: Marks {
                        image: Some(url),
                        ..marks
                    },
                });
            }
            Event::Start(_) => cursor.advance(),
            Event::Text(text) => {
                push(&mut runs, text, marks);
                cursor.advance();
            }
            Event::Code(text) => {
                push(
                    &mut runs,
                    text,
                    Marks {
                        code: true,
                        ..marks
                    },
                );
                cursor.advance();
            }
            Event::Html(text) | Event::InlineHtml(text) => {
                let cell_break = matches!(until, Some(TagEnd::TableCell))
                    && ["<br>", "<br/>", "<br />"]
                        .iter()
                        .any(|tag| text.eq_ignore_ascii_case(tag));
                push(&mut runs, if cell_break { "\n" } else { text }, marks);
                cursor.advance();
            }
            Event::InlineMath(text) | Event::DisplayMath(text) => {
                push(&mut runs, text, marks);
                cursor.advance();
            }
            Event::SoftBreak => {
                push(&mut runs, " ", marks);
                cursor.advance();
            }
            Event::HardBreak => {
                push(&mut runs, "\n", marks);
                cursor.advance();
            }
            Event::FootnoteReference(label) => {
                push(&mut runs, &format!("[{label}]"), marks);
                cursor.advance();
            }
            Event::TaskListMarker(_) => cursor.advance(),
        }
    }
    runs
}

/// Appends `text` to the run list, joining it onto the last run when the
/// marks match.
fn push(runs: &mut Vec<Inline>, text: &str, marks: Marks) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut()
        && last.marks == marks
        && last.marks.image.is_none()
    {
        last.text.push_str(text);
        return;
    }
    runs.push(Inline {
        text: text.to_string(),
        marks,
    });
}

/// The text of a run of inlines, with every mark dropped.
fn text_of(inlines: &[Inline]) -> String {
    inlines.iter().map(|inline| inline.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &[Rendered]) -> Vec<&str> {
        lines.iter().map(|line| line.marked.text.as_str()).collect()
    }

    #[test]
    fn a_paragraph_is_one_line_with_runs() {
        let lines = render("a **b *c* d** e `f` ~~g~~");
        assert_eq!(lines.len(), 1);
        let marked = &lines[0].marked;
        assert_eq!(marked.text, "a b c d e f g");
        let marks: Vec<(String, bool, bool)> = marked
            .runs
            .iter()
            .map(|(range, marks)| {
                (
                    marked.text[range.clone()].to_string(),
                    marks.bold,
                    marks.italic,
                )
            })
            .collect();
        assert_eq!(
            marks,
            vec![
                ("a ".to_string(), false, false),
                ("b ".to_string(), true, false),
                ("c".to_string(), true, true),
                (" d".to_string(), true, false),
                (" e ".to_string(), false, false),
                ("f".to_string(), false, false),
                (" ".to_string(), false, false),
                ("g".to_string(), false, false),
            ]
        );
        assert!(marked.runs[5].1.code);
        assert!(marked.runs[7].1.strike);
    }

    #[test]
    fn blocks_separate_with_a_blank_line() {
        let lines = render("one\n\ntwo");
        assert_eq!(texts(&lines), ["one", "", "two"]);
    }

    #[test]
    fn a_heading_draws_bright_and_bold() {
        let lines = render("## title");
        assert_eq!(lines[0].marked.text, "title");
        assert_eq!(lines[0].intensity, Intensity::Full);
        assert!(lines[0].marked.runs[0].1.bold);
    }

    #[test]
    fn a_fence_keeps_its_lines_as_code() {
        let lines = render("```rust\nfn main() {}\n```");
        assert_eq!(texts(&lines), ["fn main() {}"]);
        assert!(lines[0].marked.runs[0].1.code);
    }

    /// The token a run in `line` whose text is `text` carries.
    fn token_of(lines: &[Rendered], line: usize, text: &str) -> Option<Token> {
        let marked = &lines[line].marked;
        marked
            .runs
            .iter()
            .find(|(range, _)| marked.text[range.clone()].trim() == text)
            .and_then(|(_, marks)| marks.syntax)
    }

    #[test]
    fn known_code_carries_grok_night_tokens() {
        let lines = render("```rust\n// note\nlet s = \"hi\";\n```");
        assert_eq!(texts(&lines), ["// note", "let s = \"hi\";"]);
        assert!(
            lines
                .iter()
                .all(|line| line.marked.runs.iter().all(|(_, marks)| marks.code))
        );
        let keyword = token_of(&lines, 1, "let").expect("keyword token");
        let string = token_of(&lines, 1, "hi")
            .or_else(|| token_of(&lines, 1, "\"hi\""))
            .expect("string token");
        let comment = token_of(&lines, 0, "// note").expect("comment token");
        assert_ne!(keyword, string);
        assert_ne!(keyword, comment);
        // Each is the color Grok Night gives it, drawn exactly in truecolor.
        let night = grok::Palette::Night.syntect();
        let mut hl = night.highlight_lines_for_token("rust").expect("rust");
        let want = hl
            .highlight_line("// note\n", &night.syntax_set)
            .expect("highlight")
            .into_iter()
            .map(|(style, _)| Token::from_syntect(style))
            .next();
        assert_eq!(Some(comment), want);
        let ladder = Ladder::default();
        let base = ladder.style(Intensity::ThreeQuarters);
        let marks = Marks {
            code: true,
            syntax: Some(keyword),
            ..Marks::default()
        };
        let (r, g, b) = keyword.rgb;
        let style = marks.style(base, ladder);
        assert_eq!(
            style.fg,
            Some(crate::ladder::appearance(
                ratatui::style::Color::Rgb(r, g, b),
                ColorLevel::TrueColor
            ))
        );
        assert_eq!(style.bg, Some(ladder.background()));
        // An unknown fence stays plain code at the top of the ladder.
        let plain = render("```nope\nlet s = 1;\n```");
        assert!(
            plain[0]
                .marked
                .runs
                .iter()
                .all(|(_, marks)| marks.code && marks.syntax.is_none())
        );
        assert!(render("```rust\n```").is_empty());
    }

    #[test]
    fn the_near_black_field_takes_grok_night() {
        assert_eq!(palette_for(Ladder::default()), grok::Palette::Night);
        assert_eq!(
            palette_for(Ladder::new(Colors::Indexed)),
            grok::Palette::Night
        );
    }

    #[test]
    fn a_fence_still_streaming_highlights_like_the_finished_one() {
        let full = "Here:\n\n```rust\nfn main() {\n    let x = 1;\n}\n```\n";
        let done = render(full);
        let end = full.rfind("```").expect("closing fence");
        // Every prefix of the open fence lays out as the finished block does,
        // line for line, as it streams in.
        let body_start = full.find("fn main").expect("body");
        for cut in body_start + 1..end {
            let partial = render(&full[..cut]);
            let complete_lines = full[body_start..cut].matches('\n').count();
            for line in 0..complete_lines {
                assert_eq!(
                    partial[2 + line].marked,
                    done[2 + line].marked,
                    "prefix {cut}, line {line}"
                );
            }
        }
        let open = render(&full[..end]);
        assert_eq!(
            token_of(&open, 2, "fn"),
            token_of(&done, 2, "fn"),
            "the open fence highlights"
        );
        assert!(token_of(&open, 2, "fn").is_some());
    }

    #[test]
    fn code_blocks_come_out_in_order_with_their_source() {
        let source =
            "one\n\n```sh\nls -la\n```\n\n- item\n\n  ```\n  a\n  b\n  ```\n> ```\n> quoted\n> ```";
        assert_eq!(code_blocks(source), ["ls -la", "a\nb", "quoted"]);
        assert!(code_blocks("no code `here`").is_empty());
    }

    #[test]
    fn a_list_marks_and_hangs_its_items() {
        let lines = render("- one\n- two\n\n1. first\n2. second");
        assert_eq!(
            texts(&lines),
            ["• one", "• two", "", "1. first", "2. second"]
        );
        assert_eq!(lines[0].hang, 2);
        assert_eq!(lines[3].hang, 3);
    }

    #[test]
    fn a_task_list_carries_its_marks() {
        let lines = render("- [x] done\n- [ ] open");
        assert_eq!(texts(&lines), ["[x] done", "[ ] open"]);
    }

    #[test]
    fn a_quote_prefaces_its_blocks() {
        let lines = render("> **note**\n>\n> more");
        assert_eq!(texts(&lines), ["│ note", "│", "│ more"]);
    }

    #[test]
    fn a_link_marks_its_text() {
        let lines = render("see [the door](https://example.com)");
        let marked = &lines[0].marked;
        // grok-build: the text, then the URL in parentheses, muted.
        assert_eq!(marked.text, "see the door (https://example.com)");
        assert!(marked.runs.last().unwrap().1.muted);
        assert_eq!(
            marked.runs[1].1.link.as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn a_slice_of_marked_text_clips_to_the_range() {
        let marked = render("plain **bold** tail")[0].marked.clone();
        let runs = marked.runs_in(6..10);
        assert_eq!(
            runs,
            vec![(
                "bold".to_string(),
                Marks {
                    bold: true,
                    ..Marks::default()
                }
            )]
        );
    }

    #[test]
    fn raw_html_is_text() {
        let lines = render("hello <b>there</b>");
        assert_eq!(lines[0].marked.text, "hello <b>there</b>");
    }

    #[test]
    fn wrapped_lists_keep_marks_unicode_and_hanging_indentation() {
        use unicode_width::UnicodeWidthStr;
        let lines = wrapped("- **alpha βeta gamma delta** and `code`", 14);
        assert!(lines.len() > 2);
        assert!(lines[0].marked.text.starts_with("• "));
        assert!(
            lines
                .iter()
                .skip(1)
                .all(|line| line.marked.text.starts_with("  "))
        );
        assert!(lines.iter().all(|line| line.marked.text.width() <= 14));
        let runs: Vec<_> = lines
            .iter()
            .flat_map(|line| line.marked.runs_in(0..line.marked.text.len()))
            .collect();
        assert!(
            runs.iter()
                .any(|(text, marks)| text.contains("βeta") && marks.bold)
        );
        assert!(
            runs.iter()
                .any(|(text, marks)| text == "code" && marks.code)
        );
        for width in 0..4 {
            assert!(!wrapped("- 中 **é**", width).is_empty());
        }
    }

    #[test]
    fn code_and_table_headers_keep_their_structure() {
        let code = wrapped("```rust\n    let x = a * b;\n    // **literal**\n```", 80);
        assert_eq!(texts(&code), ["    let x = a * b;", "    // **literal**"]);
        assert!(
            code.iter()
                .all(|line| line.marked.runs.iter().all(|(_, marks)| marks.code))
        );
        let table = render("| Name | State |\n| --- | --- |\n| Parser | **Ready** |");
        // Boxed as grok-build boxes a table: muted borders, a bold header.
        assert_eq!(
            texts(&table),
            [
                "┌────────┬───────┐",
                "│ Name   │ State │",
                "├────────┼───────┤",
                "│ Parser │ Ready │",
                "└────────┴───────┘"
            ]
        );
        let bold = |line: &Rendered, word: &str| {
            line.marked
                .runs
                .iter()
                .any(|(range, marks)| &line.marked.text[range.clone()] == word && marks.bold)
        };
        assert!(bold(&table[1], "Name") && bold(&table[3], "Ready"));
        assert!(!bold(&table[3], "Parser"));
        assert!(table[0].marked.runs.iter().all(|(_, marks)| marks.muted));
    }

    #[test]
    fn table_cells_wrap_inside_shared_borders_and_keep_marks() {
        use unicode_width::UnicodeWidthStr;
        let source = "| Aspect | Details |\n| --- | --- |\n| **Purpose** | Text around `microcoder_long_token` and [a link](https://example.com). |\n| Result | More text so both rows need independent cell wrapping. |";
        for width in [18, 35, 70, 110] {
            let lines = wrapped(source, width);
            assert!(lines.iter().all(|line| line.marked.text.width() <= width));
            let boundaries = |text: &str| {
                let mut column = 0;
                text.chars()
                    .filter_map(|ch| {
                        let at = column;
                        column += ch.to_string().width();
                        matches!(
                            ch,
                            '┌' | '┬' | '┐' | '├' | '┼' | '┤' | '└' | '┴' | '┘' | '│'
                        )
                        .then_some(at)
                    })
                    .collect::<Vec<_>>()
            };
            let wanted = boundaries(&lines[0].marked.text);
            assert_eq!(wanted.len(), 3);
            assert!(
                lines
                    .iter()
                    .all(|line| boundaries(&line.marked.text) == wanted)
            );
            let code: String = lines
                .iter()
                .flat_map(|line| {
                    line.marked
                        .runs_in(0..line.marked.text.len())
                        .into_iter()
                        .filter(|(_, marks)| marks.code)
                        .map(|(text, _)| text)
                })
                .collect();
            assert_eq!(code, "microcoder_long_token");
            let linked: String = lines
                .iter()
                .flat_map(|line| {
                    line.marked
                        .runs_in(0..line.marked.text.len())
                        .into_iter()
                        .filter(|(_, marks)| marks.link.is_some())
                        .map(|(text, _)| text)
                })
                .collect();
            assert_eq!(linked.replace(' ', ""), "alink");
            let bold: String = lines
                .iter()
                .flat_map(|line| {
                    line.marked
                        .runs_in(0..line.marked.text.len())
                        .into_iter()
                        .filter(|(_, marks)| marks.bold)
                        .map(|(text, _)| text)
                })
                .collect();
            assert!(bold.contains("Purpose"));
        }
    }

    #[test]
    fn table_breaks_are_physical_rows_and_code_keeps_literal_html() {
        let table = wrapped(
            "| Name | Value |\n| --- | --- |\n| Breaks | first<br>second<br/>third<br />fourth<BR>fifth |\n| Code | `<br>` and <b>raw</b> |",
            60,
        );
        for word in ["first", "second", "third", "fourth", "fifth"] {
            assert_eq!(
                table
                    .iter()
                    .filter(|line| line.marked.text.contains(word))
                    .count(),
                1
            );
        }
        assert!(!table.iter().any(|line| line.marked.text.contains("<BR>")));
        assert!(
            table
                .iter()
                .any(|line| line.marked.text.contains("<b>raw</b>"))
        );
        assert!(
            table
                .iter()
                .flat_map(|line| line.marked.runs_in(0..line.marked.text.len()))
                .any(|(text, marks)| text == "<br>" && marks.code)
        );
        assert_eq!(texts(&render("outside<br>text")), ["outside<br>text"]);
    }

    #[test]
    fn nested_tables_use_their_remaining_width_and_narrow_tables_keep_content() {
        use unicode_width::UnicodeWidthStr;
        let nested = wrapped(
            "> - Table:\n>\n>   | Key | Value |\n>   | --- | --- |\n>   | 日本 | long words in the value |",
            26,
        );
        assert!(nested.iter().all(|line| line.marked.text.width() <= 26));
        let top = nested
            .iter()
            .find(|line| line.marked.text.contains('┌'))
            .unwrap();
        assert!(top.marked.text.starts_with("│   ┌"));
        let widest = top.marked.text.width();
        assert!(
            nested
                .iter()
                .filter(|line| line.marked.text.contains('│') && line.marked.text.contains("│ "))
                .any(|line| line.marked.text.width() == widest)
        );

        let source = "| A | B | C |\n| --- | --- | --- |\n| 日本語 | 👩‍💻 | tail |";
        for width in 1..14 {
            let lines = wrapped(source, width);
            let text: String = lines.iter().map(|line| line.marked.text.as_str()).collect();
            assert!(text.contains("日本語"), "width {width}");
            assert!(text.contains("👩‍💻"), "width {width}");
            assert!(text.contains("tail"), "width {width}");
            assert!(!text.contains('┌'), "width {width}");
        }
    }

    #[test]
    fn inline_marks_use_the_shared_terminal_style() {
        let ladder = Ladder::default();
        let marks = Marks {
            bold: true,
            italic: true,
            strike: true,
            code: true,
            link: Some("https://example.com".to_owned()),
            ..Marks::default()
        };
        let style = marks.style(ladder.style(Intensity::Half), ladder);
        assert_eq!(style.fg, ladder.style(Intensity::Full).fg);
        assert!(style.add_modifier.contains(
            Modifier::BOLD | Modifier::ITALIC | Modifier::CROSSED_OUT | Modifier::UNDERLINED
        ));
    }
}
