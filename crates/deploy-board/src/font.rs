//! The board's block letters: five columns by seven rows, the same cells as
//! Verse's other signs (`crates/verse-core/src/label.rs`), with the digits
//! and punctuation a commit subject needs.

/// Every character the board draws. Anything else is drawn as `?`; lower
/// case is drawn as upper case.
pub const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-:./#'()!?,+&_=<>\"%@*";

/// Columns in a cell, counting the gap after a letter.
pub const ADVANCE: usize = 6;
/// Rows in a letter.
pub const ROWS: usize = 7;

/// The character the board draws for `c`: upper case, or `?` when it has no
/// letter for it. A space stays a space.
#[must_use]
pub fn drawn(c: u8) -> u8 {
    let c = c.to_ascii_uppercase();
    match c {
        b' ' => b' ',
        b'`' => b'\'',
        b'[' | b'{' => b'(',
        b']' | b'}' => b')',
        b';' => b':',
        b'|' | b'\\' => b'/',
        b'~' | b'^' | b'$' => b'-',
        c if CHARSET.contains(&c) => c,
        _ => b'?',
    }
}

/// The rows of `c`, top first; bit 16 is the leftmost column.
#[must_use]
pub fn glyph(c: u8) -> [u8; ROWS] {
    match drawn(c) {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 2, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 30, 1, 1, 17, 14],
        b'6' => [6, 8, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 2, 12],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b':' => [0, 12, 12, 0, 12, 12, 0],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'#' => [10, 10, 31, 10, 31, 10, 10],
        b'\'' => [4, 4, 8, 0, 0, 0, 0],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b',' => [0, 0, 0, 0, 12, 4, 8],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'&' => [12, 18, 20, 8, 21, 18, 13],
        b'_' => [0, 0, 0, 0, 0, 0, 31],
        b'=' => [0, 0, 31, 0, 31, 0, 0],
        b'<' => [2, 4, 8, 16, 8, 4, 2],
        b'>' => [8, 4, 2, 1, 2, 4, 8],
        b'"' => [10, 10, 0, 0, 0, 0, 0],
        b'%' => [24, 25, 2, 4, 8, 19, 3],
        b'@' => [14, 17, 1, 13, 21, 21, 14],
        b'*' => [0, 4, 21, 14, 21, 4, 0],
        _ => [0; ROWS],
    }
}

/// The lit runs of one row: (first column, length), left first.
#[must_use]
pub fn runs(bits: u8) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut col = 0;
    while col < 5 {
        if bits & (16 >> col) == 0 {
            col += 1;
            continue;
        }
        let start = col;
        while col < 5 && bits & (16 >> col) != 0 {
            col += 1;
        }
        out.push((start, col - start));
    }
    out
}

/// The lit cells of `c` as rectangles in a letter one unit tall, its lower
/// left corner at the origin: `[left, bottom, right, top]`.
#[must_use]
pub fn rects(c: u8) -> Vec<[f64; 4]> {
    let cell = 1.0 / ROWS as f64;
    let mut out = Vec::new();
    for (row, bits) in glyph(c).into_iter().enumerate() {
        for (col, len) in runs(bits) {
            let left = col as f64 * cell;
            let bottom = (ROWS - 1 - row) as f64 * cell;
            out.push([left, bottom, left + len as f64 * cell, bottom + cell]);
        }
    }
    out
}

/// How wide `text` is at letter height `size`, without the last gap.
#[must_use]
pub fn width(text: &str, size: f64) -> f64 {
    let n = text.len();
    if n == 0 {
        return 0.0;
    }
    (n * ADVANCE - 1) as f64 * size / ROWS as f64
}

/// The step from one letter to the next at letter height `size`.
#[must_use]
pub fn advance(size: f64) -> f64 {
    ADVANCE as f64 * size / ROWS as f64
}

/// How many letters fit in `span` meters at letter height `size`.
#[must_use]
pub fn fits(span: f64, size: f64) -> usize {
    ((span + size / ROWS as f64) / advance(size)).floor() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_in_the_set_has_a_letter() {
        for &c in CHARSET {
            assert!(glyph(c).iter().any(|row| *row != 0), "{}", c as char);
            assert!(glyph(c).iter().all(|row| *row < 32), "{}", c as char);
        }
    }

    #[test]
    fn lower_case_and_unknown_characters_are_drawn() {
        assert_eq!(drawn(b'a'), b'A');
        assert_eq!(drawn(b' '), b' ');
        assert_eq!(drawn(0xe9), b'?');
        assert_eq!(drawn(b'~'), b'-');
        assert!(rects(b' ').is_empty());
    }

    #[test]
    fn runs_cover_exactly_the_lit_columns() {
        assert_eq!(runs(0b10110), vec![(0, 1), (2, 2)]);
        assert_eq!(runs(31), vec![(0, 5)]);
        assert!(runs(0).is_empty());
    }

    #[test]
    fn widths_and_fits_agree() {
        let size = 0.7;
        for n in 1..40 {
            let text = "X".repeat(n);
            assert_eq!(fits(width(&text, size) + 1e-9, size), n);
        }
    }
}
