//! The arrival, as a transform over a finished grid.
//!
//! A slide types itself in: the cells it holds appear in the order the grid
//! lays them out, and a block caret rests at the boundary while the rest is
//! still to come. Because it is a transform, the reveal is the same shape
//! on every renderer, and the text export and the snapshots take the
//! finished grid by asking for all of it.

use crate::grid::{Cell, Grid, Style};
use coder_ui::theme::Intensity;

/// How many cells `grid` holds that a reveal draws, which is every cell
/// that is not blank.
pub fn glyphs(grid: &Grid) -> usize {
    (0..grid.height())
        .map(|row| {
            (0..grid.width())
                .filter(|col| grid.get(*col, row).is_some_and(|cell| !cell.is_blank()))
                .count()
        })
        .sum()
}

/// `grid` with its first `shown` cells drawn and a caret at the boundary.
/// A `shown` at or above [`glyphs`] gives the grid back whole.
pub fn reveal(grid: &Grid, shown: usize) -> Grid {
    if shown >= glyphs(grid) {
        return grid.clone();
    }
    let mut revealed = Grid::new(grid.width(), grid.height());
    let mut left = shown;
    let mut caret = true;
    for row in 0..grid.height() {
        for col in 0..grid.width() {
            let Some(cell) = grid.get(col, row) else {
                continue;
            };
            if cell.is_blank() {
                continue;
            }
            if left > 0 {
                revealed.put(col, row, *cell);
                left -= 1;
                continue;
            }
            if caret {
                revealed.put(
                    col,
                    row,
                    Cell::new('█', Style::at(Intensity::Full).caret(true)),
                );
                caret = false;
            }
        }
    }
    revealed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Grid {
        let mut grid = Grid::new(10, 2);
        grid.put_str(0, 0, "abc", Style::at(Intensity::Full));
        grid.put_str(0, 1, "de", Style::at(Intensity::Full));
        grid
    }

    /// The count is the cells that are not blank.
    #[test]
    fn the_count_is_the_cells_that_draw() {
        assert_eq!(glyphs(&sample()), 5);
    }

    /// A partial reveal draws what has arrived and rests a caret on the
    /// next cell.
    #[test]
    fn a_partial_reveal_rests_a_caret_at_the_boundary() {
        let revealed = reveal(&sample(), 2);
        let text = revealed.to_text();
        assert!(text.starts_with("ab█"), "the reveal drew {text}");
        assert!(!text.contains('d'));
    }

    /// A reveal past the end gives the grid back whole, with no caret in
    /// it.
    #[test]
    fn a_full_reveal_is_the_grid_itself() {
        let grid = sample();
        let revealed = reveal(&grid, 99);
        assert_eq!(revealed.snapshot(), grid.snapshot());
    }
}
