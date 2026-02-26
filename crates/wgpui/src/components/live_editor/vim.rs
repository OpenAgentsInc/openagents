impl LiveEditor {
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
                        if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10)) {
                            self.vim.push_count_digit(digit as u8);
                        }
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
                        if self.vim.mode == VimMode::Visual {
                            self.vim.enter_normal();
                            self.selection = None;
                        } else {
                            self.vim.mode = VimMode::Visual;
                        }
                    }
                    "V" => {
                        if self.vim.mode == VimMode::VisualLine {
                            self.vim.enter_normal();
                            self.selection = None;
                        } else {
                            self.vim.mode = VimMode::VisualLine;
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
            if self.vim.mode == VimMode::VisualLine {
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
}

#[cfg(test)]
mod vim_tests {
    use super::*;

    #[test]
    fn vim_mode_labels_are_stable() {
        assert_eq!(VimMode::Normal.label(), "NORMAL");
        assert_eq!(VimMode::Insert.label(), "INSERT");
        assert_eq!(VimMode::VisualLine.label(), "V-LINE");
    }

    #[test]
    fn vim_state_count_defaults_to_one() {
        let mut state = VimState::new();
        assert_eq!(state.effective_count(), 1);
        state.push_count_digit(3);
        state.push_count_digit(2);
        assert_eq!(state.effective_count(), 32);
    }
}
