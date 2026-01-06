//! LiveEditor - Multi-line text editor with live markdown formatting
//!
//! This is the core editor component for Onyx and can be reused across
//! the codebase for any multi-line text editing needs.

mod cursor;

pub use cursor::{Cursor, Selection};

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::input::{Key, NamedKey};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};
use web_time::Instant;

/// Snapshot of editor state for undo/redo
#[derive(Clone)]
struct EditorSnapshot {
    lines: Vec<String>,
    cursor: Cursor,
    selection: Option<Selection>,
}

/// Multi-line text editor with live markdown formatting
pub struct LiveEditor {
    id: Option<ComponentId>,

    // Content
    lines: Vec<String>,

    // Cursor state
    cursor: Cursor,
    selection: Option<Selection>,

    // UI state
    focused: bool,
    scroll_offset: f32,

    // Cursor blink
    cursor_blink_start: Instant,

    // Mouse drag selection
    is_dragging: bool,
    drag_start_pos: Option<Cursor>,

    // Click detection for double/triple click
    last_click_time: Instant,
    click_count: u32,
    last_click_pos: (f32, f32),

    // Undo/redo
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,

    // Styling
    style: LiveEditorStyle,

    // Cached mono char width (computed during paint)
    mono_char_width: f32,

    // Callbacks
    on_change: Option<Box<dyn FnMut(&str)>>,
    on_save: Option<Box<dyn FnMut()>>,
}

/// Styling options for LiveEditor
#[derive(Debug, Clone)]
pub struct LiveEditorStyle {
    pub background: Hsla,
    pub text_color: Hsla,
    pub cursor_color: Hsla,
    pub selection_color: Hsla,
    pub line_number_color: Hsla,
    pub font_size: f32,
    pub line_height: f32,
    pub gutter_width: f32,
    pub padding: f32,
}

impl Default for LiveEditorStyle {
    fn default() -> Self {
        Self {
            background: theme::bg::APP,
            text_color: theme::text::PRIMARY,
            cursor_color: theme::accent::PRIMARY,
            selection_color: Hsla::new(210.0, 0.6, 0.4, 0.3),
            line_number_color: Hsla::new(0.0, 0.0, 0.35, 1.0), // Dark gray, barely visible
            font_size: theme::font_size::SM,
            line_height: 1.5,
            gutter_width: 56.0, // More spacing after line numbers
            padding: theme::spacing::SM,
        }
    }
}

impl LiveEditor {
    /// Create a new LiveEditor with initial content
    pub fn new(content: &str) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };

        let style = LiveEditorStyle::default();
        // Initial estimate for mono char width (will be updated in paint)
        let mono_char_width = style.font_size * 0.6;
        let now = Instant::now();

        Self {
            id: None,
            lines,
            cursor: Cursor::start(),
            selection: None,
            focused: false,
            scroll_offset: 0.0,
            cursor_blink_start: now,
            is_dragging: false,
            drag_start_pos: None,
            last_click_time: now,
            click_count: 0,
            last_click_pos: (0.0, 0.0),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            style,
            mono_char_width,
            on_change: None,
            on_save: None,
        }
    }

    /// Set component ID
    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set custom style
    pub fn with_style(mut self, style: LiveEditorStyle) -> Self {
        self.style = style;
        self
    }

    /// Set font size
    pub fn font_size(mut self, size: f32) -> Self {
        self.style.font_size = size;
        self
    }

    /// Set change callback
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Set save callback (Ctrl+S)
    pub fn on_save<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_save = Some(Box::new(f));
        self
    }

    /// Get the full content as a string
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Set content, replacing current buffer
    pub fn set_content(&mut self, content: &str) {
        self.lines = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };
        // Ensure cursor is valid
        self.cursor.line = self.cursor.line.min(self.lines.len().saturating_sub(1));
        self.cursor.column = self.cursor.column.min(self.current_line_len());
        self.selection = None;
    }

    /// Get number of lines
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get length of current line
    fn current_line_len(&self) -> usize {
        self.lines.get(self.cursor.line).map_or(0, |l| l.chars().count())
    }

    /// Get length of a specific line
    fn line_len(&self, line: usize) -> usize {
        self.lines.get(line).map_or(0, |l| l.chars().count())
    }

    /// Focus the editor
    pub fn focus(&mut self) {
        self.focused = true;
    }

    /// Blur the editor
    pub fn blur(&mut self) {
        self.focused = false;
        self.selection = None;
    }

    /// Check if editor is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    // === Cursor Movement ===

    fn move_cursor_left(&mut self) {
        if self.cursor.column > 0 {
            self.cursor.column -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.column = self.current_line_len();
        }
        self.cursor.clear_preferred_column();
    }

    fn move_cursor_right(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor.column < line_len {
            self.cursor.column += 1;
        } else if self.cursor.line < self.lines.len() - 1 {
            self.cursor.line += 1;
            self.cursor.column = 0;
        }
        self.cursor.clear_preferred_column();
    }

    fn move_cursor_up(&mut self) {
        if self.cursor.line > 0 {
            if self.cursor.preferred_column.is_none() {
                self.cursor.set_preferred_column();
            }
            self.cursor.line -= 1;
            let target_col = self.cursor.preferred_column.unwrap_or(self.cursor.column);
            self.cursor.column = target_col.min(self.current_line_len());
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor.line < self.lines.len() - 1 {
            if self.cursor.preferred_column.is_none() {
                self.cursor.set_preferred_column();
            }
            self.cursor.line += 1;
            let target_col = self.cursor.preferred_column.unwrap_or(self.cursor.column);
            self.cursor.column = target_col.min(self.current_line_len());
        }
    }

    fn move_cursor_to_line_start(&mut self) {
        self.cursor.column = 0;
        self.cursor.clear_preferred_column();
    }

    fn move_cursor_to_line_end(&mut self) {
        self.cursor.column = self.current_line_len();
        self.cursor.clear_preferred_column();
    }

    fn move_cursor_to_document_start(&mut self) {
        self.cursor.line = 0;
        self.cursor.column = 0;
        self.cursor.clear_preferred_column();
    }

    fn move_cursor_to_document_end(&mut self) {
        self.cursor.line = self.lines.len().saturating_sub(1);
        self.cursor.column = self.current_line_len();
        self.cursor.clear_preferred_column();
    }

    fn move_cursor_page_up(&mut self, visible_lines: usize) {
        let jump = visible_lines.saturating_sub(2).max(1);
        self.cursor.line = self.cursor.line.saturating_sub(jump);
        self.cursor.column = self.cursor.column.min(self.current_line_len());
    }

    fn move_cursor_page_down(&mut self, visible_lines: usize) {
        let jump = visible_lines.saturating_sub(2).max(1);
        self.cursor.line = (self.cursor.line + jump).min(self.lines.len().saturating_sub(1));
        self.cursor.column = self.cursor.column.min(self.current_line_len());
    }

    // === Word/Line Selection ===

    fn select_word_at_cursor(&mut self) {
        let line = match self.lines.get(self.cursor.line) {
            Some(l) => l,
            None => return,
        };

        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return;
        }

        let col = self.cursor.column.min(chars.len().saturating_sub(1));

        // Find word boundaries
        let mut start = col;
        while start > 0 && chars[start - 1].is_alphanumeric() {
            start -= 1;
        }

        let mut end = col;
        while end < chars.len() && chars[end].is_alphanumeric() {
            end += 1;
        }

        // If we're not on a word, select at least one character
        if start == end && end < chars.len() {
            end += 1;
        }

        let start_cursor = Cursor::new(self.cursor.line, start);
        let end_cursor = Cursor::new(self.cursor.line, end);
        self.selection = Some(Selection::new(start_cursor, end_cursor));
        self.cursor = end_cursor;
    }

    fn select_line_at_cursor(&mut self) {
        let line_len = self.current_line_len();
        let start_cursor = Cursor::new(self.cursor.line, 0);
        let end_cursor = Cursor::new(self.cursor.line, line_len);
        self.selection = Some(Selection::new(start_cursor, end_cursor));
        self.cursor = end_cursor;
    }

    // === Undo/Redo ===

    fn save_undo_state(&mut self) {
        let snapshot = EditorSnapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
            selection: self.selection,
        };
        self.undo_stack.push(snapshot);
        // Clear redo stack on new edit
        self.redo_stack.clear();
        // Limit undo stack size
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            // Save current state to redo stack
            let current = EditorSnapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
                selection: self.selection,
            };
            self.redo_stack.push(current);

            // Restore previous state
            self.lines = snapshot.lines;
            self.cursor = snapshot.cursor;
            self.selection = snapshot.selection;
        }
    }

    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            // Save current state to undo stack
            let current = EditorSnapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
                selection: self.selection,
            };
            self.undo_stack.push(current);

            // Restore redo state
            self.lines = snapshot.lines;
            self.cursor = snapshot.cursor;
            self.selection = snapshot.selection;
        }
    }

    // === Text Editing ===

    fn insert_char(&mut self, c: char) {
        self.save_undo_state();
        self.delete_selection_internal();
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            let byte_idx = line
                .char_indices()
                .nth(self.cursor.column)
                .map_or(line.len(), |(i, _)| i);
            line.insert(byte_idx, c);
            self.cursor.column += 1;
        }
        self.cursor_blink_start = Instant::now();
        self.notify_change();
    }

    fn insert_str(&mut self, s: &str) {
        self.save_undo_state();
        for c in s.chars() {
            if c == '\n' {
                self.insert_newline_internal();
            } else {
                self.insert_char_internal(c);
            }
        }
        self.cursor_blink_start = Instant::now();
        self.notify_change();
    }

    // Internal methods that don't save undo state (for batched operations)
    fn insert_char_internal(&mut self, c: char) {
        self.delete_selection_internal();
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            let byte_idx = line
                .char_indices()
                .nth(self.cursor.column)
                .map_or(line.len(), |(i, _)| i);
            line.insert(byte_idx, c);
            self.cursor.column += 1;
        }
    }

    fn insert_newline(&mut self) {
        self.save_undo_state();
        self.insert_newline_internal();
        self.cursor_blink_start = Instant::now();
        self.notify_change();
    }

    fn insert_newline_internal(&mut self) {
        self.delete_selection_internal();
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            let byte_idx = line
                .char_indices()
                .nth(self.cursor.column)
                .map_or(line.len(), |(i, _)| i);
            let remainder = line.split_off(byte_idx);
            self.lines.insert(self.cursor.line + 1, remainder);
            self.cursor.line += 1;
            self.cursor.column = 0;
        }
    }

    fn delete_backward(&mut self) {
        if self.delete_selection() {
            return;
        }

        self.save_undo_state();
        if self.cursor.column > 0 {
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                let byte_idx = line
                    .char_indices()
                    .nth(self.cursor.column - 1)
                    .map_or(0, |(i, _)| i);
                let next_byte_idx = line
                    .char_indices()
                    .nth(self.cursor.column)
                    .map_or(line.len(), |(i, _)| i);
                line.replace_range(byte_idx..next_byte_idx, "");
                self.cursor.column -= 1;
            }
            self.cursor_blink_start = Instant::now();
            self.notify_change();
        } else if self.cursor.line > 0 {
            // Merge with previous line
            let current_line = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            self.cursor.column = self.current_line_len();
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                line.push_str(&current_line);
            }
            self.cursor_blink_start = Instant::now();
            self.notify_change();
        }
    }

    fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }

        self.save_undo_state();
        let line_len = self.current_line_len();
        if self.cursor.column < line_len {
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                let byte_idx = line
                    .char_indices()
                    .nth(self.cursor.column)
                    .map_or(0, |(i, _)| i);
                let next_byte_idx = line
                    .char_indices()
                    .nth(self.cursor.column + 1)
                    .map_or(line.len(), |(i, _)| i);
                line.replace_range(byte_idx..next_byte_idx, "");
            }
            self.cursor_blink_start = Instant::now();
            self.notify_change();
        } else if self.cursor.line < self.lines.len() - 1 {
            // Merge with next line
            let next_line = self.lines.remove(self.cursor.line + 1);
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                line.push_str(&next_line);
            }
            self.cursor_blink_start = Instant::now();
            self.notify_change();
        }
    }

    // === Selection ===

    fn start_selection(&mut self) {
        self.selection = Some(Selection::new(self.cursor, self.cursor));
    }

    fn extend_selection(&mut self) {
        if let Some(sel) = &mut self.selection {
            sel.head = self.cursor;
        } else {
            self.start_selection();
        }
    }

    fn clear_selection(&mut self) {
        self.selection = None;
    }

    fn select_all(&mut self) {
        let start = Cursor::start();
        let end = Cursor::new(
            self.lines.len().saturating_sub(1),
            self.line_len(self.lines.len().saturating_sub(1)),
        );
        self.selection = Some(Selection::new(start, end));
        self.cursor = end;
    }

    fn delete_selection(&mut self) -> bool {
        if self.selection.is_none() || self.selection.as_ref().is_some_and(|s| s.is_empty()) {
            return false;
        }
        self.save_undo_state();
        self.delete_selection_internal();
        self.cursor_blink_start = Instant::now();
        self.notify_change();
        true
    }

    // Internal version that doesn't save undo state
    fn delete_selection_internal(&mut self) -> bool {
        let Some(sel) = self.selection.take() else {
            return false;
        };

        if sel.is_empty() {
            return false;
        }

        let start = sel.start();
        let end = sel.end();

        if start.line == end.line {
            // Single line selection
            if let Some(line) = self.lines.get_mut(start.line) {
                let start_byte = line
                    .char_indices()
                    .nth(start.column)
                    .map_or(0, |(i, _)| i);
                let end_byte = line
                    .char_indices()
                    .nth(end.column)
                    .map_or(line.len(), |(i, _)| i);
                line.replace_range(start_byte..end_byte, "");
            }
        } else {
            // Multi-line selection
            // Keep content before start and after end
            let prefix = self.lines.get(start.line).map_or(String::new(), |l| {
                l.char_indices()
                    .nth(start.column)
                    .map_or(l.clone(), |(i, _)| l[..i].to_string())
            });
            let suffix = self.lines.get(end.line).map_or(String::new(), |l| {
                l.char_indices()
                    .nth(end.column)
                    .map_or(String::new(), |(i, _)| l[i..].to_string())
            });

            // Remove lines between start and end (inclusive)
            self.lines.drain(start.line..=end.line);

            // Insert merged line
            self.lines.insert(start.line, prefix + &suffix);
        }

        self.cursor = start;
        true
    }

    fn get_selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        if sel.is_empty() {
            return None;
        }

        let start = sel.start();
        let end = sel.end();

        if start.line == end.line {
            self.lines.get(start.line).map(|line| {
                line.chars()
                    .skip(start.column)
                    .take(end.column - start.column)
                    .collect()
            })
        } else {
            let mut result = String::new();
            for line_idx in start.line..=end.line {
                if let Some(line) = self.lines.get(line_idx) {
                    if line_idx == start.line {
                        result.push_str(&line.chars().skip(start.column).collect::<String>());
                    } else if line_idx == end.line {
                        result.push('\n');
                        result.push_str(&line.chars().take(end.column).collect::<String>());
                    } else {
                        result.push('\n');
                        result.push_str(line);
                    }
                }
            }
            Some(result)
        }
    }

    // === Callbacks ===

    fn notify_change(&mut self) {
        let content = self.content();
        if let Some(on_change) = &mut self.on_change {
            on_change(&content);
        }
    }

    fn notify_save(&mut self) {
        if let Some(on_save) = &mut self.on_save {
            on_save();
        }
    }

    // === Rendering Helpers ===

    fn line_y(&self, line: usize, bounds: &Bounds) -> f32 {
        let line_height = self.style.font_size * self.style.line_height;
        bounds.origin.y + self.style.padding + (line as f32 * line_height) - self.scroll_offset
    }

    fn cursor_position_from_point(&self, x: f32, y: f32, bounds: &Bounds) -> Cursor {
        let line_height = self.style.font_size * self.style.line_height;
        let content_y = y - bounds.origin.y - self.style.padding + self.scroll_offset;
        let line = ((content_y / line_height).floor() as usize).min(self.lines.len().saturating_sub(1));

        let text_x = bounds.origin.x + self.style.gutter_width + self.style.padding;
        let relative_x = (x - text_x).max(0.0);
        let column = ((relative_x / self.mono_char_width).round() as usize).min(self.line_len(line));

        Cursor::new(line, column)
    }

    fn ensure_cursor_visible(&mut self, bounds: &Bounds) {
        let line_height = self.style.font_size * self.style.line_height;
        let cursor_y = self.cursor.line as f32 * line_height;
        let visible_height = bounds.size.height - self.style.padding * 2.0;

        if cursor_y < self.scroll_offset {
            self.scroll_offset = cursor_y;
        } else if cursor_y + line_height > self.scroll_offset + visible_height {
            self.scroll_offset = cursor_y + line_height - visible_height;
        }
    }
}

impl Component for LiveEditor {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Update cached mono char width
        self.mono_char_width = cx.text.measure_styled_mono("M", self.style.font_size, FontStyle::default());

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(self.style.background),
        );

        let line_height = self.style.font_size * self.style.line_height;
        let text_x = bounds.origin.x + self.style.gutter_width + self.style.padding;
        let visible_height = bounds.size.height - self.style.padding * 2.0;

        // Calculate visible line range
        let first_visible = (self.scroll_offset / line_height).floor() as usize;
        let visible_lines = (visible_height / line_height).ceil() as usize + 1;
        let last_visible = (first_visible + visible_lines).min(self.lines.len());

        // Render visible lines
        for line_idx in first_visible..last_visible {
            let y = self.line_y(line_idx, &bounds);

            // Skip if outside visible area
            if y + line_height < bounds.origin.y || y > bounds.origin.y + bounds.size.height {
                continue;
            }

            // Line number (also mono)
            let line_num = format!("{:>4}", line_idx + 1);
            let gutter_x = bounds.origin.x + self.style.padding;
            let line_num_run = cx.text.layout_styled_mono(
                &line_num,
                Point::new(gutter_x, y),
                self.style.font_size,
                self.style.line_number_color,
                FontStyle::default(),
            );
            cx.scene.draw_text(line_num_run);

            // Line content (mono font)
            if let Some(line) = self.lines.get(line_idx) {
                if !line.is_empty() {
                    let text_run = cx.text.layout_styled_mono(
                        line,
                        Point::new(text_x, y),
                        self.style.font_size,
                        self.style.text_color,
                        FontStyle::default(),
                    );
                    cx.scene.draw_text(text_run);
                }
            }

            // Selection highlight for this line
            if let Some(sel) = &self.selection {
                if !sel.is_empty() {
                    let start = sel.start();
                    let end = sel.end();

                    if line_idx >= start.line && line_idx <= end.line {
                        let line_len = self.line_len(line_idx);
                        let sel_start_col = if line_idx == start.line { start.column } else { 0 };
                        let sel_end_col = if line_idx == end.line { end.column } else { line_len };

                        if sel_start_col < sel_end_col {
                            let sel_x = text_x + sel_start_col as f32 * self.mono_char_width;
                            let sel_width = (sel_end_col - sel_start_col) as f32 * self.mono_char_width;

                            cx.scene.draw_quad(
                                Quad::new(Bounds::new(sel_x, y, sel_width, line_height))
                                    .with_background(self.style.selection_color),
                            );
                        }
                    }
                }
            }
        }

        // Cursor with blinking (500ms on, 500ms off)
        if self.focused {
            let elapsed = self.cursor_blink_start.elapsed().as_millis();
            let cursor_visible = (elapsed / 500) % 2 == 0;

            if cursor_visible {
                let cursor_y = self.line_y(self.cursor.line, &bounds);
                let cursor_x = text_x + self.cursor.column as f32 * self.mono_char_width;
                // Shift cursor up slightly to align with text
                let cursor_offset_y = -2.0;

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(cursor_x, cursor_y + cursor_offset_y, 2.0, line_height))
                        .with_background(self.style.cursor_color),
                );
            }
        }

        // Scrollbar
        let total_content_height = self.lines.len() as f32 * line_height;
        if total_content_height > visible_height {
            let scrollbar_width = 8.0;
            let scrollbar_x = bounds.origin.x + bounds.size.width - scrollbar_width - 2.0;

            // Track
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    scrollbar_x,
                    bounds.origin.y + self.style.padding,
                    scrollbar_width,
                    visible_height,
                ))
                .with_background(Hsla::new(0.0, 0.0, 0.3, 0.2)),
            );

            // Thumb
            let thumb_ratio = visible_height / total_content_height;
            let thumb_height = (visible_height * thumb_ratio).max(20.0);
            let scroll_ratio = self.scroll_offset / (total_content_height - visible_height);
            let thumb_y = bounds.origin.y + self.style.padding + scroll_ratio * (visible_height - thumb_height);

            cx.scene.draw_quad(
                Quad::new(Bounds::new(scrollbar_x, thumb_y, scrollbar_width, thumb_height))
                    .with_background(Hsla::new(0.0, 0.0, 0.5, 0.5))
                    .with_corner_radius(4.0),
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    self.focused = true;
                    self.cursor_blink_start = Instant::now();

                    let new_cursor = self.cursor_position_from_point(*x, *y, &bounds);

                    // Detect double/triple click
                    let now = Instant::now();
                    let time_since_last = now.duration_since(self.last_click_time).as_millis();
                    let distance = ((x - self.last_click_pos.0).powi(2) + (y - self.last_click_pos.1).powi(2)).sqrt();

                    if time_since_last < 400 && distance < 5.0 {
                        self.click_count += 1;
                    } else {
                        self.click_count = 1;
                    }
                    self.last_click_time = now;
                    self.last_click_pos = (*x, *y);

                    match self.click_count {
                        1 => {
                            // Single click - position cursor and start drag
                            self.cursor = new_cursor;
                            self.clear_selection();
                            self.is_dragging = true;
                            self.drag_start_pos = Some(new_cursor);
                        }
                        2 => {
                            // Double click - select word
                            self.cursor = new_cursor;
                            self.select_word_at_cursor();
                            self.is_dragging = false;
                        }
                        _ => {
                            // Triple+ click - select line
                            self.cursor = new_cursor;
                            self.select_line_at_cursor();
                            self.is_dragging = false;
                            self.click_count = 3; // Cap at 3
                        }
                    }

                    if let Some(id) = self.id {
                        cx.set_focus(id);
                    }
                    return EventResult::Handled;
                } else if self.focused {
                    self.blur();
                    self.is_dragging = false;
                    cx.clear_focus();
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseMove { x, y } => {
                if self.is_dragging && self.focused {
                    if let Some(start) = self.drag_start_pos {
                        let new_cursor = self.cursor_position_from_point(*x, *y, &bounds);
                        self.cursor = new_cursor;
                        self.selection = Some(Selection::new(start, new_cursor));
                    }
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left {
                    self.is_dragging = false;
                    return EventResult::Handled;
                }
            }

            InputEvent::Scroll { dy, .. } => {
                if self.focused {
                    let line_height = self.style.font_size * self.style.line_height;
                    let max_scroll = (self.lines.len() as f32 * line_height - bounds.size.height + self.style.padding * 2.0).max(0.0);
                    self.scroll_offset = (self.scroll_offset - dy * line_height * 3.0).clamp(0.0, max_scroll);
                    return EventResult::Handled;
                }
            }

            InputEvent::KeyDown { key, modifiers } => {
                if !self.focused {
                    return EventResult::Ignored;
                }

                match key {
                    Key::Character(c) => {
                        if modifiers.ctrl || modifiers.meta {
                            match c.as_str() {
                                "a" | "A" => self.select_all(),
                                "c" | "C" => {
                                    if let Some(text) = self.get_selected_text() {
                                        cx.write_clipboard(&text);
                                    }
                                }
                                "x" | "X" => {
                                    if let Some(text) = self.get_selected_text() {
                                        cx.write_clipboard(&text);
                                        self.delete_selection();
                                    }
                                }
                                "v" | "V" => {
                                    if let Some(text) = cx.read_clipboard() {
                                        self.delete_selection();
                                        self.insert_str(&text);
                                    }
                                }
                                "s" | "S" => {
                                    self.notify_save();
                                }
                                "z" => {
                                    // Ctrl+Z = undo
                                    self.undo();
                                    self.cursor_blink_start = Instant::now();
                                }
                                "Z" => {
                                    // Ctrl+Shift+Z = redo
                                    self.redo();
                                    self.cursor_blink_start = Instant::now();
                                }
                                "y" | "Y" => {
                                    // Ctrl+Y = redo (alternative)
                                    self.redo();
                                    self.cursor_blink_start = Instant::now();
                                }
                                _ => {}
                            }
                        } else {
                            self.delete_selection();
                            self.insert_str(c);
                        }
                        self.ensure_cursor_visible(&bounds);
                        return EventResult::Handled;
                    }

                    Key::Named(named) => {
                        let shift = modifiers.shift;

                        match named {
                            NamedKey::Space => {
                                self.delete_selection();
                                self.insert_char(' ');
                            }
                            NamedKey::Enter => {
                                self.insert_newline();
                            }
                            NamedKey::Backspace => {
                                self.delete_backward();
                            }
                            NamedKey::Delete => {
                                self.delete_forward();
                            }
                            NamedKey::ArrowLeft => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_line_start();
                                    } else {
                                        self.move_cursor_left();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_line_start();
                                    } else {
                                        self.move_cursor_left();
                                    }
                                }
                            }
                            NamedKey::ArrowRight => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_line_end();
                                    } else {
                                        self.move_cursor_right();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_line_end();
                                    } else {
                                        self.move_cursor_right();
                                    }
                                }
                            }
                            NamedKey::ArrowUp => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_document_start();
                                    } else {
                                        self.move_cursor_up();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_document_start();
                                    } else {
                                        self.move_cursor_up();
                                    }
                                }
                            }
                            NamedKey::ArrowDown => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_document_end();
                                    } else {
                                        self.move_cursor_down();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_document_end();
                                    } else {
                                        self.move_cursor_down();
                                    }
                                }
                            }
                            NamedKey::Home => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_to_line_start();
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_to_line_start();
                                }
                            }
                            NamedKey::End => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_to_line_end();
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_to_line_end();
                                }
                            }
                            NamedKey::PageUp => {
                                let line_height = self.style.font_size * self.style.line_height;
                                let visible_height = bounds.size.height - self.style.padding * 2.0;
                                let visible_lines = (visible_height / line_height) as usize;
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_page_up(visible_lines);
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_page_up(visible_lines);
                                }
                                self.cursor_blink_start = Instant::now();
                            }
                            NamedKey::PageDown => {
                                let line_height = self.style.font_size * self.style.line_height;
                                let visible_height = bounds.size.height - self.style.padding * 2.0;
                                let visible_lines = (visible_height / line_height) as usize;
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_page_down(visible_lines);
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_page_down(visible_lines);
                                }
                                self.cursor_blink_start = Instant::now();
                            }
                            NamedKey::Tab => {
                                self.delete_selection();
                                self.insert_str("    "); // 4 spaces
                            }
                            NamedKey::Escape => {
                                self.blur();
                                cx.clear_focus();
                            }
                            _ => {}
                        }
                        self.ensure_cursor_visible(&bounds);
                        return EventResult::Handled;
                    }
                }
            }

            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let line_height = self.style.font_size * self.style.line_height;
        let height = self.lines.len() as f32 * line_height + self.style.padding * 2.0;
        (None, Some(height.max(100.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let editor = LiveEditor::new("Hello\nWorld");
        assert_eq!(editor.line_count(), 2);
        assert_eq!(editor.content(), "Hello\nWorld");
    }

    #[test]
    fn test_empty_editor() {
        let editor = LiveEditor::new("");
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.content(), "");
    }

    #[test]
    fn test_cursor_movement() {
        let mut editor = LiveEditor::new("Hello\nWorld");
        assert_eq!(editor.cursor.line, 0);
        assert_eq!(editor.cursor.column, 0);

        editor.move_cursor_right();
        assert_eq!(editor.cursor.column, 1);

        editor.move_cursor_down();
        assert_eq!(editor.cursor.line, 1);
        assert_eq!(editor.cursor.column, 1);

        editor.move_cursor_to_line_end();
        assert_eq!(editor.cursor.column, 5);
    }

    #[test]
    fn test_insert_char() {
        let mut editor = LiveEditor::new("Hello");
        editor.cursor.column = 5;
        editor.insert_char('!');
        assert_eq!(editor.content(), "Hello!");
    }

    #[test]
    fn test_insert_newline() {
        let mut editor = LiveEditor::new("HelloWorld");
        editor.cursor.column = 5;
        editor.insert_newline();
        assert_eq!(editor.content(), "Hello\nWorld");
        assert_eq!(editor.cursor.line, 1);
        assert_eq!(editor.cursor.column, 0);
    }

    #[test]
    fn test_delete_backward() {
        let mut editor = LiveEditor::new("Hello");
        editor.cursor.column = 5;
        editor.delete_backward();
        assert_eq!(editor.content(), "Hell");
    }

    #[test]
    fn test_delete_at_line_start() {
        let mut editor = LiveEditor::new("Hello\nWorld");
        editor.cursor.line = 1;
        editor.cursor.column = 0;
        editor.delete_backward();
        assert_eq!(editor.content(), "HelloWorld");
    }
}
