//! Editor trait for vim integration

use std::ops::Range;

/// A position in a text buffer
pub trait Position: Clone + Copy + Ord + Eq {
    /// Get the line number (0-indexed)
    fn line(&self) -> usize;
    /// Get the column/character offset (0-indexed)
    fn column(&self) -> usize;
    /// Create a position from line and column
    fn new(line: usize, column: usize) -> Self;
}

/// Simple position implementation
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct SimplePosition {
    pub line: usize,
    pub column: usize,
}

impl Position for SimplePosition {
    fn line(&self) -> usize {
        self.line
    }

    fn column(&self) -> usize {
        self.column
    }

    fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl SimplePosition {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A range of positions in the buffer
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRange<P: Position> {
    pub start: P,
    pub end: P,
}

impl<P: Position> TextRange<P> {
    pub fn new(start: P, end: P) -> Self {
        if start <= end {
            Self { start, end }
        } else {
            Self {
                start: end,
                end: start,
            }
        }
    }

    /// Check if the range is empty
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Check if a position is within this range
    pub fn contains(&self, pos: P) -> bool {
        pos >= self.start && pos < self.end
    }
}

/// Result of a vim key handling
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyResult {
    /// Key was handled, editor should update display
    Handled,
    /// Key was not handled, let editor handle it
    Ignored,
    /// Mode changed (for UI updates)
    ModeChanged(crate::Mode),
    /// Text was changed (for undo grouping)
    TextChanged,
    /// Need to enter command mode (: prompt)
    EnterCommand,
    /// Need to enter search mode (/ or ? prompt)
    EnterSearch { forward: bool },
}

/// Keyboard key representation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Escape,
    Enter,
    Backspace,
    Delete,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Keyboard modifiers
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }

    pub fn any(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.cmd
    }
}

/// Character classification for word movement
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharClass {
    /// Whitespace
    Whitespace,
    /// Word character (alphanumeric or underscore)
    Word,
    /// Punctuation/symbol
    Punctuation,
    /// Line ending
    Newline,
}

impl CharClass {
    pub fn of(c: char) -> Self {
        if c == '\n' || c == '\r' {
            CharClass::Newline
        } else if c.is_whitespace() {
            CharClass::Whitespace
        } else if c.is_alphanumeric() || c == '_' {
            CharClass::Word
        } else {
            CharClass::Punctuation
        }
    }
}

/// Editor interface that vim operates on
///
/// This trait abstracts the editor operations needed for vim commands.
/// Implement this trait to add vim support to any editor.
pub trait VimEditor {
    /// Position type used by this editor
    type Pos: Position;

    // === Content queries ===

    /// Get total number of lines
    fn line_count(&self) -> usize;

    /// Get length of a line (excluding newline)
    fn line_len(&self, line: usize) -> usize;

    /// Get character at position (None if out of bounds)
    fn char_at(&self, pos: Self::Pos) -> Option<char>;

    /// Get text of a line
    fn line_text(&self, line: usize) -> &str;

    /// Get full text content
    fn text(&self) -> String;

    // === Cursor ===

    /// Get current cursor position
    fn cursor(&self) -> Self::Pos;

    /// Set cursor position
    fn set_cursor(&mut self, pos: Self::Pos);

    // === Selection ===

    /// Get current selection (None if no selection)
    fn selection(&self) -> Option<TextRange<Self::Pos>>;

    /// Set selection (None to clear)
    fn set_selection(&mut self, range: Option<TextRange<Self::Pos>>);

    /// Get selected text (None if no selection)
    fn selected_text(&self) -> Option<String>;

    // === Editing ===

    /// Insert text at position
    fn insert(&mut self, pos: Self::Pos, text: &str);

    /// Delete text in range
    fn delete(&mut self, range: TextRange<Self::Pos>);

    /// Replace text in range with new text
    fn replace(&mut self, range: TextRange<Self::Pos>, text: &str) {
        self.delete(range);
        self.insert(range.start, text);
    }

    // === Undo ===

    /// Begin an undo group (atomic operation)
    fn begin_undo_group(&mut self);

    /// End an undo group
    fn end_undo_group(&mut self);

    /// Undo last change
    fn undo(&mut self) -> bool;

    /// Redo last undo
    fn redo(&mut self) -> bool;

    // === Clipboard ===

    /// Read from system clipboard
    fn clipboard_read(&self) -> Option<String>;

    /// Write to system clipboard
    fn clipboard_write(&mut self, text: &str);

    // === Viewport (optional) ===

    /// Get visible line range (for H/M/L motions)
    fn visible_lines(&self) -> Range<usize> {
        0..self.line_count()
    }

    /// Get number of lines that fit in viewport
    fn viewport_lines(&self) -> usize {
        self.visible_lines().len()
    }

    /// Scroll to make line visible
    fn scroll_to_line(&mut self, _line: usize) {
        // Default: no-op
    }

    // === Helpers ===

    /// Get character class at position
    fn char_class_at(&self, pos: Self::Pos) -> CharClass {
        self.char_at(pos).map(CharClass::of).unwrap_or(CharClass::Newline)
    }

    /// Check if line is empty or only whitespace
    fn is_blank_line(&self, line: usize) -> bool {
        self.line_text(line).trim().is_empty()
    }

    /// Get word at position
    fn word_at(&self, pos: Self::Pos) -> Option<TextRange<Self::Pos>> {
        let line = self.line_text(pos.line());
        if line.is_empty() {
            return None;
        }

        let col = pos.column().min(line.len().saturating_sub(1));
        let chars: Vec<char> = line.chars().collect();

        if col >= chars.len() {
            return None;
        }

        // Find word boundaries
        let class = CharClass::of(chars[col]);
        if class == CharClass::Whitespace {
            return None;
        }

        let mut start = col;
        while start > 0 && CharClass::of(chars[start - 1]) == class {
            start -= 1;
        }

        let mut end = col;
        while end < chars.len() && CharClass::of(chars[end]) == class {
            end += 1;
        }

        Some(TextRange::new(
            Self::Pos::new(pos.line(), start),
            Self::Pos::new(pos.line(), end),
        ))
    }

    /// Clamp column to valid range for line (respecting vim column clamping)
    fn clamp_column(&self, line: usize, column: usize, allow_end: bool) -> usize {
        let len = self.line_len(line);
        if len == 0 {
            0
        } else if allow_end {
            column.min(len)
        } else {
            column.min(len.saturating_sub(1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_position() {
        let p1 = SimplePosition::new(0, 0);
        let p2 = SimplePosition::new(0, 5);
        let p3 = SimplePosition::new(1, 0);

        assert!(p1 < p2);
        assert!(p2 < p3);
        assert_eq!(p1.line(), 0);
        assert_eq!(p2.column(), 5);
    }

    #[test]
    fn test_text_range() {
        let start = SimplePosition::new(0, 0);
        let end = SimplePosition::new(0, 5);
        let range = TextRange::new(start, end);

        assert!(!range.is_empty());
        assert!(range.contains(SimplePosition::new(0, 2)));
        assert!(!range.contains(SimplePosition::new(0, 5)));
    }

    #[test]
    fn test_char_class() {
        assert_eq!(CharClass::of(' '), CharClass::Whitespace);
        assert_eq!(CharClass::of('\t'), CharClass::Whitespace);
        assert_eq!(CharClass::of('a'), CharClass::Word);
        assert_eq!(CharClass::of('_'), CharClass::Word);
        assert_eq!(CharClass::of('5'), CharClass::Word);
        assert_eq!(CharClass::of('.'), CharClass::Punctuation);
        assert_eq!(CharClass::of('\n'), CharClass::Newline);
    }
}
