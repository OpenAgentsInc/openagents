# Onyx Primitives

Component design guide for reusable WGPUI components built for Onyx.

## LiveEditor

Multi-line text editor with live markdown formatting. The core component of Onyx.

### Location

`crates/wgpui/src/components/live_editor/`

### API

```rust
pub struct LiveEditor {
    // Content
    buffer: TextBuffer,              // From editor crate

    // Cursor state
    cursor: Cursor,
    selection: Option<Selection>,

    // Parsing
    blocks: Vec<Block>,
    active_block_index: usize,

    // Rendering
    scroll_offset: f32,
    line_height: f32,

    // Styling
    style: LiveEditorStyle,

    // Callbacks
    on_change: Option<Box<dyn Fn(&str)>>,
    on_save: Option<Box<dyn Fn()>>,
}

impl LiveEditor {
    /// Create a new editor with initial content
    pub fn new(content: &str) -> Self;

    /// Set the content, replacing current buffer
    pub fn set_content(&mut self, content: &str);

    /// Get current content as string
    pub fn content(&self) -> String;

    /// Check if buffer has unsaved changes
    pub fn is_dirty(&self) -> bool;

    /// Mark buffer as clean (after save)
    pub fn mark_clean(&mut self);
}
```

### Styling

```rust
pub struct LiveEditorStyle {
    // Background
    pub background: Hsla,

    // Text
    pub text_color: Hsla,
    pub font_size: f32,
    pub line_height: f32,

    // Cursor
    pub cursor_color: Hsla,
    pub cursor_width: f32,

    // Selection
    pub selection_color: Hsla,

    // Gutter (line numbers)
    pub gutter_background: Hsla,
    pub gutter_text_color: Hsla,
    pub gutter_width: f32,

    // Markdown-specific
    pub header_sizes: [f32; 6],     // H1-H6 font sizes
    pub code_background: Hsla,
    pub code_font_size: f32,
    pub link_color: Hsla,
    pub bold_weight: u16,
}
```

### Events

```rust
impl Component for LiveEditor {
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            // Text input
            InputEvent::KeyDown { key: Key::Character(c), .. } => {
                self.insert_char(c);
                EventResult::Handled
            }

            // Cursor movement
            InputEvent::KeyDown { key: Key::Named(NamedKey::ArrowUp), .. } => {
                self.move_cursor_up();
                EventResult::Handled
            }

            // Selection
            InputEvent::KeyDown { key: Key::Named(NamedKey::ArrowRight), modifiers, .. }
                if modifiers.shift => {
                self.extend_selection_right();
                EventResult::Handled
            }

            // Save
            InputEvent::KeyDown { key: Key::Character("s"), modifiers, .. }
                if modifiers.ctrl => {
                if let Some(on_save) = &self.on_save {
                    on_save();
                }
                EventResult::Handled
            }

            // Mouse click to position cursor
            InputEvent::MouseDown { x, y, .. } => {
                let pos = self.position_from_point(*x, *y);
                self.set_cursor(pos);
                EventResult::Handled
            }

            _ => EventResult::Ignored
        }
    }
}
```

## Block Abstraction

How markdown blocks are identified and tracked.

### Block Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum BlockType {
    /// Plain text paragraph
    Paragraph,

    /// Header with level 1-6
    Header { level: u8 },

    /// Fenced code block with optional language
    CodeBlock { language: Option<String> },

    /// Unordered list (-, *, +)
    UnorderedList,

    /// Ordered list (1., 2., etc.)
    OrderedList,

    /// Blockquote (> prefixed)
    Blockquote,

    /// Horizontal rule (---, ***, ___)
    HorizontalRule,

    /// Empty line(s)
    BlankLines,
}
```

### Block Structure

```rust
#[derive(Debug, Clone)]
pub struct Block {
    /// Block type
    pub block_type: BlockType,

    /// Start line (0-indexed)
    pub start_line: usize,

    /// End line (exclusive)
    pub end_line: usize,

    /// Byte offset in buffer where block starts
    pub start_offset: usize,

    /// Byte offset in buffer where block ends
    pub end_offset: usize,

    /// Cached rendered height (for scrolling)
    pub rendered_height: Option<f32>,
}

impl Block {
    /// Check if a line number is within this block
    pub fn contains_line(&self, line: usize) -> bool {
        line >= self.start_line && line < self.end_line
    }

    /// Get the raw text content of this block
    pub fn content<'a>(&self, buffer: &'a TextBuffer) -> &'a str {
        buffer.slice(self.start_offset..self.end_offset)
    }
}
```

### Block Parser

```rust
pub struct BlockParser;

impl BlockParser {
    /// Parse buffer into blocks
    pub fn parse(buffer: &TextBuffer) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut current_line = 0;
        let mut current_offset = 0;

        for line in buffer.lines() {
            // Detect block boundaries based on line content
            // Headers: starts with #
            // Code blocks: starts with ```
            // Lists: starts with -, *, +, or number.
            // Blockquotes: starts with >
            // Blank: empty or whitespace only
            // Paragraph: everything else
        }

        blocks
    }

    /// Incrementally update blocks after edit
    pub fn update(
        blocks: &mut Vec<Block>,
        buffer: &TextBuffer,
        edit_start: usize,
        edit_end: usize,
    ) {
        // Find affected blocks
        // Reparse only those blocks
        // Adjust offsets for blocks after edit
    }
}
```

## Cursor Model

Line/column tracking with block awareness.

### Cursor Structure

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cursor {
    /// Line number (0-indexed)
    pub line: usize,

    /// Column number (0-indexed, in graphemes)
    pub column: usize,

    /// Preferred column (for up/down navigation)
    pub preferred_column: Option<usize>,

    /// Byte offset in buffer (for efficient operations)
    pub offset: usize,
}

impl Cursor {
    /// Create cursor at start of document
    pub fn start() -> Self {
        Self {
            line: 0,
            column: 0,
            preferred_column: None,
            offset: 0,
        }
    }

    /// Get which block this cursor is in
    pub fn block_index(&self, blocks: &[Block]) -> usize {
        blocks
            .iter()
            .position(|b| b.contains_line(self.line))
            .unwrap_or(0)
    }

    /// Move cursor, updating offset from line/column
    pub fn move_to(&mut self, line: usize, column: usize, buffer: &TextBuffer) {
        self.line = line;
        self.column = column;
        self.offset = buffer.line_to_byte(line) + column;
    }
}
```

### Cursor Movement

```rust
impl LiveEditor {
    pub fn move_cursor_left(&mut self) {
        if self.cursor.column > 0 {
            self.cursor.column -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.column = self.line_length(self.cursor.line);
        }
        self.update_cursor_offset();
        self.clear_selection();
    }

    pub fn move_cursor_right(&mut self) {
        let line_len = self.line_length(self.cursor.line);
        if self.cursor.column < line_len {
            self.cursor.column += 1;
        } else if self.cursor.line < self.line_count() - 1 {
            self.cursor.line += 1;
            self.cursor.column = 0;
        }
        self.update_cursor_offset();
        self.clear_selection();
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            let line_len = self.line_length(self.cursor.line);
            self.cursor.column = self.cursor.preferred_column
                .unwrap_or(self.cursor.column)
                .min(line_len);
        }
        self.update_cursor_offset();
        self.clear_selection();
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor.line < self.line_count() - 1 {
            self.cursor.line += 1;
            let line_len = self.line_length(self.cursor.line);
            self.cursor.column = self.cursor.preferred_column
                .unwrap_or(self.cursor.column)
                .min(line_len);
        }
        self.update_cursor_offset();
        self.clear_selection();
    }
}
```

## Selection Model

Range selection across blocks.

### Selection Structure

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    /// Selection anchor (where selection started)
    pub anchor: Cursor,

    /// Selection head (where cursor currently is)
    pub head: Cursor,
}

impl Selection {
    /// Get the start of the selection (earlier position)
    pub fn start(&self) -> Cursor {
        if self.anchor.offset <= self.head.offset {
            self.anchor
        } else {
            self.head
        }
    }

    /// Get the end of the selection (later position)
    pub fn end(&self) -> Cursor {
        if self.anchor.offset >= self.head.offset {
            self.anchor
        } else {
            self.head
        }
    }

    /// Check if selection is empty (anchor == head)
    pub fn is_empty(&self) -> bool {
        self.anchor.offset == self.head.offset
    }

    /// Get selected text from buffer
    pub fn text<'a>(&self, buffer: &'a TextBuffer) -> &'a str {
        let start = self.start().offset;
        let end = self.end().offset;
        buffer.slice(start..end)
    }
}
```

### Selection Operations

```rust
impl LiveEditor {
    /// Start a new selection at current cursor
    pub fn start_selection(&mut self) {
        self.selection = Some(Selection {
            anchor: self.cursor,
            head: self.cursor,
        });
    }

    /// Extend selection to current cursor
    pub fn extend_selection(&mut self) {
        if let Some(sel) = &mut self.selection {
            sel.head = self.cursor;
        }
    }

    /// Select all content
    pub fn select_all(&mut self) {
        let start = Cursor::start();
        let end = self.cursor_at_end();
        self.selection = Some(Selection {
            anchor: start,
            head: end,
        });
        self.cursor = end;
    }

    /// Delete selected text
    pub fn delete_selection(&mut self) {
        if let Some(sel) = self.selection.take() {
            if !sel.is_empty() {
                let start = sel.start().offset;
                let end = sel.end().offset;
                self.buffer.remove(start..end);
                self.cursor = sel.start();
                self.reparse_blocks();
            }
        }
    }
}
```

## Rendering Modes

Raw text vs formatted rendering per-block.

### Raw Text Rendering

For the active block (where cursor is):

```rust
fn render_raw_block(
    block: &Block,
    buffer: &TextBuffer,
    cursor: &Cursor,
    selection: Option<&Selection>,
    style: &LiveEditorStyle,
    cx: &mut PaintContext,
) {
    let content = block.content(buffer);
    let mut y = block_start_y;

    for (line_idx, line) in content.lines().enumerate() {
        let abs_line = block.start_line + line_idx;

        // Render line number in gutter
        render_line_number(abs_line + 1, y, style, cx);

        // Render raw text (monospace)
        let text_run = cx.text_system.layout(
            line,
            style.font_size,
            FontStyle::default(),
        );
        cx.scene.add_text_run(text_run, point(gutter_width, y));

        // Render cursor if on this line
        if abs_line == cursor.line {
            render_cursor(cursor.column, y, style, cx);
        }

        // Render selection highlight if applicable
        if let Some(sel) = selection {
            render_selection_on_line(abs_line, sel, y, style, cx);
        }

        y += style.line_height;
    }
}
```

### Formatted Rendering

For non-active blocks:

```rust
fn render_formatted_block(
    block: &Block,
    buffer: &TextBuffer,
    style: &LiveEditorStyle,
    cx: &mut PaintContext,
) {
    let content = block.content(buffer);

    match block.block_type {
        BlockType::Header { level } => {
            let font_size = style.header_sizes[level as usize - 1];
            let styled = parse_inline_formatting(content);
            render_styled_text(&styled, font_size, true, style, cx);
        }

        BlockType::Paragraph => {
            let styled = parse_inline_formatting(content);
            render_styled_text(&styled, style.font_size, false, style, cx);
        }

        BlockType::CodeBlock { ref language } => {
            // Background
            cx.scene.add_quad(Quad {
                bounds: code_block_bounds,
                background: style.code_background,
                ..Default::default()
            });

            // Syntax highlighted code
            if let Some(lang) = language {
                render_highlighted_code(content, lang, style, cx);
            } else {
                render_monospace_text(content, style, cx);
            }
        }

        BlockType::UnorderedList | BlockType::OrderedList => {
            render_list(content, &block.block_type, style, cx);
        }

        BlockType::Blockquote => {
            // Left border
            cx.scene.add_quad(Quad {
                bounds: Bounds { x: 0.0, y, width: 4.0, height: block_height },
                background: style.text_color.with_alpha(0.3),
                ..Default::default()
            });

            let styled = parse_inline_formatting(strip_quote_prefix(content));
            render_styled_text(&styled, style.font_size, false, style, cx);
        }

        _ => {}
    }
}
```

### Inline Formatting Parser

```rust
#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub link: Option<String>,
}

/// Parse inline markdown formatting (bold, italic, code, links)
pub fn parse_inline_formatting(text: &str) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut bold = false;
    let mut italic = false;

    // State machine parsing:
    // ** or __ -> toggle bold
    // * or _ -> toggle italic
    // ` -> code span
    // [text](url) -> link
    // ~~ -> strikethrough

    spans
}
```

## Integration Pattern

How other crates consume these components.

### Basic Usage

```rust
use wgpui::components::LiveEditor;

// Create editor with content
let mut editor = LiveEditor::new("# Hello World\n\nSome content.");

// Set callbacks
editor.on_change(|content| {
    println!("Content changed: {} chars", content.len());
});

editor.on_save(|| {
    println!("Save requested");
});

// In your app's paint method
impl Component for MyApp {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.editor.paint(bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        self.editor.event(event, bounds, cx)
    }
}
```

### With Document Integration

```rust
use document::{Document, Frontmatter};
use wgpui::components::LiveEditor;

pub struct DocumentEditor {
    document: Document,
    editor: LiveEditor,
}

impl DocumentEditor {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let document = Document::load(path)?;
        let editor = LiveEditor::new(&document.content);

        Ok(Self { document, editor })
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.document.content = self.editor.content();
        self.document.frontmatter.modified = Some(Utc::now());
        self.document.save()?;
        self.editor.mark_clean();
        Ok(())
    }
}
```

### FileTree Integration

```rust
use wgpui::components::{FileTree, FileTreeItem};

let items = vec![
    FileTreeItem::file("note1.md"),
    FileTreeItem::file("note2.md"),
    FileTreeItem::file("note3.md"),
];

let mut tree = FileTree::new(items);

tree.on_select(|path| {
    // Open file in editor
    open_document(path);
});

tree.on_create(|parent_path| {
    // Create new file dialog
});

tree.on_delete(|path| {
    // Confirm and delete
});
```

### TabBar Integration

```rust
use wgpui::components::{TabBar, Tab};

let tabs = vec![
    Tab::new("note1.md", false),  // (name, dirty)
    Tab::new("note2.md", true),   // Has unsaved changes
];

let mut tab_bar = TabBar::new(tabs);

tab_bar.on_select(|index| {
    // Switch to document
    switch_document(index);
});

tab_bar.on_close(|index| {
    // Prompt save if dirty, then close
    close_document(index);
});
```

## Theming

All components respect the global WGPUI theme.

```rust
use wgpui::theme::{Theme, set_global_theme};

// Components automatically use theme colors
let editor = LiveEditor::new("content");

// Override specific styles
editor.style.header_sizes = [32.0, 28.0, 24.0, 20.0, 18.0, 16.0];
editor.style.code_background = hsla(0.0, 0.0, 0.1, 1.0);
```

### Theme Tokens Used

| Token | Component Use |
|-------|---------------|
| `background` | Editor background |
| `surface` | Gutter background |
| `text.primary` | Main text color |
| `text.muted` | Line numbers |
| `accent.primary` | Cursor, links |
| `border.default` | Code block borders |
