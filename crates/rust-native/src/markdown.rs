//! Markdown parsed in Rust into a closed block tree for a native adapter.
//!
//! The application parses; the adapter only lays out typed blocks and inline
//! spans. A link carries its destination; the adapter opens it only when
//! [`opens`] admits it (an `https` URL), and never loads anything on its
//! own. Bare URLs and `openagents.com` paths in text become links
//! ([`markdown_stream::autolink`]), and a site path such as `/projects`
//! points at `https://openagents.com`. Images appear as their alternative
//! text; raw HTML is dropped. Nothing in a document can load a resource or
//! run code.
//!
//! A reply's component blocks (```` ```openui-lang ````, #11187) are drawn as
//! their Markdown fallback: links, numbered steps, and each command a code
//! block with its copy control ([`openui_lang::embed::fallback`]).
//!
//! [`IncrementalMarkdown`] keeps a streaming message's blocks current while
//! text arrives, reparsing only the tail, and offers a mended display copy of
//! the tail's half-written syntax.

mod incremental;
mod mend;

pub use incremental::IncrementalMarkdown;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Text styles on one inline run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
    /// Inline code.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub code: bool,
    /// A link's destination. An adapter opens it only when [`opens`]
    /// admits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// A task-list item's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph {
        spans: Vec<Span>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<Item>,
    },
    Code {
        language: Option<String>,
        text: String,
    },
    Quote {
        blocks: Vec<Block>,
    },
    Table {
        align: Vec<Align>,
        header: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    Rule,
}

/// The UTF-8 bytes of every text field in `blocks`.
pub fn text_bytes(blocks: &[Block]) -> usize {
    fn spans(spans: &[Span]) -> usize {
        spans
            .iter()
            .map(|s| s.text.len() + s.link.as_ref().map_or(0, String::len))
            .sum()
    }
    blocks
        .iter()
        .map(|block| match block {
            Block::Heading { spans: s, .. } | Block::Paragraph { spans: s } => spans(s),
            Block::List { items, .. } => items.iter().map(|i| text_bytes(&i.blocks)).sum(),
            Block::Code { language, text } => text.len() + language.as_ref().map_or(0, String::len),
            Block::Quote { blocks } => text_bytes(blocks),
            Block::Table { header, rows, .. } => {
                header.iter().map(|c| spans(c)).sum::<usize>()
                    + rows
                        .iter()
                        .flat_map(|r| r.iter())
                        .map(|c| spans(c))
                        .sum::<usize>()
            }
            Block::Rule => 0,
        })
        .sum()
}

/// The document as plain text, one line per paragraph, list item, or table
/// row, for adapters that cannot lay out blocks.
pub fn plain(blocks: &[Block]) -> String {
    fn spans(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }
    let mut out = Vec::new();
    for block in blocks {
        match block {
            Block::Heading { spans: s, .. } | Block::Paragraph { spans: s } => out.push(spans(s)),
            Block::List {
                ordered,
                start,
                items,
            } => {
                for (index, item) in items.iter().enumerate() {
                    let marker = match (item.checked, ordered) {
                        (Some(true), _) => "[x] ".to_owned(),
                        (Some(false), _) => "[ ] ".to_owned(),
                        (None, true) => format!("{}. ", start + index as u64),
                        (None, false) => "- ".to_owned(),
                    };
                    out.push(format!(
                        "{marker}{}",
                        plain(&item.blocks).replace('\n', " ")
                    ));
                }
            }
            Block::Code { text, .. } => out.push(text.trim_end().to_owned()),
            Block::Quote { blocks } => {
                out.extend(plain(blocks).lines().map(|line| format!("> {line}")))
            }
            Block::Table { header, rows, .. } => {
                let row = |cells: &[Vec<Span>]| {
                    cells
                        .iter()
                        .map(|c| spans(c))
                        .collect::<Vec<_>>()
                        .join(" | ")
                };
                out.push(row(header));
                out.extend(rows.iter().map(|r| row(r)));
            }
            Block::Rule => out.push("---".into()),
        }
    }
    out.join("\n")
}

/// The deepest block nesting in `blocks`, counting one level per block.
pub fn depth(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|block| {
            1 + match block {
                Block::List { items, .. } => {
                    items.iter().map(|i| depth(&i.blocks)).max().unwrap_or(0)
                }
                Block::Quote { blocks } => depth(blocks),
                _ => 0,
            }
        })
        .max()
        .unwrap_or(0)
}

/// Parse CommonMark with tables, strikethrough, and task lists. A
/// component block becomes its Markdown fallback first ([`shown`]).
pub fn parse(markdown: &str) -> Vec<Block> {
    parse_starts(&shown(markdown), false).0
}

/// The Markdown drawn for `text`: `text` itself, with each component block
/// (```` ```openui-lang ````) replaced by its Markdown fallback. A block
/// still streaming in shows only its finished parts.
pub fn shown(text: &str) -> Cow<'_, str> {
    if text.contains(openui_lang::LANG) {
        Cow::Owned(openui_lang::embed::fallback(text))
    } else {
        Cow::Borrowed(text)
    }
}

/// Where a site path points.
const SITE: &str = "https://openagents.com";

/// Whether an adapter may open `destination`: only an `https` URL with a
/// host. The adapter opens it in the system browser.
pub fn opens(destination: &str) -> bool {
    destination
        .get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
        && destination[8..]
            .chars()
            .next()
            .is_some_and(char::is_alphanumeric)
        && !destination.chars().any(char::is_whitespace)
}

/// A link destination as drawn: a site path (`/projects`) points at
/// `https://openagents.com`; anything else is kept as written.
fn destination(url: &str) -> String {
    if url.starts_with('/') && !url.starts_with("//") {
        format!("{SITE}{url}")
    } else {
        url.to_owned()
    }
}

/// `spans` with each bare URL in plain text made a link. With `growing`, a
/// URL that runs to the end of the last span may still be streaming in, so
/// it stays text.
fn autolinked(spans: Vec<Span>, growing: bool) -> Vec<Span> {
    let last = spans.len().saturating_sub(1);
    let mut out = Vec::with_capacity(spans.len());
    for (index, span) in spans.into_iter().enumerate() {
        if span.code || span.link.is_some() {
            out.push(span);
            continue;
        }
        let found = markdown_stream::autolink::find(&span.text, growing && index == last);
        if found.is_empty() {
            out.push(span);
            continue;
        }
        let piece = |text: &str, link: Option<String>| Span {
            text: text.to_owned(),
            link,
            ..span.clone()
        };
        let mut at = 0;
        for link in found {
            if link.range.start > at {
                out.push(piece(&span.text[at..link.range.start], None));
            }
            out.push(piece(&span.text[link.range.clone()], Some(link.href)));
            at = link.range.end;
        }
        if at < span.text.len() {
            out.push(piece(&span.text[at..], None));
        }
    }
    out
}

/// Where a top-level block begins: the byte offset of the line it starts on,
/// and how many top-level blocks come before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Start {
    offset: usize,
    before: usize,
}

/// Parse `markdown` and report where each top-level block begins. With
/// `streaming`, the text may still grow, so a bare URL at its very end
/// stays text.
fn parse_starts(markdown: &str, streaming: bool) -> (Vec<Block>, Vec<Start>) {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut builder = Builder::default();
    let mut starts = Vec::new();
    let mut depth = 0usize;
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        if depth == 0 && !matches!(event, Event::End(_)) {
            let offset = markdown[..range.start]
                .rfind(['\n', '\r'])
                .map_or(0, |at| at + 1);
            starts.push(Start {
                offset,
                before: builder.root_len(),
            });
        }
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        builder.at_end = streaming && range.end >= markdown.len();
        builder.event(event);
    }
    (builder.finish(), starts)
}

/// A container being built: its blocks, and the inline runs of the block
/// that is open inside it.
enum Frame {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        items: Vec<Item>,
    },
    Item {
        checked: Option<bool>,
        blocks: Vec<Block>,
    },
    Table {
        align: Vec<Align>,
        header: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
        row: Vec<Vec<Span>>,
        in_head: bool,
    },
}

#[derive(Default)]
struct Builder {
    frames: Vec<Frame>,
    /// Inline runs of the open paragraph, heading, or table cell.
    spans: Vec<Span>,
    /// The open leaf: a heading level, a paragraph, a code block, or a cell.
    leaf: Option<Leaf>,
    style: Style,
    /// The current event reaches the end of a source still streaming in.
    at_end: bool,
    /// The open leaf's last text reached the end of a streaming source.
    growing: bool,
}

enum Leaf {
    Heading(u8),
    Paragraph,
    Code(Option<String>, String),
    Cell,
}

#[derive(Default, Clone)]
struct Style {
    bold: u32,
    italic: u32,
    strike: u32,
    link: Vec<String>,
}

impl Builder {
    /// Finished top-level blocks.
    fn root_len(&self) -> usize {
        match self.frames.first() {
            Some(Frame::Root(blocks)) => blocks.len(),
            _ => 0,
        }
    }

    fn push_block(&mut self, block: Block) {
        if self.frames.is_empty() {
            self.frames.push(Frame::Root(vec![]));
        }
        match self.frames.last_mut() {
            Some(Frame::Root(blocks) | Frame::Quote(blocks) | Frame::Item { blocks, .. }) => {
                blocks.push(block)
            }
            // Loose text directly in a list becomes its own item.
            Some(Frame::List { items, .. }) => items.push(Item {
                checked: None,
                blocks: vec![block],
            }),
            Some(Frame::Table { .. }) | None => {}
        }
    }

    fn text(&mut self, text: &str, code: bool) {
        if let Some(Leaf::Code(_, body)) = &mut self.leaf {
            body.push_str(text);
            return;
        }
        if self.leaf.is_none() {
            // Inline text outside a paragraph, as in a tight list item.
            self.leaf = Some(Leaf::Paragraph);
        }
        self.growing = self.at_end && !code;
        let span = Span {
            text: text.to_owned(),
            bold: self.style.bold > 0,
            italic: self.style.italic > 0,
            strike: self.style.strike > 0,
            code,
            link: self.style.link.last().cloned(),
        };
        match self.spans.last_mut() {
            Some(last)
                if last.bold == span.bold
                    && last.italic == span.italic
                    && last.strike == span.strike
                    && last.code == span.code
                    && last.link == span.link =>
            {
                last.text.push_str(&span.text)
            }
            _ => self.spans.push(span),
        }
    }

    fn close_leaf(&mut self) {
        let spans = autolinked(std::mem::take(&mut self.spans), self.growing);
        self.growing = false;
        match self.leaf.take() {
            Some(Leaf::Heading(level)) => self.push_block(Block::Heading { level, spans }),
            Some(Leaf::Paragraph) if !spans.is_empty() => {
                self.push_block(Block::Paragraph { spans })
            }
            Some(Leaf::Code(language, text)) => self.push_block(Block::Code { language, text }),
            Some(Leaf::Cell) => {
                if let Some(Frame::Table { row, .. }) = self.frames.last_mut() {
                    row.push(spans);
                }
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text, false),
            Event::Code(text) => self.text(&text, true),
            Event::SoftBreak => self.text(" ", false),
            Event::HardBreak => self.text("\n", false),
            Event::Rule => {
                self.close_leaf();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(checked) => {
                if let Some(Frame::Item { checked: state, .. }) = self.frames.last_mut() {
                    *state = Some(checked);
                }
            }
            // Raw HTML, math, and footnote references stay as their text.
            Event::Html(text) | Event::InlineHtml(text) => self.text(&text, true),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.text(&text, true),
            Event::FootnoteReference(text) => self.text(&format!("[{text}]"), false),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.close_leaf();
                self.leaf = Some(Leaf::Paragraph);
            }
            Tag::Heading { level, .. } => {
                self.close_leaf();
                self.leaf = Some(Leaf::Heading(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                }));
            }
            Tag::CodeBlock(kind) => {
                self.close_leaf();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .filter(|l| !l.is_empty())
                        .map(str::to_owned),
                    CodeBlockKind::Indented => None,
                };
                self.leaf = Some(Leaf::Code(language, String::new()));
            }
            // Each HTML block is its own paragraph of code text.
            Tag::HtmlBlock => self.close_leaf(),
            Tag::BlockQuote(_) => {
                self.close_leaf();
                self.frames.push(Frame::Quote(vec![]));
            }
            Tag::List(start) => {
                self.close_leaf();
                self.frames.push(Frame::List {
                    ordered: start.is_some(),
                    start: start.unwrap_or(1),
                    items: vec![],
                });
            }
            Tag::Item => {
                self.close_leaf();
                self.frames.push(Frame::Item {
                    checked: None,
                    blocks: vec![],
                });
            }
            Tag::Table(align) => {
                self.close_leaf();
                self.frames.push(Frame::Table {
                    align: align
                        .into_iter()
                        .map(|a| match a {
                            Alignment::None => Align::None,
                            Alignment::Left => Align::Left,
                            Alignment::Center => Align::Center,
                            Alignment::Right => Align::Right,
                        })
                        .collect(),
                    header: vec![],
                    rows: vec![],
                    row: vec![],
                    in_head: false,
                });
            }
            Tag::TableHead => {
                if let Some(Frame::Table { in_head, row, .. }) = self.frames.last_mut() {
                    *in_head = true;
                    row.clear();
                }
            }
            Tag::TableRow => {
                if let Some(Frame::Table { row, .. }) = self.frames.last_mut() {
                    row.clear();
                }
            }
            Tag::TableCell => {
                self.spans.clear();
                self.leaf = Some(Leaf::Cell);
            }
            Tag::Emphasis => self.style.italic += 1,
            Tag::Strong => self.style.bold += 1,
            Tag::Strikethrough => self.style.strike += 1,
            Tag::Link { dest_url, .. } => self.style.link.push(destination(&dest_url)),
            // An image shows its alternative text, which follows as text.
            Tag::Image { .. } => {}
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::HtmlBlock => {
                self.close_leaf()
            }
            TagEnd::BlockQuote(_) => {
                self.close_leaf();
                if let Some(Frame::Quote(blocks)) = self.frames.pop() {
                    self.push_block(Block::Quote { blocks });
                }
            }
            TagEnd::Item => {
                self.close_leaf();
                if let Some(Frame::Item { checked, blocks }) = self.frames.pop()
                    && let Some(Frame::List { items, .. }) = self.frames.last_mut()
                {
                    items.push(Item { checked, blocks });
                }
            }
            TagEnd::List(_) => {
                self.close_leaf();
                if let Some(Frame::List {
                    ordered,
                    start,
                    items,
                }) = self.frames.pop()
                {
                    self.push_block(Block::List {
                        ordered,
                        start,
                        items,
                    });
                }
            }
            TagEnd::TableCell => self.close_leaf(),
            TagEnd::TableHead => {
                if let Some(Frame::Table {
                    header,
                    row,
                    in_head,
                    ..
                }) = self.frames.last_mut()
                {
                    *header = std::mem::take(row);
                    *in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(Frame::Table {
                    rows,
                    row,
                    in_head: false,
                    ..
                }) = self.frames.last_mut()
                {
                    rows.push(std::mem::take(row));
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table {
                    align,
                    header,
                    rows,
                    ..
                }) = self.frames.pop()
                {
                    self.push_block(Block::Table {
                        align,
                        header,
                        rows,
                    });
                }
            }
            TagEnd::Emphasis => self.style.italic = self.style.italic.saturating_sub(1),
            TagEnd::Strong => self.style.bold = self.style.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.style.strike = self.style.strike.saturating_sub(1),
            TagEnd::Link => {
                self.style.link.pop();
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.close_leaf();
        // Close anything a truncated document left open.
        while self.frames.len() > 1 {
            match self.frames.pop() {
                Some(Frame::Quote(blocks)) => self.push_block(Block::Quote { blocks }),
                Some(Frame::Item { checked, blocks }) => {
                    if let Some(Frame::List { items, .. }) = self.frames.last_mut() {
                        items.push(Item { checked, blocks });
                    }
                }
                Some(Frame::List {
                    ordered,
                    start,
                    items,
                }) => self.push_block(Block::List {
                    ordered,
                    start,
                    items,
                }),
                Some(Frame::Table {
                    align,
                    header,
                    rows,
                    ..
                }) => self.push_block(Block::Table {
                    align,
                    header,
                    rows,
                }),
                _ => {}
            }
        }
        match self.frames.pop() {
            Some(Frame::Root(blocks)) => blocks,
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> Span {
        Span {
            text: text.into(),
            ..Span::default()
        }
    }

    #[test]
    fn parses_common_blocks() {
        let doc = parse(
            "# Title\n\nSome **bold** and `code` with a [link](https://x.test).\n\n- [x] done\n- [ ] open\n\n```rust\nfn main() {}\n```\n\n> quoted\n\n| a | b |\n|:--|--:|\n| 1 | 2 |\n\n---\n",
        );
        assert_eq!(
            doc[0],
            Block::Heading {
                level: 1,
                spans: vec![plain("Title")]
            }
        );
        let Block::Paragraph { spans } = &doc[1] else {
            panic!("paragraph")
        };
        assert_eq!(spans[0], plain("Some "));
        assert!(spans[1].bold);
        assert!(spans[3].code);
        assert_eq!(spans[5].link.as_deref(), Some("https://x.test"));
        let Block::List {
            ordered: false,
            items,
            ..
        } = &doc[2]
        else {
            panic!("list")
        };
        assert_eq!(items[0].checked, Some(true));
        assert_eq!(items[1].checked, Some(false));
        assert_eq!(
            doc[3],
            Block::Code {
                language: Some("rust".into()),
                text: "fn main() {}\n".into()
            }
        );
        assert!(matches!(&doc[4], Block::Quote { blocks } if blocks.len() == 1));
        let Block::Table {
            align,
            header,
            rows,
        } = &doc[5]
        else {
            panic!("table")
        };
        assert_eq!(align, &vec![Align::Left, Align::Right]);
        assert_eq!(header.len(), 2);
        assert_eq!(rows[0][1], vec![plain("2")]);
        assert_eq!(doc[6], Block::Rule);
        assert!(text_bytes(&doc) > 0);
    }

    #[test]
    fn html_stays_text_and_nesting_is_measured() {
        let doc = parse("<script>x</script>\n\n- a\n  - b\n    > c\n");
        assert!(matches!(&doc[0], Block::Paragraph { spans } if spans[0].code));
        assert!(depth(&doc) >= 4);
    }

    fn links(blocks: &[Block]) -> Vec<(String, String)> {
        fn walk(blocks: &[Block], out: &mut Vec<(String, String)>) {
            for block in blocks {
                match block {
                    Block::Heading { spans, .. } | Block::Paragraph { spans } => out.extend(
                        spans
                            .iter()
                            .filter_map(|s| Some((s.text.clone(), s.link.clone()?))),
                    ),
                    Block::List { items, .. } => {
                        for item in items {
                            walk(&item.blocks, out);
                        }
                    }
                    Block::Quote { blocks } => walk(blocks, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(blocks, &mut out);
        out
    }

    #[test]
    fn bare_urls_and_site_paths_become_links() {
        let doc = parse(
            "Open **https://openagents.com/projects** or openagents.com/device, then [the docs](/docs).\n\n\
             - see https://example.test/a_b.\n\n\
             Not `https://in.code/x`, not [https://a.test](https://b.test).\n\n\
             ```\nhttps://in.block/x\n```\n",
        );
        assert_eq!(
            links(&doc),
            [
                (
                    "https://openagents.com/projects".into(),
                    "https://openagents.com/projects".into()
                ),
                (
                    "openagents.com/device".into(),
                    "https://openagents.com/device".into()
                ),
                ("the docs".into(), "https://openagents.com/docs".into()),
                (
                    "https://example.test/a_b".into(),
                    "https://example.test/a_b".into()
                ),
                ("https://a.test".into(), "https://b.test".into()),
            ]
        );
        let Block::Paragraph { spans } = &doc[0] else {
            panic!("paragraph")
        };
        // The link keeps the text's style, and the text around it stays.
        assert!(spans[1].bold && spans[1].link.is_some());
        assert_eq!(spans[0].text, "Open ");
        assert_eq!(
            super::plain(&doc).lines().next(),
            Some("Open https://openagents.com/projects or openagents.com/device, then the docs.")
        );
    }

    #[test]
    fn only_https_destinations_open() {
        assert!(opens("https://openagents.com/device"));
        assert!(opens("HTTPS://example.test"));
        for no in [
            "http://example.test",
            "/projects",
            "javascript:alert(1)",
            "https://",
            "https:///x",
            "https://a b",
            "",
        ] {
            assert!(!opens(no), "{no}");
        }
    }

    const REPLY: &str = "Connect it on the web or on your computer.\n\n```openui-lang\n\
root = Columns([web, computer])\n\
web = Card(\"On the web\", [Text(\"Pick a repository.\"), Button(\"Connect GitHub\", href=\"/projects\")])\n\
computer = Card(\"On your computer\", [Steps([install, login])])\n\
install = Step(\"Install Coder\", [Command(\"curl -fsSL https://openagents.com/cli/install.sh | bash\")])\n\
login = Step(\"Sign in\", [CodeBlock(\"coder login\", \"bash\")])\n\
```\n";

    #[test]
    fn a_component_block_is_drawn_as_its_fallback() {
        let doc = parse(REPLY);
        let text = super::plain(&doc);
        assert!(!text.contains("root ="), "{text}");
        assert!(!text.contains("Columns("), "{text}");
        assert!(
            links(&doc).contains(&(
                "Connect GitHub".into(),
                "https://openagents.com/projects".into()
            )),
            "{doc:?}"
        );
        let Some(Block::List {
            ordered: true,
            items,
            ..
        }) = doc.iter().find(|b| matches!(b, Block::List { .. }))
        else {
            panic!("numbered steps: {doc:?}")
        };
        assert_eq!(items.len(), 2);
        assert!(items[0].blocks.contains(&Block::Code {
            language: Some("bash".into()),
            text: "curl -fsSL https://openagents.com/cli/install.sh | bash\n".into()
        }));
        assert!(items[1].blocks.contains(&Block::Code {
            language: Some("bash".into()),
            text: "coder login\n".into()
        }));
    }
}
