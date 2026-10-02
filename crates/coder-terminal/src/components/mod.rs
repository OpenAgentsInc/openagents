//! The pieces a conversational terminal draws: turns, cards, run rows, and
//! list overlays.
//!
//! Each component is pure. It reads no clock, no environment, and no
//! terminal; it takes text and a width and returns ratatui values styled
//! through a [`Ladder`][crate::Ladder], so the same call serves truecolor, a
//! 256-color palette, and `NO_COLOR`.
//!
//! - [`turn`] draws a finished turn, a reply still streaming, and the notes
//!   under a turn.
//! - [`card`] draws a framed card in the transcript: a welcome, a pairing
//!   code, help.
//! - [`run`] draws one row of a Coder run. The caller maps its run events
//!   into [`run::RunRow`]s; the module knows no protocol.
//! - [`overlay`] draws a centered framed list over the screen.
//!
//! Transcript components return `Vec<Line<'static>>` whose rows are never
//! wider than the width they were given; the overlay draws into a
//! [`Buffer`][ratatui::buffer::Buffer] directly.

pub mod card;
pub mod diff;
pub mod overlay;
pub mod run;
pub mod turn;

pub use card::Card;
pub use overlay::{Item, ListOverlay};
pub use run::{FileRow, RunRow, ToolRow};
pub use turn::Who;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::wrap_rows;

/// The cells a turn's text sits in from the left edge, past the one cell
/// the screen leaves at its edge, so text starts two cells in. grok-build
/// starts at five; on a narrow pane that wasted a tenth of the width
/// (owner, 2026-10-02: "definitely less padding on the left").
pub(crate) const INDENT: usize = 1;

/// The indent a row gets at `width`: [`INDENT`], less at widths too narrow
/// to give it and still leave two cells, room for a wide character.
pub(crate) fn indent_at(width: usize) -> usize {
    INDENT.min(width.saturating_sub(2))
}

/// The display width of `text` in cells.
pub(crate) fn cells(text: &str) -> usize {
    text.width()
}

/// Soft-wraps one logical line: the first row is at most `width` cells and
/// every later row at most `width - hang`, so the caller can hang the
/// continuations under the text rather than its lead. Rows come back
/// without the hang; an empty line is one empty row.
pub(crate) fn wrap_hanging(text: &str, width: usize, hang: usize) -> Vec<String> {
    let width = width.max(1);
    let narrow = width.saturating_sub(hang).max(1);
    let first = wrap_rows(text, width);
    let Some(head) = first.first() else {
        return vec![String::new()];
    };
    let mut rows = vec![text[head.clone()].to_owned()];
    if first.len() == 1 {
        return rows;
    }
    let rest = text[head.end..].trim_start_matches(' ');
    if rest.is_empty() {
        return rows;
    }
    rows.extend(
        wrap_rows(rest, narrow)
            .into_iter()
            .map(|range| rest[range].to_owned()),
    );
    rows
}

/// Every logical line of `text` wrapped with [`wrap_hanging`], one entry per
/// row, each paired with whether it continues a logical line.
pub(crate) fn wrap_paragraphs(text: &str, width: usize, hang: usize) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        for (index, row) in wrap_hanging(&sanitize(line), width, hang)
            .into_iter()
            .enumerate()
        {
            out.push((index > 0, row));
        }
    }
    out
}

/// The leading graphemes of `text` that fit in `width` cells, cut without
/// a mark.
pub(crate) fn cut(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let w = grapheme.width();
        if used + w > width {
            break;
        }
        used += w;
        out.push_str(grapheme);
    }
    out
}

/// `text` clipped to `width` cells, its last visible cell an ellipsis when
/// anything was cut.
pub(crate) fn clip(text: &str, width: usize) -> String {
    if cells(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = cut(text, width - 1);
    out.push('…');
    out
}

/// Tabs become spaces and other control characters drop, so a row's width
/// is the width the terminal will draw.
pub(crate) fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hanging_rows_are_narrower_after_the_first() {
        let rows = wrap_hanging("aaa bbb ccc ddd", 8, 2);
        assert_eq!(rows, ["aaa bbb", "ccc", "ddd"]);
        assert!(rows.iter().skip(1).all(|row| cells(row) <= 6));
    }

    #[test]
    fn clip_marks_what_it_cut() {
        assert_eq!(clip("abcdef", 4), "abc…");
        assert_eq!(clip("abc", 4), "abc");
        assert_eq!(clip("日本語", 4), "日…");
        assert_eq!(clip("abc", 0), "");
    }

    #[test]
    fn sanitize_drops_controls_and_expands_tabs() {
        assert_eq!(sanitize("a\tb\x1b[0m"), "a    b[0m");
    }
}
