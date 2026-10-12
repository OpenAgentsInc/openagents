//! A file's diff as grok-build draws an edit.
//!
//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! `crates/codegen/xai-grok-pager/src/scrollback/blocks/tool/edit.rs`, its
//! hunk-only phase with the default `DiffRenderConfig` (two-cell indent, one
//! line-number column). Each line is numbered — the new line's number, or
//! the old one for a removed line — then marked `-`, `+`, or blank, then its
//! text syntax-highlighted with the palette's syntect theme, the old and new
//! sides each with their own highlighter.
//!
//! Adapted to look as Claude Code draws an edit: a removed or added line's
//! whole row, from its number to the row's end, sits on a dark red or green
//! band; when a removed line is replaced by an added one, the words that
//! changed sit on a brighter red or green; and `...` stands between hunks.
//! The input is a unified diff's text (`@@` hunks) rather than grok-build's
//! structured edit records, and a word wider than the row breaks rather than
//! overflowing.

use std::ops::Range;
use std::path::Path;

use code_highlight::grok::syntect::easy::HighlightLines;
use code_highlight::grok::{self, ColorLevel, DiffColors, Syntect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;

use super::{cells, sanitize};

/// What a diff line does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeTag {
    Equal,
    Delete,
    Insert,
}

/// One line of a hunk: its text and its old and new line numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub text: String,
    pub lo: usize,
    pub ln: usize,
    pub tag: ChangeTag,
}

pub type DiffHunk = Vec<DiffLine>;

/// Layout constants.
const INDENT: &str = "  ";
/// The cells grok-build's indent takes before the line numbers.
pub const INDENT_CELLS: usize = INDENT.len();
/// The cells between the line number and the marker, and between the
/// marker and the text.
const MARK_GAP: &str = " ";
/// The separator between hunks.
const HUNK_SEPARATOR: &str = "...";
/// The cells after the line number: a gap, the marker, a gap.
const MARK_CELLS: usize = 3;

/// The hunks of a unified diff. Lines before the first `@@`, file headers,
/// and "\ No newline at end of file" markers are not lines of a hunk.
pub fn hunks(patch: &str) -> Vec<DiffHunk> {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let (mut lo, mut ln) = (0usize, 0usize);
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("@@") {
            let (old, new) = hunk_starts(header);
            lo = old;
            ln = new;
            hunks.push(Vec::new());
            continue;
        }
        let Some(hunk) = hunks.last_mut() else {
            continue;
        };
        let (tag, text) = match line.as_bytes().first() {
            Some(b'+') => (ChangeTag::Insert, &line[1..]),
            Some(b'-') => (ChangeTag::Delete, &line[1..]),
            Some(b' ') => (ChangeTag::Equal, &line[1..]),
            Some(b'\\') => continue,
            None => (ChangeTag::Equal, ""),
            Some(_) => (ChangeTag::Equal, line),
        };
        let (at_lo, at_ln) = (lo, ln);
        match tag {
            ChangeTag::Equal => {
                lo += 1;
                ln += 1;
            }
            ChangeTag::Delete => lo += 1,
            ChangeTag::Insert => ln += 1,
        }
        hunk.push(DiffLine {
            text: text.to_owned(),
            lo: at_lo,
            ln: at_ln,
            tag,
        });
    }
    hunks
}

/// The old and new start lines of an `@@ -a,b +c,d @@` header.
fn hunk_starts(header: &str) -> (usize, usize) {
    let mut old = 0;
    let mut new = 0;
    for part in header.split_whitespace() {
        let number = |text: &str| {
            text.split(',')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0)
        };
        if let Some(rest) = part.strip_prefix('-') {
            old = number(rest);
        } else if let Some(rest) = part.strip_prefix('+') {
            new = number(rest);
        }
    }
    (old, new)
}

/// The cell where [`lines`]' code starts, `lead` cells in: after the
/// indent, the line-number column and the `+`/`-` mark. What sits left
/// of it is decoration a copy leaves out.
#[must_use]
pub fn content_column(patch: &str, lead: usize) -> usize {
    let gutter = hunks(patch).iter().map(gutter_width).max().unwrap_or(1);
    lead + INDENT.len() + gutter + MARK_CELLS
}

/// The text [`lines`] draws for a hunk separator row.
pub const SEPARATOR: &str = HUNK_SEPARATOR;

/// The drawn rows of `patch` for the file at `path`, `lead` cells in, each
/// at most `width` cells, in `palette` at `level`.
pub fn lines(
    patch: &str,
    path: &str,
    lead: usize,
    width: usize,
    palette: grok::Palette,
    level: ColorLevel,
) -> Vec<Line<'static>> {
    let hunks = hunks(patch);
    let syntect = palette.syntect();
    use coder_ui::source_theme as t;
    let apply = |color: rust_native::style::Color| {
        grok::color::quantize_color(Color::Rgb(color.red, color.green, color.blue), level)
    };
    let theme = DiffColors {
        delete_bg: apply(t::DIFF_DELETE_BG),
        delete_fg: apply(t::DIFF_DELETE_FG),
        insert_bg: apply(t::DIFF_INSERT_BG),
        insert_fg: apply(t::DIFF_INSERT_FG),
        equal_fg: apply(t::GRAY),
        gutter_fg: apply(t::GRAY),
        text_primary: apply(t::TEXT_PRIMARY),
        muted: apply(t::GRAY),
    };
    // The changed words of a replaced line; none without a band to sit on.
    let words =
        (!theme.uses_line_fg()).then(|| (apply(t::DIFF_DELETE_WORD), apply(t::DIFF_INSERT_WORD)));
    let room = width.saturating_sub(lead);
    // One number column for the whole diff, so every hunk lines up.
    let gutter = hunks.iter().map(gutter_width).max().unwrap_or(1);
    let mut out = Vec::new();
    for (i, hunk) in hunks.iter().enumerate() {
        if i > 0 && !out.is_empty() {
            let row = super::clip(&format!("{INDENT}{HUNK_SEPARATOR}"), room);
            out.push(Line::from(vec![
                Span::raw(" ".repeat(lead)),
                Span::styled(row, Style::default().fg(theme.muted)),
            ]));
        }
        if hunk.is_empty() {
            continue;
        }
        let total = INDENT.len() + gutter + MARK_CELLS;
        let content_width = room.saturating_sub(total);
        let texts: Vec<String> = hunk
            .iter()
            .map(|line| sanitize(line.text.trim_end_matches(['\r', '\n'])))
            .collect();
        let changed_words = words.map(|_| word_changes_in(hunk, &texts));
        // A diff interleaves two file versions; give each side its own highlighter so a multi-line construct can't leak across sides
        // Equal lines render on the new side and advance both
        let mut old_highlighter = syntect.highlight_lines_by_file_path(Path::new(path));
        let mut new_highlighter = syntect.highlight_lines_by_file_path(Path::new(path));
        for (index, line) in hunk.iter().enumerate() {
            let text = &texts[index];
            let mut content_spans = match line.tag {
                ChangeTag::Delete => render_content_spans(
                    text,
                    line.tag,
                    &theme,
                    &mut old_highlighter,
                    syntect,
                    level,
                ),
                ChangeTag::Insert => render_content_spans(
                    text,
                    line.tag,
                    &theme,
                    &mut new_highlighter,
                    syntect,
                    level,
                ),
                ChangeTag::Equal => {
                    let spans = render_content_spans(
                        text,
                        line.tag,
                        &theme,
                        &mut new_highlighter,
                        syntect,
                        level,
                    );
                    advance_highlighter(&mut old_highlighter, text, syntect);
                    spans
                }
            };
            if let (Some((delete_word, insert_word)), Some(changed)) = (words, &changed_words) {
                let bg = if line.tag == ChangeTag::Delete {
                    delete_word
                } else {
                    insert_word
                };
                content_spans = mark_words(content_spans, &changed[index], bg);
            }
            for row in assemble_rows(line, content_spans, gutter, total, content_width, &theme) {
                let mut spans = vec![Span::raw(" ".repeat(lead))];
                spans.extend(fit(row, room));
                out.push(Line::from(spans));
            }
        }
    }
    out
}

/// The spans cut to `room` cells: the gutter alone can be wider than a
/// very narrow row.
fn fit(row: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let mut used = 0;
    let mut out = Vec::new();
    for span in row {
        let w = cells(&span.content);
        if used + w <= room {
            used += w;
            out.push(span);
            continue;
        }
        let piece = super::cut(&span.content, room - used);
        if !piece.is_empty() {
            out.push(Span::styled(piece, span.style));
        }
        break;
    }
    out
}

/// The single line-number column's width for a hunk.
fn gutter_width(hunk: &DiffHunk) -> usize {
    let mut max_num = 1usize;
    for line in hunk {
        max_num = max_num.max(line.lo.max(1)).max(line.ln.max(1));
    }
    max_num.ilog10() as usize + 1
}

/// Gutter then content, wrapped to `content_width`, each row's content on
/// the change's band to the end of the row.
fn assemble_rows(
    line: &DiffLine,
    content_spans: Vec<Span<'static>>,
    gutter: usize,
    total: usize,
    content_width: usize,
    theme: &DiffColors,
) -> Vec<Vec<Span<'static>>> {
    let bg = match line.tag {
        ChangeTag::Equal => None,
        ChangeTag::Delete => Some(theme.delete_bg),
        ChangeTag::Insert => Some(theme.insert_bg),
    }
    .filter(|bg| *bg != Color::Reset);
    let raw_content: String = content_spans.iter().map(|s| s.content.as_ref()).collect();
    let wrapped_lines = wrap_text(&raw_content, content_width);
    let styled_rows = match project_styles_onto_wrap_segments(&content_spans, &wrapped_lines) {
        Some(rows) if rows.len() == wrapped_lines.len() => rows,
        _ => {
            let style = match line.tag {
                ChangeTag::Equal => Style::default().fg(theme.equal_fg),
                ChangeTag::Delete | ChangeTag::Insert => {
                    if theme.uses_line_fg() {
                        let fg = if line.tag == ChangeTag::Delete {
                            theme.delete_fg
                        } else {
                            theme.insert_fg
                        };
                        Style::default().fg(fg)
                    } else {
                        Style::default().fg(theme.text_primary)
                    }
                }
            };
            wrapped_lines
                .iter()
                .map(|wrapped| vec![Span::styled(wrapped.clone(), style)])
                .collect()
        }
    };
    let mut rows = Vec::new();
    for (i, (wrapped, row)) in wrapped_lines.iter().zip(styled_rows).enumerate() {
        let mut spans = Vec::new();
        if i == 0 {
            render_gutter(&mut spans, line, gutter, theme, bg);
        } else {
            spans.push(Span::raw(INDENT));
            let blank = " ".repeat(total - INDENT.len());
            spans.push(match bg {
                Some(bg) => Span::styled(blank, Style::default().bg(bg)),
                None => Span::raw(blank),
            });
        }
        match bg {
            Some(bg) => {
                // A changed word keeps its brighter band.
                spans.extend(row.into_iter().map(|span| {
                    let style = if span.style.bg.is_some() {
                        span.style
                    } else {
                        span.style.bg(bg)
                    };
                    span.style(style)
                }));
                let pad = content_width.saturating_sub(cells(wrapped));
                if pad > 0 {
                    spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
                }
            }
            None => spans.extend(row),
        }
        rows.push(spans);
    }
    rows
}

/// The line number, then the change's marker, both in the change's light
/// green or red, on the row's band when the line has one, as Claude Code
/// draws them; never plain white.
fn render_gutter(
    spans: &mut Vec<Span<'static>>,
    line: &DiffLine,
    w: usize,
    theme: &DiffColors,
    bg: Option<Color>,
) {
    spans.push(Span::raw(INDENT));
    let (number, marker, fg) = match line.tag {
        ChangeTag::Equal => (line.ln, ' ', theme.gutter_fg),
        ChangeTag::Delete => (line.lo, '-', theme.delete_fg),
        ChangeTag::Insert => (line.ln, '+', theme.insert_fg),
    };
    let style = match bg {
        Some(bg) => Style::default().fg(fg).bg(bg),
        None => Style::default().fg(fg),
    };
    spans.push(Span::styled(format!("{number:>w$}"), style));
    spans.push(Span::styled(format!("{MARK_GAP}{marker}{MARK_GAP}"), style));
}

/// The changed words of each line of a hunk, as byte ranges of its text:
/// each run of removed lines is paired, line by line, with the run of added
/// lines right after it. An unpaired line, a context line, and a pair too
/// unlike to compare word by word have none.
fn word_changes_in(hunk: &DiffHunk, texts: &[String]) -> Vec<Vec<Range<usize>>> {
    let mut changes = vec![Vec::new(); hunk.len()];
    let mut i = 0;
    while i < hunk.len() {
        if hunk[i].tag != ChangeTag::Delete {
            i += 1;
            continue;
        }
        let deletes = i;
        while i < hunk.len() && hunk[i].tag == ChangeTag::Delete {
            i += 1;
        }
        let inserts = i;
        while i < hunk.len() && hunk[i].tag == ChangeTag::Insert {
            i += 1;
        }
        let pairs = (inserts - deletes).min(i - inserts);
        for k in 0..pairs {
            let (old, new) = (deletes + k, inserts + k);
            if let Some((a, b)) = word_changes(&texts[old], &texts[new]) {
                changes[old] = a;
                changes[new] = b;
            }
        }
    }
    changes
}

/// Words, runs of spaces, and single other characters.
fn tokens(text: &str) -> Vec<Range<usize>> {
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        }
    };
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut last = None;
    for (at, c) in text.char_indices() {
        let kind = class(c);
        let end = at + c.len_utf8();
        match out.last_mut() {
            Some(token) if kind != 2 && last == Some(kind) => token.end = end,
            _ => out.push(at..end),
        }
        last = Some(kind);
    }
    out
}

/// The changed words of `old` and of `new`, by the longest common run of
/// their tokens, with words split only by spaces merged into one range;
/// `None` when the lines share too little for word highlights to help.
fn word_changes(old: &str, new: &str) -> Option<(Vec<Range<usize>>, Vec<Range<usize>>)> {
    let a = tokens(old);
    let b = tokens(new);
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 || n.saturating_mul(m) > 250_000 {
        return None;
    }
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i * (m + 1) + j] = if old[a[i].clone()] == new[b[j].clone()] {
                table[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                table[(i + 1) * (m + 1) + j].max(table[i * (m + 1) + j + 1])
            };
        }
    }
    let (mut keep_a, mut keep_b) = (vec![false; n], vec![false; m]);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if old[a[i].clone()] == new[b[j].clone()] {
            keep_a[i] = true;
            keep_b[j] = true;
            i += 1;
            j += 1;
        } else if table[(i + 1) * (m + 1) + j] >= table[i * (m + 1) + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let common: usize = a
        .iter()
        .zip(&keep_a)
        .filter(|(token, kept)| **kept && !old[(*token).clone()].trim().is_empty())
        .map(|(token, _)| token.len())
        .sum();
    let longest = old.trim().len().max(new.trim().len());
    // Mostly rewritten: the row's band says enough.
    if common * 4 < longest {
        return None;
    }
    let ranges = |text: &str, tokens: &[Range<usize>], kept: &[bool]| {
        let mut out: Vec<Range<usize>> = Vec::new();
        // Only spaces since the last changed word: the next one joins it.
        let mut joinable = false;
        for (token, kept) in tokens.iter().zip(kept) {
            if *kept {
                joinable = joinable && text[token.clone()].trim().is_empty();
                continue;
            }
            match out.last_mut() {
                Some(last) if joinable || last.end == token.start => last.end = token.end,
                _ => out.push(token.clone()),
            }
            joinable = true;
        }
        out
    };
    Some((ranges(old, &a, &keep_a), ranges(new, &b, &keep_b)))
}

/// The content spans split at the `changed` byte ranges, those on `bg`.
fn mark_words(
    spans: Vec<Span<'static>>,
    changed: &[Range<usize>],
    bg: Color,
) -> Vec<Span<'static>> {
    if changed.is_empty() {
        return spans;
    }
    let mut out = Vec::new();
    let mut at = 0;
    for span in spans {
        let text = span.content.as_ref();
        let end = at + text.len();
        let mut cut = at;
        let mut edges: Vec<usize> = changed
            .iter()
            .flat_map(|r| [r.start, r.end])
            .filter(|edge| *edge > at && *edge < end)
            .collect();
        edges.push(end);
        for edge in edges {
            let Some(piece) = text.get(cut - at..edge - at) else {
                continue;
            };
            if piece.is_empty() {
                continue;
            }
            let inside = changed.iter().any(|r| r.start <= cut && cut < r.end);
            let style = if inside {
                span.style.bg(bg)
            } else {
                span.style
            };
            out.push(Span::styled(piece.to_owned(), style));
            cut = edge;
        }
        at = end;
    }
    out
}

/// Span with `style`; empty text paints a single space so the row keeps a visible background band and stays selectable.
fn painted(text: &str, style: Style) -> Span<'static> {
    let text = if text.is_empty() { " " } else { text };
    Span::styled(text.to_string(), style)
}

fn advance_highlighter(
    highlighter: &mut Option<HighlightLines<'_>>,
    content: &str,
    syntect: &Syntect,
) {
    if let Some(hl) = highlighter.as_mut() {
        let _ = hl.highlight_line(&format!("{content}\n"), &syntect.syntax_set);
    }
}

/// Render content spans with syntax highlighting.
fn render_content_spans(
    content: &str,
    tag: ChangeTag,
    theme: &DiffColors,
    highlighter: &mut Option<HighlightLines<'_>>,
    syntect: &Syntect,
    level: ColorLevel,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    if tag != ChangeTag::Equal && theme.uses_line_fg() {
        let fg = match tag {
            ChangeTag::Delete => theme.delete_fg,
            _ => theme.insert_fg,
        };
        spans.push(painted(content, Style::default().fg(fg)));
        return spans;
    }

    // Try syntax highlighting
    if let Some(hl) = highlighter.as_mut()
        && let Ok(ranges) = hl.highlight_line(&format!("{content}\n"), &syntect.syntax_set)
    {
        let mut wrote = false;
        for (style, segment) in ranges {
            let mut text = segment.to_owned();
            while text.ends_with('\n') || text.ends_with('\r') {
                text.pop();
            }
            if text.is_empty() {
                continue;
            }
            spans.push(Span::styled(text, {
                let mut style = grok::color::syntect_to_ratatui_fg(style, ColorLevel::TrueColor);
                style.fg = style
                    .fg
                    .map(|color| crate::ladder::appearance(color, level));
                style
            }));
            wrote = true;
        }
        if wrote {
            return spans;
        }
    }

    // Fallback: plain text
    let style = match tag {
        ChangeTag::Equal => Style::default().fg(theme.equal_fg),
        ChangeTag::Delete | ChangeTag::Insert => Style::default().fg(theme.text_primary),
    };
    spans.push(painted(content, style));

    spans
}

/// grok-build's word wrap, by cells, with a word wider than the row broken
/// at grapheme edges.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_inclusive(|c: char| c.is_whitespace()) {
        let word_width = cells(word);

        if current_width + word_width > max_width && !current_line.is_empty() {
            lines.push(std::mem::take(&mut current_line));
            current_width = 0;
        }

        if word_width > max_width {
            for grapheme in word.graphemes(true) {
                let w = cells(grapheme);
                if current_width + w > max_width && !current_line.is_empty() {
                    lines.push(std::mem::take(&mut current_line));
                    current_width = 0;
                }
                current_line.push_str(grapheme);
                current_width += w;
            }
            continue;
        }

        current_line.push_str(word);
        current_width += word_width;
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Walks the source spans with a monotonic cursor, splitting only at existing span edges or wrap edges. Each whole
/// [`Style`] is copied onto owned substrings, so the concat of every returned row's text equals its wrap segment.
fn project_styles_onto_wrap_segments(
    content_spans: &[Span<'static>],
    wrapped_segments: &[String],
) -> Option<Vec<Vec<Span<'static>>>> {
    let spans_text: String = content_spans.iter().map(|s| s.content.as_ref()).collect();
    if spans_text != wrapped_segments.concat() {
        return None;
    }

    let mut rows = Vec::with_capacity(wrapped_segments.len());
    let mut span_idx = 0;
    let mut span_byte = 0;
    for segment in wrapped_segments {
        let mut row = Vec::new();
        let mut remaining = segment.len();
        while remaining > 0 {
            let span = content_spans.get(span_idx)?;
            let available = span.content.len().checked_sub(span_byte)?;
            if available == 0 {
                span_idx += 1;
                span_byte = 0;
                continue;
            }
            let take = available.min(remaining);
            // `get` fails closed if either edge is not a char boundary.
            let piece = span.content.get(span_byte..span_byte + take)?;
            row.push(Span::styled(piece.to_owned(), span.style));
            span_byte += take;
            remaining -= take;
        }
        rows.push(row);
    }
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use grok::Palette;

    const PATCH: &str = "@@ -1,3 +1,3 @@\n fn main() {\n-    let x = 1;\n+    let x = 2;\n }\n@@ -10 +10 @@\n-a\n+b";

    fn text(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| line.to_string().trim_end().to_owned())
            .collect()
    }

    #[test]
    fn a_patch_parses_into_numbered_hunks() {
        let hunks = hunks(PATCH);
        assert_eq!(hunks.len(), 2);
        let tags: Vec<_> = hunks[0].iter().map(|l| (l.tag, l.lo, l.ln)).collect();
        assert_eq!(
            tags,
            [
                (ChangeTag::Equal, 1, 1),
                (ChangeTag::Delete, 2, 2),
                (ChangeTag::Insert, 3, 2),
                (ChangeTag::Equal, 3, 3),
            ]
        );
        assert_eq!(hunks[1][0].lo, 10);
    }

    #[test]
    fn a_diff_draws_marked_numbered_lines_on_bands_reaching_the_row_end() {
        let palette = Palette::Night;
        let delete_bg = crate::ladder::rgb(coder_ui::coder_noir::DIFF_DELETE_ROW);
        let insert_bg = crate::ladder::rgb(coder_ui::coder_noir::DIFF_INSERT_ROW);
        let lines = lines(PATCH, "src/main.rs", 4, 60, palette, ColorLevel::TrueColor);
        assert_eq!(
            text(&lines),
            [
                "       1   fn main() {",
                "       2 -     let x = 1;",
                "       2 +     let x = 2;",
                "       3   }",
                "      ...",
                "      10 - a",
                "      10 + b",
            ]
        );
        for line in &lines {
            assert!(line.width() <= 60, "{line}");
        }
        // The removed line: its band starts at the number and runs to the
        // row's end, under the keyword's Grok Night color.
        let removed = &lines[1];
        assert_eq!(removed.width(), 60);
        assert_eq!(removed.spans[2].content.as_ref(), " 2");
        assert_eq!(removed.spans[2].style.bg, Some(delete_bg));
        assert_eq!(removed.spans[3].content.as_ref(), " - ");
        // The number and the marker keep the change's color on the band,
        // not white.
        let danger = crate::ladder::rgb(coder_ui::coder_noir::DIFF_DELETE_MARK);
        let success = crate::ladder::rgb(coder_ui::coder_noir::DIFF_INSERT_MARK);
        assert_eq!(removed.spans[2].style.fg, Some(danger));
        assert_eq!(removed.spans[3].style.fg, Some(danger));
        assert_eq!(lines[2].spans[3].content.as_ref(), " + ");
        assert_eq!(lines[2].spans[3].style.fg, Some(success));
        assert!(removed.spans[2..].iter().all(|s| s.style.bg.is_some()));
        let keyword = removed
            .spans
            .iter()
            .find(|span| span.content.as_ref() == "let")
            .expect("keyword span");
        assert_eq!(keyword.style.bg, Some(delete_bg));
        let Some(Color::Rgb(r, g, b)) = keyword.style.fg else {
            panic!("keyword fg {:?}", keyword.style.fg);
        };
        assert_ne!((r, g, b), (0xb2, 0xb2, 0xb2), "keyword drew as plain text");
        assert_eq!(
            removed.spans.last().map(|span| span.style.bg),
            Some(Some(delete_bg))
        );
        // The added line on green from its number; the context line on none.
        assert_eq!(lines[2].width(), 60);
        assert_eq!(lines[2].spans[2].style.bg, Some(insert_bg));
        assert_eq!(
            lines[2].spans.last().map(|span| span.style.bg),
            Some(Some(insert_bg))
        );
        assert!(lines[0].spans.iter().all(|s| s.style.bg.is_none()));
    }

    #[test]
    fn a_replaced_line_highlights_the_words_that_changed() {
        let patch = "@@ -251,3 +251,3 @@\n context\n-/// Reads a file inside the workspace.\n+/// Reads a file on this computer.\n end\n";
        let delete_word = crate::ladder::rgb(coder_ui::coder_noir::DIFF_DELETE_WORD);
        let insert_word = crate::ladder::rgb(coder_ui::coder_noir::DIFF_INSERT_WORD);
        let lines = lines(patch, "a.rs", 0, 60, Palette::Night, ColorLevel::TrueColor);
        assert_eq!(
            text(&lines),
            [
                "  251   context",
                "  252 - /// Reads a file inside the workspace.",
                "  252 + /// Reads a file on this computer.",
                "  253   end",
            ]
        );
        let on = |line: &Line<'_>, bg: Color| -> String {
            line.spans
                .iter()
                .filter(|s| s.style.bg == Some(bg))
                .map(|s| s.content.as_ref())
                .collect()
        };
        assert_eq!(on(&lines[1], delete_word), "inside the workspace");
        assert_eq!(on(&lines[2], insert_word), "on this computer");
        // Unchanged words keep the row's band.
        let reads = lines[2]
            .spans
            .iter()
            .find(|s| s.content.contains("Reads"))
            .unwrap();
        assert_eq!(
            reads.style.bg,
            Some(crate::ladder::rgb(coder_ui::coder_noir::DIFF_INSERT_ROW))
        );
    }

    #[test]
    fn unlike_lines_and_unpaired_lines_get_no_word_highlights() {
        assert_eq!(word_changes("let a = 1;", "fn other(x: u8) {}"), None);
        let (old, new) = word_changes("x.foo(1, 2)", "x.foo(1, 3)").unwrap();
        assert_eq!(old, [9..10]);
        assert_eq!(new, [9..10]);
        let hunk = hunks("@@ -1 +1,2 @@\n-a b c\n+a b d\n+added\n").remove(0);
        let texts: Vec<String> = hunk.iter().map(|l| l.text.clone()).collect();
        let changes = word_changes_in(&hunk, &texts);
        assert_eq!(changes, [vec![4..5], vec![4..5], vec![]]);
    }

    #[test]
    fn without_color_changed_lines_carry_their_color_as_text() {
        let lines = lines(
            PATCH,
            "src/main.rs",
            0,
            40,
            Palette::Night,
            ColorLevel::None,
        );
        assert!(
            lines
                .iter()
                .all(|line| line.spans.iter().all(|s| s.style.bg.is_none()))
        );
    }

    #[test]
    fn long_lines_wrap_under_the_gutter_within_the_width() {
        let patch = format!("@@ -1 +1 @@\n+{}", "word ".repeat(30) + &"x".repeat(80));
        for width in [0usize, 1, 5, 12, 40] {
            for line in lines(
                &patch,
                "a.txt",
                2,
                width,
                Palette::Night,
                ColorLevel::TrueColor,
            ) {
                assert!(line.width() <= width.max(2), "{width}: {line}");
            }
        }
    }
}
