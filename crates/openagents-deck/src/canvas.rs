//! The canvas every slide is composed on.
//!
//! JetBrains Mono advances 0.6 em per cell at the 1.3 line height every
//! Coder surface measures in, so 108 cells by 28 rows is 64.8 by 36.4 em:
//! 16:9 to within a fifth of a percent. Authoring against fixed cells means
//! a slide reads the same on a laptop, on a projector, and in the text
//! export, and the window's only job is to pick one type size and center
//! the result.

/// The canvas width, in cells.
pub const CELLS: usize = 108;

/// The canvas height, in rows.
pub const ROWS: usize = 28;

/// The cells the safe area leaves at each side.
pub const PAD_COLS: usize = 4;

/// The rows the safe area leaves at the top and above the foot.
pub const PAD_ROWS: usize = 2;

/// The canvas a slide lays out on: a width in cells and a height in rows.
/// [`Canvas::DEFAULT`] is the one the deck ships; a test or the text export
/// may take another.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Canvas {
    pub cells: usize,
    pub rows: usize,
}

impl Canvas {
    /// The canvas the deck is authored on.
    pub const DEFAULT: Canvas = Canvas {
        cells: CELLS,
        rows: ROWS,
    };

    /// A canvas of `cells` by `rows`, never narrower than one column of
    /// body or shorter than the chrome.
    pub fn new(cells: usize, rows: usize) -> Canvas {
        Canvas {
            cells: cells.max(2 * PAD_COLS + 8),
            rows: rows.max(2 * PAD_ROWS + 4),
        }
    }

    /// The width of the body, in cells.
    pub fn body_cells(&self) -> usize {
        self.cells - 2 * PAD_COLS
    }

    /// The height of the body, in rows: the canvas without its padding and
    /// without the foot.
    pub fn body_rows(&self) -> usize {
        self.rows - 2 * PAD_ROWS - 1
    }

    /// The row the foot draws on.
    pub fn foot_row(&self) -> usize {
        self.rows - 1
    }
}

impl Default for Canvas {
    fn default() -> Canvas {
        Canvas::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canvas is 16:9 at the character metrics the surfaces measure in,
    /// within half a percent.
    #[test]
    fn the_canvas_is_sixteen_by_nine() {
        let ratio = (CELLS as f32 * 0.6) / (ROWS as f32 * 1.3);
        assert!(
            (ratio - 16.0 / 9.0).abs() < 0.01,
            "the canvas is {ratio}:1, not 16:9"
        );
    }

    /// A canvas smaller than the chrome grows to hold it.
    #[test]
    fn a_small_canvas_holds_the_chrome() {
        let canvas = Canvas::new(4, 2);
        assert!(canvas.body_cells() > 0);
        assert!(canvas.body_rows() > 0);
    }
}
