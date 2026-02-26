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

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::input::{Key, NamedKey};
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

    /// Set a status message to display in the status bar
    pub fn set_status(&mut self, message: &str, color: Hsla) {
        self.status_message = Some((message.to_string(), color));
    }

    /// Clear the status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

}
