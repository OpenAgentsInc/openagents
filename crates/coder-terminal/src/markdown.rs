//! A reply's Markdown as styled display lines.
//!
//! `pulldown-cmark` events project onto a tree of blocks — paragraphs,
//! headings, fenced code, quotes, lists, tables, rules — each holding
//! flat inline runs with their marks. [`render`] lays the tree out as
//! [`Marked`] lines: marked text the draw loop wraps and styles. Raw
//! HTML in the source is text.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::Intensity;

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

/// One logical display line of a rendered reply: marked text, the amber
/// it draws at, and `hang` — extra cells continuation rows indent by so
/// a list item's wraps sit under its text, not its marker.
pub struct Rendered {
    pub marked: Marked,
    pub intensity: Intensity,
    pub hang: usize,
}

/// Lays `source` out as display lines. Blocks separate with a blank
/// line; a paragraph is one logical line (the draw loop wraps it), a
/// code block is one line per source line.
pub fn render(source: &str) -> Vec<Rendered> {
    let mut lines = Vec::new();
    blocks_lines(&parse(source), "", 0, &mut lines);
    lines
}

fn blocks_lines(blocks: &[Block], prefix: &str, hang: usize, out: &mut Vec<Rendered>) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            out.push(Rendered {
                marked: Marked::default(),
                intensity: Intensity::Half,
                hang: 0,
            });
        }
        block_lines(block, prefix, hang, out);
    }
}

fn block_lines(block: &Block, prefix: &str, hang: usize, out: &mut Vec<Rendered>) {
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
        Block::Code { source, .. } => {
            let code = Marks {
                code: true,
                ..Marks::default()
            };
            for line in source.lines() {
                let mut marked = Marked::default();
                marked.push(prefix, &Marks::default());
                marked.push(line, &code);
                out.push(Rendered {
                    marked,
                    intensity: Intensity::ThreeQuarters,
                    hang,
                });
            }
        }
        Block::Quote(blocks) => {
            blocks_lines(blocks, &format!("{prefix}│ "), hang + 2, out);
        }
        Block::List { start, items } => {
            for (index, item) in items.iter().enumerate() {
                let marker = match (start, item.task) {
                    (_, Some(true)) => "[x] ".to_string(),
                    (_, Some(false)) => "[ ] ".to_string(),
                    (Some(first), None) => format!("{}. ", first + index as u64),
                    (None, None) => "• ".to_string(),
                };
                item_lines(item, &marker, prefix, hang, out);
            }
        }
        Block::Table { header, rows, .. } => {
            out.push(marked_line(
                &header
                    .iter()
                    .flat_map(|cell| cell.iter())
                    .cloned()
                    .collect::<Vec<_>>(),
                prefix,
                hang,
                Intensity::Full,
            ));
            for row in rows {
                let cells: Vec<Inline> = row
                    .iter()
                    .enumerate()
                    .flat_map(|(index, cell)| {
                        let mut cell = cell.clone();
                        if index > 0 {
                            cell.insert(
                                0,
                                Inline {
                                    text: " │ ".to_string(),
                                    marks: Marks::default(),
                                },
                            );
                        }
                        cell
                    })
                    .collect();
                out.push(marked_line(&cells, prefix, hang, Intensity::ThreeQuarters));
            }
        }
        Block::Rule => out.push(Rendered {
            marked: Marked::plain(format!("{prefix}{}", "─".repeat(24))),
            intensity: Intensity::Half,
            hang,
        }),
    }
}

/// A list item's blocks: the marker leads the first line, padding the
/// rest, and the item's wraps hang under the marker.
fn item_lines(item: &Item, marker: &str, prefix: &str, hang: usize, out: &mut Vec<Rendered>) {
    let width = marker.chars().count();
    for (index, block) in item.blocks.iter().enumerate() {
        if index > 0 {
            out.push(Rendered {
                marked: Marked::default(),
                intensity: Intensity::Half,
                hang: 0,
            });
        }
        let lead = if index == 0 {
            marker.to_string()
        } else {
            " ".repeat(width)
        };
        block_lines(block, &format!("{prefix}{lead}"), hang + width, out);
    }
}

fn marked_line(inlines: &[Inline], prefix: &str, hang: usize, intensity: Intensity) -> Rendered {
    let mut marked = Marked::default();
    marked.push(prefix, &Marks::default());
    for inline in inlines {
        marked.push(&inline.text, &inline.marks);
    }
    Rendered {
        marked,
        intensity,
        hang,
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
    Code {
        language: Option<String>,
        source: String,
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
    let events: Vec<Event> = Parser::new_ext(source, options()).collect();
    let mut cursor = Cursor { events, at: 0 };
    blocks(&mut cursor, None)
}

/// A position in the event stream.
struct Cursor<'a> {
    events: Vec<Event<'a>>,
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
        Tag::CodeBlock(kind) => vec![code(cursor, kind)],
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

fn code(cursor: &mut Cursor<'_>, kind: CodeBlockKind<'_>) -> Block {
    let language = match kind {
        CodeBlockKind::Fenced(info) => info
            .split_whitespace()
            .next()
            .map(str::to_string)
            .filter(|language| !language.is_empty()),
        CodeBlockKind::Indented => None,
    };
    let mut source = String::new();
    while let Some(event) = cursor.take() {
        match event {
            Event::End(TagEnd::CodeBlock) => break,
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                source.push_str(&text)
            }
            Event::SoftBreak | Event::HardBreak => source.push('\n'),
            _ => {}
        }
    }
    if source.ends_with('\n') {
        source.pop();
    }
    Block::Code { language, source }
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
                push(&mut runs, text, marks);
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
        assert_eq!(texts(&lines), ["│ note", "", "│ more"]);
    }

    #[test]
    fn a_link_marks_its_text() {
        let lines = render("see [the door](https://example.com)");
        let marked = &lines[0].marked;
        assert_eq!(marked.text, "see the door");
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
}
