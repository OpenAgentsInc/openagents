//! Selecting and copying transcript text the way a person expects:
//! drag across rows, double-click a word, triple-click a line.
//!
//! The model follows grok-build's scrollback selection (Apache-2.0, see
//! the crate's NOTICE): a selection's ends are rows of the whole
//! transcript, not of the screen, so it stays on the same text while the
//! view scrolls; each drawn row carries a [`RowCopy`] saying which of its
//! cells are decoration (gutter marks, hanging indents, borders) and
//! whether it continues the row above (a soft wrap). Copies skip the
//! decoration and join wrapped rows back into the line they came from.

use std::ops::Range;
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// What a copy takes from one drawn row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowCopy {
    /// Cells at the row's start that are decoration: `❯`, `●`, `⎿`, a
    /// Run box's bar, a diff's line numbers and `+`/`-` column, a
    /// hanging indent.
    pub lead: u16,
    /// Further decoration cells, such as a quote bar or a table's
    /// borders. `Some(ch)` copies the range as `ch` (a table's inner
    /// border copies as a tab).
    pub skip: Vec<(Range<u16>, Option<char>)>,
    /// The row continues the one above it (a soft wrap): a copy joins
    /// them with this text instead of a line break.
    pub joiner: Option<String>,
    /// The row is all decoration (a table's rule, a hunk separator) and
    /// is left out of copies.
    pub omit: bool,
}

impl RowCopy {
    /// A row whose first `lead` cells are decoration.
    #[must_use]
    pub fn lead(lead: u16) -> Self {
        Self {
            lead,
            ..Self::default()
        }
    }

    /// The same row drawn `cells` further right (a mark put in front).
    #[must_use]
    pub fn shifted(mut self, cells: u16) -> Self {
        self.lead = self.lead.saturating_add(cells);
        for (range, _) in &mut self.skip {
            *range = range.start.saturating_add(cells)..range.end.saturating_add(cells);
        }
        self
    }

    fn decoration(&self, col: u16) -> Option<(Range<u16>, Option<char>)> {
        if col < self.lead {
            return Some((0..self.lead, None));
        }
        self.skip
            .iter()
            .find(|(range, _)| range.contains(&col))
            .cloned()
    }
}

/// A cell of the transcript: `row` counts from the transcript's first
/// row, `col` from the transcript's left edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub row: usize,
    pub col: u16,
}

/// A run of selected cells, both ends included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Point,
    pub head: Point,
}

impl Selection {
    /// The selection's first and last cells in reading order.
    #[must_use]
    pub fn ordered(&self) -> (Point, Point) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// The selected columns of `row`, end excluded.
    #[must_use]
    pub fn cols(&self, row: usize) -> Option<Range<u16>> {
        let (start, end) = self.ordered();
        if row < start.row || row > end.row {
            return None;
        }
        let lo = if row == start.row { start.col } else { 0 };
        let hi = if row == end.row {
            end.col.saturating_add(1)
        } else {
            u16::MAX
        };
        (lo < hi).then_some(lo..hi)
    }
}

/// Each grapheme of `text` with the column it starts at and its width.
fn cells(text: &str) -> Vec<(u16, u16, &str)> {
    let mut col = 0u16;
    let mut out = Vec::new();
    for grapheme in text.graphemes(true) {
        let width = grapheme.width().min(usize::from(u16::MAX)) as u16;
        out.push((col, width, grapheme));
        col = col.saturating_add(width);
    }
    out
}

/// The copyable text of `cols` of one row: decoration left out, trailing
/// blanks trimmed.
#[must_use]
pub fn row_text(text: &str, copy: &RowCopy, cols: Range<u16>) -> String {
    let mut out = String::new();
    let mut table = false;
    for (col, width, grapheme) in cells(text) {
        let end = col.saturating_add(width.max(1));
        if end <= cols.start || col >= cols.end {
            continue;
        }
        match copy.decoration(col) {
            Some((range, Some(ch))) => {
                if range.start == col {
                    out.push(ch);
                    table |= ch == '\t';
                }
            }
            Some((_, None)) => {}
            None => out.push_str(grapheme),
        }
    }
    if table {
        // A table row: cells padded to their column, separated by tabs.
        return out
            .split('\t')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\t")
            .trim_matches('\t')
            .to_owned();
    }
    out.trim_end().to_owned()
}

/// The text a selection copies. `row` gives a transcript row's drawn
/// text and its [`RowCopy`]; rows it has no answer for end the copy.
pub fn copy_text<'a>(
    selection: &Selection,
    row: impl Fn(usize) -> Option<(String, &'a RowCopy)>,
) -> String {
    let (start, end) = selection.ordered();
    let mut out = String::new();
    let mut previous: Option<usize> = None;
    for index in start.row..=end.row {
        let Some((text, copy)) = row(index) else {
            break;
        };
        if copy.omit {
            continue;
        }
        let Some(cols) = selection.cols(index) else {
            continue;
        };
        let piece = row_text(&text, copy, cols);
        if let Some(last) = previous {
            match &copy.joiner {
                Some(joiner) if last + 1 == index => out.push_str(joiner),
                _ => out.push('\n'),
            }
        }
        out.push_str(&piece);
        previous = Some(index);
    }
    out.trim_end_matches('\n').to_owned()
}

/// Characters that end a word for double-click, besides whitespace.
/// Path and URL punctuation (`/ . : - _ ~ @ # ? = & % +`) stays inside.
const WORD_BREAKS: &str = "()[]{}<>\"'`,;|│";

fn in_word(grapheme: &str) -> bool {
    !grapheme.chars().all(char::is_whitespace) && !grapheme.chars().any(|c| WORD_BREAKS.contains(c))
}

/// The cells of the word under `col` on one row, both ends included;
/// a blank or a break character selects just itself.
#[must_use]
pub fn word_at(text: &str, copy: &RowCopy, col: u16) -> Option<(u16, u16)> {
    let cells = cells(text);
    let at = cells
        .iter()
        .position(|(start, width, _)| col >= *start && col < start + (*width).max(1))?;
    let usable = |index: usize| {
        let (start, _, grapheme) = cells[index];
        copy.decoration(start).is_none() && in_word(grapheme)
    };
    if !usable(at) {
        let (start, width, _) = cells[at];
        return copy
            .decoration(start)
            .is_none()
            .then_some((start, start + width.max(1) - 1));
    }
    let mut first = at;
    while first > 0 && usable(first - 1) {
        first -= 1;
    }
    let mut last = at;
    while last + 1 < cells.len() && usable(last + 1) {
        last += 1;
    }
    let (end_col, end_width, _) = cells[last];
    Some((cells[first].0, end_col + end_width.max(1) - 1))
}

/// The rows of the logical line through `row`: up and down across soft
/// wraps. `joined` says whether a row continues the one above it.
pub fn line_rows(row: usize, count: usize, joined: impl Fn(usize) -> bool) -> (usize, usize) {
    let mut first = row;
    while first > 0 && joined(first) {
        first -= 1;
    }
    let mut last = row;
    while last + 1 < count && joined(last + 1) {
        last += 1;
    }
    (first, last)
}

/// Draws the selection over the rows `first_row..` shown in `area`:
/// `rows` holds each visible row's text and [`RowCopy`]. Only text cells
/// light up; decoration and blank row ends stay as drawn.
pub fn paint(
    buf: &mut Buffer,
    area: Rect,
    first_row: usize,
    rows: &[(String, RowCopy)],
    selection: &Selection,
    style: Style,
) {
    for (offset, (text, copy)) in rows.iter().enumerate().take(usize::from(area.height)) {
        if copy.omit {
            continue;
        }
        let Some(cols) = selection.cols(first_row + offset) else {
            continue;
        };
        let content_end = text.trim_end().width().min(usize::from(u16::MAX)) as u16;
        let y = area.y + offset as u16;
        for (col, width, _) in cells(text) {
            if col >= content_end || col >= area.width {
                break;
            }
            let end = col.saturating_add(width.max(1));
            if end <= cols.start || col >= cols.end || copy.decoration(col).is_some() {
                continue;
            }
            for x in col..end.min(area.width) {
                if let Some(cell) = buf.cell_mut((area.x + x, y)) {
                    cell.set_style(style);
                }
            }
        }
    }
}

/// How far a drag past the transcript's top or bottom edge scrolls each
/// tick: negative is up. Rows nearer the edge than `EDGE` count, and the
/// farther past it the pointer is, the faster it goes.
#[must_use]
pub fn edge_scroll(row: u16, area: Rect) -> i16 {
    const EDGE: u16 = 1;
    if area.height == 0 {
        return 0;
    }
    let speed = |distance: u16| match distance {
        0..=2 => 1,
        3..=5 => 2,
        6..=10 => 3,
        _ => 5,
    };
    let top = area.y.saturating_add(EDGE);
    let bottom = area.bottom().saturating_sub(EDGE);
    if row < top {
        -speed(top - row)
    } else if row >= bottom {
        speed(row - bottom + 1)
    } else {
        0
    }
}

/// Counts presses for double- and triple-click: a press within
/// [`Clicks::WINDOW`] of the last, on the same row and within a cell of
/// it, counts one more, up to three.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clicks {
    last: Option<(Instant, Point, u8)>,
}

impl Clicks {
    pub const WINDOW: Duration = Duration::from_millis(500);

    pub fn press(&mut self, now: Instant, at: Point) -> u8 {
        let count = match self.last {
            Some((time, point, count))
                if now.duration_since(time) <= Self::WINDOW
                    && point.row == at.row
                    && point.col.abs_diff(at.col) <= 1
                    && count < 3 =>
            {
                count + 1
            }
            _ => 1,
        };
        self.last = Some((now, at, count));
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn rows(list: &[(&str, RowCopy)]) -> Vec<(String, RowCopy)> {
        list.iter()
            .map(|(text, copy)| ((*text).to_owned(), copy.clone()))
            .collect()
    }

    fn copy_of(rows: &[(String, RowCopy)], selection: Selection) -> String {
        copy_text(&selection, |index| {
            rows.get(index).map(|(text, copy)| (text.clone(), copy))
        })
    }

    fn select(from: (usize, u16), to: (usize, u16)) -> Selection {
        Selection {
            anchor: Point {
                row: from.0,
                col: from.1,
            },
            head: Point {
                row: to.0,
                col: to.1,
            },
        }
    }

    #[test]
    fn wrapped_rows_join_back_and_marks_stay_out() {
        let joined = RowCopy {
            joiner: Some(" ".into()),
            ..RowCopy::lead(2)
        };
        let transcript = rows(&[
            ("● The quick brown", RowCopy::lead(2)),
            ("  fox jumps", joined.clone()),
            ("", RowCopy::default()),
            ("● Next", RowCopy::lead(2)),
        ]);
        assert_eq!(
            copy_of(&transcript, select((0, 0), (3, 10))),
            "The quick brown fox jumps\n\nNext"
        );
        // Dragged right to left, from inside a word.
        assert_eq!(
            copy_of(&transcript, select((1, 4), (0, 6))),
            "quick brown fox"
        );
    }

    #[test]
    fn mid_word_wraps_join_with_nothing() {
        let transcript = rows(&[
            ("     abcdef", RowCopy::lead(5)),
            (
                "     ghij",
                RowCopy {
                    joiner: Some(String::new()),
                    ..RowCopy::lead(5)
                },
            ),
        ]);
        assert_eq!(copy_of(&transcript, select((0, 0), (1, 80))), "abcdefghij");
    }

    #[test]
    fn diff_columns_borders_and_rules_are_left_out() {
        let diff = RowCopy::lead(10);
        let table_row = RowCopy {
            skip: vec![(0..1, None), (6..7, Some('\t')), (12..13, None)],
            ..RowCopy::default()
        };
        let transcript = rows(&[
            ("     12 + let a = 1;", diff.clone()),
            ("     13 - let b = 2;", diff),
            (
                "┌─────┬─────┐",
                RowCopy {
                    omit: true,
                    ..RowCopy::default()
                },
            ),
            ("│ a   │ bb  │", table_row),
        ]);
        assert_eq!(
            copy_of(&transcript, select((0, 0), (3, 40))),
            "let a = 1;\nlet b = 2;\na\tbb"
        );
    }

    #[test]
    fn wide_characters_select_whole() {
        let transcript = rows(&[("❯ 日本語 text", RowCopy::lead(2))]);
        // Column 3 is the right half of 日.
        assert_eq!(copy_of(&transcript, select((0, 3), (0, 6))), "日本語");
    }

    #[test]
    fn words_keep_paths_and_stop_at_quotes() {
        let copy = RowCopy::lead(2);
        let text = "● open \"src/ui.rs:12\" now";
        let (lo, hi) = word_at(text, &copy, 12).unwrap();
        assert_eq!(row_text(text, &copy, lo..hi + 1), "src/ui.rs:12");
        // The mark is not a word.
        assert_eq!(word_at(text, &copy, 0), None);
    }

    #[test]
    fn a_line_spans_its_wraps() {
        let joined = [false, false, true, true, false];
        assert_eq!(line_rows(3, 5, |row| joined[row]), (1, 3));
        assert_eq!(line_rows(4, 5, |row| joined[row]), (4, 4));
    }

    #[test]
    fn a_selection_keeps_its_text_across_scrolling() {
        // The same selection drawn at two scroll positions lights the
        // same text, wherever it sits on screen.
        let transcript = rows(&[
            ("● one", RowCopy::lead(2)),
            ("● two", RowCopy::lead(2)),
            ("● three", RowCopy::lead(2)),
        ]);
        let selection = select((1, 2), (1, 4));
        let style = Style::default().bg(Color::White).fg(Color::Black);
        for first in [0usize, 1] {
            let area = Rect::new(0, 0, 10, 2);
            let mut buf = Buffer::empty(area);
            let visible = &transcript[first..(first + 2).min(transcript.len())];
            for (y, (text, _)) in visible.iter().enumerate() {
                buf.set_string(0, y as u16, text, Style::default());
            }
            paint(&mut buf, area, first, visible, &selection, style);
            let lit: Vec<(u16, u16)> = (0..2)
                .flat_map(|y| (0..10).map(move |x| (x, y)))
                .filter(|&(x, y)| buf[(x, y)].bg == Color::White)
                .collect();
            let y = (1 - first) as u16;
            assert_eq!(lit, vec![(2, y), (3, y), (4, y)]);
        }
        assert_eq!(copy_of(&transcript, selection), "two");
    }

    #[test]
    fn edge_scroll_speeds_up_past_the_edge() {
        let area = Rect::new(0, 5, 40, 20);
        assert_eq!(edge_scroll(12, area), 0);
        assert_eq!(edge_scroll(5, area), -1);
        assert_eq!(edge_scroll(0, area), -3);
        assert_eq!(edge_scroll(24, area), 1);
        assert_eq!(edge_scroll(30, area), 3);
    }

    #[test]
    fn clicks_count_to_three() {
        let mut clicks = Clicks::default();
        let now = Instant::now();
        let at = Point { row: 4, col: 7 };
        assert_eq!(clicks.press(now, at), 1);
        assert_eq!(clicks.press(now, Point { col: 8, ..at }), 2);
        assert_eq!(clicks.press(now, at), 3);
        assert_eq!(clicks.press(now, at), 1);
        assert_eq!(clicks.press(now + Clicks::WINDOW * 2, at), 1);
        assert_eq!(
            clicks.press(now + Clicks::WINDOW * 2, Point { row: 5, ..at }),
            1
        );
    }
}
