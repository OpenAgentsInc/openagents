//! A file's diff as grok-build draws an edit.
//!
//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! `crates/codegen/xai-grok-pager/src/scrollback/blocks/tool/edit.rs`, its
//! hunk-only phase with the default `DiffRenderConfig` (two-cell indent, one
//! line-number column, no gutter band, "…" between hunks). Each line is
//! numbered in the gutter — the new line's number, or the old one for a
//! removed line, red or green on a change — then its text syntax-highlighted
//! with the palette's syntect theme, the old and new sides each with their
//! own highlighter. Removed and added lines sit on Grok Night's (or Grok
//! Day's) red and green bands, from the text to the row's end.
//!
//! Adapted: the input is a unified diff's text (`@@` hunks) rather than
//! grok-build's structured edit records, and a word wider than the row
//! breaks rather than overflowing.

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
pub(crate) const INDENT_CELLS: usize = INDENT.len();
const CONTENT_GAP: &str = "  ";
/// The separator between hunks.
const HUNK_SEPARATOR: &str = "…";

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
    let room = width.saturating_sub(lead);
    let mut out = Vec::new();
    for (i, hunk) in hunks.iter().enumerate() {
        if i > 0 && !out.is_empty() {
            let sep_text = match hunks.get(i - 1).and_then(|prev| hunk_gap_lines(prev, hunk)) {
                Some(1) => format!("{HUNK_SEPARATOR} 1 unchanged line"),
                Some(n) => format!("{HUNK_SEPARATOR} {n} unchanged lines"),
                None => HUNK_SEPARATOR.to_owned(),
            };
            let row = super::clip(&format!("{INDENT}{sep_text}"), room);
            out.push(Line::from(vec![
                Span::raw(" ".repeat(lead)),
                Span::styled(row, Style::default().fg(theme.muted)),
            ]));
        }
        if hunk.is_empty() {
            continue;
        }
        let gutter = gutter_width(hunk);
        let total = INDENT.len() + gutter + CONTENT_GAP.len();
        let content_width = room.saturating_sub(total);
        // A diff interleaves two file versions; give each side its own highlighter so a multi-line construct can't leak across sides
        // Equal lines render on the new side and advance both
        let mut old_highlighter = syntect.highlight_lines_by_file_path(Path::new(path));
        let mut new_highlighter = syntect.highlight_lines_by_file_path(Path::new(path));
        for line in hunk {
            let text = sanitize(line.text.trim_end_matches(['\r', '\n']));
            let content_spans = match line.tag {
                ChangeTag::Delete => render_content_spans(
                    &text,
                    line.tag,
                    &theme,
                    &mut old_highlighter,
                    syntect,
                    level,
                ),
                ChangeTag::Insert => render_content_spans(
                    &text,
                    line.tag,
                    &theme,
                    &mut new_highlighter,
                    syntect,
                    level,
                ),
                ChangeTag::Equal => {
                    let spans = render_content_spans(
                        &text,
                        line.tag,
                        &theme,
                        &mut new_highlighter,
                        syntect,
                        level,
                    );
                    advance_highlighter(&mut old_highlighter, &text, syntect);
                    spans
                }
            };
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

/// Unchanged new-file lines hidden between two hunks, when computable.
fn hunk_gap_lines(prev: &DiffHunk, next: &DiffHunk) -> Option<usize> {
    let prev_last = prev.iter().rev().find(|l| l.tag != ChangeTag::Delete)?.ln;
    let next_first = next.iter().find(|l| l.tag != ChangeTag::Delete)?.ln;
    next_first
        .checked_sub(prev_last)
        .and_then(|d| d.checked_sub(1))
        .filter(|n| *n > 0)
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
            render_gutter(&mut spans, line, gutter, theme);
        } else {
            spans.push(Span::raw(" ".repeat(total)));
        }
        match bg {
            Some(bg) => {
                spans.extend(row.into_iter().map(|span| {
                    let style = span.style.bg(bg);
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

/// Render the line number gutter.
fn render_gutter(spans: &mut Vec<Span<'static>>, line: &DiffLine, w: usize, theme: &DiffColors) {
    let gutter_style = Style::default().fg(theme.gutter_fg);
    spans.push(Span::raw(INDENT));
    // Single mode: one column with the relevant line number
    match line.tag {
        ChangeTag::Equal => {
            spans.push(Span::styled(format!("{:>w$}", line.ln), gutter_style));
        }
        ChangeTag::Delete => {
            spans.push(Span::styled(
                format!("{:>w$}", line.lo),
                Style::default().fg(theme.delete_fg),
            ));
        }
        ChangeTag::Insert => {
            spans.push(Span::styled(
                format!("{:>w$}", line.ln),
                Style::default().fg(theme.insert_fg),
            ));
        }
    }
    // Gap between gutter and content
    spans.push(Span::raw(CONTENT_GAP));
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
    fn a_diff_draws_numbered_highlighted_lines_on_bands() {
        let palette = Palette::Night;
        let delete_fg = crate::ladder::rgb(coder_ui::coder_noir::DANGER);
        let delete_bg = crate::ladder::rgb(coder_ui::coder_noir::DANGER_CONTAINER);
        let insert_bg = crate::ladder::rgb(coder_ui::coder_noir::SUCCESS_CONTAINER);
        let lines = lines(PATCH, "src/main.rs", 4, 60, palette, ColorLevel::TrueColor);
        assert_eq!(
            text(&lines),
            [
                "      1  fn main() {",
                "      2      let x = 1;",
                "      2      let x = 2;",
                "      3  }",
                "      … 6 unchanged lines",
                "      10  a",
                "      10  b",
            ]
        );
        for line in &lines {
            assert!(line.width() <= 60, "{line}");
        }
        // The removed line: red number, red band to the row's end, and the
        // keyword in its Grok Night color on that band.
        let removed = &lines[1];
        assert_eq!(removed.width(), 60);
        assert_eq!(removed.spans[2].style.fg, Some(delete_fg));
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
        // The added line on green; the context line on no band.
        assert!(
            lines[2]
                .spans
                .iter()
                .skip(4)
                .all(|s| s.style.bg == Some(insert_bg))
        );
        assert!(lines[0].spans.iter().all(|s| s.style.bg.is_none()));
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
