use std::cmp::{max, min};

use crate::buffer::TextBuffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn zero() -> Self {
        Self { line: 0, column: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caret {
    pub position: Position,
    pub anchor: Position,
    pub preferred_column: usize,
}

impl Caret {
    pub fn new(position: Position) -> Self {
        Self {
            position,
            anchor: position,
            preferred_column: position.column,
        }
    }

    pub fn is_selection(&self) -> bool {
        self.position != self.anchor
    }

    pub fn selection_range(&self, buffer: &TextBuffer) -> Option<SelectionRange> {
        if !self.is_selection() {
            return None;
        }

        let start_char = buffer.position_to_char(self.anchor);
        let end_char = buffer.position_to_char(self.position);
        let (start_char, end_char) = if start_char <= end_char {
            (start_char, end_char)
        } else {
            (end_char, start_char)
        };

        let start = buffer.char_to_position(start_char);
        let end = buffer.char_to_position(end_char);
        Some(SelectionRange { start, end })
    }

    pub fn collapse_to_start(&mut self, buffer: &TextBuffer) {
        let start = self.selection_start(buffer);
        self.position = start;
        self.anchor = start;
        self.preferred_column = self.position.column;
    }

    pub fn collapse_to_end(&mut self, buffer: &TextBuffer) {
        let end = self.selection_end(buffer);
        self.position = end;
        self.anchor = end;
        self.preferred_column = self.position.column;
    }

    pub fn selection_start(&self, buffer: &TextBuffer) -> Position {
        let start_char = min(
            buffer.position_to_char(self.position),
            buffer.position_to_char(self.anchor),
        );
        buffer.char_to_position(start_char)
    }

    pub fn selection_end(&self, buffer: &TextBuffer) -> Position {
        let end_char = max(
            buffer.position_to_char(self.position),
            buffer.position_to_char(self.anchor),
        );
        buffer.char_to_position(end_char)
    }
}
