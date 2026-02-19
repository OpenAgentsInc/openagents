//! LiveEditor - Multi-line text editor with live markdown formatting
//!
//! This is the core editor component for Onyx and can be reused across
//! the codebase for any multi-line text editing needs.

mod block;
mod cursor;

/// Fixed font size for the status bar (doesn't scale with zoom)
const STATUS_BAR_FONT_SIZE: f32 = 12.0;

/// Default font size used as baseline for scaling calculations
const DEFAULT_FONT_SIZE: f32 = 14.0;

/// Base max content width at default font size
const BASE_MAX_CONTENT_WIDTH: f32 = 768.0;

use block::{
    BlockParser, BlockType, header_font_scale, inline_code_background, parse_inline,
    strip_blockquote_prefix, strip_header_prefix, strip_list_prefix,
};
pub use cursor::{Cursor, Selection};
pub use vim::Mode as VimMode;

/// Pending operator waiting for a motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingOperator {
    Delete,
    Change,
    Yank,
}

/// Vim state for the editor (using vim crate's Mode)
#[derive(Debug, Clone, Default)]
pub struct VimState {
    /// Current vim mode
    pub mode: vim::Mode,
    /// Count prefix (e.g., "3" in "3j")
    pub count: Option<usize>,
    /// Pending operator
    pub pending_operator: Option<PendingOperator>,
    /// Vim-specific register (for yanked text)
    pub register: Option<String>,
    /// Visual mode anchor
    pub visual_anchor: Option<Cursor>,
    /// Pending 'g' key for gg motion
    pub pending_g: bool,
}

impl VimState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset_pending(&mut self) {
        self.count = None;
        self.pending_operator = None;
        self.pending_g = false;
    }

    pub fn effective_count(&self) -> usize {
        self.count.unwrap_or(1)
    }

    pub fn push_count_digit(&mut self, digit: u8) {
        let current = self.count.unwrap_or(0);
        self.count = Some(current * 10 + digit as usize);
    }

    pub fn enter_insert(&mut self) {
        self.mode = vim::Mode::Insert;
        self.reset_pending();
    }

    pub fn enter_normal(&mut self) {
        self.mode = vim::Mode::Normal;
        self.reset_pending();
        self.visual_anchor = None;
    }

    pub fn enter_visual(&mut self, anchor: Cursor) {
        self.mode = vim::Mode::Visual;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }

    pub fn enter_visual_line(&mut self, anchor: Cursor) {
        self.mode = vim::Mode::VisualLine;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }
}

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::input::{Key, Modifiers, NamedKey};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};
use web_time::Instant;

type LiveEditorTextHandler = Box<dyn FnMut(&str) + 'static>;
type LiveEditorActionHandler = Box<dyn FnMut() + 'static>;

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

    // Vim mode
    vim_enabled: bool,
    vim: VimState,

    // Callbacks
    on_change: Option<LiveEditorTextHandler>,
    on_save: Option<LiveEditorActionHandler>,

    // Status message (for voice transcription, etc.)
    status_message: Option<(String, Hsla)>,

    // Background opacity (0.0 = transparent, 1.0 = opaque)
    background_opacity: f32,
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
    pub wrap_text: bool,
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
            padding: 48.0,     // Comfortable vertical padding
            wrap_text: true,   // Enable word wrapping by default
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
            vim_enabled: true,
            vim: VimState::new(),
            on_change: None,
            on_save: None,
            status_message: None,
            background_opacity: 1.0,
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

    /// Zoom in (increase font size)
    pub fn zoom_in(&mut self) {
        self.style.font_size = (self.style.font_size + 2.0).min(48.0);
        self.mono_char_width = self.style.font_size * 0.6;
    }

    /// Zoom out (decrease font size)
    pub fn zoom_out(&mut self) {
        self.style.font_size = (self.style.font_size - 2.0).max(8.0);
        self.mono_char_width = self.style.font_size * 0.6;
    }

    /// Reset zoom to default
    pub fn zoom_reset(&mut self) {
        self.style.font_size = theme::font_size::SM;
        self.mono_char_width = self.style.font_size * 0.6;
    }

    /// Set background opacity (0.0 = transparent, 1.0 = opaque)
    pub fn set_background_opacity(&mut self, opacity: f32) {
        self.background_opacity = opacity;
    }

    /// Get the max content width scaled by current font size
    fn scaled_max_content_width(&self) -> f32 {
        let scale = self.style.font_size / DEFAULT_FONT_SIZE;
        BASE_MAX_CONTENT_WIDTH * scale
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

    // === Vim Mode ===

    /// Enable or disable vim mode
    pub fn set_vim_mode(&mut self, enabled: bool) {
        self.vim_enabled = enabled;
        if enabled {
            self.vim = VimState::new();
        }
    }

    /// Toggle vim mode
    pub fn toggle_vim_mode(&mut self) {
        self.set_vim_mode(!self.vim_enabled);
    }

    /// Check if vim mode is enabled
    pub fn vim_enabled(&self) -> bool {
        self.vim_enabled
    }

    /// Get current vim mode (for status bar)
    pub fn vim_mode(&self) -> Option<VimMode> {
        if self.vim_enabled {
            Some(self.vim.mode)
        } else {
            None
        }
    }

    /// Enter vim insert mode (if vim is enabled)
    pub fn enter_insert_mode(&mut self) {
        if self.vim_enabled {
            self.vim.enter_insert();
        }
    }

    /// Set a status message to display in the status bar
    pub fn set_status(&mut self, message: &str, color: Hsla) {
        self.status_message = Some((message.to_string(), color));
    }

    /// Clear the status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    // === Vim Key Handling ===

    fn handle_vim_key(
        &mut self,
        key: &Key,
        modifiers: &Modifiers,
        bounds: &Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        match self.vim.mode {
            VimMode::Normal => self.handle_vim_normal(key, modifiers, bounds, cx),
            VimMode::Insert | VimMode::Replace => self.handle_vim_insert(key, modifiers, bounds),
            VimMode::Visual | VimMode::VisualLine | VimMode::VisualBlock => {
                self.handle_vim_visual(key, modifiers, bounds, cx)
            }
        }
    }

    fn handle_vim_normal(
        &mut self,
        key: &Key,
        modifiers: &Modifiers,
        bounds: &Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        self.cursor_blink_start = Instant::now();

        match key {
            Key::Character(c) => {
                // Handle Ctrl/Cmd combinations in normal mode
                if modifiers.ctrl || modifiers.meta {
                    match c.as_str() {
                        "r" | "R" => {
                            self.redo();
                            self.ensure_cursor_visible(bounds);
                            return EventResult::Handled;
                        }
                        "f" | "F" => {
                            // Ctrl+F - page down
                            let line_height = self.style.font_size * self.style.line_height;
                            let visible_height = bounds.size.height - self.style.padding * 2.0;
                            let visible_lines = (visible_height / line_height) as usize;
                            self.vim_move_down(visible_lines);
                            self.ensure_cursor_visible(bounds);
                            return EventResult::Handled;
                        }
                        "b" | "B" => {
                            // Ctrl+B - page up
                            let line_height = self.style.font_size * self.style.line_height;
                            let visible_height = bounds.size.height - self.style.padding * 2.0;
                            let visible_lines = (visible_height / line_height) as usize;
                            self.vim_move_up(visible_lines);
                            self.ensure_cursor_visible(bounds);
                            return EventResult::Handled;
                        }
                        "d" | "D" => {
                            // Ctrl+D - half page down
                            let line_height = self.style.font_size * self.style.line_height;
                            let visible_height = bounds.size.height - self.style.padding * 2.0;
                            let visible_lines = (visible_height / line_height) as usize / 2;
                            self.vim_move_down(visible_lines.max(1));
                            self.ensure_cursor_visible(bounds);
                            return EventResult::Handled;
                        }
                        "u" | "U" => {
                            // Ctrl+U - half page up
                            let line_height = self.style.font_size * self.style.line_height;
                            let visible_height = bounds.size.height - self.style.padding * 2.0;
                            let visible_lines = (visible_height / line_height) as usize / 2;
                            self.vim_move_up(visible_lines.max(1));
                            self.ensure_cursor_visible(bounds);
                            return EventResult::Handled;
                        }
                        _ => return EventResult::Ignored,
                    }
                }

                match c.as_str() {
                    // Count prefix
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                        let digit = c.chars().next().unwrap().to_digit(10).unwrap() as u8;
                        self.vim.push_count_digit(digit);
                    }
                    "0" => {
                        if self.vim.count.is_some() {
                            self.vim.push_count_digit(0);
                        } else {
                            self.vim_line_start();
                        }
                    }

                    // Basic motions
                    "h" => {
                        let count = self.vim.effective_count();
                        self.vim_move_left(count);
                        self.vim.reset_pending();
                    }
                    "j" => {
                        let count = self.vim.effective_count();
                        self.vim_move_down(count);
                        self.vim.reset_pending();
                    }
                    "k" => {
                        let count = self.vim.effective_count();
                        self.vim_move_up(count);
                        self.vim.reset_pending();
                    }
                    "l" => {
                        let count = self.vim.effective_count();
                        self.vim_move_right(count);
                        self.vim.reset_pending();
                    }

                    // Word motions
                    "w" => {
                        let count = self.vim.effective_count();
                        if !self.vim_execute_motion_with_operator("w", count, cx) {
                            self.vim_word_forward(count);
                        }
                        self.vim.reset_pending();
                    }
                    "b" => {
                        let count = self.vim.effective_count();
                        if !self.vim_execute_motion_with_operator("b", count, cx) {
                            self.vim_word_backward(count);
                        }
                        self.vim.reset_pending();
                    }
                    "e" => {
                        let count = self.vim.effective_count();
                        if !self.vim_execute_motion_with_operator("e", count, cx) {
                            self.vim_word_end(count);
                        }
                        self.vim.reset_pending();
                    }

                    // Line motions
                    "$" => {
                        let count = self.vim.effective_count();
                        if !self.vim_execute_motion_with_operator("$", count, cx) {
                            self.vim_line_end();
                        }
                        self.vim.reset_pending();
                    }
                    "^" => {
                        let count = self.vim.effective_count();
                        if !self.vim_execute_motion_with_operator("^", count, cx) {
                            self.vim_first_non_blank();
                        }
                        self.vim.reset_pending();
                    }

                    // Paragraph motions
                    "{" => {
                        let count = self.vim.effective_count();
                        if !self.vim_execute_motion_with_operator("{", count, cx) {
                            self.vim_paragraph_backward(count);
                        }
                        self.vim.reset_pending();
                    }
                    "}" => {
                        let count = self.vim.effective_count();
                        if !self.vim_execute_motion_with_operator("}", count, cx) {
                            self.vim_paragraph_forward(count);
                        }
                        self.vim.reset_pending();
                    }

                    // Document motions
                    "g" => {
                        if self.vim.pending_g {
                            // gg - go to start
                            self.vim_document_start();
                            self.vim.pending_g = false;
                            self.vim.reset_pending();
                        } else {
                            self.vim.pending_g = true;
                        }
                    }
                    "G" => {
                        let line = self.vim.count;
                        self.vim_document_end(line);
                        self.vim.reset_pending();
                    }

                    // Operators
                    "d" => {
                        if self.vim.pending_operator == Some(PendingOperator::Delete) {
                            // dd - delete line(s)
                            let count = self.vim.effective_count();
                            self.vim_delete_lines(count);
                            self.vim.reset_pending();
                        } else {
                            self.vim.pending_operator = Some(PendingOperator::Delete);
                        }
                    }
                    "c" => {
                        if self.vim.pending_operator == Some(PendingOperator::Change) {
                            // cc - change line(s)
                            let count = self.vim.effective_count();
                            self.vim_change_lines(count);
                        } else {
                            self.vim.pending_operator = Some(PendingOperator::Change);
                        }
                    }
                    "y" => {
                        if self.vim.pending_operator == Some(PendingOperator::Yank) {
                            // yy - yank line(s)
                            let count = self.vim.effective_count();
                            self.vim_yank_lines(count, cx);
                            self.vim.reset_pending();
                        } else {
                            self.vim.pending_operator = Some(PendingOperator::Yank);
                        }
                    }

                    // Single-key operations
                    "x" => {
                        self.vim_delete_char();
                        self.vim.reset_pending();
                    }
                    "p" => {
                        self.vim_paste_after(cx);
                        self.vim.reset_pending();
                    }
                    "P" => {
                        self.vim_paste_before(cx);
                        self.vim.reset_pending();
                    }
                    "u" => {
                        self.undo();
                        self.vim.reset_pending();
                    }

                    // Insert mode entry
                    "i" => {
                        self.vim.enter_insert();
                    }
                    "a" => {
                        // Append after cursor
                        let line_len = self.current_line_len();
                        if self.cursor.column < line_len {
                            self.cursor.column += 1;
                        }
                        self.vim.enter_insert();
                    }
                    "I" => {
                        self.vim_first_non_blank();
                        self.vim.enter_insert();
                    }
                    "A" => {
                        self.cursor.column = self.current_line_len();
                        self.vim.enter_insert();
                    }
                    "o" => {
                        // Open line below
                        self.cursor.column = self.current_line_len();
                        self.insert_newline();
                        self.vim.enter_insert();
                    }
                    "O" => {
                        // Open line above
                        self.cursor.column = 0;
                        self.insert_newline();
                        self.cursor.line = self.cursor.line.saturating_sub(1);
                        self.vim.enter_insert();
                    }

                    // Visual mode
                    "v" => {
                        self.vim.enter_visual(self.cursor);
                        self.selection = Some(Selection::new(self.cursor, self.cursor));
                    }
                    "V" => {
                        self.vim.enter_visual_line(self.cursor);
                        self.select_line(self.cursor.line);
                    }

                    // Ignore all other characters in normal mode (don't insert them!)
                    _ => {}
                }

                self.ensure_cursor_visible(bounds);
                EventResult::Handled
            }

            Key::Named(named) => {
                // Let Cmd/Ctrl+Arrow keys pass through to standard handler
                if modifiers.meta || modifiers.ctrl {
                    match named {
                        NamedKey::ArrowUp
                        | NamedKey::ArrowDown
                        | NamedKey::ArrowLeft
                        | NamedKey::ArrowRight => {
                            return EventResult::Ignored;
                        }
                        _ => {}
                    }
                }

                match named {
                    NamedKey::Escape => {
                        self.vim.reset_pending();
                        self.selection = None;
                    }
                    NamedKey::Enter => {
                        // Enter in normal mode moves down (like j)
                        self.vim_move_down(1);
                    }
                    _ => {}
                }
                self.ensure_cursor_visible(bounds);
                EventResult::Handled
            }
        }
    }

    fn handle_vim_insert(
        &mut self,
        key: &Key,
        _modifiers: &Modifiers,
        bounds: &Bounds,
    ) -> EventResult {
        match key {
            Key::Named(NamedKey::Escape) => {
                // Exit insert mode, move cursor back one
                if self.cursor.column > 0 {
                    self.cursor.column -= 1;
                }
                self.vim.enter_normal();
                self.ensure_cursor_visible(bounds);
                EventResult::Handled
            }
            // Delegate all other keys to standard handler
            _ => EventResult::Ignored,
        }
    }

    fn handle_vim_visual(
        &mut self,
        key: &Key,
        modifiers: &Modifiers,
        bounds: &Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        self.cursor_blink_start = Instant::now();

        // Let Cmd/Ctrl+Arrow keys pass through to standard handler
        if (modifiers.meta || modifiers.ctrl)
            && matches!(
                key,
                Key::Named(
                    NamedKey::ArrowUp
                        | NamedKey::ArrowDown
                        | NamedKey::ArrowLeft
                        | NamedKey::ArrowRight
                )
            )
        {
            return EventResult::Ignored;
        }

        match key {
            Key::Named(NamedKey::Escape) => {
                self.vim.enter_normal();
                self.selection = None;
                EventResult::Handled
            }

            Key::Character(c) => {
                match c.as_str() {
                    // Motions extend selection
                    "h" => {
                        let count = self.vim.effective_count();
                        self.vim_move_left(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "j" => {
                        let count = self.vim.effective_count();
                        self.vim_move_down(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "k" => {
                        let count = self.vim.effective_count();
                        self.vim_move_up(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "l" => {
                        let count = self.vim.effective_count();
                        self.vim_move_right(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "w" => {
                        let count = self.vim.effective_count();
                        self.vim_word_forward(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "b" => {
                        let count = self.vim.effective_count();
                        self.vim_word_backward(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "e" => {
                        let count = self.vim.effective_count();
                        self.vim_word_end(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "0" => {
                        self.vim_line_start();
                        self.update_visual_selection();
                    }
                    "$" => {
                        self.vim_line_end();
                        self.update_visual_selection();
                    }
                    "^" => {
                        self.vim_first_non_blank();
                        self.update_visual_selection();
                    }
                    "{" => {
                        let count = self.vim.effective_count();
                        self.vim_paragraph_backward(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "}" => {
                        let count = self.vim.effective_count();
                        self.vim_paragraph_forward(count);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "G" => {
                        let line = self.vim.count;
                        self.vim_document_end(line);
                        self.update_visual_selection();
                        self.vim.reset_pending();
                    }
                    "g" => {
                        if self.vim.pending_g {
                            self.vim_document_start();
                            self.update_visual_selection();
                            self.vim.pending_g = false;
                        } else {
                            self.vim.pending_g = true;
                        }
                    }

                    // Operators on selection
                    "d" | "x" => {
                        self.delete_selection();
                        self.vim.enter_normal();
                    }
                    "c" => {
                        self.delete_selection();
                        self.vim.enter_insert();
                    }
                    "y" => {
                        if let Some(text) = self.get_selected_text() {
                            self.vim.register = Some(text.clone());
                            cx.write_clipboard(&text);
                        }
                        self.vim.enter_normal();
                        self.selection = None;
                    }

                    // Mode switches
                    "v" => {
                        if self.vim.mode == vim::Mode::Visual {
                            self.vim.enter_normal();
                            self.selection = None;
                        } else {
                            self.vim.mode = vim::Mode::Visual;
                        }
                    }
                    "V" => {
                        if self.vim.mode == vim::Mode::VisualLine {
                            self.vim.enter_normal();
                            self.selection = None;
                        } else {
                            self.vim.mode = vim::Mode::VisualLine;
                            self.update_visual_line_selection();
                        }
                    }

                    // Ignore all other characters in visual mode (don't insert them!)
                    _ => {}
                }

                self.ensure_cursor_visible(bounds);
                EventResult::Handled
            }

            _ => EventResult::Handled,
        }
    }

    fn update_visual_selection(&mut self) {
        if let Some(anchor) = self.vim.visual_anchor {
            if self.vim.mode == vim::Mode::VisualLine {
                self.update_visual_line_selection();
            } else {
                self.selection = Some(Selection::new(anchor, self.cursor));
            }
        }
    }

    fn update_visual_line_selection(&mut self) {
        if let Some(anchor) = self.vim.visual_anchor {
            let start_line = anchor.line.min(self.cursor.line);
            let end_line = anchor.line.max(self.cursor.line);

            let start = Cursor::new(start_line, 0);
            let end = Cursor::new(end_line, self.line_len(end_line));
            self.selection = Some(Selection::new(start, end));
        }
    }

    // === Vim Motions ===

    fn vim_move_left(&mut self, count: usize) {
        for _ in 0..count {
            if self.cursor.column > 0 {
                self.cursor.column -= 1;
            }
        }
        self.cursor.clear_preferred_column();
    }

    fn vim_move_right(&mut self, count: usize) {
        for _ in 0..count {
            let line_len = self.current_line_len();
            // In vim normal mode, cursor can't go past last char
            if self.cursor.column < line_len.saturating_sub(1) {
                self.cursor.column += 1;
            }
        }
        self.cursor.clear_preferred_column();
    }

    fn vim_move_up(&mut self, count: usize) {
        if self.cursor.preferred_column.is_none() {
            self.cursor.set_preferred_column();
        }
        for _ in 0..count {
            if self.cursor.line > 0 {
                self.cursor.line -= 1;
            }
        }
        let target = self.cursor.preferred_column.unwrap_or(self.cursor.column);
        let max_col = self.current_line_len().saturating_sub(1);
        self.cursor.column = target.min(max_col.max(0));
    }

    fn vim_move_down(&mut self, count: usize) {
        if self.cursor.preferred_column.is_none() {
            self.cursor.set_preferred_column();
        }
        for _ in 0..count {
            if self.cursor.line < self.lines.len().saturating_sub(1) {
                self.cursor.line += 1;
            }
        }
        let target = self.cursor.preferred_column.unwrap_or(self.cursor.column);
        let max_col = self.current_line_len().saturating_sub(1);
        self.cursor.column = target.min(max_col.max(0));
    }

    fn vim_line_start(&mut self) {
        self.cursor.column = 0;
        self.cursor.clear_preferred_column();
    }

    fn vim_line_end(&mut self) {
        let len = self.current_line_len();
        self.cursor.column = len.saturating_sub(1).max(0);
        self.cursor.clear_preferred_column();
    }

    fn vim_first_non_blank(&mut self) {
        if let Some(line) = self.lines.get(self.cursor.line) {
            self.cursor.column = line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
        }
        self.cursor.clear_preferred_column();
    }

    fn vim_document_start(&mut self) {
        self.cursor.line = 0;
        self.cursor.column = 0;
        self.cursor.clear_preferred_column();
    }

    fn vim_document_end(&mut self, line: Option<usize>) {
        if let Some(target_line) = line {
            self.cursor.line =
                (target_line.saturating_sub(1)).min(self.lines.len().saturating_sub(1));
        } else {
            self.cursor.line = self.lines.len().saturating_sub(1);
        }
        self.cursor.column = 0;
        self.cursor.clear_preferred_column();
    }

    fn vim_word_forward(&mut self, count: usize) {
        for _ in 0..count {
            self.move_to_next_word_start();
        }
    }

    fn vim_word_backward(&mut self, count: usize) {
        for _ in 0..count {
            self.move_to_prev_word_start();
        }
    }

    fn vim_word_end(&mut self, count: usize) {
        for _ in 0..count {
            self.move_to_word_end_internal();
        }
    }

    fn vim_paragraph_forward(&mut self, count: usize) {
        for _ in 0..count {
            let mut found_non_blank = false;
            while self.cursor.line < self.lines.len().saturating_sub(1) {
                self.cursor.line += 1;
                let is_blank = self
                    .lines
                    .get(self.cursor.line)
                    .map(|l| l.trim().is_empty())
                    .unwrap_or(true);
                if !is_blank {
                    found_non_blank = true;
                } else if found_non_blank {
                    break;
                }
            }
        }
        self.cursor.column = 0;
    }

    fn vim_paragraph_backward(&mut self, count: usize) {
        for _ in 0..count {
            let mut found_non_blank = false;
            while self.cursor.line > 0 {
                self.cursor.line -= 1;
                let is_blank = self
                    .lines
                    .get(self.cursor.line)
                    .map(|l| l.trim().is_empty())
                    .unwrap_or(true);
                if !is_blank {
                    found_non_blank = true;
                } else if found_non_blank {
                    break;
                }
            }
        }
        self.cursor.column = 0;
    }

    fn move_to_next_word_start(&mut self) {
        let Some(line) = self.lines.get(self.cursor.line) else {
            return;
        };
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor.column;

        // Skip current word
        while col < chars.len() && Self::is_vim_word_char(chars[col]) {
            col += 1;
        }
        // Skip non-word
        while col < chars.len()
            && !Self::is_vim_word_char(chars[col])
            && !chars[col].is_whitespace()
        {
            col += 1;
        }
        // Skip whitespace
        while col < chars.len() && chars[col].is_whitespace() {
            col += 1;
        }

        if col >= chars.len() && self.cursor.line < self.lines.len().saturating_sub(1) {
            self.cursor.line += 1;
            self.cursor.column = 0;
            // Skip leading whitespace on new line
            if let Some(next_line) = self.lines.get(self.cursor.line) {
                self.cursor.column = next_line
                    .chars()
                    .position(|c| !c.is_whitespace())
                    .unwrap_or(0);
            }
        } else {
            self.cursor.column = col.min(chars.len().saturating_sub(1).max(0));
        }
    }

    fn move_to_prev_word_start(&mut self) {
        if self.cursor.column == 0 && self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.column = self.current_line_len().saturating_sub(1).max(0);
            return;
        }

        let Some(line) = self.lines.get(self.cursor.line) else {
            return;
        };
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor.column;

        // Move back one if we're at a word start
        col = col.saturating_sub(1);

        // Skip whitespace
        while col > 0 && chars.get(col).is_some_and(|c| c.is_whitespace()) {
            col -= 1;
        }
        // Skip to start of word
        while col > 0
            && chars
                .get(col.saturating_sub(1))
                .is_some_and(|&c| Self::is_vim_word_char(c))
        {
            col -= 1;
        }

        self.cursor.column = col;
    }

    fn move_to_word_end_internal(&mut self) {
        let Some(line) = self.lines.get(self.cursor.line) else {
            return;
        };
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor.column;

        // Move forward one to start
        if col < chars.len().saturating_sub(1) {
            col += 1;
        }

        // Skip whitespace
        while col < chars.len() && chars[col].is_whitespace() {
            col += 1;
        }
        // Move to end of word
        while col < chars.len().saturating_sub(1) && Self::is_vim_word_char(chars[col + 1]) {
            col += 1;
        }

        if col >= chars.len() && self.cursor.line < self.lines.len().saturating_sub(1) {
            self.cursor.line += 1;
            self.cursor.column = 0;
            self.move_to_word_end_internal(); // Recurse
        } else {
            self.cursor.column = col.min(chars.len().saturating_sub(1).max(0));
        }
    }

    fn is_vim_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    // === Vim Operations ===

    fn vim_delete_char(&mut self) {
        if self.current_line_len() > 0 {
            self.save_undo_state();
            self.delete_forward();
        }
    }

    fn vim_delete_lines(&mut self, count: usize) {
        self.save_undo_state();
        let start_line = self.cursor.line;
        let end_line = (start_line + count).min(self.lines.len());
        let lines_to_delete = end_line - start_line;

        if self.lines.len() > lines_to_delete {
            for _ in 0..lines_to_delete {
                if start_line < self.lines.len() {
                    self.lines.remove(start_line);
                }
            }
            if self.cursor.line >= self.lines.len() {
                self.cursor.line = self.lines.len().saturating_sub(1);
            }
        } else {
            // Deleting all lines, leave one empty
            self.lines.clear();
            self.lines.push(String::new());
            self.cursor.line = 0;
        }
        self.cursor.column = 0;
        self.notify_change();
    }

    fn vim_change_lines(&mut self, count: usize) {
        self.save_undo_state();
        let start_line = self.cursor.line;
        let end_line = (start_line + count).min(self.lines.len());

        // Delete all but the first line, then clear the first
        for _ in (start_line + 1)..end_line {
            if start_line + 1 < self.lines.len() {
                self.lines.remove(start_line + 1);
            }
        }
        if let Some(line) = self.lines.get_mut(start_line) {
            line.clear();
        }
        self.cursor.column = 0;
        self.vim.enter_insert();
        self.notify_change();
    }

    fn vim_yank_lines(&mut self, count: usize, cx: &mut EventContext) {
        let start_line = self.cursor.line;
        let end_line = (start_line + count).min(self.lines.len());

        let mut text = String::new();
        for i in start_line..end_line {
            if let Some(line) = self.lines.get(i) {
                text.push_str(line);
                text.push('\n');
            }
        }

        if !text.is_empty() {
            self.vim.register = Some(text.clone());
            cx.write_clipboard(&text);
        }
    }

    fn vim_paste_after(&mut self, cx: &mut EventContext) {
        let text = self.vim.register.clone().or_else(|| cx.read_clipboard());

        if let Some(text) = text {
            self.save_undo_state();
            if text.ends_with('\n') {
                // Line paste - insert below current line
                let line = self.cursor.line;
                self.lines
                    .insert(line + 1, text.trim_end_matches('\n').to_string());
                self.cursor.line = line + 1;
                self.cursor.column = 0;
            } else {
                // Character paste - insert after cursor
                if self.cursor.column < self.current_line_len() {
                    self.cursor.column += 1;
                }
                self.insert_str(&text);
            }
            self.notify_change();
        }
    }

    fn vim_paste_before(&mut self, cx: &mut EventContext) {
        let text = self.vim.register.clone().or_else(|| cx.read_clipboard());

        if let Some(text) = text {
            self.save_undo_state();
            if text.ends_with('\n') {
                // Line paste - insert above current line
                let line = self.cursor.line;
                self.lines
                    .insert(line, text.trim_end_matches('\n').to_string());
                self.cursor.column = 0;
            } else {
                // Character paste - insert at cursor
                self.insert_str(&text);
            }
            self.notify_change();
        }
    }

    /// Delete text from start cursor to end cursor (exclusive)
    fn vim_delete_range(&mut self, start: Cursor, end: Cursor, cx: &mut EventContext) {
        self.save_undo_state();

        // Normalize so start <= end
        let (start, end) = if (start.line, start.column) <= (end.line, end.column) {
            (start, end)
        } else {
            (end, start)
        };

        // Extract and yank the text being deleted
        let deleted_text = self.extract_text_range(start, end);
        if !deleted_text.is_empty() {
            self.vim.register = Some(deleted_text.clone());
            cx.write_clipboard(&deleted_text);
        }

        // Delete the range
        if start.line == end.line {
            // Same line deletion
            if let Some(line) = self.lines.get_mut(start.line) {
                let chars: Vec<char> = line.chars().collect();
                let start_col = start.column.min(chars.len());
                let end_col = end.column.min(chars.len());
                let new_line: String = chars[..start_col]
                    .iter()
                    .chain(chars[end_col..].iter())
                    .collect();
                *line = new_line;
            }
        } else {
            // Multi-line deletion
            let start_line_text: String = self
                .lines
                .get(start.line)
                .map(|l| l.chars().take(start.column).collect())
                .unwrap_or_default();
            let end_line_text: String = self
                .lines
                .get(end.line)
                .map(|l| l.chars().skip(end.column).collect())
                .unwrap_or_default();

            // Remove lines between start and end
            for _ in start.line..=end.line.min(self.lines.len().saturating_sub(1)) {
                if start.line < self.lines.len() {
                    self.lines.remove(start.line);
                }
            }

            // Insert merged line
            let merged = format!("{}{}", start_line_text, end_line_text);
            if start.line <= self.lines.len() {
                self.lines.insert(start.line, merged);
            } else {
                self.lines.push(merged);
            }
        }

        self.cursor = start;
        // Clamp cursor to valid position
        self.cursor.line = self.cursor.line.min(self.lines.len().saturating_sub(1));
        self.cursor.column = self.cursor.column.min(self.current_line_len());
        self.notify_change();
    }

    /// Extract text between two cursor positions
    fn extract_text_range(&self, start: Cursor, end: Cursor) -> String {
        if start.line == end.line {
            self.lines
                .get(start.line)
                .map(|l| {
                    let chars: Vec<char> = l.chars().collect();
                    let start_col = start.column.min(chars.len());
                    let end_col = end.column.min(chars.len());
                    chars[start_col..end_col].iter().collect()
                })
                .unwrap_or_default()
        } else {
            let mut text = String::new();
            // First line (from start.column to end)
            if let Some(line) = self.lines.get(start.line) {
                let chars: Vec<char> = line.chars().collect();
                text.extend(chars.iter().skip(start.column));
                text.push('\n');
            }
            // Middle lines (full lines)
            for i in (start.line + 1)..end.line {
                if let Some(line) = self.lines.get(i) {
                    text.push_str(line);
                    text.push('\n');
                }
            }
            // Last line (from start to end.column)
            if let Some(line) = self.lines.get(end.line) {
                let chars: Vec<char> = line.chars().collect();
                let end_col = end.column.min(chars.len());
                text.extend(chars.iter().take(end_col));
            }
            text
        }
    }

    /// Change text from start cursor to end cursor (delete and enter insert mode)
    fn vim_change_range(&mut self, start: Cursor, end: Cursor, cx: &mut EventContext) {
        self.vim_delete_range(start, end, cx);
        self.vim.enter_insert();
    }

    /// Yank text from start cursor to end cursor
    fn vim_yank_range(&mut self, start: Cursor, end: Cursor, cx: &mut EventContext) {
        let (start, end) = if (start.line, start.column) <= (end.line, end.column) {
            (start, end)
        } else {
            (end, start)
        };

        let text = self.extract_text_range(start, end);
        if !text.is_empty() {
            self.vim.register = Some(text.clone());
            cx.write_clipboard(&text);
        }
    }

    /// Execute pending operator with a motion
    /// Returns true if an operator was executed, false if just a motion
    fn vim_execute_motion_with_operator(
        &mut self,
        motion: &str,
        count: usize,
        cx: &mut EventContext,
    ) -> bool {
        let pending_op = self.vim.pending_operator;

        if pending_op.is_none() {
            // No pending operator, just execute the motion
            return false;
        }

        // Record start position
        let start = self.cursor;

        // Execute the motion to find end position
        match motion {
            "w" => self.vim_word_forward(count),
            "b" => self.vim_word_backward(count),
            "e" => self.vim_word_end(count),
            "$" => self.vim_line_end(),
            "^" | "0" => self.vim_first_non_blank(),
            "}" => self.vim_paragraph_forward(count),
            "{" => self.vim_paragraph_backward(count),
            _ => return false,
        }

        let end = self.cursor;

        // Execute the operator on the range
        match pending_op {
            Some(PendingOperator::Delete) => {
                self.vim_delete_range(start, end, cx);
            }
            Some(PendingOperator::Change) => {
                self.vim_change_range(start, end, cx);
            }
            Some(PendingOperator::Yank) => {
                self.vim_yank_range(start, end, cx);
                // Restore cursor position after yank
                self.cursor = start;
            }
            None => unreachable!(),
        }

        self.vim.reset_pending();
        true
    }

    /// Get length of current line
    fn current_line_len(&self) -> usize {
        self.lines
            .get(self.cursor.line)
            .map_or(0, |l| l.chars().count())
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

    /// Insert a string at the current cursor position
    pub fn insert_str(&mut self, s: &str) {
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

    /// Select an entire line
    pub fn select_line(&mut self, line: usize) {
        if line < self.lines.len() {
            let start = Cursor::new(line, 0);
            let end = Cursor::new(line, self.line_len(line));
            self.selection = Some(Selection::new(start, end));
            self.cursor = end;
        }
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
                let start_byte = line.char_indices().nth(start.column).map_or(0, |(i, _)| i);
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

    /// Wrap a line of text to fit within the given width.
    /// Returns a vector of (start_col, text_segment) pairs.
    fn wrap_line(&self, line: &str, max_chars: usize) -> Vec<(usize, String)> {
        if max_chars == 0 || line.is_empty() {
            return vec![(0, line.to_string())];
        }

        let chars: Vec<char> = line.chars().collect();
        if chars.len() <= max_chars {
            return vec![(0, line.to_string())];
        }

        let mut segments = Vec::new();
        let mut start = 0;

        while start < chars.len() {
            let end = (start + max_chars).min(chars.len());

            // Try to break at a word boundary (space) if not at end
            let break_at = if end < chars.len() {
                // Look backwards for a space to break at
                let mut break_pos = end;
                while break_pos > start && !chars[break_pos - 1].is_whitespace() {
                    break_pos -= 1;
                }
                // If no space found, just break at max_chars
                if break_pos == start { end } else { break_pos }
            } else {
                end
            };

            let segment: String = chars[start..break_at].iter().collect();
            segments.push((start, segment));
            start = break_at;
        }

        if segments.is_empty() {
            segments.push((0, line.to_string()));
        }

        segments
    }

    /// Calculate max characters per line based on available width
    fn max_chars_per_line(&self, available_width: f32, char_width: f32) -> usize {
        if char_width <= 0.0 {
            return usize::MAX;
        }
        ((available_width / char_width).floor() as usize).max(1)
    }

    fn cursor_position_from_point(&self, x: f32, y: f32, bounds: &Bounds) -> Cursor {
        let line_height = self.style.font_size * self.style.line_height;
        let content_y = y - bounds.origin.y - self.style.padding + self.scroll_offset;

        // Center content with max width 768px (must match paint)
        let max_content_width = self.scaled_max_content_width();
        let content_width = bounds.size.width.min(max_content_width);
        let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
        let text_x = content_x + self.style.padding;

        // Calculate available width for text and max chars for wrapping
        let available_text_width = content_width - self.style.padding * 2.0;
        let max_chars = if self.style.wrap_text {
            self.max_chars_per_line(available_text_width, self.mono_char_width)
        } else {
            usize::MAX
        };

        // Calculate clicked visual row
        let clicked_visual_row = (content_y / line_height).floor() as usize;

        // Build wrapped line mapping to find logical line and column
        let mut visual_row = 0;

        for (line_idx, line) in self.lines.iter().enumerate() {
            // Add title margin (extra row) after line 0
            if line_idx == 1 {
                visual_row += 1;
            }

            let segments = if self.style.wrap_text {
                self.wrap_line(line, max_chars)
            } else {
                vec![(0, line.clone())]
            };

            for (start_col, _) in segments.iter() {
                if visual_row == clicked_visual_row {
                    // Calculate column within this segment
                    let mut parser = BlockParser::new();
                    for (i, l) in self.lines.iter().enumerate() {
                        if i == line_idx {
                            break;
                        }
                        parser.detect_block_type_at(l, i);
                    }
                    let char_width = match parser.detect_block_type_at(line, line_idx) {
                        BlockType::Header(level) => self.mono_char_width * header_font_scale(level),
                        _ => self.mono_char_width,
                    };

                    let relative_x = (x - text_x).max(0.0);
                    let col_in_segment = (relative_x / char_width).round() as usize;
                    let column = (*start_col + col_in_segment).min(self.line_len(line_idx));
                    return Cursor::new(line_idx, column);
                }
                visual_row += 1;
            }
        }

        // Fallback: clicked below all content, go to end of last line
        let last_line = self.lines.len().saturating_sub(1);
        Cursor::new(last_line, self.line_len(last_line))
    }

    fn ensure_cursor_visible(&mut self, bounds: &Bounds) {
        let line_height = self.style.font_size * self.style.line_height;
        let status_bar_height = 24.0;
        let visible_height = bounds.size.height - self.style.padding * 2.0 - status_bar_height;

        // Calculate available width for text and max chars for wrapping
        let max_content_width = self.scaled_max_content_width();
        let content_width = bounds.size.width.min(max_content_width);
        let available_text_width = content_width - self.style.padding * 2.0;
        let max_chars = if self.style.wrap_text {
            self.max_chars_per_line(available_text_width, self.mono_char_width)
        } else {
            usize::MAX
        };

        // Calculate cursor's visual row
        let mut cursor_visual_row = 0;
        for (line_idx, line) in self.lines.iter().enumerate() {
            if line_idx == 1 {
                cursor_visual_row += 1;
            }

            if line_idx == self.cursor.line {
                // Find which segment the cursor is in
                let segments = if self.style.wrap_text {
                    self.wrap_line(line, max_chars)
                } else {
                    vec![(0, line.clone())]
                };

                for (seg_idx, (start_col, segment)) in segments.iter().enumerate() {
                    let segment_end = start_col + segment.chars().count();
                    if self.cursor.column <= segment_end || seg_idx == segments.len() - 1 {
                        break;
                    }
                    cursor_visual_row += 1;
                }
                break;
            }

            let segments = if self.style.wrap_text {
                self.wrap_line(line, max_chars)
            } else {
                vec![(0, line.clone())]
            };
            cursor_visual_row += segments.len();
        }

        let cursor_y = cursor_visual_row as f32 * line_height;

        if cursor_y < self.scroll_offset {
            self.scroll_offset = cursor_y;
        } else if cursor_y + line_height > self.scroll_offset + visible_height {
            self.scroll_offset = cursor_y + line_height - visible_height;
        }
    }

    /// Render a line with markdown formatting
    /// `is_continuation` indicates this is a wrapped continuation (not the start of the line)
    #[expect(clippy::too_many_arguments)]
    fn render_formatted_line(
        &self,
        line: &str,
        block_type: BlockType,
        x: f32,
        y: f32,
        _line_height: f32,
        is_continuation: bool,
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
                // Only render bullet on first segment, not continuations
                let content = if is_continuation {
                    // Continuation: indent to align with content after bullet
                    current_x += self.mono_char_width * 2.0;
                    line
                } else {
                    // First segment: render bullet point then content
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
                    content
                };

                // Render content with inline formatting
                self.render_inline_formatted(content, current_x, y, cx);
            }

            BlockType::OrderedList => {
                // Only render number on first segment, not continuations
                if is_continuation {
                    // Continuation: indent to align with content after number
                    // Use a fixed indent (4 chars: "N. " padding)
                    current_x += self.mono_char_width * 4.0;
                    self.render_inline_formatted(line, current_x, y, cx);
                } else {
                    // First segment: render number then content
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
            }

            BlockType::Blockquote => {
                // Render blockquote bar and content
                let content = strip_blockquote_prefix(line);
                let bar_color = Hsla::new(210.0, 0.5, 0.5, 0.7);

                // Draw vertical bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        x,
                        y,
                        3.0,
                        self.style.font_size * self.style.line_height,
                    ))
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
        self.mono_char_width =
            cx.text
                .measure_styled_mono("M", self.style.font_size, FontStyle::default());

        // Background with configurable opacity
        if self.background_opacity > 0.0 {
            let bg = self.style.background;
            let bg_with_opacity = Hsla::new(bg.h, bg.s, bg.l, bg.a * self.background_opacity);
            cx.scene
                .draw_quad(Quad::new(bounds).with_background(bg_with_opacity));
        }

        let line_height = self.style.font_size * self.style.line_height;
        let status_bar_height = 24.0;
        let visible_height = bounds.size.height - self.style.padding * 2.0 - status_bar_height;

        // Center content with max width 768px
        let max_content_width = self.scaled_max_content_width();
        let content_width = bounds.size.width.min(max_content_width);
        let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
        let text_x = content_x + self.style.padding;

        // Calculate available width for text (content width minus padding on both sides)
        let available_text_width = content_width - self.style.padding * 2.0;
        let max_chars = if self.style.wrap_text {
            self.max_chars_per_line(available_text_width, self.mono_char_width)
        } else {
            usize::MAX
        };

        // Parse block types for all lines
        let mut block_parser = BlockParser::new();
        let block_types: Vec<BlockType> = self
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| block_parser.detect_block_type_at(line, i))
            .collect();

        // Compute wrapped lines info: (logical_line_idx, segment_start_col, segment_text, visual_row)
        let mut wrapped_info: Vec<(usize, usize, String, usize)> = Vec::new();
        let mut visual_row = 0;
        let mut cursor_visual_row = 0;
        let mut cursor_visual_col = 0;

        for (line_idx, line) in self.lines.iter().enumerate() {
            // Add title margin (extra row) after line 0
            if line_idx == 1 {
                visual_row += 1;
            }

            let segments = if self.style.wrap_text {
                self.wrap_line(line, max_chars)
            } else {
                vec![(0, line.clone())]
            };

            for (seg_idx, (start_col, segment)) in segments.iter().enumerate() {
                // Track cursor position in visual coordinates
                if line_idx == self.cursor.line && self.cursor.column >= *start_col {
                    let segment_end = start_col + segment.chars().count();
                    if self.cursor.column <= segment_end || seg_idx == segments.len() - 1 {
                        cursor_visual_row = visual_row;
                        cursor_visual_col = self.cursor.column - start_col;
                    }
                }

                wrapped_info.push((line_idx, *start_col, segment.clone(), visual_row));
                visual_row += 1;
            }
        }

        let total_visual_rows = visual_row;

        // Calculate visible row range
        let first_visible_row = (self.scroll_offset / line_height).floor() as usize;
        let visible_rows = (visible_height / line_height).ceil() as usize + 1;
        let last_visible_row = (first_visible_row + visible_rows).min(total_visual_rows);

        // Render visible wrapped segments
        for (line_idx, start_col, segment, vis_row) in wrapped_info.iter() {
            if *vis_row < first_visible_row || *vis_row >= last_visible_row {
                continue;
            }

            let y = bounds.origin.y + self.style.padding + (*vis_row as f32 * line_height)
                - self.scroll_offset;

            // Skip if outside visible area
            if y + line_height < bounds.origin.y || y > bounds.origin.y + bounds.size.height {
                continue;
            }

            let block_type = block_types
                .get(*line_idx)
                .copied()
                .unwrap_or(BlockType::Paragraph);
            let is_cursor_line = *line_idx == self.cursor.line;

            if segment.is_empty() {
                // Empty segment, nothing to render
            } else if is_cursor_line {
                // Cursor line: render raw markdown but keep font size for headers
                let font_size = match block_type {
                    BlockType::Header(level) => self.style.font_size * header_font_scale(level),
                    _ => self.style.font_size,
                };
                let text_run = cx.text.layout_styled_mono(
                    segment,
                    Point::new(text_x, y),
                    font_size,
                    self.style.text_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(text_run);
            } else {
                // Non-cursor line: render formatted markdown
                // is_continuation is true when start_col > 0 (wrapped segment, not start of line)
                let is_continuation = *start_col > 0;
                self.render_formatted_line(
                    segment,
                    block_type,
                    text_x,
                    y,
                    line_height,
                    is_continuation,
                    cx,
                );
            }

            // Selection highlight for this segment
            if let Some(sel) = &self.selection
                && !sel.is_empty()
            {
                let sel_start = sel.start();
                let sel_end = sel.end();

                if *line_idx >= sel_start.line && *line_idx <= sel_end.line {
                    let segment_end_col = start_col + segment.chars().count();

                    // Calculate selection range within this segment
                    let line_sel_start = if *line_idx == sel_start.line {
                        sel_start.column
                    } else {
                        0
                    };
                    let line_sel_end = if *line_idx == sel_end.line {
                        sel_end.column
                    } else {
                        self.line_len(*line_idx)
                    };

                    // Intersect with segment range
                    let seg_sel_start = line_sel_start.max(*start_col).saturating_sub(*start_col);
                    let seg_sel_end = line_sel_end.min(segment_end_col).saturating_sub(*start_col);

                    if seg_sel_start < seg_sel_end {
                        let char_width = match block_type {
                            BlockType::Header(level) => {
                                let scale = header_font_scale(level);
                                cx.text.measure_styled_mono(
                                    "M",
                                    self.style.font_size * scale,
                                    FontStyle::default(),
                                )
                            }
                            _ => self.mono_char_width,
                        };
                        let sel_x = text_x + seg_sel_start as f32 * char_width;
                        let sel_width = (seg_sel_end - seg_sel_start) as f32 * char_width;

                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(sel_x, y, sel_width, line_height))
                                .with_background(self.style.selection_color),
                        );
                    }
                }
            }
        }

        // Cursor with blinking (500ms on, 500ms off)
        if self.focused {
            let elapsed = self.cursor_blink_start.elapsed().as_millis();
            let cursor_visible = (elapsed / 500).is_multiple_of(2);

            if cursor_visible {
                let cursor_y =
                    bounds.origin.y + self.style.padding + (cursor_visual_row as f32 * line_height)
                        - self.scroll_offset;

                // Get cursor char width - scale for headers
                let cursor_block_type = block_types
                    .get(self.cursor.line)
                    .copied()
                    .unwrap_or(BlockType::Paragraph);
                let cursor_char_width = match cursor_block_type {
                    BlockType::Header(level) => {
                        let scale = header_font_scale(level);
                        cx.text.measure_styled_mono(
                            "M",
                            self.style.font_size * scale,
                            FontStyle::default(),
                        )
                    }
                    _ => self.mono_char_width,
                };

                let cursor_x = text_x + cursor_visual_col as f32 * cursor_char_width;
                // Shift cursor up slightly to align with text
                let cursor_offset_y = -2.0;

                // Block cursor in vim normal/visual mode, line cursor otherwise
                let (cursor_width, cursor_color) = if self.vim_enabled {
                    match self.vim.mode {
                        VimMode::Normal
                        | VimMode::Visual
                        | VimMode::VisualLine
                        | VimMode::VisualBlock => {
                            // Block cursor with semi-transparent background
                            (cursor_char_width, self.style.cursor_color.with_alpha(0.7))
                        }
                        VimMode::Insert | VimMode::Replace => {
                            // Line cursor
                            (2.0, self.style.cursor_color)
                        }
                    }
                } else {
                    // Standard line cursor when vim disabled
                    (2.0, self.style.cursor_color)
                };

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        cursor_x,
                        cursor_y + cursor_offset_y,
                        cursor_width,
                        line_height,
                    ))
                    .with_background(cursor_color),
                );
            }
        }

        // Scrollbar
        let total_content_height = total_visual_rows as f32 * line_height;
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
            let thumb_y = bounds.origin.y
                + self.style.padding
                + scroll_ratio * (visible_height - thumb_height);

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    scrollbar_x,
                    thumb_y,
                    scrollbar_width,
                    thumb_height,
                ))
                .with_background(Hsla::new(0.0, 0.0, 0.5, 0.5))
                .with_corner_radius(4.0),
            );
        }

        // Status bar at bottom
        let status_bar_y = bounds.origin.y + bounds.size.height - status_bar_height;
        let status_y = status_bar_y + 4.0;

        // Vim mode indicator (left side)
        if let Some(vim_mode) = self.vim_mode() {
            let mode_text = vim_mode.label();
            let mode_color = match vim_mode {
                VimMode::Normal => Hsla::new(210.0, 0.7, 0.6, 1.0), // Blue
                VimMode::Insert => Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                VimMode::Replace => Hsla::new(30.0, 0.7, 0.6, 1.0), // Orange
                VimMode::Visual | VimMode::VisualLine | VimMode::VisualBlock => {
                    Hsla::new(280.0, 0.6, 0.6, 1.0)
                } // Purple
            };

            let mode_x = bounds.origin.x + 12.0;
            let mode_run = cx.text.layout_styled_mono(
                mode_text,
                Point::new(mode_x, status_y),
                STATUS_BAR_FONT_SIZE,
                mode_color,
                FontStyle::default(),
            );
            cx.scene.draw_text(mode_run);
        }

        // Status message (center-left, after vim mode)
        if let Some((message, color)) = &self.status_message {
            let status_msg_x = bounds.origin.x + 100.0; // After vim mode indicator
            let status_msg_run = cx.text.layout_styled_mono(
                message,
                Point::new(status_msg_x, status_y),
                STATUS_BAR_FONT_SIZE,
                *color,
                FontStyle::default(),
            );
            cx.scene.draw_text(status_msg_run);
        }

        // Line:Col indicator (right side)
        let status_text = format!(
            "Ln {}, Col {}",
            self.cursor.line + 1,
            self.cursor.column + 1
        );
        let status_x = bounds.origin.x + bounds.size.width - 120.0;
        let status_run = cx.text.layout_styled_mono(
            &status_text,
            Point::new(status_x, status_y),
            STATUS_BAR_FONT_SIZE,
            Hsla::new(0.0, 0.0, 0.5, 1.0),
            FontStyle::default(),
        );
        cx.scene.draw_text(status_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseDown {
                button,
                x,
                y,
                modifiers,
            } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    self.focused = true;
                    self.cursor_blink_start = Instant::now();

                    let new_cursor = self.cursor_position_from_point(*x, *y, &bounds);

                    // Shift+click extends selection from current cursor to click position
                    if modifiers.shift {
                        let anchor = self
                            .selection
                            .as_ref()
                            .map(|s| s.anchor)
                            .unwrap_or(self.cursor);
                        self.selection = Some(Selection::new(anchor, new_cursor));
                        self.cursor = new_cursor;
                        self.is_dragging = true;
                        self.drag_start_pos = Some(anchor);
                        if let Some(id) = self.id {
                            cx.set_focus(id);
                        }
                        return EventResult::Handled;
                    }

                    // Detect double/triple click
                    let now = Instant::now();
                    let time_since_last = now.duration_since(self.last_click_time).as_millis();
                    let distance = ((x - self.last_click_pos.0).powi(2)
                        + (y - self.last_click_pos.1).powi(2))
                    .sqrt();

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
                    let status_bar_height = 24.0;
                    let visible_height =
                        bounds.size.height - self.style.padding * 2.0 - status_bar_height;

                    // Calculate total visual rows accounting for wrapped lines
                    let max_content_width = self.scaled_max_content_width();
                    let content_width = bounds.size.width.min(max_content_width);
                    let available_text_width = content_width - self.style.padding * 2.0;
                    let max_chars = if self.style.wrap_text {
                        self.max_chars_per_line(available_text_width, self.mono_char_width)
                    } else {
                        usize::MAX
                    };

                    let mut total_visual_rows = 0;
                    for (line_idx, line) in self.lines.iter().enumerate() {
                        if line_idx == 1 {
                            total_visual_rows += 1; // Title margin
                        }
                        let segments = if self.style.wrap_text {
                            self.wrap_line(line, max_chars)
                        } else {
                            vec![(0, line.clone())]
                        };
                        total_visual_rows += segments.len();
                    }

                    let total_content_height = total_visual_rows as f32 * line_height;
                    let max_scroll = (total_content_height - visible_height).max(0.0);
                    self.scroll_offset =
                        (self.scroll_offset - dy * line_height * 3.0).clamp(0.0, max_scroll);
                    return EventResult::Handled;
                }
            }

            InputEvent::KeyDown { key, modifiers } => {
                if !self.focused {
                    return EventResult::Ignored;
                }

                // Handle vim mode
                if self.vim_enabled {
                    let result = self.handle_vim_key(key, modifiers, &bounds, cx);
                    if result != EventResult::Ignored {
                        return result;
                    }
                    // If vim handler didn't consume the key, fall through to standard handling
                    // (only happens for some keys in insert mode)
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
