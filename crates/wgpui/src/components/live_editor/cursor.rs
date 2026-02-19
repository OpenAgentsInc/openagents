//! Cursor management for LiveEditor

/// Cursor position in a document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Line number (0-indexed)
    pub line: usize,
    /// Column number (0-indexed, in characters)
    pub column: usize,
    /// Preferred column for vertical movement
    pub preferred_column: Option<usize>,
}

impl Cursor {
    /// Create a cursor at the start of the document
    pub fn start() -> Self {
        Self {
            line: 0,
            column: 0,
            preferred_column: None,
        }
    }

    /// Create a cursor at a specific position
    pub fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            preferred_column: None,
        }
    }

    /// Set preferred column (for up/down navigation)
    pub fn set_preferred_column(&mut self) {
        self.preferred_column = Some(self.column);
    }

    /// Clear preferred column
    pub fn clear_preferred_column(&mut self) {
        self.preferred_column = None;
    }
}

/// Text selection range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Selection anchor (where selection started)
    pub anchor: Cursor,
    /// Selection head (current cursor position)
    pub head: Cursor,
}

impl Selection {
    /// Create a new selection
    pub fn new(anchor: Cursor, head: Cursor) -> Self {
        Self { anchor, head }
    }

    /// Check if selection is empty
    pub fn is_empty(&self) -> bool {
        self.anchor.line == self.head.line && self.anchor.column == self.head.column
    }

    /// Get the start of the selection (earlier position)
    pub fn start(&self) -> Cursor {
        if self.anchor.line < self.head.line
            || (self.anchor.line == self.head.line && self.anchor.column <= self.head.column)
        {
            self.anchor
        } else {
            self.head
        }
    }

    /// Get the end of the selection (later position)
    pub fn end(&self) -> Cursor {
        if self.anchor.line > self.head.line
            || (self.anchor.line == self.head.line && self.anchor.column >= self.head.column)
        {
            self.anchor
        } else {
            self.head
        }
    }
}
