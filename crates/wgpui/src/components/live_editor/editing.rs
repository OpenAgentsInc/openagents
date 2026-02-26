impl LiveEditor {
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
}

#[cfg(test)]
mod editing_tests {
    use super::*;

    #[test]
    fn wrap_line_prefers_word_boundaries() {
        let editor = LiveEditor::new("alpha beta gamma");
        let wrapped = editor.wrap_line("alpha beta gamma", 6);
        assert!(!wrapped.is_empty());
        assert!(wrapped[0].1.trim_end().starts_with("alpha"));
    }
}
