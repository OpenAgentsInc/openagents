use std::ops::Range;

use ropey::Rope;

use crate::caret::Position;

#[derive(Clone, Debug)]
pub struct TextBuffer {
    rope: Rope,
}

impl TextBuffer {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    pub fn line_text(&self, line_idx: usize) -> String {
        let idx = line_idx.min(self.line_count().saturating_sub(1));
        let mut line = self.rope.line(idx).to_string();
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        line
    }

    pub fn line_len(&self, line_idx: usize) -> usize {
        self.line_text(line_idx).chars().count()
    }

    pub fn position_to_char(&self, position: Position) -> usize {
        let line = position.line.min(self.line_count().saturating_sub(1));
        let line_start = self.rope.line_to_char(line);
        let line_len = self.line_len(line);
        let column = position.column.min(line_len);
        line_start + column
    }

    pub fn char_to_position(&self, char_idx: usize) -> Position {
        let idx = char_idx.min(self.len_chars());
        let line = self.rope.char_to_line(idx);
        let line_start = self.rope.line_to_char(line);
        let line_len = self.line_len(line);
        let column = idx.saturating_sub(line_start).min(line_len);
        Position { line, column }
    }

    pub fn slice(&self, range: Range<usize>) -> String {
        self.rope.slice(range).to_string()
    }

    pub fn insert(&mut self, char_idx: usize, text: &str) {
        let idx = char_idx.min(self.len_chars());
        self.rope.insert(idx, text);
    }

    pub fn remove(&mut self, range: Range<usize>) {
        let start = range.start.min(self.len_chars());
        let end = range.end.min(self.len_chars());
        if start < end {
            self.rope.remove(start..end);
        }
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new("")
    }
}
