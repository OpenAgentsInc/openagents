use std::ops::Range;

use crate::buffer::TextBuffer;
use crate::caret::{Caret, Position, SelectionRange};

#[derive(Clone, Debug)]
pub struct EditorSnapshot {
    pub text: String,
    pub cursors: Vec<Caret>,
}

#[derive(Clone, Debug)]
pub struct Editor {
    buffer: TextBuffer,
    cursors: Vec<Caret>,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    revision: u64,
}

impl Editor {
    pub fn new(text: &str) -> Self {
        let buffer = TextBuffer::new(text);
        Self {
            buffer,
            cursors: vec![Caret::new(Position::zero())],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
        }
    }

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(text);
        self.cursors = vec![Caret::new(Position::zero())];
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.bump_revision();
    }

    pub fn cursors(&self) -> &[Caret] {
        &self.cursors
    }

    pub fn set_cursor(&mut self, position: Position) {
        let clamped = self.clamp_position(position);
        self.cursors = vec![Caret::new(clamped)];
    }

    pub fn add_cursor(&mut self, position: Position) {
        let clamped = self.clamp_position(position);
        self.cursors.push(Caret::new(clamped));
        self.dedupe_cursors();
    }

    pub fn set_cursors(&mut self, cursors: Vec<Caret>) {
        if cursors.is_empty() {
            self.cursors = vec![Caret::new(Position::zero())];
        } else {
            self.cursors = cursors
                .into_iter()
                .map(|mut c| {
                    let pos = self.clamp_position(c.position);
                    let anchor = self.clamp_position(c.anchor);
                    c.position = pos;
                    c.anchor = anchor;
                    c.preferred_column = pos.column;
                    c
                })
                .collect();
            self.normalize_cursors();
        }
    }

    pub fn selection_ranges(&self) -> Vec<Range<usize>> {
        self.merged_ranges()
    }

    pub fn selected_text(&self) -> Option<String> {
        let ranges = self.merged_ranges();
        if ranges.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for range in ranges {
            parts.push(self.buffer.slice(range));
        }
        Some(parts.join("\n"))
    }

    pub fn select_all(&mut self) {
        let end = self.buffer.char_to_position(self.buffer.len_chars());
        let mut cursor = Caret::new(Position::zero());
        cursor.position = end;
        cursor.anchor = Position::zero();
        cursor.preferred_column = end.column;
        self.cursors = vec![cursor];
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.push_undo();
        self.normalize_cursors();

        let mut ranges = self.cursor_ranges();
        ranges.sort_by(|a, b| b.start.cmp(&a.start));
        let insert_len = text.chars().count();

        for range in ranges {
            if range.start != range.end {
                self.buffer.remove(range.start..range.end);
            }
            self.buffer.insert(range.start, text);
            let new_pos = self.buffer.char_to_position(range.start + insert_len);
            let cursor = &mut self.cursors[range.index];
            cursor.position = new_pos;
            cursor.anchor = new_pos;
            cursor.preferred_column = new_pos.column;
        }

        self.dedupe_cursors();
        self.bump_revision();
    }

    pub fn insert_newline(&mut self) {
        self.push_undo();
        self.normalize_cursors();

        let mut ranges = self.cursor_ranges();
        ranges.sort_by(|a, b| b.start.cmp(&a.start));

        for range in ranges {
            let cursor_line = self.cursors[range.index].position.line;
            let indent = self.indent_for_line(cursor_line);
            let insert_text = format!("\n{}", indent);
            let insert_len = insert_text.chars().count();

            if range.start != range.end {
                self.buffer.remove(range.start..range.end);
            }
            self.buffer.insert(range.start, &insert_text);
            let new_pos = self.buffer.char_to_position(range.start + insert_len);
            let cursor = &mut self.cursors[range.index];
            cursor.position = new_pos;
            cursor.anchor = new_pos;
            cursor.preferred_column = new_pos.column;
        }

        self.dedupe_cursors();
        self.bump_revision();
    }

    pub fn delete_backward(&mut self) {
        self.push_undo();
        self.normalize_cursors();
        let mut changed = false;

        let mut ranges = self.cursor_ranges();
        ranges.sort_by(|a, b| b.start.cmp(&a.start));

        for range in ranges {
            let cursor = &mut self.cursors[range.index];
            let target_range = if range.start != range.end {
                range.start..range.end
            } else if range.start > 0 {
                (range.start - 1)..range.start
            } else {
                continue;
            };

            self.buffer.remove(target_range.clone());
            changed = true;
            let new_pos = self.buffer.char_to_position(target_range.start);
            cursor.position = new_pos;
            cursor.anchor = new_pos;
            cursor.preferred_column = new_pos.column;
        }

        self.dedupe_cursors();
        if changed {
            self.bump_revision();
        }
    }

    pub fn delete_forward(&mut self) {
        self.push_undo();
        self.normalize_cursors();
        let mut changed = false;

        let mut ranges = self.cursor_ranges();
        ranges.sort_by(|a, b| b.start.cmp(&a.start));

        for range in ranges {
            let cursor = &mut self.cursors[range.index];
            let target_range = if range.start != range.end {
                range.start..range.end
            } else if range.start < self.buffer.len_chars() {
                range.start..(range.start + 1)
            } else {
                continue;
            };

            self.buffer.remove(target_range.clone());
            changed = true;
            let new_pos = self.buffer.char_to_position(target_range.start);
            cursor.position = new_pos;
            cursor.anchor = new_pos;
            cursor.preferred_column = new_pos.column;
        }

        self.dedupe_cursors();
        if changed {
            self.bump_revision();
        }
    }

    pub fn move_left(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            if cursor.is_selection() && !select {
                cursor.collapse_to_start(&self.buffer);
                continue;
            }
            let idx = self.buffer.position_to_char(cursor.position);
            if idx == 0 {
                if !select {
                    cursor.anchor = cursor.position;
                }
                continue;
            }
            let new_pos = self.buffer.char_to_position(idx - 1);
            cursor.position = new_pos;
            cursor.preferred_column = new_pos.column;
            if !select {
                cursor.anchor = new_pos;
            }
        }
        self.dedupe_cursors();
    }

    pub fn move_right(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            if cursor.is_selection() && !select {
                cursor.collapse_to_end(&self.buffer);
                continue;
            }
            let idx = self.buffer.position_to_char(cursor.position);
            if idx >= self.buffer.len_chars() {
                if !select {
                    cursor.anchor = cursor.position;
                }
                continue;
            }
            let new_pos = self.buffer.char_to_position(idx + 1);
            cursor.position = new_pos;
            cursor.preferred_column = new_pos.column;
            if !select {
                cursor.anchor = new_pos;
            }
        }
        self.dedupe_cursors();
    }

    pub fn move_up(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            let line = cursor.position.line;
            if line == 0 {
                if !select {
                    cursor.anchor = cursor.position;
                }
                continue;
            }
            let new_line = line - 1;
            let target_col = cursor.preferred_column;
            let line_len = self.buffer.line_len(new_line);
            let new_col = target_col.min(line_len);
            let new_pos = Position {
                line: new_line,
                column: new_col,
            };
            cursor.position = new_pos;
            if !select {
                cursor.anchor = new_pos;
            }
        }
        self.dedupe_cursors();
    }

    pub fn move_down(&mut self, select: bool) {
        let last_line = self.buffer.line_count().saturating_sub(1);
        for cursor in &mut self.cursors {
            let line = cursor.position.line;
            if line >= last_line {
                if !select {
                    cursor.anchor = cursor.position;
                }
                continue;
            }
            let new_line = line + 1;
            let target_col = cursor.preferred_column;
            let line_len = self.buffer.line_len(new_line);
            let new_col = target_col.min(line_len);
            let new_pos = Position {
                line: new_line,
                column: new_col,
            };
            cursor.position = new_pos;
            if !select {
                cursor.anchor = new_pos;
            }
        }
        self.dedupe_cursors();
    }

    pub fn move_line_start(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            let new_pos = Position {
                line: cursor.position.line,
                column: 0,
            };
            cursor.position = new_pos;
            cursor.preferred_column = new_pos.column;
            if !select {
                cursor.anchor = new_pos;
            }
        }
        self.dedupe_cursors();
    }

    pub fn move_line_end(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            let line_len = self.buffer.line_len(cursor.position.line);
            let new_pos = Position {
                line: cursor.position.line,
                column: line_len,
            };
            cursor.position = new_pos;
            cursor.preferred_column = new_pos.column;
            if !select {
                cursor.anchor = new_pos;
            }
        }
        self.dedupe_cursors();
    }

    pub fn move_doc_start(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            cursor.position = Position::zero();
            cursor.preferred_column = 0;
            if !select {
                cursor.anchor = cursor.position;
            }
        }
        self.dedupe_cursors();
    }

    pub fn move_doc_end(&mut self, select: bool) {
        let end = self.buffer.char_to_position(self.buffer.len_chars());
        for cursor in &mut self.cursors {
            cursor.position = end;
            cursor.preferred_column = end.column;
            if !select {
                cursor.anchor = cursor.position;
            }
        }
        self.dedupe_cursors();
    }

    pub fn add_cursor_above(&mut self) {
        let mut new_cursors = Vec::new();
        for cursor in &self.cursors {
            if cursor.position.line == 0 {
                continue;
            }
            let new_line = cursor.position.line - 1;
            let line_len = self.buffer.line_len(new_line);
            let new_col = cursor.preferred_column.min(line_len);
            let mut new_cursor = Caret::new(Position {
                line: new_line,
                column: new_col,
            });
            new_cursor.preferred_column = cursor.preferred_column;
            new_cursors.push(new_cursor);
        }
        self.cursors.extend(new_cursors);
        self.normalize_cursors();
    }

    pub fn add_cursor_below(&mut self) {
        let last_line = self.buffer.line_count().saturating_sub(1);
        let mut new_cursors = Vec::new();
        for cursor in &self.cursors {
            if cursor.position.line >= last_line {
                continue;
            }
            let new_line = cursor.position.line + 1;
            let line_len = self.buffer.line_len(new_line);
            let new_col = cursor.preferred_column.min(line_len);
            let mut new_cursor = Caret::new(Position {
                line: new_line,
                column: new_col,
            });
            new_cursor.preferred_column = cursor.preferred_column;
            new_cursors.push(new_cursor);
        }
        self.cursors.extend(new_cursors);
        self.normalize_cursors();
    }

    pub fn undo(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        let current = self.snapshot();
        self.redo_stack.push(current);
        self.restore(snapshot);
    }

    pub fn redo(&mut self) {
        let Some(snapshot) = self.redo_stack.pop() else {
            return;
        };
        let current = self.snapshot();
        self.undo_stack.push(current);
        self.restore(snapshot);
    }

    pub fn selection_ranges_by_line(&self) -> Vec<SelectionRange> {
        let mut ranges = Vec::new();
        for cursor in &self.cursors {
            if let Some(range) = cursor.selection_range(&self.buffer) {
                ranges.push(range);
            }
        }
        ranges
    }

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            text: self.buffer.text(),
            cursors: self.cursors.clone(),
        }
    }

    fn restore(&mut self, snapshot: EditorSnapshot) {
        self.buffer.set_text(&snapshot.text);
        self.cursors = if snapshot.cursors.is_empty() {
            vec![Caret::new(Position::zero())]
        } else {
            snapshot.cursors
        };
        self.normalize_cursors();
        self.bump_revision();
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn clamp_position(&self, position: Position) -> Position {
        let line = position
            .line
            .min(self.buffer.line_count().saturating_sub(1));
        let line_len = self.buffer.line_len(line);
        let column = position.column.min(line_len);
        Position { line, column }
    }

    fn indent_for_line(&self, line: usize) -> String {
        let text = self.buffer.line_text(line);
        text.chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect()
    }

    fn merged_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges: Vec<Range<usize>> = self
            .cursor_ranges()
            .into_iter()
            .filter_map(|range| {
                if range.start == range.end {
                    None
                } else {
                    Some(range.start..range.end)
                }
            })
            .collect();

        if ranges.is_empty() {
            return ranges;
        }

        ranges.sort_by(|a, b| a.start.cmp(&b.start));
        let mut merged = Vec::new();
        let mut current = ranges[0].clone();

        for range in ranges.into_iter().skip(1) {
            if range.start <= current.end {
                current.end = current.end.max(range.end);
            } else {
                merged.push(current);
                current = range;
            }
        }
        merged.push(current);
        merged
    }

    fn cursor_ranges(&self) -> Vec<CursorRange> {
        let mut ranges = Vec::new();
        for (index, cursor) in self.cursors.iter().enumerate() {
            let start = self.buffer.position_to_char(cursor.anchor);
            let end = self.buffer.position_to_char(cursor.position);
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            ranges.push(CursorRange { index, start, end });
        }
        ranges
    }

    fn normalize_cursors(&mut self) {
        let mut ranges = self.cursor_ranges();
        if ranges.is_empty() {
            self.cursors = vec![Caret::new(Position::zero())];
            return;
        }

        ranges.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            if let Some(last) = merged.last_mut() {
                if range.start <= last.end {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            merged.push(range.start..range.end);
        }

        let mut cursors = Vec::new();
        for range in merged {
            let start_pos = self.buffer.char_to_position(range.start);
            let end_pos = self.buffer.char_to_position(range.end);
            let mut cursor = Caret::new(start_pos);
            if range.start != range.end {
                cursor.position = end_pos;
                cursor.anchor = start_pos;
            } else {
                cursor.position = start_pos;
                cursor.anchor = start_pos;
            }
            cursor.preferred_column = cursor.position.column;
            cursors.push(cursor);
        }

        if cursors.is_empty() {
            cursors.push(Caret::new(Position::zero()));
        }
        self.cursors = cursors;
    }

    fn dedupe_cursors(&mut self) {
        let mut ranges = self.cursor_ranges();
        ranges.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
        let mut cursors = Vec::new();
        let mut last_range: Option<(usize, usize)> = None;
        for range in ranges {
            if last_range == Some((range.start, range.end)) {
                continue;
            }
            last_range = Some((range.start, range.end));
            if let Some(cursor) = self.cursors.get(range.index).cloned() {
                cursors.push(cursor);
            }
        }
        if cursors.is_empty() {
            cursors.push(Caret::new(Position::zero()));
        }
        self.cursors = cursors;
    }
}

#[derive(Clone, Debug)]
struct CursorRange {
    index: usize,
    start: usize,
    end: usize,
}
