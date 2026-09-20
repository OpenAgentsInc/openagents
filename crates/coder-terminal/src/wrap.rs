//! Soft-wrapping of composer text into display rows.
//!
//! A logical line breaks at the last space that fits; a word longer than the
//! row breaks mid-word. The spaces a break consumes belong to neither row, so
//! wrapped continuations never lead with a space. Hard newlines always start
//! a fresh row, and every logical line contributes at least one row — an
//! empty line is a row of its own. The row ranges cover the text in order,
//! so a caret anywhere in the text, its very end included, lands in exactly
//! one row.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Soft-wraps `text` into byte ranges, one range per display row, no row
/// wider than `width` cells.
pub fn wrap_rows(text: &str, width: usize) -> Vec<Range<usize>> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut line_start = 0;

    loop {
        let line_end = text[line_start..]
            .find('\n')
            .map_or(text.len(), |offset| line_start + offset);
        wrap_one_line(text, line_start..line_end, width, &mut rows);
        if line_end == text.len() {
            break;
        }
        line_start = line_end + 1;
    }

    if rows.is_empty() {
        rows.push(0..0);
    }
    rows
}

fn wrap_one_line(text: &str, line: Range<usize>, width: usize, rows: &mut Vec<Range<usize>>) {
    let mut cursor = line.start;
    loop {
        let mut consumed = 0;
        let mut end = cursor;
        // The last point where a break would land between words.
        let mut break_at = None;
        let mut overflowed = false;

        for (offset, grapheme) in text[cursor..line.end].grapheme_indices(true) {
            let at = cursor + offset;
            let width_of = grapheme.width().max(1);
            if consumed + width_of > width {
                overflowed = true;
                break;
            }
            if grapheme == " " && at > cursor {
                break_at = Some(at);
            }
            consumed += width_of;
            end = at + grapheme.len();
        }

        if !overflowed {
            rows.push(cursor..line.end);
            return;
        }

        // Break at the space if there was one, else mid-word at the last
        // grapheme that fit. A row that fits nothing at all still advances by
        // one grapheme, so this cannot spin.
        let (row_end, mut next) = match break_at {
            Some(space) => (space, space + 1),
            None if end > cursor => (end, end),
            None => {
                let one = text[cursor..line.end]
                    .grapheme_indices(true)
                    .next()
                    .map_or(line.end, |(_, grapheme)| cursor + grapheme.len());
                (one, one)
            }
        };
        rows.push(cursor..row_end);
        // A run of spaces at a break belongs to the break, not the next row.
        while next < line.end && text[next..].starts_with(' ') {
            next += 1;
        }
        cursor = next;
        if cursor >= line.end {
            return;
        }
    }
}

/// The index of the row holding byte offset `caret`.
///
/// A caret at a wrap point — exactly the end of one row and the start of the
/// next — belongs to the later row, so the caret cell follows the text the
/// user is typing.
pub fn row_of(rows: &[Range<usize>], caret: usize) -> usize {
    for (index, row) in rows.iter().enumerate() {
        if caret < row.start {
            return index.saturating_sub(1);
        }
        if caret < row.end {
            return index;
        }
        if caret == row.end {
            let split_here = rows.get(index + 1).is_some_and(|next| next.start == caret);
            if !split_here {
                return index;
            }
        }
    }
    rows.len().saturating_sub(1)
}

/// The byte offset within `row` closest to display `column`.
pub fn byte_at_column(text: &str, row: Range<usize>, column: usize) -> usize {
    let mut byte = row.start;
    let mut seen = 0;
    for (offset, grapheme) in text[row.clone()].grapheme_indices(true) {
        let width = grapheme.width().max(1);
        if seen + width > column {
            return row.start + offset;
        }
        seen += width;
        byte = row.start + offset + grapheme.len();
    }
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows_of(text: &str, width: usize) -> Vec<String> {
        wrap_rows(text, width)
            .iter()
            .map(|range| text[range.clone()].to_owned())
            .collect()
    }

    #[test]
    fn wrap_breaks_at_a_space() {
        assert_eq!(
            rows_of("the quick brown fox", 10),
            ["the quick", "brown fox"]
        );
    }

    #[test]
    fn wrap_splits_a_word_that_cannot_fit() {
        assert_eq!(rows_of("abcdefghij", 4), ["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrap_gives_an_empty_line_its_own_row() {
        assert_eq!(rows_of("a\n\nb", 10), ["a", "", "b"]);
    }

    #[test]
    fn a_space_run_at_a_break_is_swallowed() {
        assert_eq!(rows_of("one   two", 4), ["one", "two"]);
    }

    #[test]
    fn a_caret_at_a_wrap_point_joins_the_later_row() {
        let text = "the quick brown";
        let rows = wrap_rows(text, 10);
        assert_eq!(&text[rows[0].clone()], "the quick");
        // The caret after "the quick" sits at the split: it belongs to row 1.
        assert_eq!(row_of(&rows, rows[1].start), 1);
    }

    #[test]
    fn a_caret_at_text_end_joins_the_last_row() {
        let text = "hi";
        let rows = wrap_rows(text, 10);
        assert_eq!(row_of(&rows, text.len()), 0);
    }

    #[test]
    fn wide_characters_wrap_by_cells_not_code_points() {
        // Three wide characters are six cells: two fit in five, the third
        // wraps rather than overflowing the row.
        assert_eq!(rows_of("日本語", 5), ["日本", "語"]);
    }

    #[test]
    fn a_grapheme_cluster_never_splits_at_a_wrap() {
        let flag = "\u{1F1EF}\u{1F1F5}"; // regional indicators J + P
        let text = format!("ab{flag}cd");
        let rows = rows_of(&text, 3);
        assert!(
            rows.iter()
                .all(|row| !row.contains('\u{1F1EF}') || row.contains('\u{1F1F5}'))
        );
        assert_eq!(rows.concat(), text);
    }

    #[test]
    fn byte_at_column_walks_graphemes() {
        let text = "héllo";
        let rows = wrap_rows(text, 20);
        assert_eq!(byte_at_column(text, rows[0].clone(), 3), 4);
    }
}
