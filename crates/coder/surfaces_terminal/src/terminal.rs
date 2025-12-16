//! Terminal widget for rendering terminal output.

use crate::ansi::{AnsiParser, DEFAULT_FG};
use crate::buffer::TerminalBuffer;
use coder_ui_runtime::Signal;
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget};
use wgpui::{Bounds, InputEvent, NamedKey, Point, Quad};

/// Cursor style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorStyle {
    /// Block cursor.
    Block,
    /// Underline cursor.
    Underline,
    /// Bar/beam cursor.
    Bar,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

/// Terminal widget.
pub struct Terminal {
    /// Terminal buffer.
    buffer: TerminalBuffer,
    /// ANSI parser.
    parser: AnsiParser,
    /// Scroll offset (lines from bottom).
    scroll_offset: Signal<usize>,
    /// Whether cursor is visible.
    cursor_visible: bool,
    /// Cursor style.
    cursor_style: CursorStyle,
    /// Character width in pixels.
    char_width: f32,
    /// Line height in pixels.
    line_height: f32,
    /// Font size.
    font_size: f32,
    /// Whether terminal has focus.
    focused: bool,
    /// Selection start (row, col).
    selection_start: Option<(usize, usize)>,
    /// Selection end (row, col).
    selection_end: Option<(usize, usize)>,
}

impl Terminal {
    /// Create a new terminal with given dimensions.
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            buffer: TerminalBuffer::new(cols, rows),
            parser: AnsiParser::new(),
            scroll_offset: Signal::new(0),
            cursor_visible: true,
            cursor_style: CursorStyle::default(),
            char_width: 8.4,
            line_height: 18.0,
            font_size: 13.0,
            focused: false,
            selection_start: None,
            selection_end: None,
        }
    }

    /// Create a terminal with default 80x24 dimensions.
    pub fn default_size() -> Self {
        Self::new(80, 24)
    }

    /// Get the buffer.
    pub fn buffer(&self) -> &TerminalBuffer {
        &self.buffer
    }

    /// Get mutable buffer.
    pub fn buffer_mut(&mut self) -> &mut TerminalBuffer {
        &mut self.buffer
    }

    /// Write raw text (no ANSI parsing).
    pub fn write_raw(&mut self, text: &str) {
        self.buffer.write_str(text);
    }

    /// Write text with ANSI escape sequence parsing.
    pub fn write(&mut self, text: &str) {
        let segments = self.parser.parse(text);
        self.buffer.write_segments(&segments);
    }

    /// Clear the terminal.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.scroll_offset.set(0);
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.buffer.resize(cols, rows);
    }

    /// Set cursor visibility.
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    /// Set cursor style.
    pub fn set_cursor_style(&mut self, style: CursorStyle) {
        self.cursor_style = style;
    }

    /// Set font size.
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
        // Approximate character dimensions
        self.char_width = size * 0.6;
        self.line_height = size * 1.4;
    }

    /// Scroll up by given lines.
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.buffer.scrollback_size();
        self.scroll_offset
            .update(|offset| *offset = (*offset + lines).min(max_scroll));
    }

    /// Scroll down by given lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset
            .update(|offset| *offset = offset.saturating_sub(lines));
    }

    /// Scroll to bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset.set(0);
    }

    /// Get the current scroll offset.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset.get_untracked()
    }

    /// Calculate required size for given dimensions.
    pub fn size_for_dimensions(&self, cols: usize, rows: usize) -> (f32, f32) {
        (
            cols as f32 * self.char_width,
            rows as f32 * self.line_height,
        )
    }

    /// Calculate dimensions that fit in given bounds.
    pub fn dimensions_for_bounds(&self, bounds: Bounds) -> (usize, usize) {
        let cols = (bounds.size.width / self.char_width).floor() as usize;
        let rows = (bounds.size.height / self.line_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    /// Get text selection.
    pub fn selection(&self) -> Option<String> {
        let (start, end) = match (self.selection_start, self.selection_end) {
            (Some(s), Some(e)) => {
                if s.0 < e.0 || (s.0 == e.0 && s.1 <= e.1) {
                    (s, e)
                } else {
                    (e, s)
                }
            }
            _ => return None,
        };

        let mut result = String::new();
        let scrollback = self.buffer.scrollback_size();

        for row in start.0..=end.0 {
            if let Some(line) = self.buffer.line(scrollback + row) {
                let start_col = if row == start.0 { start.1 } else { 0 };
                let end_col = if row == end.0 {
                    end.1
                } else {
                    line.len().saturating_sub(1)
                };

                for col in start_col..=end_col {
                    if let Some(cell) = line.get(col) {
                        result.push(cell.char);
                    }
                }

                if row < end.0 && !line.wrapped {
                    result.push('\n');
                }
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }

    /// Convert screen position to cell position.
    fn screen_to_cell(&self, bounds: Bounds, point: Point) -> (usize, usize) {
        let x = point.x - bounds.origin.x;
        let y = point.y - bounds.origin.y;

        let col = (x / self.char_width).floor() as usize;
        let row = (y / self.line_height).floor() as usize;

        (
            row.min(self.buffer.rows() - 1),
            col.min(self.buffer.cols() - 1),
        )
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::default_size()
    }
}

impl Widget for Terminal {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw background
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(wgpui::theme::bg::SURFACE));

        let scroll_offset = self.scroll_offset.get_untracked();
        let scrollback = self.buffer.scrollback_size();

        // Calculate which lines to show
        let first_line = scrollback.saturating_sub(scroll_offset);

        // Draw each visible line
        for row in 0..self.buffer.rows() {
            let line_idx = first_line + row;
            let Some(line) = self.buffer.line(line_idx) else {
                continue;
            };

            let y = bounds.origin.y + (row as f32) * self.line_height;

            // Track current style for run batching
            let mut run_start = 0;
            let mut run_text = String::new();
            let mut run_style = None;

            for (col, cell) in line.cells().iter().enumerate() {
                // Check if this cell is selected
                let is_selected = self.selection_start.is_some()
                    && self.selection_end.is_some()
                    && self.is_cell_selected(row, col);

                let cell_style = if is_selected {
                    // Invert colors for selection
                    let mut s = cell.style;
                    s.inverse = !s.inverse;
                    Some(s)
                } else {
                    Some(cell.style)
                };

                // Batch characters with same style
                if run_style == cell_style {
                    run_text.push(cell.char);
                } else {
                    // Flush previous run
                    if !run_text.is_empty() {
                        if let Some(style) = run_style {
                            self.draw_text_run(
                                cx,
                                bounds.origin.x + (run_start as f32) * self.char_width,
                                y,
                                &run_text,
                                style,
                            );
                        }
                    }
                    run_start = col;
                    run_text.clear();
                    run_text.push(cell.char);
                    run_style = cell_style;
                }
            }

            // Flush final run
            if !run_text.is_empty() {
                if let Some(style) = run_style {
                    self.draw_text_run(
                        cx,
                        bounds.origin.x + (run_start as f32) * self.char_width,
                        y,
                        &run_text,
                        style,
                    );
                }
            }
        }

        // Draw cursor if visible and focused
        if self.cursor_visible && self.focused && scroll_offset == 0 {
            let (cursor_row, cursor_col) = self.buffer.cursor();
            let cursor_x = bounds.origin.x + (cursor_col as f32) * self.char_width;
            let cursor_y = bounds.origin.y + (cursor_row as f32) * self.line_height;

            let cursor_bounds = match self.cursor_style {
                CursorStyle::Block => {
                    Bounds::new(cursor_x, cursor_y, self.char_width, self.line_height)
                }
                CursorStyle::Underline => Bounds::new(
                    cursor_x,
                    cursor_y + self.line_height - 2.0,
                    self.char_width,
                    2.0,
                ),
                CursorStyle::Bar => Bounds::new(cursor_x, cursor_y, 2.0, self.line_height),
            };

            cx.scene
                .draw_quad(Quad::new(cursor_bounds).with_background(DEFAULT_FG));
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseDown { position, .. } => {
                if bounds.contains(*position) {
                    self.focused = true;
                    let cell = self.screen_to_cell(bounds, *position);
                    self.selection_start = Some(cell);
                    self.selection_end = Some(cell);
                    return EventResult::Handled;
                } else {
                    self.focused = false;
                }
            }
            InputEvent::MouseMove { position, .. } => {
                if self.selection_start.is_some() && bounds.contains(*position) {
                    let cell = self.screen_to_cell(bounds, *position);
                    self.selection_end = Some(cell);
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { .. } => {
                // Selection complete
            }
            InputEvent::Wheel { delta, .. } => {
                if bounds.contains(Point::new(0.0, 0.0)) {
                    // Simplified - would need mouse position
                    if delta.y > 0.0 {
                        self.scroll_up(3);
                    } else if delta.y < 0.0 {
                        self.scroll_down(3);
                    }
                    return EventResult::Handled;
                }
            }
            InputEvent::KeyDown { key, modifiers, .. } => {
                if !self.focused {
                    return EventResult::Ignored;
                }

                // Handle keyboard shortcuts
                if modifiers.command() || modifiers.ctrl {
                    match key {
                        wgpui::Key::Character(c) if c == "c" => {
                            // Copy selection
                            // Would integrate with clipboard
                            return EventResult::Handled;
                        }
                        _ => {}
                    }
                }

                // Page up/down
                match key {
                    wgpui::Key::Named(NamedKey::PageUp) => {
                        self.scroll_up(self.buffer.rows());
                        return EventResult::Handled;
                    }
                    wgpui::Key::Named(NamedKey::PageDown) => {
                        self.scroll_down(self.buffer.rows());
                        return EventResult::Handled;
                    }
                    wgpui::Key::Named(NamedKey::Home) if modifiers.command() || modifiers.ctrl => {
                        self.scroll_offset.set(self.buffer.scrollback_size());
                        return EventResult::Handled;
                    }
                    wgpui::Key::Named(NamedKey::End) if modifiers.command() || modifiers.ctrl => {
                        self.scroll_to_bottom();
                        return EventResult::Handled;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }
}

impl Terminal {
    /// Draw a text run with given style.
    fn draw_text_run(
        &self,
        cx: &mut PaintContext,
        x: f32,
        y: f32,
        text: &str,
        style: crate::ansi::AnsiStyle,
    ) {
        // Draw background if not transparent
        let bg = style.effective_bg();
        if bg.a > 0.0 {
            let width = text.chars().count() as f32 * self.char_width;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, y, width, self.line_height)).with_background(bg),
            );
        }

        // Draw text
        let fg = style.effective_fg();
        let run = cx.text.layout(text, Point::new(x, y), self.font_size, fg);
        cx.scene.draw_text(run);

        // Draw underline if needed
        if style.underline {
            let width = text.chars().count() as f32 * self.char_width;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, y + self.line_height - 1.0, width, 1.0))
                    .with_background(fg),
            );
        }

        // Draw strikethrough if needed
        if style.strikethrough {
            let width = text.chars().count() as f32 * self.char_width;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, y + self.line_height / 2.0, width, 1.0))
                    .with_background(fg),
            );
        }
    }

    /// Check if a cell is within the selection.
    fn is_cell_selected(&self, row: usize, col: usize) -> bool {
        let (start, end) = match (self.selection_start, self.selection_end) {
            (Some(s), Some(e)) => {
                if s.0 < e.0 || (s.0 == e.0 && s.1 <= e.1) {
                    (s, e)
                } else {
                    (e, s)
                }
            }
            _ => return false,
        };

        if row < start.0 || row > end.0 {
            return false;
        }

        if row == start.0 && row == end.0 {
            col >= start.1 && col <= end.1
        } else if row == start.0 {
            col >= start.1
        } else if row == end.0 {
            col <= end.1
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_creation() {
        let terminal = Terminal::new(80, 24);
        assert_eq!(terminal.buffer().cols(), 80);
        assert_eq!(terminal.buffer().rows(), 24);
    }

    #[test]
    fn test_terminal_write_raw() {
        let mut terminal = Terminal::new(80, 24);
        terminal.write_raw("Hello");

        assert_eq!(terminal.buffer().cursor(), (0, 5));
    }

    #[test]
    fn test_terminal_write_ansi() {
        let mut terminal = Terminal::new(80, 24);
        terminal.write("\x1b[31mRed\x1b[0m Normal");

        // Text should be written
        assert_eq!(terminal.buffer().cursor(), (0, 10));
    }

    #[test]
    fn test_terminal_scroll() {
        let mut terminal = Terminal::new(80, 24);
        terminal.scroll_up(5);
        assert_eq!(terminal.scroll_offset(), 0); // Can't scroll past scrollback

        // Add some scrollback
        for i in 0..30 {
            terminal.write(&format!("Line {}\n", i));
        }

        terminal.scroll_up(5);
        assert!(terminal.scroll_offset() > 0);

        terminal.scroll_to_bottom();
        assert_eq!(terminal.scroll_offset(), 0);
    }

    #[test]
    fn test_terminal_clear() {
        let mut terminal = Terminal::new(80, 24);
        terminal.write("Hello");
        terminal.clear();

        assert_eq!(terminal.buffer().cursor(), (0, 0));
    }

    #[test]
    fn test_cursor_style() {
        let mut terminal = Terminal::new(80, 24);
        terminal.set_cursor_style(CursorStyle::Bar);
        assert_eq!(terminal.cursor_style, CursorStyle::Bar);
    }

    #[test]
    fn test_dimensions_calculation() {
        let terminal = Terminal::new(80, 24);
        let (width, height) = terminal.size_for_dimensions(80, 24);
        assert!(width > 0.0);
        assert!(height > 0.0);
    }
}
