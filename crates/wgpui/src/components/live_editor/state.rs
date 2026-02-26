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

/// Local vim-mode enum used by LiveEditor.
///
/// This preserves the existing editing-mode behavior while decoupling
/// `wgpui` from the archived standalone `vim` crate.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub enum VimMode {
    #[default]
    Normal,
    Insert,
    Replace,
    Visual,
    VisualLine,
    VisualBlock,
}

impl VimMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Replace => "REPLACE",
            Self::Visual => "VISUAL",
            Self::VisualLine => "V-LINE",
            Self::VisualBlock => "V-BLOCK",
        }
    }
}

/// Pending operator waiting for a motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingOperator {
    Delete,
    Change,
    Yank,
}

/// Vim state for the editor
#[derive(Debug, Clone, Default)]
pub struct VimState {
    /// Current vim mode
    pub mode: VimMode,
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
        self.mode = VimMode::Insert;
        self.reset_pending();
    }

    pub fn enter_normal(&mut self) {
        self.mode = VimMode::Normal;
        self.reset_pending();
        self.visual_anchor = None;
    }

    pub fn enter_visual(&mut self, anchor: Cursor) {
        self.mode = VimMode::Visual;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }

    pub fn enter_visual_line(&mut self, anchor: Cursor) {
        self.mode = VimMode::VisualLine;
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

}
