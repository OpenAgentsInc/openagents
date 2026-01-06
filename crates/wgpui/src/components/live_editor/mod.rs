//! LiveEditor - Multi-line text editor with live markdown formatting
//!
//! This is the core editor component for Onyx and can be reused across
//! the codebase for any multi-line text editing needs.

mod block;
mod cursor;

pub use cursor::{Cursor, Selection};
use block::{BlockParser, BlockType, parse_inline, header_font_scale, strip_header_prefix, strip_list_prefix, strip_blockquote_prefix, inline_code_background};

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
            cursor_color: Hsla::new(0.0, 0.0, 1.0, 1.0), // White
            selection_color: Hsla::new(210.0, 0.6, 0.5, 0.5),
            line_number_color: Hsla::new(0.0, 0.0, 0.18, 1.0), // Very dark gray
            font_size: theme::font_size::SM,
            line_height: 1.5,
            gutter_width: 0.0, // No gutter - line numbers in status bar
            padding: 96.0, // Comfortable vertical padding
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

        // Center content with max width 768px (must match paint)
        let max_content_width = 768.0;
        let content_width = bounds.size.width.min(max_content_width);
        let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
        let text_x = content_x + self.style.padding;

        // Check if clicked line is a header - need scaled char width
        let char_width = if let Some(line_text) = self.lines.get(line) {
            let mut parser = BlockParser::new();
            // Need to parse up to this line to track code block state
            for (i, l) in self.lines.iter().enumerate() {
                if i == line {
                    break;
                }
                parser.detect_block_type(l);
            }
            match parser.detect_block_type(line_text) {
                BlockType::Header(level) => {
                    self.mono_char_width * header_font_scale(level)
                }
                _ => self.mono_char_width,
            }
        } else {
            self.mono_char_width
        };

        let relative_x = (x - text_x).max(0.0);
        let column = ((relative_x / char_width).round() as usize).min(self.line_len(line));

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

    /// Render a line with markdown formatting
    fn render_formatted_line(
        &self,
        line: &str,
        block_type: BlockType,
        x: f32,
        y: f32,
        _line_height: f32,
        cx: &mut PaintContext,
    ) {
        let mut current_x = x;

        match block_type {
            BlockType::Header(level) => {
                // Render header with larger font
                let content = strip_header_prefix(line);
                let font_size = self.style.font_size * header_font_scale(level);
                let spans = parse_inline(content);

                for span in spans {
                    let mut style = FontStyle::default();
                    if span.bold {
                        style.bold = true;
                    }
                    if span.italic {
                        style.italic = true;
                    }

                    let text_run = cx.text.layout_styled_mono(
                        &span.text,
                        Point::new(current_x, y),
                        font_size,
                        self.style.text_color,
                        style,
                    );
                    current_x += span.text.chars().count() as f32 * (font_size * 0.6);
                    cx.scene.draw_text(text_run);
                }
            }

            BlockType::CodeBlock | BlockType::CodeFence => {
                // Render code with monospace, slightly dimmed
                let code_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
                let text_run = cx.text.layout_styled_mono(
                    line,
                    Point::new(x, y),
                    self.style.font_size,
                    code_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(text_run);
            }

            BlockType::UnorderedList => {
                // Render bullet point then content
                let content = strip_list_prefix(line);
                let bullet = "\u{2022} "; // bullet character
                let bullet_run = cx.text.layout_styled_mono(
                    bullet,
                    Point::new(current_x, y),
                    self.style.font_size,
                    self.style.text_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(bullet_run);
                current_x += self.mono_char_width * 2.0;

                // Render content with inline formatting
                self.render_inline_formatted(content, current_x, y, cx);
            }

            BlockType::OrderedList => {
                // Render number then content
                let content = strip_list_prefix(line);
                // Extract the number from original line
                let num: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
                let prefix = format!("{}. ", num);
                let prefix_run = cx.text.layout_styled_mono(
                    &prefix,
                    Point::new(current_x, y),
                    self.style.font_size,
                    self.style.text_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(prefix_run);
                current_x += self.mono_char_width * prefix.len() as f32;

                // Render content with inline formatting
                self.render_inline_formatted(content, current_x, y, cx);
            }

            BlockType::Blockquote => {
                // Render blockquote bar and content
                let content = strip_blockquote_prefix(line);
                let bar_color = Hsla::new(210.0, 0.5, 0.5, 0.7);

                // Draw vertical bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, y, 3.0, self.style.font_size * self.style.line_height))
                        .with_background(bar_color),
                );

                // Render content with italic style
                let text_run = cx.text.layout_styled_mono(
                    content,
                    Point::new(x + 12.0, y),
                    self.style.font_size,
                    Hsla::new(0.0, 0.0, 0.7, 1.0),
                    FontStyle::italic(),
                );
                cx.scene.draw_text(text_run);
            }

            BlockType::HorizontalRule => {
                // Draw a horizontal line
                let rule_y = y + (self.style.font_size * self.style.line_height) / 2.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, rule_y, 200.0, 1.0))
                        .with_background(Hsla::new(0.0, 0.0, 0.3, 1.0)),
                );
            }

            BlockType::Empty => {
                // Nothing to render
            }

            BlockType::Paragraph => {
                // Render paragraph with inline formatting
                self.render_inline_formatted(line, x, y, cx);
            }
        }
    }

    /// Render text with inline formatting (bold, italic, code, etc.)
    fn render_inline_formatted(&self, text: &str, x: f32, y: f32, cx: &mut PaintContext) {
        let spans = parse_inline(text);
        let mut current_x = x;

        for span in spans {
            if span.text.is_empty() {
                continue;
            }

            if span.code {
                // Inline code with background
                let bg_padding = 2.0;
                let code_width = span.text.chars().count() as f32 * self.mono_char_width;

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        current_x - bg_padding,
                        y,
                        code_width + bg_padding * 2.0,
                        self.style.font_size * self.style.line_height,
                    ))
                    .with_background(inline_code_background())
                    .with_corner_radius(3.0),
                );

                let text_run = cx.text.layout_styled_mono(
                    &span.text,
                    Point::new(current_x, y),
                    self.style.font_size,
                    Hsla::new(30.0, 0.8, 0.7, 1.0), // Orange-ish for code
                    FontStyle::default(),
                );
                cx.scene.draw_text(text_run);
                current_x += code_width;
            } else {
                // Regular text with bold/italic
                let mut style = FontStyle::default();
                if span.bold {
                    style.bold = true;
                }
                if span.italic {
                    style.italic = true;
                }

                let text_run = cx.text.layout_styled_mono(
                    &span.text,
                    Point::new(current_x, y),
                    self.style.font_size,
                    self.style.text_color,
                    style,
                );
                cx.scene.draw_text(text_run);
                current_x += span.text.chars().count() as f32 * self.mono_char_width;
            }
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
        let status_bar_height = 24.0;
        let visible_height = bounds.size.height - self.style.padding * 2.0 - status_bar_height;

        // Center content with max width 768px
        let max_content_width = 768.0;
        let content_width = bounds.size.width.min(max_content_width);
        let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
        let text_x = content_x + self.style.padding;

        // Calculate visible line range
        let first_visible = (self.scroll_offset / line_height).floor() as usize;
        let visible_lines = (visible_height / line_height).ceil() as usize + 1;
        let last_visible = (first_visible + visible_lines).min(self.lines.len());

        // Parse block types for all lines
        let mut block_parser = BlockParser::new();
        let block_types: Vec<BlockType> = self.lines.iter()
            .map(|line| block_parser.detect_block_type(line))
            .collect();

        // Render visible lines
        for line_idx in first_visible..last_visible {
            let y = self.line_y(line_idx, &bounds);

            // Skip if outside visible area
            if y + line_height < bounds.origin.y || y > bounds.origin.y + bounds.size.height {
                continue;
            }

            // Get line content and block type
            let line = self.lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");
            let block_type = block_types.get(line_idx).copied().unwrap_or(BlockType::Paragraph);
            let is_cursor_line = line_idx == self.cursor.line;

            if line.is_empty() {
                // Empty line, nothing to render
            } else if is_cursor_line {
                // Cursor line: render raw markdown but keep font size for headers
                let font_size = match block_type {
                    BlockType::Header(level) => self.style.font_size * header_font_scale(level),
                    _ => self.style.font_size,
                };
                let text_run = cx.text.layout_styled_mono(
                    line,
                    Point::new(text_x, y),
                    font_size,
                    self.style.text_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(text_run);
            } else {
                // Non-cursor line: render formatted markdown
                self.render_formatted_line(line, block_type, text_x, y, line_height, cx);
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

                // Get cursor char width - scale for headers
                let cursor_block_type = block_types.get(self.cursor.line).copied().unwrap_or(BlockType::Paragraph);
                let cursor_char_width = match cursor_block_type {
                    BlockType::Header(level) => {
                        let scale = header_font_scale(level);
                        cx.text.measure_styled_mono("M", self.style.font_size * scale, FontStyle::default())
                    }
                    _ => self.mono_char_width,
                };

                let cursor_x = text_x + self.cursor.column as f32 * cursor_char_width;
                // Shift cursor up slightly to align with text
                let cursor_offset_y = -2.0;

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(cursor_x, cursor_y + cursor_offset_y, 1.0, line_height))
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

        // Status bar at bottom
        let status_bar_y = bounds.origin.y + bounds.size.height - status_bar_height;

        // Line:Col indicator
        let status_text = format!("Ln {}, Col {}", self.cursor.line + 1, self.cursor.column + 1);
        let status_x = bounds.origin.x + bounds.size.width - 120.0;
        let status_y = status_bar_y + 4.0;
        let status_run = cx.text.layout_styled_mono(
            &status_text,
            Point::new(status_x, status_y),
            self.style.font_size * 0.85,
            Hsla::new(0.0, 0.0, 0.5, 1.0),
            FontStyle::default(),
        );
        cx.scene.draw_text(status_run);
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
                        // Reset blink timer so cursor shows immediately after movement
                        self.cursor_blink_start = Instant::now();
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
