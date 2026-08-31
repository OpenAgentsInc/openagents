//! Markdown and code highlighting for the coder transcript.
//!
//! A model writes markdown. Until now the transcript put it on the screen
//! verbatim, so `**important**` arrived with its asterisks and a fenced block
//! of Rust looked like prose.
//!
//! Two properties shape this module.
//!
//! **It renders what has arrived so far.** A reply is drawn on every chunk, so
//! this is called on text whose last line is usually half-written. Nothing here
//! waits for a closing delimiter before it will draw: an unclosed `**` renders
//! as the two characters that are actually there, and re-renders as bold on the
//! chunk that closes it. A fence with no closing fence keeps highlighting the
//! rows inside it rather than holding them back. That is why the parse is
//! per-line and single-pass — a parser that needed the whole document could not
//! draw a document that is still being written.
//!
//! **It is pure.** `render` takes text and a width and returns rows. It reads
//! no terminal and no clock, so a test asserts on the rows a reader would see.
//!
//! The highlighter is a lexer, not a parser: it knows each language's comment
//! syntax, string delimiters, keyword set, and number grammar, and colours
//! those. It does not know types from values or resolve anything. That is the
//! honest ceiling of a few hundred lines, and it is most of what highlighting
//! buys you in a transcript.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::UnicodeWidthStr as _;

/// The rail down the left of a fenced code block, and its two end caps.
const CODE_RAIL: &str = "│ ";
const CODE_OPEN: &str = "╭─";
const CODE_CLOSE: &str = "╰─";
/// The marker in front of a bullet list item.
const BULLET: &str = "• ";
/// The rail down the left of a block quote.
const QUOTE_RAIL: &str = "▎ ";

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn heading_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Render markdown into rows no wider than `width` columns.
pub fn render(text: &str, width: usize) -> Vec<Line<'static>> {
    let width = width.max(4);
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut fence: Option<Fence> = None;

    for raw in text.split('\n') {
        match fence.as_mut() {
            Some(open) => {
                if closes(raw, open) {
                    rows.push(Line::from(Span::styled(CODE_CLOSE.to_string(), dim())));
                    fence = None;
                } else {
                    rows.push(code_row(raw, open, width));
                }
            }
            None => match opens(raw) {
                Some(open) => {
                    let label = if open.language.is_empty() {
                        CODE_OPEN.to_string()
                    } else {
                        format!("{CODE_OPEN} {}", open.language)
                    };
                    rows.push(Line::from(Span::styled(label, dim())));
                    fence = Some(open);
                }
                None => rows.extend(block_row(raw, width)),
            },
        }
    }
    rows
}

// ------------------------------------------------------------------ blocks

/// One non-code line, as however many wrapped rows it needs.
fn block_row(raw: &str, width: usize) -> Vec<Line<'static>> {
    let indent = raw.len() - raw.trim_start().len();
    let body = raw.trim_start();

    if body.is_empty() {
        return vec![Line::from("")];
    }

    if is_rule(body) {
        return vec![Line::from(Span::styled("─".repeat(width), dim()))];
    }

    if let Some(rest) = heading(body) {
        let mut spans = vec![Span::styled(rest.to_string(), heading_style())];
        // A heading is short enough to wrap as one styled run.
        if rest.width() > width {
            spans = inline(rest, heading_style());
        }
        return wrap_spans(spans, width, 0, 0);
    }

    if let Some(rest) = body.strip_prefix("> ").or_else(|| body.strip_prefix(">")) {
        let lead = Span::styled(QUOTE_RAIL.to_string(), dim());
        let text = inline(rest, dim().add_modifier(Modifier::ITALIC));
        let mut spans = vec![lead];
        spans.extend(text);
        return wrap_spans(spans, width, QUOTE_RAIL.width(), indent);
    }

    if let Some(rest) = bullet(body) {
        let mut spans = vec![Span::styled(BULLET.to_string(), dim())];
        spans.extend(inline(rest, Style::default()));
        return wrap_spans(spans, width, BULLET.width(), indent);
    }

    if let Some((marker, rest)) = ordered(body) {
        let hang = marker.width();
        let mut spans = vec![Span::styled(marker, dim())];
        spans.extend(inline(rest, Style::default()));
        return wrap_spans(spans, width, hang, indent);
    }

    wrap_spans(inline(body, Style::default()), width, 0, indent)
}

fn is_rule(body: &str) -> bool {
    let trimmed = body.trim_end();
    let first = match trimmed.chars().next() {
        Some(c @ ('-' | '*' | '_')) => c,
        _ => return false,
    };
    trimmed.len() >= 3 && trimmed.chars().all(|c| c == first)
}

/// The text of an ATX heading, with the `#` run and its space removed.
fn heading(body: &str) -> Option<&str> {
    let hashes = body.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &body[hashes..];
    // `#tag` is not a heading; `# ` is.
    rest.strip_prefix(' ').map(str::trim_end)
}

fn bullet(body: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = body.strip_prefix(marker) {
            return Some(rest);
        }
    }
    None
}

/// `12. ` at the head of a line, returned as the marker and the rest.
fn ordered(body: &str) -> Option<(String, &str)> {
    let digits = body.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let rest = &body[digits..];
    let rest = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    Some((format!("{} ", &body[..digits + 1]), rest))
}

// ------------------------------------------------------------------ fences

struct Fence {
    /// The character the fence was opened with, and how many of it.
    marker: char,
    length: usize,
    language: String,
    /// Set while a `/* … */` run is open, so the next row starts as comment.
    in_block_comment: bool,
}

/// A fence opener, if this line is one.
fn opens(raw: &str) -> Option<Fence> {
    let body = raw.trim_start();
    let marker = match body.chars().next() {
        Some(c @ ('`' | '~')) => c,
        _ => return None,
    };
    let length = body.chars().take_while(|c| *c == marker).count();
    if length < 3 {
        return None;
    }
    let language = body[length..].trim().to_lowercase();
    // ```rust,ignore and ```rust {n} both name rust.
    let language = language
        .split(|c: char| c == ',' || c.is_whitespace())
        .next()
        .unwrap_or("")
        .to_string();
    Some(Fence {
        marker,
        length,
        language,
        in_block_comment: false,
    })
}

/// Whether this line closes `open`.
fn closes(raw: &str, open: &Fence) -> bool {
    let body = raw.trim();
    !body.is_empty()
        && body.chars().count() >= open.length
        && body.chars().all(|c| c == open.marker)
}

/// One row of code: the rail, then the highlighted line, truncated to fit.
///
/// Code is truncated rather than wrapped. A wrapped line of code reads as two
/// statements, and the indentation that carries a block's structure is lost on
/// the continuation.
fn code_row(raw: &str, fence: &mut Fence, width: usize) -> Line<'static> {
    let syntax = Syntax::for_language(&fence.language);
    let body = width.saturating_sub(CODE_RAIL.width());
    let (spans, still_open) = highlight(raw, &syntax, fence.in_block_comment);
    fence.in_block_comment = still_open;
    let mut row = vec![Span::styled(CODE_RAIL.to_string(), dim())];
    row.extend(truncate_spans(spans, body));
    Line::from(row)
}

// ------------------------------------------------------------------ inline

/// Parse one line of inline markdown into styled spans.
///
/// `base` is the style text carries when no inline mark applies, so a
/// blockquote's body stays dim and italic under its own emphasis.
fn inline(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;

    macro_rules! flush {
        () => {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), base));
            }
        };
    }

    while i < text.len() {
        let rest = &text[i..];

        // A backslash escapes the next character, which is then literal.
        if let Some(escaped) = rest.strip_prefix('\\')
            && let Some(c) = escaped.chars().next()
            && "\\`*_[]()#-+.!>".contains(c)
        {
            plain.push(c);
            i += 1 + c.len_utf8();
            continue;
        }

        if bytes[i] == b'`' {
            let ticks = rest.chars().take_while(|c| *c == '`').count();
            let fence = "`".repeat(ticks);
            if let Some(end) = rest[ticks..].find(&fence) {
                flush!();
                let code = &rest[ticks..ticks + end];
                spans.push(Span::styled(
                    code.to_string(),
                    Style::default().fg(Color::Yellow),
                ));
                i += ticks + end + ticks;
                continue;
            }
        }

        let emphasis = [
            ("**", Modifier::BOLD),
            ("__", Modifier::BOLD),
            ("*", Modifier::ITALIC),
            ("_", Modifier::ITALIC),
        ]
        .into_iter()
        .find_map(|(mark, modifier)| {
            let after = rest.strip_prefix(mark)?;
            // An empty run (`**` on its own) is not emphasis, and a run with no
            // closer is text that has not finished arriving.
            let end = after.find(mark).filter(|end| *end > 0)?;
            Some((mark.len(), end, modifier))
        });
        if let Some((mark, end, modifier)) = emphasis {
            flush!();
            spans.push(Span::styled(
                rest[mark..mark + end].to_string(),
                base.add_modifier(modifier),
            ));
            i += mark + end + mark;
            continue;
        }

        if bytes[i] == b'['
            && let Some(link) = link_at(rest)
        {
            flush!();
            spans.push(Span::styled(
                link.text.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
            ));
            spans.push(Span::styled(format!(" ({})", link.url), dim()));
            i += link.consumed;
            continue;
        }

        let ch = rest.chars().next().expect("rest is non-empty");
        plain.push(ch);
        i += ch.len_utf8();
    }
    flush!();
    spans
}

struct Link<'a> {
    text: &'a str,
    url: &'a str,
    consumed: usize,
}

/// `[text](url)` at the head of `rest`.
fn link_at(rest: &str) -> Option<Link<'_>> {
    let close = rest.find("](")?;
    let url_start = close + 2;
    let url_end = url_start + rest[url_start..].find(')')?;
    let text = &rest[1..close];
    let url = &rest[url_start..url_end];
    if text.is_empty() || url.is_empty() || text.contains('[') {
        return None;
    }
    Some(Link {
        text,
        url,
        consumed: url_end + 1,
    })
}

// ------------------------------------------------------------- highlighting

/// What one language's lexer needs to know.
struct Syntax {
    keywords: &'static [&'static str],
    line_comment: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    quotes: &'static [char],
    /// A leading sigil that marks its word as a symbol: `:atom`, `@attr`, `$var`.
    sigils: &'static [char],
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];
const ELIXIR_KEYWORDS: &[&str] = &[
    "def",
    "defp",
    "defmodule",
    "defstruct",
    "defmacro",
    "defimpl",
    "defprotocol",
    "do",
    "end",
    "fn",
    "case",
    "cond",
    "if",
    "else",
    "unless",
    "with",
    "for",
    "receive",
    "try",
    "rescue",
    "after",
    "catch",
    "raise",
    "import",
    "alias",
    "require",
    "use",
    "when",
    "nil",
    "true",
    "false",
];
const PYTHON_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield",
];
const JS_KEYWORDS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "of",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "yield",
];
const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
    "nil",
    "true",
    "false",
];
const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "return", "short",
    "signed", "sizeof", "static", "struct", "switch", "typedef", "union", "unsigned", "void",
    "volatile", "while",
];
const SHELL_KEYWORDS: &[&str] = &[
    "case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function", "if", "in",
    "local", "return", "then", "until", "while",
];
const JSON_KEYWORDS: &[&str] = &["true", "false", "null"];

impl Syntax {
    fn for_language(language: &str) -> Self {
        match language {
            "rust" | "rs" => Syntax {
                keywords: RUST_KEYWORDS,
                line_comment: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"'],
                sigils: &[],
            },
            "elixir" | "ex" | "exs" | "heex" => Syntax {
                keywords: ELIXIR_KEYWORDS,
                line_comment: &["#"],
                block_comment: None,
                quotes: &['"'],
                sigils: &[':', '@'],
            },
            "python" | "py" => Syntax {
                keywords: PYTHON_KEYWORDS,
                line_comment: &["#"],
                block_comment: None,
                quotes: &['"', '\''],
                sigils: &['@'],
            },
            "javascript" | "js" | "jsx" | "typescript" | "ts" | "tsx" => Syntax {
                keywords: JS_KEYWORDS,
                line_comment: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\'', '`'],
                sigils: &[],
            },
            "go" => Syntax {
                keywords: GO_KEYWORDS,
                line_comment: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '`'],
                sigils: &[],
            },
            "c" | "cpp" | "c++" | "h" | "java" | "swift" | "kotlin" => Syntax {
                keywords: C_KEYWORDS,
                line_comment: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                sigils: &[],
            },
            "sh" | "bash" | "zsh" | "shell" | "console" => Syntax {
                keywords: SHELL_KEYWORDS,
                line_comment: &["#"],
                block_comment: None,
                quotes: &['"', '\''],
                sigils: &['$'],
            },
            "json" => Syntax {
                keywords: JSON_KEYWORDS,
                line_comment: &[],
                block_comment: None,
                quotes: &['"'],
                sigils: &[],
            },
            "toml" | "ini" => Syntax {
                keywords: &["true", "false"],
                line_comment: &["#"],
                block_comment: None,
                quotes: &['"', '\''],
                sigils: &[],
            },
            "yaml" | "yml" => Syntax {
                keywords: &["true", "false", "null"],
                line_comment: &["#"],
                block_comment: None,
                quotes: &['"', '\''],
                sigils: &[],
            },
            "sql" => Syntax {
                keywords: &[
                    "select", "from", "where", "insert", "into", "values", "update", "set",
                    "delete", "join", "left", "inner", "outer", "on", "group", "order", "by",
                    "limit", "create", "table", "index", "drop", "alter", "and", "or", "not",
                    "null", "as",
                ],
                line_comment: &["--"],
                block_comment: Some(("/*", "*/")),
                quotes: &['\'', '"'],
                sigils: &[],
            },
            // A language nothing here knows gets no invented colouring.
            _ => Syntax {
                keywords: &[],
                line_comment: &[],
                block_comment: None,
                quotes: &[],
                sigils: &[],
            },
        }
    }
}

fn keyword_style() -> Style {
    Style::default().fg(Color::Magenta)
}
fn string_style() -> Style {
    Style::default().fg(Color::Green)
}
fn comment_style() -> Style {
    dim().add_modifier(Modifier::ITALIC)
}
fn number_style() -> Style {
    Style::default().fg(Color::Yellow)
}
fn symbol_style() -> Style {
    Style::default().fg(Color::Cyan)
}

/// Lex one line. Returns its spans and whether a block comment is still open.
fn highlight(line: &str, syntax: &Syntax, mut in_block: bool) -> (Vec<Span<'static>>, bool) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let mut i = 0usize;

    macro_rules! flush {
        () => {
            if !plain.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut plain)));
            }
        };
    }

    while i < line.len() {
        let rest = &line[i..];

        if in_block {
            let (_, close) = syntax
                .block_comment
                .expect("in_block implies a block syntax");
            match rest.find(close) {
                Some(end) => {
                    spans.push(Span::styled(
                        rest[..end + close.len()].to_string(),
                        comment_style(),
                    ));
                    i += end + close.len();
                    in_block = false;
                }
                None => {
                    spans.push(Span::styled(rest.to_string(), comment_style()));
                    i = line.len();
                }
            }
            continue;
        }

        if let Some((open, close)) = syntax.block_comment
            && let Some(after_open) = rest.strip_prefix(open)
        {
            flush!();
            match after_open.find(close) {
                Some(end) => {
                    let stop = open.len() + end + close.len();
                    spans.push(Span::styled(rest[..stop].to_string(), comment_style()));
                    i += stop;
                }
                None => {
                    spans.push(Span::styled(rest.to_string(), comment_style()));
                    i = line.len();
                    in_block = true;
                }
            }
            continue;
        }

        if let Some(marker) = syntax
            .line_comment
            .iter()
            .find(|marker| rest.starts_with(**marker))
        {
            let _ = marker;
            flush!();
            spans.push(Span::styled(rest.to_string(), comment_style()));
            break;
        }

        let ch = rest.chars().next().expect("rest is non-empty");

        if syntax.quotes.contains(&ch) {
            flush!();
            let (literal, consumed) = string_at(rest, ch);
            spans.push(Span::styled(literal, string_style()));
            i += consumed;
            continue;
        }

        if ch.is_ascii_digit() && !preceded_by_word(line, i) {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_'))
                .unwrap_or(rest.len());
            flush!();
            spans.push(Span::styled(rest[..end].to_string(), number_style()));
            i += end;
            continue;
        }

        if syntax.sigils.contains(&ch) {
            let end = rest[ch.len_utf8()..]
                .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '?' || c == '!'))
                .map_or(rest.len(), |offset| ch.len_utf8() + offset);
            if end > ch.len_utf8() {
                flush!();
                spans.push(Span::styled(rest[..end].to_string(), symbol_style()));
                i += end;
                continue;
            }
        }

        if ch.is_alphabetic() || ch == '_' {
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let word = &rest[..end];
            if syntax.keywords.contains(&word) {
                flush!();
                spans.push(Span::styled(word.to_string(), keyword_style()));
            } else {
                plain.push_str(word);
            }
            i += end;
            continue;
        }

        plain.push(ch);
        i += ch.len_utf8();
    }
    flush!();
    (spans, in_block)
}

/// Whether the byte before `at` is part of a word, so `x2` is not a number.
fn preceded_by_word(line: &str, at: usize) -> bool {
    line[..at]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// The string literal starting at `rest[0]`, and how many bytes it took.
///
/// An unterminated literal runs to the end of the line, which is what a
/// half-written line of a streaming reply looks like.
fn string_at(rest: &str, quote: char) -> (String, usize) {
    let mut escaped = false;
    for (offset, ch) in rest.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            let stop = offset + ch.len_utf8();
            return (rest[..stop].to_string(), stop);
        }
    }
    (rest.to_string(), rest.len())
}

// ------------------------------------------------------------------- layout

/// Soft-wrap styled spans to `width`, keeping each grapheme's style.
///
/// `hanging` is the number of columns a continuation row is indented by, so a
/// wrapped list item lines up under its own text rather than under its bullet.
/// `indent` is a left margin applied to every row, which is how a nested list
/// keeps its nesting.
pub fn wrap_spans(
    spans: Vec<Span<'static>>,
    width: usize,
    hanging: usize,
    indent: usize,
) -> Vec<Line<'static>> {
    let indent = indent.min(width.saturating_sub(2));
    let body = width.saturating_sub(indent).max(2);
    let hanging = hanging.min(body.saturating_sub(1));

    // Flatten to graphemes so a break can land anywhere and the style follows.
    let mut cells: Vec<(String, Style)> = Vec::new();
    for span in spans {
        for grapheme in span.content.graphemes(true) {
            cells.push((grapheme.to_string(), span.style));
        }
    }
    if cells.is_empty() {
        return vec![Line::from(" ".repeat(indent))];
    }

    let mut rows: Vec<Vec<(String, Style)>> = Vec::new();
    let mut row: Vec<(String, Style)> = Vec::new();
    let mut used = 0usize;
    let mut break_at: Option<(usize, usize)> = None;
    let mut limit = body;

    for (grapheme, style) in cells {
        let w = grapheme.width().max(1);
        if used + w > limit && !row.is_empty() {
            match break_at {
                Some((index, _)) if index > 0 && index < row.len() => {
                    let tail = row.split_off(index);
                    while row.last().is_some_and(|(g, _)| g == " ") {
                        row.pop();
                    }
                    rows.push(std::mem::take(&mut row));
                    row = tail.into_iter().skip_while(|(g, _)| g == " ").collect();
                }
                _ => rows.push(std::mem::take(&mut row)),
            }
            used = row.iter().map(|(g, _)| g.width().max(1)).sum();
            break_at = None;
            limit = body.saturating_sub(hanging).max(1);
        }
        if grapheme == " " {
            break_at = Some((row.len(), used));
        }
        row.push((grapheme, style));
        used += w;
    }
    rows.push(row);

    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let pad = indent + if index == 0 { 0 } else { hanging };
            let mut spans: Vec<Span<'static>> = Vec::new();
            if pad > 0 {
                spans.push(Span::raw(" ".repeat(pad)));
            }
            spans.extend(coalesce(row));
            Line::from(spans)
        })
        .collect()
}

/// Join neighbouring graphemes that share a style back into spans.
fn coalesce(cells: Vec<(String, Style)>) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (grapheme, style) in cells {
        match spans.last_mut() {
            Some(last) if last.style == style => last.content.to_mut().push_str(&grapheme),
            _ => spans.push(Span::styled(grapheme, style)),
        }
    }
    spans
}

/// Cut styled spans to `width` columns, marking the cut with an ellipsis.
pub fn truncate_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(|span| span.content.width()).sum();
    if total <= width {
        return spans;
    }
    if width == 0 {
        return Vec::new();
    }
    let budget = width.saturating_sub(1);
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for span in spans {
        if used >= budget {
            break;
        }
        let mut kept = String::new();
        for grapheme in span.content.graphemes(true) {
            let w = grapheme.width().max(1);
            if used + w > budget {
                break;
            }
            kept.push_str(grapheme);
            used += w;
        }
        if !kept.is_empty() {
            out.push(Span::styled(kept, span.style));
        }
    }
    out.push(Span::styled("…".to_string(), dim()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of a row, styles dropped.
    fn text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(rows: &[Line<'_>]) -> Vec<String> {
        rows.iter().map(text).collect()
    }

    /// The style covering the first occurrence of `needle` in a row.
    fn style_of(line: &Line<'_>, needle: &str) -> Option<Style> {
        line.spans
            .iter()
            .find(|span| span.content.contains(needle))
            .map(|span| span.style)
    }

    #[test]
    fn bold_loses_its_asterisks_and_gains_the_modifier() {
        let rows = render("this is **important** text", 60);
        assert_eq!(texts(&rows), vec!["this is important text".to_string()]);
        let style = style_of(&rows[0], "important").expect("a span for the bold run");
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn an_unclosed_bold_run_renders_as_the_characters_that_are_there() {
        // What half of a streamed chunk looks like. It must not swallow the
        // asterisks waiting for a closer that has not arrived.
        let rows = render("this is **import", 60);
        assert_eq!(texts(&rows), vec!["this is **import".to_string()]);
    }

    #[test]
    fn inline_code_is_coloured_and_loses_its_backticks() {
        let rows = render("run `mix precommit` first", 60);
        assert_eq!(texts(&rows), vec!["run mix precommit first".to_string()]);
        assert_eq!(
            style_of(&rows[0], "mix precommit").and_then(|s| s.fg),
            Some(Color::Yellow)
        );
    }

    #[test]
    fn a_heading_drops_its_hashes_and_is_bold_cyan() {
        let rows = render("## What changed", 60);
        assert_eq!(texts(&rows), vec!["What changed".to_string()]);
        let style = style_of(&rows[0], "What changed").expect("a heading span");
        assert_eq!(style.fg, Some(Color::Cyan));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading() {
        let rows = render("#73 is the issue", 60);
        assert_eq!(texts(&rows), vec!["#73 is the issue".to_string()]);
    }

    #[test]
    fn a_bullet_becomes_a_bullet_and_wraps_under_its_own_text() {
        let rows = render("- alpha beta gamma delta", 14);
        assert_eq!(
            texts(&rows),
            vec!["• alpha beta".to_string(), "  gamma delta".to_string()]
        );
    }

    #[test]
    fn a_numbered_item_keeps_its_number() {
        let rows = render("3. third thing", 40);
        assert_eq!(texts(&rows), vec!["3. third thing".to_string()]);
    }

    #[test]
    fn a_link_shows_its_text_and_its_url() {
        let rows = render("see [the issue](https://openagents.com/i/73)", 80);
        assert_eq!(
            texts(&rows),
            vec!["see the issue (https://openagents.com/i/73)".to_string()]
        );
        let style = style_of(&rows[0], "the issue").expect("a link span");
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn a_fenced_block_gets_a_rail_and_end_caps() {
        let rows = render("```rust\nlet x = 1;\n```", 40);
        assert_eq!(
            texts(&rows),
            vec![
                "╭─ rust".to_string(),
                "│ let x = 1;".to_string(),
                "╰─".to_string(),
            ]
        );
    }

    #[test]
    fn a_fence_that_has_not_closed_yet_still_renders_its_code() {
        // The state a fenced block is in for every chunk but its last.
        let rows = render("```rust\nfn main() {", 40);
        assert_eq!(
            texts(&rows),
            vec!["╭─ rust".to_string(), "│ fn main() {".to_string()]
        );
    }

    #[test]
    fn rust_keywords_strings_and_comments_are_each_coloured_apart() {
        let rows = render("```rust\nlet s = \"hi\"; // note\n```", 60);
        let code = &rows[1];
        assert_eq!(
            style_of(code, "let").and_then(|s| s.fg),
            Some(Color::Magenta)
        );
        assert_eq!(
            style_of(code, "\"hi\"").and_then(|s| s.fg),
            Some(Color::Green)
        );
        assert_eq!(
            style_of(code, "// note").and_then(|s| s.fg),
            Some(Color::DarkGray)
        );
    }

    #[test]
    fn elixir_atoms_and_hash_comments_are_recognised() {
        let rows = render("```elixir\ndef run, do: :ok # go\n```", 60);
        let code = &rows[1];
        assert_eq!(
            style_of(code, "def").and_then(|s| s.fg),
            Some(Color::Magenta)
        );
        assert_eq!(style_of(code, ":ok").and_then(|s| s.fg), Some(Color::Cyan));
        assert_eq!(
            style_of(code, "# go").and_then(|s| s.fg),
            Some(Color::DarkGray)
        );
    }

    /// A `#` is a comment in Elixir and is not one in Rust. A highlighter that
    /// used one comment rule everywhere would grey out the rest of this line.
    #[test]
    fn the_comment_rule_is_the_languages_own() {
        let rows = render("```rust\nlet n = 1; # not a comment here\n```", 60);
        assert!(
            style_of(&rows[1], "# not").is_none_or(|s| s.fg != Some(Color::DarkGray)),
            "a Rust line was greyed out from a `#`"
        );
    }

    #[test]
    fn an_unknown_language_is_left_plain() {
        let rows = render("```brainfuck\n+++[->+<]\n```", 40);
        let plain = rows[1]
            .spans
            .iter()
            .skip(1)
            .all(|span| span.style == Style::default());
        assert!(plain, "an unknown language was given invented colours");
    }

    #[test]
    fn a_code_line_too_wide_for_the_pane_is_cut_not_wrapped() {
        let rows = render("```rust\nlet a_very_long_identifier = 1;\n```", 16);
        assert_eq!(texts(&rows)[1], "│ let a_very_lo…");
    }

    #[test]
    fn a_block_comment_stays_open_across_rows() {
        let rows = render("```rust\n/* one\ntwo */ let x = 1;\n```", 60);
        assert_eq!(
            style_of(&rows[2], "two */").and_then(|s| s.fg),
            Some(Color::DarkGray)
        );
        assert_eq!(
            style_of(&rows[2], "let").and_then(|s| s.fg),
            Some(Color::Magenta)
        );
    }

    #[test]
    fn a_quote_gets_a_rail() {
        let rows = render("> quoted", 40);
        assert_eq!(texts(&rows), vec!["▎ quoted".to_string()]);
    }

    #[test]
    fn a_rule_fills_the_width() {
        let rows = render("---", 8);
        assert_eq!(texts(&rows), vec!["────────".to_string()]);
    }

    #[test]
    fn wrapping_keeps_the_style_of_the_run_it_split() {
        let rows = render("**alpha beta gamma delta epsilon**", 14);
        assert_eq!(rows.len(), 3);
        for row in &rows {
            for span in &row.spans {
                if span.content.trim().is_empty() {
                    continue;
                }
                assert!(
                    span.style.add_modifier.contains(Modifier::BOLD),
                    "wrapping dropped the bold from {:?}",
                    span.content
                );
            }
        }
    }

    #[test]
    fn no_row_is_wider_than_the_width_it_was_given() {
        let source = "A paragraph with a ridiculouslylongunbrokenidentifier in it, plus \
                      `code` and **bold**.\n\n- a bullet that also needs to wrap somewhere\n\n\
                      ```rust\nfn f() { let s = \"a string that is quite long indeed\"; }\n```";
        for width in [8usize, 13, 20, 41, 80] {
            for row in render(source, width) {
                let drawn: usize = row.spans.iter().map(|s| s.content.width()).sum();
                assert!(
                    drawn <= width,
                    "a row of {drawn} columns was drawn into {width}: {:?}",
                    text(&row)
                );
            }
        }
    }

    #[test]
    fn plain_prose_survives_a_round_trip_unchanged() {
        let rows = render("just some ordinary words", 60);
        assert_eq!(texts(&rows), vec!["just some ordinary words".to_string()]);
    }
}
