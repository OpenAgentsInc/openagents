//! The part of a streaming Markdown reply that renders cleanly mid-stream.
//!
//! While an answer streams, the text so far often ends in half-written
//! syntax: an open code fence, half a table row, `[link text](` with no
//! destination yet, `**bold` with no close, a heading or list marker with
//! nothing after it. Rendered as is, the syntax shows as raw characters and
//! then snaps into structure when the rest arrives, which reads as a flash
//! and moves the page. [`renderable`] cuts the text at the last complete
//! construct instead (#11112):
//!
//! - An open code fence is closed, so the code so far shows as code. A last
//!   line that may be the closing fence is held back.
//! - A last line that is still being written and could still become
//!   structure is held back: a table row (a line starting with a pipe, or
//!   with a pipe under a piped line), a fence's opening line, or a line
//!   of nothing but markers (`-`, `1.`, `##`, `>`, `---`) or digits (the
//!   start of `12.`).
//!   A last row that closes with a pipe and has its table's full count of
//!   cells is finished and shows, so a stopped table keeps its grid.
//! - A table's header row is held back until its delimiter row is complete,
//!   so it never shows as a paragraph of pipes.
//! - In the paragraph, heading, or list item still being written, inline
//!   syntax that has not closed yet is held back from its opening marker:
//!   emphasis, strikethrough, a code span, a link or image, an autolink or
//!   tag.
//! - Partial plain text is kept.
//!
//! Every other block is settled and shown as written. When the reply ends,
//! renderers draw the whole text, so the final render is exactly the
//! one-shot render. Raw HTML is still a renderer's concern: the web renderer
//! shows it as text.

pub mod autolink;

use std::borrow::Cow;
use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// The prefix of `partial`, a reply still streaming in, that renders
/// without half-written syntax; with an open code fence closed, so the code
/// so far shows. Borrowed when no fence needed closing.
#[must_use]
pub fn renderable(partial: &str) -> Cow<'_, str> {
    let mut end = partial.len();
    // Each pass can only shorten the text; a cut can expose a new tail
    // (`- **bo` becomes `- `, a bare marker), so repeat until it holds.
    for _ in 0..16 {
        match step(&partial[..end]) {
            Step::Keep => break,
            Step::At(at) => end = at,
            Step::Close(closed) => return Cow::Owned(closed),
        }
    }
    Cow::Borrowed(&partial[..end])
}

enum Step {
    Keep,
    At(usize),
    Close(String),
}

/// An open code fence: its character, run length, and the indentation and
/// quote markers before it, which its closing line repeats.
struct Fence {
    mark: u8,
    len: usize,
    prefix: String,
}

fn step(text: &str) -> Step {
    let complete = text.rfind('\n').map_or(0, |at| at + 1);
    let tail = &text[complete..];

    let mut fence: Option<Fence> = None;
    for line in text[..complete].split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        match &fence {
            Some(open) => {
                if closes(content, open) {
                    fence = None;
                }
            }
            None => fence = opens(content),
        }
    }
    if let Some(open) = fence {
        let rest = container(tail).trim();
        let closing = rest.is_empty() || rest.bytes().all(|b| b == open.mark);
        let mut out = String::from(if closing { &text[..complete] } else { text });
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&open.prefix);
        out.push_str(&char::from(open.mark).to_string().repeat(open.len));
        return Step::Close(out);
    }

    let mut end = text.len();
    if !tail.is_empty() {
        let line = container(tail);
        // A table row: it starts with a pipe, or has one under a piped
        // line. A pipe mid-sentence (a shell command) is text.
        let above = text[..complete].lines().next_back().unwrap_or("");
        let row = line.starts_with('|') || (tail.contains('|') && above.contains('|'));
        let row = row && !finished_row(&text[..complete], line);
        let held = fence_run(line).is_some_and(|(_, n)| n >= 3) || row || bare_marker(line);
        if held {
            end = complete;
        }
    }
    if end == complete
        && let Some(header) = unconfirmed_header(&text[..complete])
    {
        end = header;
    }
    if let Some(at) = open_inline(&text[..end]) {
        end = end.min(at);
    }
    if end < text.len() {
        Step::At(end)
    } else {
        Step::Keep
    }
}

/// A line without its indentation and quote markers.
fn container(line: &str) -> &str {
    line.trim_start_matches([' ', '\t', '>'])
}

/// Whether `line`, a last line still being written, is nothing but block
/// markers so far: list, heading, and quote markers, a rule or setext
/// underline, or the digits an ordered list's marker starts with.
fn bare_marker(line: &str) -> bool {
    let mut rest = line;
    loop {
        rest = rest.trim_start_matches([' ', '\t', '>']);
        let bytes = rest.as_bytes();
        let spaced = |at: usize| bytes.get(at).is_none_or(|b| *b == b' ' || *b == b'\t');
        match bytes.first() {
            None => return true,
            Some(b'-' | b'+' | b'*') if spaced(1) => rest = &rest[1..],
            Some(b'#') => {
                let hashes = bytes.iter().take_while(|&&b| b == b'#').count();
                if hashes > 6 || !spaced(hashes) {
                    return false;
                }
                rest = &rest[hashes..];
            }
            Some(b'0'..=b'9') => {
                let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
                return digits <= 9
                    && match bytes.get(digits) {
                        None => true,
                        Some(b'.' | b')') => rest[digits + 1..].trim().is_empty(),
                        Some(_) => false,
                    };
            }
            Some(&mark @ (b'-' | b'=' | b'*' | b'_')) => {
                return rest.bytes().all(|b| b == mark || b == b' ' || b == b'\t');
            }
            Some(_) => return false,
        }
    }
}

/// A line's leading fence character and run length.
fn fence_run(line: &str) -> Option<(u8, usize)> {
    let mark = *line.as_bytes().first()?;
    if mark != b'`' && mark != b'~' {
        return None;
    }
    Some((mark, line.bytes().take_while(|&b| b == mark).count()))
}

fn opens(content: &str) -> Option<Fence> {
    let rest = container(content);
    let (mark, len) = fence_run(rest)?;
    if len < 3 || (mark == b'`' && rest[len..].contains('`')) {
        return None;
    }
    Some(Fence {
        mark,
        len,
        prefix: content[..content.len() - rest.len()].to_owned(),
    })
}

fn closes(content: &str, open: &Fence) -> bool {
    let rest = container(content);
    fence_run(rest).is_some_and(|(mark, len)| {
        mark == open.mark && len >= open.len && rest[len..].trim().is_empty()
    })
}

/// A table delimiter row: `| --- | :-: |`.
fn delimiter(line: &str) -> bool {
    let line = container(line).trim();
    line.contains('|')
        && line.contains('-')
        && line
            .chars()
            .all(|c| matches!(c, '|' | ':' | '-' | ' ' | '\t'))
}

/// A line's table cells: split at unescaped pipes, without the empty
/// cells a leading or trailing pipe makes.
fn cells(line: &str) -> usize {
    let line = container(line).trim();
    let bytes = line.as_bytes();
    let pipes = (0..bytes.len())
        .filter(|&i| bytes[i] == b'|' && (i == 0 || bytes[i - 1] != b'\\'))
        .count();
    let leading = usize::from(line.starts_with('|'));
    let trailing = usize::from(line.len() > 1 && line.ends_with('|') && !line.ends_with("\\|"));
    (pipes + 1).saturating_sub(leading + trailing)
}

/// Whether `line`, a last table row still being written, is already whole:
/// it closes with a pipe and has as many cells as its table's delimiter
/// row, so the rest of the stream cannot change how it draws. A finished
/// row then shows as it will in the final render, and the table keeps its
/// grid when the reply ends or is stopped.
fn finished_row(complete: &str, line: &str) -> bool {
    let trimmed = line.trim_end();
    if !trimmed.ends_with('|') || trimmed.ends_with("\\|") {
        return false;
    }
    complete
        .lines()
        .rev()
        .take_while(|above| above.contains('|') && !above.trim().is_empty())
        .find(|above| delimiter(above))
        .is_some_and(|delimiter_row| cells(delimiter_row) == cells(trimmed))
}

/// Where the last line of `complete` starts, when it may be a table's
/// header row whose delimiter row hasn't arrived: it starts with a pipe,
/// and no delimiter row precedes it in its run of piped lines. (A header
/// written without a leading pipe shows as text until its delimiter row
/// arrives; answers write tables with leading pipes.)
fn unconfirmed_header(complete: &str) -> Option<usize> {
    let mut lines = Vec::new();
    let mut at = 0;
    for line in complete.split_inclusive('\n') {
        lines.push((at, line.trim_end_matches(['\n', '\r'])));
        at += line.len();
    }
    let &(start, last) = lines.last()?;
    if !container(last).starts_with('|') || delimiter(last) {
        return None;
    }
    let table = lines
        .iter()
        .rev()
        .skip(1)
        .take_while(|(_, line)| line.contains('|') && !line.trim().is_empty())
        .any(|(_, line)| delimiter(line));
    (!table).then_some(start)
}

/// A block that holds inline text, as the parser reports it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leaf {
    Paragraph,
    Heading,
    Item,
    Other,
}

/// Where unclosed inline syntax starts in the block still being written:
/// the last paragraph, heading, or list item, when it reaches the end of
/// `body` and no blank line (or, for a heading, line end) has closed it.
fn open_inline(body: &str) -> Option<usize> {
    let trimmed = body.trim_end().len();
    if trimmed == 0 {
        return None;
    }
    let after = &body[trimmed..];
    if after.matches('\n').count() >= 2 {
        return None;
    }
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    // Open blocks, innermost last, each with where it starts.
    let mut blocks: Vec<(Leaf, usize)> = Vec::new();
    // The text runs of the most recent leaf block, and that block.
    let mut runs: Vec<Range<usize>> = Vec::new();
    let mut owner: Option<(Leaf, usize)> = None;
    let mut reaches_end = false;
    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                let leaf = match tag {
                    Tag::Paragraph => Some(Leaf::Paragraph),
                    Tag::Heading { .. } => Some(Leaf::Heading),
                    Tag::Item => Some(Leaf::Item),
                    Tag::CodeBlock(_)
                    | Tag::HtmlBlock
                    | Tag::Table(_)
                    | Tag::TableHead
                    | Tag::TableRow
                    | Tag::TableCell
                    | Tag::BlockQuote(_)
                    | Tag::List(_) => Some(Leaf::Other),
                    _ => None,
                };
                if let Some(leaf) = leaf {
                    blocks.push((leaf, range.start));
                    // Any new block ends the text of the one before it.
                    owner = None;
                    runs.clear();
                    reaches_end = false;
                }
            }
            Event::End(tag) => {
                let block = matches!(
                    tag,
                    TagEnd::Paragraph
                        | TagEnd::Heading(_)
                        | TagEnd::Item
                        | TagEnd::CodeBlock
                        | TagEnd::HtmlBlock
                        | TagEnd::Table
                        | TagEnd::TableHead
                        | TagEnd::TableRow
                        | TagEnd::TableCell
                        | TagEnd::BlockQuote(_)
                        | TagEnd::List(_)
                );
                if block && let Some(closed) = blocks.pop() {
                    if owner == Some(closed) {
                        reaches_end = range.end >= trimmed;
                    }
                }
            }
            Event::Text(_) => {
                if let Some(&(leaf, start)) = blocks.last()
                    && leaf != Leaf::Other
                {
                    if owner != Some((leaf, start)) {
                        owner = Some((leaf, start));
                        runs.clear();
                    }
                    runs.push(range);
                }
            }
            // A rule is a block of its own; the text before it is settled.
            Event::Rule => {
                owner = None;
                runs.clear();
            }
            _ => {}
        }
    }
    let (leaf, _) = owner?;
    if !reaches_end || (leaf == Leaf::Heading && after.contains('\n')) {
        return None;
    }
    let bytes = body.as_bytes();
    for run in runs {
        let mut i = run.start;
        while i < run.end {
            let b = bytes[i];
            if i > 0 && bytes[i - 1] == b'\\' {
                i += 1;
                continue;
            }
            match b {
                b'`' => return Some(i),
                // The start of an image, `![`.
                b'!' if i + 1 == body.len() => return Some(i),
                b'[' => {
                    return Some(if i > 0 && bytes[i - 1] == b'!' {
                        i - 1
                    } else {
                        i
                    });
                }
                b'<' if bytes.get(i + 1).is_none_or(u8::is_ascii_alphabetic) => {
                    return Some(i);
                }
                b'*' | b'_' | b'~' => {
                    let count = bytes[i..].iter().take_while(|&&x| x == b).count();
                    let before = body[..i].chars().next_back();
                    let next = body[i + count..].chars().next();
                    let opens = next.is_none_or(|n| !n.is_whitespace());
                    let can = match b {
                        b'_' => opens && !before.is_some_and(char::is_alphanumeric),
                        b'~' => opens && (count >= 2 || next.is_none()),
                        _ => opens,
                    };
                    if can {
                        return Some(i);
                    }
                    i += count;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests;
