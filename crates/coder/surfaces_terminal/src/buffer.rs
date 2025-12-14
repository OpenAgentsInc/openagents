//! Terminal scrollback buffer with styled cells.

use crate::ansi::AnsiStyle;

/// A single cell in the terminal buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// Character at this cell.
    pub char: char,
    /// Style attributes.
    pub style: AnsiStyle,
}

impl Cell {
    /// Create a new cell with the given character and style.
    pub fn new(char: char, style: AnsiStyle) -> Self {
        Self { char, style }
    }

    /// Create an empty cell.
    pub fn empty() -> Self {
        Self {
            char: ' ',
            style: AnsiStyle::default(),
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::empty()
    }
}

/// A line in the terminal buffer.
#[derive(Clone, Debug)]
pub struct Line {
    /// Cells in this line.
    cells: Vec<Cell>,
    /// Whether this line wraps to the next.
    pub wrapped: bool,
}

impl Line {
    /// Create a new empty line.
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            wrapped: false,
        }
    }

    /// Create a line with specified width.
    pub fn with_width(width: usize) -> Self {
        Self {
            cells: vec![Cell::empty(); width],
            wrapped: false,
        }
    }

    /// Get a cell at the given column.
    pub fn get(&self, col: usize) -> Option<&Cell> {
        self.cells.get(col)
    }

    /// Set a cell at the given column.
    pub fn set(&mut self, col: usize, cell: Cell) {
        if col >= self.cells.len() {
            self.cells.resize(col + 1, Cell::empty());
        }
        self.cells[col] = cell;
    }

    /// Get the length of this line (number of cells).
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Check if line is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() || self.cells.iter().all(|c| c.char == ' ')
    }

    /// Get all cells.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Truncate line to given width.
    pub fn truncate(&mut self, width: usize) {
        self.cells.truncate(width);
    }

    /// Clear the line.
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Convert line to string.
    pub fn to_string(&self) -> String {
        self.cells.iter().map(|c| c.char).collect()
    }
}

impl Default for Line {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminal buffer with scrollback.
pub struct TerminalBuffer {
    /// All lines (scrollback + visible).
    lines: Vec<Line>,
    /// Number of columns.
    cols: usize,
    /// Number of visible rows.
    rows: usize,
    /// Maximum scrollback lines.
    max_scrollback: usize,
    /// Current cursor row (relative to viewport).
    cursor_row: usize,
    /// Current cursor column.
    cursor_col: usize,
    /// Current style for new text.
    current_style: AnsiStyle,
}

impl TerminalBuffer {
    /// Create a new terminal buffer.
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut lines = Vec::with_capacity(rows);
        for _ in 0..rows {
            lines.push(Line::with_width(cols));
        }

        Self {
            lines,
            cols,
            rows,
            max_scrollback: 10000,
            cursor_row: 0,
            cursor_col: 0,
            current_style: AnsiStyle::default(),
        }
    }

    /// Get the number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Get the number of visible rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Get the total number of lines (including scrollback).
    pub fn total_lines(&self) -> usize {
        self.lines.len()
    }

    /// Get the scrollback size.
    pub fn scrollback_size(&self) -> usize {
        self.lines.len().saturating_sub(self.rows)
    }

    /// Get cursor position.
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// Set cursor position.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.cursor_row = row.min(self.rows.saturating_sub(1));
        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    /// Get current style.
    pub fn current_style(&self) -> AnsiStyle {
        self.current_style
    }

    /// Set current style.
    pub fn set_style(&mut self, style: AnsiStyle) {
        self.current_style = style;
    }

    /// Get a line at the given index (0 is oldest scrollback).
    pub fn line(&self, index: usize) -> Option<&Line> {
        self.lines.get(index)
    }

    /// Get a visible line (0 is first visible row).
    pub fn visible_line(&self, row: usize) -> Option<&Line> {
        if row >= self.rows {
            return None;
        }
        let scrollback = self.scrollback_size();
        self.lines.get(scrollback + row)
    }

    /// Get a mutable visible line.
    fn visible_line_mut(&mut self, row: usize) -> Option<&mut Line> {
        if row >= self.rows {
            return None;
        }
        let scrollback = self.scrollback_size();
        self.lines.get_mut(scrollback + row)
    }

    /// Write a character at the cursor position.
    pub fn write_char(&mut self, c: char) {
        match c {
            '\n' => {
                self.newline();
            }
            '\r' => {
                self.cursor_col = 0;
            }
            '\t' => {
                // Move to next tab stop (every 8 columns)
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            }
            _ => {
                // Regular character
                if self.cursor_col >= self.cols {
                    // Line wrap
                    let cursor_row = self.cursor_row;
                    if let Some(line) = self.visible_line_mut(cursor_row) {
                        line.wrapped = true;
                    }
                    self.newline();
                }

                let cursor_row = self.cursor_row;
                let cursor_col = self.cursor_col;
                let style = self.current_style;
                if let Some(line) = self.visible_line_mut(cursor_row) {
                    line.set(cursor_col, Cell::new(c, style));
                }
                self.cursor_col += 1;
            }
        }
    }

    /// Write a string at the cursor position.
    pub fn write_str(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }

    /// Write styled segments from ANSI parser.
    pub fn write_segments(&mut self, segments: &[(String, AnsiStyle)]) {
        for (text, style) in segments {
            self.current_style = *style;
            self.write_str(text);
        }
    }

    /// Move to next line, scrolling if necessary.
    fn newline(&mut self) {
        self.cursor_col = 0;
        if self.cursor_row + 1 >= self.rows {
            // Scroll up
            self.scroll_up();
        } else {
            self.cursor_row += 1;
        }
    }

    /// Scroll up by one line.
    fn scroll_up(&mut self) {
        // Add a new line at the bottom
        self.lines.push(Line::with_width(self.cols));

        // Trim scrollback if needed
        while self.lines.len() > self.max_scrollback + self.rows {
            self.lines.remove(0);
        }
    }

    /// Clear the visible area.
    pub fn clear(&mut self) {
        let scrollback = self.scrollback_size();
        for i in 0..self.rows {
            if let Some(line) = self.lines.get_mut(scrollback + i) {
                line.clear();
                *line = Line::with_width(self.cols);
            }
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    /// Clear from cursor to end of line.
    pub fn clear_to_end_of_line(&mut self) {
        let cursor_row = self.cursor_row;
        let cursor_col = self.cursor_col;
        let cols = self.cols;
        if let Some(line) = self.visible_line_mut(cursor_row) {
            for col in cursor_col..cols {
                line.set(col, Cell::empty());
            }
        }
    }

    /// Clear from cursor to end of screen.
    pub fn clear_to_end_of_screen(&mut self) {
        self.clear_to_end_of_line();
        let cols = self.cols;
        for row in (self.cursor_row + 1)..self.rows {
            if let Some(line) = self.visible_line_mut(row) {
                *line = Line::with_width(cols);
            }
        }
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;

        // Ensure we have enough lines
        while self.lines.len() < rows {
            self.lines.push(Line::with_width(cols));
        }

        // Clamp cursor
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
    }

    /// Get visible lines as an iterator.
    pub fn visible_lines(&self) -> impl Iterator<Item = &Line> {
        let scrollback = self.scrollback_size();
        self.lines.iter().skip(scrollback).take(self.rows)
    }

    /// Get all lines as an iterator.
    pub fn all_lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter()
    }
}

impl Default for TerminalBuffer {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_creation() {
        let cell = Cell::new('A', AnsiStyle::default());
        assert_eq!(cell.char, 'A');

        let empty = Cell::empty();
        assert_eq!(empty.char, ' ');
    }

    #[test]
    fn test_line_operations() {
        let mut line = Line::with_width(10);
        assert_eq!(line.len(), 10);

        line.set(5, Cell::new('X', AnsiStyle::default()));
        assert_eq!(line.get(5).unwrap().char, 'X');
    }

    #[test]
    fn test_buffer_creation() {
        let buffer = TerminalBuffer::new(80, 24);
        assert_eq!(buffer.cols(), 80);
        assert_eq!(buffer.rows(), 24);
        assert_eq!(buffer.cursor(), (0, 0));
    }

    #[test]
    fn test_buffer_write() {
        let mut buffer = TerminalBuffer::new(80, 24);
        buffer.write_str("Hello");

        assert_eq!(buffer.cursor(), (0, 5));
        let line = buffer.visible_line(0).unwrap();
        assert_eq!(line.get(0).unwrap().char, 'H');
        assert_eq!(line.get(4).unwrap().char, 'o');
    }

    #[test]
    fn test_buffer_newline() {
        let mut buffer = TerminalBuffer::new(80, 24);
        buffer.write_str("Hello\nWorld");

        assert_eq!(buffer.cursor(), (1, 5));
        let line0 = buffer.visible_line(0).unwrap();
        let line1 = buffer.visible_line(1).unwrap();
        assert_eq!(line0.get(0).unwrap().char, 'H');
        assert_eq!(line1.get(0).unwrap().char, 'W');
    }

    #[test]
    fn test_buffer_scroll() {
        let mut buffer = TerminalBuffer::new(80, 3);

        // Write 5 lines (should scroll 2 into scrollback)
        buffer.write_str("Line 1\n");
        buffer.write_str("Line 2\n");
        buffer.write_str("Line 3\n");
        buffer.write_str("Line 4\n");
        buffer.write_str("Line 5");

        assert_eq!(buffer.scrollback_size(), 2);
        assert_eq!(buffer.total_lines(), 5);
    }

    #[test]
    fn test_buffer_clear() {
        let mut buffer = TerminalBuffer::new(80, 24);
        buffer.write_str("Hello");
        buffer.clear();

        assert_eq!(buffer.cursor(), (0, 0));
        let line = buffer.visible_line(0).unwrap();
        assert_eq!(line.get(0).unwrap().char, ' ');
    }

    #[test]
    fn test_buffer_resize() {
        let mut buffer = TerminalBuffer::new(80, 24);
        buffer.resize(40, 12);

        assert_eq!(buffer.cols(), 40);
        assert_eq!(buffer.rows(), 12);
    }
}
