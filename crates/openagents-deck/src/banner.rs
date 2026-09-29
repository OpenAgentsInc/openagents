//! The block face the opening and the closing slide draw in.
//!
//! The grid has one type size, so a headline larger than the body is drawn
//! out of cells rather than set in a larger face. Each glyph is 5 cells
//! wide and 7 rows tall with one column of space after it, filled with
//! `█` at full intensity. The face covers the upper-case letters, the
//! digits, and seven marks (`. - / % $ + ,`); a character outside that set draws as a space,
//! so a title that needs more is body type instead.

use crate::grid::{Cell, Grid, Style};
use coder_ui::theme::Intensity;

/// The cells one glyph takes.
pub const GLYPH_CELLS: usize = 5;

/// The rows one glyph takes.
pub const GLYPH_ROWS: usize = 7;

/// The cells between two glyphs.
pub const GAP: usize = 1;

/// The face: one entry per character, seven rows of five columns, `#`
/// where the cell is filled.
const FACE: &[(char, [&str; GLYPH_ROWS])] = &[
    (
        ' ',
        [
            "     ", "     ", "     ", "     ", "     ", "     ", "     ",
        ],
    ),
    (
        'A',
        [
            ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
    ),
    (
        'B',
        [
            "####.", "#...#", "#...#", "####.", "#...#", "#...#", "####.",
        ],
    ),
    (
        'C',
        [
            ".###.", "#...#", "#....", "#....", "#....", "#...#", ".###.",
        ],
    ),
    (
        'D',
        [
            "####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####.",
        ],
    ),
    (
        'E',
        [
            "#####", "#....", "#....", "####.", "#....", "#....", "#####",
        ],
    ),
    (
        'F',
        [
            "#####", "#....", "#....", "####.", "#....", "#....", "#....",
        ],
    ),
    (
        'G',
        [
            ".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".###.",
        ],
    ),
    (
        'H',
        [
            "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
    ),
    (
        'I',
        [
            "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####",
        ],
    ),
    (
        'J',
        [
            "..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##..",
        ],
    ),
    (
        'K',
        [
            "#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#",
        ],
    ),
    (
        'L',
        [
            "#....", "#....", "#....", "#....", "#....", "#....", "#####",
        ],
    ),
    (
        'M',
        [
            "#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#",
        ],
    ),
    (
        'N',
        [
            "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#",
        ],
    ),
    (
        'O',
        [
            ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ],
    ),
    (
        'P',
        [
            "####.", "#...#", "#...#", "####.", "#....", "#....", "#....",
        ],
    ),
    (
        'Q',
        [
            ".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#",
        ],
    ),
    (
        'R',
        [
            "####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#",
        ],
    ),
    (
        'S',
        [
            ".####", "#....", "#....", ".###.", "....#", "....#", "####.",
        ],
    ),
    (
        'T',
        [
            "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..",
        ],
    ),
    (
        'U',
        [
            "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ],
    ),
    (
        'V',
        [
            "#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#..",
        ],
    ),
    (
        'W',
        [
            "#...#", "#...#", "#...#", "#...#", "#.#.#", "##.##", "#...#",
        ],
    ),
    (
        'X',
        [
            "#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#",
        ],
    ),
    (
        'Y',
        [
            "#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#..",
        ],
    ),
    (
        'Z',
        [
            "#####", "....#", "...#.", "..#..", ".#...", "#....", "#####",
        ],
    ),
    (
        '0',
        [
            ".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###.",
        ],
    ),
    (
        '1',
        [
            "..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###.",
        ],
    ),
    (
        '2',
        [
            ".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####",
        ],
    ),
    (
        '3',
        [
            "#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###.",
        ],
    ),
    (
        '4',
        [
            "...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#.",
        ],
    ),
    (
        '5',
        [
            "#####", "#....", "####.", "....#", "....#", "#...#", ".###.",
        ],
    ),
    (
        '6',
        [
            "..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###.",
        ],
    ),
    (
        '7',
        [
            "#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#...",
        ],
    ),
    (
        '8',
        [
            ".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###.",
        ],
    ),
    (
        '9',
        [
            ".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##..",
        ],
    ),
    (
        '.',
        [
            ".....", ".....", ".....", ".....", ".....", ".##..", ".##..",
        ],
    ),
    (
        '-',
        [
            ".....", ".....", ".....", "#####", ".....", ".....", ".....",
        ],
    ),
    (
        '/',
        [
            "....#", "...#.", "...#.", "..#..", ".#...", ".#...", "#....",
        ],
    ),
    (
        '%',
        [
            "##..#", "##..#", "...#.", "..#..", ".#...", "#..##", "#..##",
        ],
    ),
    (
        '$',
        [
            "..#..", ".####", "#.#..", ".###.", "..#.#", "####.", "..#..",
        ],
    ),
    (
        '+',
        [
            ".....", "..#..", "..#..", "#####", "..#..", "..#..", ".....",
        ],
    ),
    (
        ',',
        [
            ".....", ".....", ".....", ".....", ".##..", "..#..", ".#...",
        ],
    ),
];

/// The rows for `glyph`, when the face carries it.
fn rows(glyph: char) -> Option<&'static [&'static str; GLYPH_ROWS]> {
    let upper = glyph.to_ascii_uppercase();
    FACE.iter()
        .find(|(known, _)| *known == upper)
        .map(|(_, rows)| rows)
}

/// Whether the face draws every character of `text`.
pub fn covers(text: &str) -> bool {
    text.chars().all(|glyph| rows(glyph).is_some())
}

/// The cells `text` takes in the face.
pub fn width(text: &str) -> usize {
    let count = text.chars().count();
    match count {
        0 => 0,
        count => count * GLYPH_CELLS + (count - 1) * GAP,
    }
}

/// `text` in the block face, as a grid of its own size. A character the
/// face does not carry draws as a space.
pub fn banner(text: &str) -> Grid {
    let mut grid = Grid::new(width(text), GLYPH_ROWS);
    let style = Style::at(Intensity::Full);
    for (index, glyph) in text.chars().enumerate() {
        let Some(rows) = rows(glyph) else {
            continue;
        };
        let left = index * (GLYPH_CELLS + GAP);
        for (row, line) in rows.iter().enumerate() {
            for (col, mark) in line.chars().enumerate() {
                if mark == '#' {
                    grid.put(left + col, row, Cell::new('█', style));
                }
            }
        }
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The face carries the product's name, and the banner is as wide as
    /// the name takes.
    #[test]
    fn the_face_draws_the_name() {
        assert!(covers("TEST-TIME"));
        let grid = banner("TEST-TIME");
        assert_eq!(grid.width(), width("TEST-TIME"));
        assert_eq!(grid.height(), GLYPH_ROWS);
        assert!(grid.to_text().contains('█'));
    }

    /// Every entry is seven rows of five columns, so no glyph shifts the
    /// ones after it.
    #[test]
    fn every_glyph_is_five_by_seven() {
        for (glyph, rows) in FACE {
            for line in rows {
                assert_eq!(
                    line.chars().count(),
                    GLYPH_CELLS,
                    "{glyph} has a row of the wrong width"
                );
            }
        }
    }

    /// A character outside the face leaves its cells blank rather than
    /// drawing something else.
    #[test]
    fn a_character_outside_the_face_draws_blank() {
        assert!(!covers("é"));
        assert_eq!(banner("é").to_text().trim(), "");
    }
}
