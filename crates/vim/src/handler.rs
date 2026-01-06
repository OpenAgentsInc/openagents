//! Vim key handler - dispatches keys to appropriate mode handlers

use crate::{
    editor::{CharClass, Key, KeyResult, Modifiers, Position, TextRange, VimEditor},
    Mode, Motion, Object, Operator, VimState,
};

/// Vim handler that processes keys and updates editor state
pub struct VimHandler {
    /// Core vim state
    pub state: VimState,
}

impl Default for VimHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl VimHandler {
    /// Create a new vim handler
    pub fn new() -> Self {
        Self {
            state: VimState::new(),
        }
    }

    /// Get current mode
    pub fn mode(&self) -> Mode {
        self.state.mode
    }

    /// Get vim state reference
    pub fn state(&self) -> &VimState {
        &self.state
    }

    /// Get mutable vim state reference
    pub fn state_mut(&mut self) -> &mut VimState {
        &mut self.state
    }

    /// Handle a key press
    pub fn handle_key<E: VimEditor>(&mut self, key: Key, mods: Modifiers, editor: &mut E) -> KeyResult {
        match self.state.mode {
            Mode::Normal => self.handle_normal(key, mods, editor),
            Mode::Insert => self.handle_insert(key, mods, editor),
            Mode::Replace => self.handle_replace(key, mods, editor),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                self.handle_visual(key, mods, editor)
            }
        }
    }

    /// Handle key in normal mode
    fn handle_normal<E: VimEditor>(&mut self, key: Key, mods: Modifiers, editor: &mut E) -> KeyResult {
        // Handle Ctrl combinations first
        if mods.ctrl {
            return self.handle_ctrl_key(key, editor);
        }

        match key {
            Key::Escape => {
                self.state.reset_pending();
                KeyResult::Handled
            }

            Key::Char(c) => self.handle_normal_char(c, editor),

            // Arrow keys as motions
            Key::Left => {
                self.execute_motion(Motion::Left, editor);
                KeyResult::Handled
            }
            Key::Right => {
                self.execute_motion(Motion::Right, editor);
                KeyResult::Handled
            }
            Key::Up => {
                self.execute_motion(Motion::Up, editor);
                KeyResult::Handled
            }
            Key::Down => {
                self.execute_motion(Motion::Down, editor);
                KeyResult::Handled
            }
            Key::Home => {
                self.execute_motion(Motion::LineStart, editor);
                KeyResult::Handled
            }
            Key::End => {
                self.execute_motion(Motion::LineEnd, editor);
                KeyResult::Handled
            }

            _ => KeyResult::Ignored,
        }
    }

    /// Handle normal mode character
    fn handle_normal_char<E: VimEditor>(&mut self, c: char, editor: &mut E) -> KeyResult {
        // Check for pending g prefix
        if self.state.pending_keys == "g" {
            self.state.pending_keys.clear();
            return self.handle_g_command(c, editor);
        }

        // Check for pending find character
        if let Some(op) = self.state.active_operator() {
            match op {
                Operator::FindForward { before } => {
                    let before = *before;
                    self.state.pop_operator();
                    let motion = Motion::FindChar { char: c, before };
                    self.state.last_find = Some(motion.clone());
                    self.execute_motion(motion, editor);
                    return KeyResult::Handled;
                }
                Operator::FindBackward { after } => {
                    let after = *after;
                    self.state.pop_operator();
                    let motion = Motion::FindCharBackward { char: c, after };
                    self.state.last_find = Some(motion.clone());
                    self.execute_motion(motion, editor);
                    return KeyResult::Handled;
                }
                Operator::Replace => {
                    self.state.pop_operator();
                    self.replace_char(c, editor);
                    return KeyResult::TextChanged;
                }
                Operator::Register => {
                    self.state.pop_operator();
                    self.state.selected_register = Some(c);
                    return KeyResult::Handled;
                }
                Operator::Mark => {
                    self.state.pop_operator();
                    let cursor = editor.cursor();
                    self.state.set_mark(c, (cursor.line(), cursor.column()));
                    return KeyResult::Handled;
                }
                _ => {}
            }
        }

        // Check for pending text object (i/a)
        if let Some(Operator::Object { around }) = self.state.active_operator().cloned() {
            if let Some(object) = Object::from_char(c) {
                self.state.pop_operator();
                return self.execute_object(object, around, editor);
            } else {
                self.state.reset_pending();
                return KeyResult::Handled;
            }
        }

        // Count digits (0 is special - goes to line start unless part of count)
        if c.is_ascii_digit() && (c != '0' || self.state.count.is_some()) {
            self.state.push_count_digit(c as u8 - b'0');
            return KeyResult::Handled;
        }

        match c {
            // Text object selection (must be before mode changes)
            'i' if self.state.has_pending_operator() => {
                self.state.push_operator(Operator::Object { around: false });
                KeyResult::Handled
            }
            'a' if self.state.has_pending_operator() => {
                self.state.push_operator(Operator::Object { around: true });
                KeyResult::Handled
            }

            // Mode changes
            'i' => {
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'I' => {
                self.execute_motion(Motion::FirstNonBlank, editor);
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'a' => {
                // Move right one character, then insert
                let cursor = editor.cursor();
                let line_len = editor.line_len(cursor.line());
                if cursor.column() < line_len {
                    editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + 1));
                }
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'A' => {
                self.execute_motion(Motion::LineEnd, editor);
                let cursor = editor.cursor();
                let line_len = editor.line_len(cursor.line());
                editor.set_cursor(E::Pos::new(cursor.line(), line_len));
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'o' => {
                // Open line below
                let cursor = editor.cursor();
                let line_len = editor.line_len(cursor.line());
                editor.set_cursor(E::Pos::new(cursor.line(), line_len));
                editor.insert(E::Pos::new(cursor.line(), line_len), "\n");
                editor.set_cursor(E::Pos::new(cursor.line() + 1, 0));
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'O' => {
                // Open line above
                let cursor = editor.cursor();
                editor.set_cursor(E::Pos::new(cursor.line(), 0));
                editor.insert(E::Pos::new(cursor.line(), 0), "\n");
                editor.set_cursor(E::Pos::new(cursor.line(), 0));
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'R' => {
                self.state.enter_replace();
                KeyResult::ModeChanged(Mode::Replace)
            }

            // Visual modes
            'v' => {
                let cursor = editor.cursor();
                self.state.enter_visual((cursor.line(), cursor.column()));
                KeyResult::ModeChanged(Mode::Visual)
            }
            'V' => {
                let cursor = editor.cursor();
                self.state.enter_visual_line((cursor.line(), cursor.column()));
                KeyResult::ModeChanged(Mode::VisualLine)
            }

            // Basic motions
            'h' => {
                self.execute_motion(Motion::Left, editor);
                KeyResult::Handled
            }
            'j' => {
                self.execute_motion(Motion::Down, editor);
                KeyResult::Handled
            }
            'k' => {
                self.execute_motion(Motion::Up, editor);
                KeyResult::Handled
            }
            'l' => {
                self.execute_motion(Motion::Right, editor);
                KeyResult::Handled
            }

            // Line motions
            '0' => {
                self.execute_motion(Motion::LineStart, editor);
                KeyResult::Handled
            }
            '^' => {
                self.execute_motion(Motion::FirstNonBlank, editor);
                KeyResult::Handled
            }
            '$' => {
                self.execute_motion(Motion::LineEnd, editor);
                KeyResult::Handled
            }

            // Word motions
            'w' => {
                self.execute_motion(
                    Motion::NextWordStart {
                        ignore_punctuation: false,
                    },
                    editor,
                );
                KeyResult::Handled
            }
            'W' => {
                self.execute_motion(
                    Motion::NextWordStart {
                        ignore_punctuation: true,
                    },
                    editor,
                );
                KeyResult::Handled
            }
            'e' => {
                self.execute_motion(
                    Motion::NextWordEnd {
                        ignore_punctuation: false,
                    },
                    editor,
                );
                KeyResult::Handled
            }
            'E' => {
                self.execute_motion(
                    Motion::NextWordEnd {
                        ignore_punctuation: true,
                    },
                    editor,
                );
                KeyResult::Handled
            }
            'b' => {
                self.execute_motion(
                    Motion::PrevWordStart {
                        ignore_punctuation: false,
                    },
                    editor,
                );
                KeyResult::Handled
            }
            'B' => {
                self.execute_motion(
                    Motion::PrevWordStart {
                        ignore_punctuation: true,
                    },
                    editor,
                );
                KeyResult::Handled
            }

            // Paragraph motions
            '{' => {
                self.execute_motion(Motion::ParagraphBackward, editor);
                KeyResult::Handled
            }
            '}' => {
                self.execute_motion(Motion::ParagraphForward, editor);
                KeyResult::Handled
            }

            // Document motions
            'G' => {
                let line = self.state.count.map(|c| c.saturating_sub(1));
                self.execute_motion(Motion::DocumentEnd { line }, editor);
                self.state.reset_pending();
                KeyResult::Handled
            }
            'g' => {
                self.state.pending_keys = "g".to_string();
                KeyResult::Handled
            }

            // Find motions
            'f' => {
                self.state.push_operator(Operator::FindForward { before: false });
                KeyResult::Handled
            }
            't' => {
                self.state.push_operator(Operator::FindForward { before: true });
                KeyResult::Handled
            }
            'F' => {
                self.state
                    .push_operator(Operator::FindBackward { after: false });
                KeyResult::Handled
            }
            'T' => {
                self.state.push_operator(Operator::FindBackward { after: true });
                KeyResult::Handled
            }
            ';' => {
                if let Some(motion) = self.state.last_find.clone() {
                    self.execute_motion(motion, editor);
                }
                KeyResult::Handled
            }
            ',' => {
                if let Some(motion) = self.state.last_find.clone() {
                    let reversed = match motion {
                        Motion::FindChar { char, before } => {
                            Motion::FindCharBackward { char, after: before }
                        }
                        Motion::FindCharBackward { char, after } => {
                            Motion::FindChar { char, before: after }
                        }
                        _ => motion,
                    };
                    self.execute_motion(reversed, editor);
                }
                KeyResult::Handled
            }

            // Matching bracket
            '%' => {
                self.execute_motion(Motion::MatchingBracket, editor);
                KeyResult::Handled
            }

            // Operators
            'd' => {
                if self.state.has_pending_operator() {
                    // dd - delete line
                    if matches!(self.state.active_operator(), Some(Operator::Delete)) {
                        self.state.pop_operator();
                        return self.delete_lines(editor);
                    }
                }
                self.state.push_operator(Operator::Delete);
                KeyResult::Handled
            }
            'c' => {
                if self.state.has_pending_operator() {
                    // cc - change line
                    if matches!(self.state.active_operator(), Some(Operator::Change)) {
                        self.state.pop_operator();
                        return self.change_lines(editor);
                    }
                }
                self.state.push_operator(Operator::Change);
                KeyResult::Handled
            }
            'y' => {
                if self.state.has_pending_operator() {
                    // yy - yank line
                    if matches!(self.state.active_operator(), Some(Operator::Yank)) {
                        self.state.pop_operator();
                        return self.yank_lines(editor);
                    }
                }
                self.state.push_operator(Operator::Yank);
                KeyResult::Handled
            }
            'D' => {
                // Delete to end of line
                self.delete_to_end_of_line(editor);
                KeyResult::TextChanged
            }
            'C' => {
                // Change to end of line
                self.delete_to_end_of_line(editor);
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'Y' => {
                // Yank line (like yy)
                self.yank_lines(editor)
            }

            // Single character operations
            'x' => {
                self.delete_char_forward(editor);
                KeyResult::TextChanged
            }
            'X' => {
                self.delete_char_backward(editor);
                KeyResult::TextChanged
            }
            's' => {
                // Substitute character
                self.delete_char_forward(editor);
                self.state.enter_insert();
                KeyResult::ModeChanged(Mode::Insert)
            }
            'S' => {
                // Substitute line
                self.change_lines(editor)
            }
            'r' => {
                self.state.push_operator(Operator::Replace);
                KeyResult::Handled
            }

            // Paste
            'p' => {
                self.paste_after(editor);
                KeyResult::TextChanged
            }
            'P' => {
                self.paste_before(editor);
                KeyResult::TextChanged
            }

            // Undo/redo
            'u' => {
                editor.undo();
                KeyResult::TextChanged
            }

            // Join lines
            'J' => {
                self.join_lines(editor);
                KeyResult::TextChanged
            }

            // Register selection
            '"' => {
                self.state.push_operator(Operator::Register);
                KeyResult::Handled
            }

            // Marks
            'm' => {
                self.state.push_operator(Operator::Mark);
                KeyResult::Handled
            }
            '\'' => {
                // Go to mark (line)
                self.state.pending_keys = "'".to_string();
                KeyResult::Handled
            }
            '`' => {
                // Go to mark (position)
                self.state.pending_keys = "`".to_string();
                KeyResult::Handled
            }

            // Search
            '/' => KeyResult::EnterSearch { forward: true },
            '?' => KeyResult::EnterSearch { forward: false },
            'n' => {
                self.execute_motion(Motion::SearchNext, editor);
                KeyResult::Handled
            }
            'N' => {
                self.execute_motion(Motion::SearchPrev, editor);
                KeyResult::Handled
            }
            '*' => {
                self.execute_motion(Motion::SearchWordForward, editor);
                KeyResult::Handled
            }
            '#' => {
                self.execute_motion(Motion::SearchWordBackward, editor);
                KeyResult::Handled
            }

            // Command mode
            ':' => KeyResult::EnterCommand,

            // Screen position
            'H' => {
                self.execute_motion(Motion::WindowTop, editor);
                KeyResult::Handled
            }
            'M' => {
                self.execute_motion(Motion::WindowMiddle, editor);
                KeyResult::Handled
            }
            'L' => {
                self.execute_motion(Motion::WindowBottom, editor);
                KeyResult::Handled
            }

            // Case toggle
            '~' => {
                self.toggle_case_char(editor);
                KeyResult::TextChanged
            }

            _ => {
                // Unknown key - ignore but consume to prevent insertion
                KeyResult::Handled
            }
        }
    }

    /// Handle g-prefixed commands
    fn handle_g_command<E: VimEditor>(&mut self, c: char, editor: &mut E) -> KeyResult {
        match c {
            'g' => {
                // gg - go to start of document
                self.execute_motion(Motion::DocumentStart, editor);
                KeyResult::Handled
            }
            '_' => {
                // g_ - go to last non-blank
                self.execute_motion(Motion::LastNonBlank, editor);
                KeyResult::Handled
            }
            'e' => {
                // ge - go to end of previous word
                self.execute_motion(
                    Motion::PrevWordEnd {
                        ignore_punctuation: false,
                    },
                    editor,
                );
                KeyResult::Handled
            }
            'E' => {
                // gE - go to end of previous WORD
                self.execute_motion(
                    Motion::PrevWordEnd {
                        ignore_punctuation: true,
                    },
                    editor,
                );
                KeyResult::Handled
            }
            _ => KeyResult::Handled,
        }
    }

    /// Handle Ctrl key combinations
    fn handle_ctrl_key<E: VimEditor>(&mut self, key: Key, editor: &mut E) -> KeyResult {
        match key {
            Key::Char('f') | Key::Char('F') => {
                self.execute_motion(Motion::PageDown, editor);
                KeyResult::Handled
            }
            Key::Char('b') | Key::Char('B') => {
                self.execute_motion(Motion::PageUp, editor);
                KeyResult::Handled
            }
            Key::Char('d') | Key::Char('D') => {
                self.execute_motion(Motion::HalfPageDown, editor);
                KeyResult::Handled
            }
            Key::Char('u') | Key::Char('U') => {
                self.execute_motion(Motion::HalfPageUp, editor);
                KeyResult::Handled
            }
            Key::Char('r') | Key::Char('R') => {
                editor.redo();
                KeyResult::TextChanged
            }
            Key::Char('o') | Key::Char('O') => {
                // Jump back
                if let Some((line, col)) = self.state.jump_back() {
                    editor.set_cursor(E::Pos::new(line, col));
                }
                KeyResult::Handled
            }
            Key::Char('i') | Key::Char('I') => {
                // Jump forward
                if let Some((line, col)) = self.state.jump_forward() {
                    editor.set_cursor(E::Pos::new(line, col));
                }
                KeyResult::Handled
            }
            _ => KeyResult::Ignored,
        }
    }

    /// Handle key in insert mode
    fn handle_insert<E: VimEditor>(&mut self, key: Key, mods: Modifiers, editor: &mut E) -> KeyResult {
        match key {
            Key::Escape => {
                // Move cursor back one position on leaving insert mode
                let cursor = editor.cursor();
                if cursor.column() > 0 {
                    editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() - 1));
                }
                self.state.enter_normal();
                KeyResult::ModeChanged(Mode::Normal)
            }

            Key::Char(c) if !mods.ctrl && !mods.alt => {
                let cursor = editor.cursor();
                editor.insert(cursor, &c.to_string());
                editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + 1));
                KeyResult::TextChanged
            }

            Key::Enter => {
                let cursor = editor.cursor();
                editor.insert(cursor, "\n");
                editor.set_cursor(E::Pos::new(cursor.line() + 1, 0));
                KeyResult::TextChanged
            }

            Key::Backspace => {
                let cursor = editor.cursor();
                if cursor.column() > 0 {
                    let start = E::Pos::new(cursor.line(), cursor.column() - 1);
                    editor.delete(TextRange::new(start, cursor));
                    editor.set_cursor(start);
                } else if cursor.line() > 0 {
                    // Join with previous line
                    let prev_line_len = editor.line_len(cursor.line() - 1);
                    let start = E::Pos::new(cursor.line() - 1, prev_line_len);
                    editor.delete(TextRange::new(start, cursor));
                    editor.set_cursor(start);
                }
                KeyResult::TextChanged
            }

            Key::Delete => {
                let cursor = editor.cursor();
                let line_len = editor.line_len(cursor.line());
                if cursor.column() < line_len {
                    let end = E::Pos::new(cursor.line(), cursor.column() + 1);
                    editor.delete(TextRange::new(cursor, end));
                } else if cursor.line() < editor.line_count() - 1 {
                    // Join with next line
                    let end = E::Pos::new(cursor.line() + 1, 0);
                    editor.delete(TextRange::new(cursor, end));
                }
                KeyResult::TextChanged
            }

            Key::Tab => {
                let cursor = editor.cursor();
                editor.insert(cursor, "    ");
                editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + 4));
                KeyResult::TextChanged
            }

            // Arrow keys in insert mode
            Key::Left => {
                let cursor = editor.cursor();
                if cursor.column() > 0 {
                    editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() - 1));
                }
                KeyResult::Handled
            }
            Key::Right => {
                let cursor = editor.cursor();
                let line_len = editor.line_len(cursor.line());
                if cursor.column() < line_len {
                    editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + 1));
                }
                KeyResult::Handled
            }
            Key::Up => {
                let cursor = editor.cursor();
                if cursor.line() > 0 {
                    let new_col = editor.clamp_column(cursor.line() - 1, cursor.column(), true);
                    editor.set_cursor(E::Pos::new(cursor.line() - 1, new_col));
                }
                KeyResult::Handled
            }
            Key::Down => {
                let cursor = editor.cursor();
                if cursor.line() < editor.line_count() - 1 {
                    let new_col = editor.clamp_column(cursor.line() + 1, cursor.column(), true);
                    editor.set_cursor(E::Pos::new(cursor.line() + 1, new_col));
                }
                KeyResult::Handled
            }

            _ => KeyResult::Ignored,
        }
    }

    /// Handle key in replace mode
    fn handle_replace<E: VimEditor>(&mut self, key: Key, mods: Modifiers, editor: &mut E) -> KeyResult {
        match key {
            Key::Escape => {
                self.state.enter_normal();
                KeyResult::ModeChanged(Mode::Normal)
            }

            Key::Char(c) if !mods.ctrl && !mods.alt => {
                let cursor = editor.cursor();
                let line_len = editor.line_len(cursor.line());

                if cursor.column() < line_len {
                    // Replace character
                    let end = E::Pos::new(cursor.line(), cursor.column() + 1);
                    editor.replace(TextRange::new(cursor, end), &c.to_string());
                } else {
                    // At end of line, insert
                    editor.insert(cursor, &c.to_string());
                }
                editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + 1));
                KeyResult::TextChanged
            }

            Key::Backspace => {
                let cursor = editor.cursor();
                if cursor.column() > 0 {
                    editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() - 1));
                }
                KeyResult::Handled
            }

            _ => self.handle_insert(key, mods, editor),
        }
    }

    /// Handle key in visual modes
    fn handle_visual<E: VimEditor>(&mut self, key: Key, mods: Modifiers, editor: &mut E) -> KeyResult {
        if mods.ctrl {
            return self.handle_ctrl_key(key, editor);
        }

        match key {
            Key::Escape => {
                editor.set_selection(None);
                self.state.enter_normal();
                KeyResult::ModeChanged(Mode::Normal)
            }

            Key::Char('v') => {
                if self.state.mode == Mode::Visual {
                    editor.set_selection(None);
                    self.state.enter_normal();
                    KeyResult::ModeChanged(Mode::Normal)
                } else {
                    let cursor = editor.cursor();
                    self.state.enter_visual((cursor.line(), cursor.column()));
                    KeyResult::ModeChanged(Mode::Visual)
                }
            }

            Key::Char('V') => {
                if self.state.mode == Mode::VisualLine {
                    editor.set_selection(None);
                    self.state.enter_normal();
                    KeyResult::ModeChanged(Mode::Normal)
                } else {
                    let cursor = editor.cursor();
                    self.state.enter_visual_line((cursor.line(), cursor.column()));
                    KeyResult::ModeChanged(Mode::VisualLine)
                }
            }

            // Motions in visual mode
            Key::Char(c) => {
                match c {
                    // Basic motions
                    'h' | 'j' | 'k' | 'l' | '0' | '^' | '$' | 'w' | 'W' | 'e' | 'E' | 'b' | 'B'
                    | '{' | '}' | 'G' | 'g' | 'H' | 'M' | 'L' => {
                        // Execute motion and update selection
                        let result = self.handle_normal_char(c, editor);
                        self.update_visual_selection(editor);
                        result
                    }

                    // Operations on selection
                    'd' | 'x' => {
                        self.delete_selection(editor);
                        self.state.enter_normal();
                        KeyResult::ModeChanged(Mode::Normal)
                    }
                    'c' | 's' => {
                        self.delete_selection(editor);
                        self.state.enter_insert();
                        KeyResult::ModeChanged(Mode::Insert)
                    }
                    'y' => {
                        self.yank_selection(editor);
                        editor.set_selection(None);
                        self.state.enter_normal();
                        KeyResult::ModeChanged(Mode::Normal)
                    }

                    // Switch anchor/cursor
                    'o' => {
                        if let Some(anchor) = self.state.visual_anchor {
                            let cursor = editor.cursor();
                            self.state.visual_anchor = Some((cursor.line(), cursor.column()));
                            editor.set_cursor(E::Pos::new(anchor.0, anchor.1));
                            self.update_visual_selection(editor);
                        }
                        KeyResult::Handled
                    }

                    _ => KeyResult::Handled,
                }
            }

            Key::Left | Key::Right | Key::Up | Key::Down => {
                let result = self.handle_normal(key, mods, editor);
                self.update_visual_selection(editor);
                result
            }

            _ => KeyResult::Handled,
        }
    }

    // === Motion execution ===

    /// Execute a motion
    fn execute_motion<E: VimEditor>(&mut self, motion: Motion, editor: &mut E) {
        let count = self.state.effective_count();

        // Check if there's a pending operator
        if let Some(op) = self.state.pop_operator() {
            // Execute operator with motion
            self.execute_operator_motion(op, motion, count, editor);
            self.state.reset_pending();
        } else {
            // Just move cursor
            for _ in 0..count {
                self.move_cursor(motion.clone(), editor);
            }
            self.state.clear_count();
        }
    }

    /// Move cursor according to motion
    fn move_cursor<E: VimEditor>(&mut self, motion: Motion, editor: &mut E) {
        let cursor = editor.cursor();
        let line_count = editor.line_count();

        let new_pos = match motion {
            Motion::Left => {
                let new_col = cursor.column().saturating_sub(1);
                E::Pos::new(cursor.line(), new_col)
            }
            Motion::Right => {
                let line_len = editor.line_len(cursor.line());
                let new_col = (cursor.column() + 1).min(line_len.saturating_sub(1));
                E::Pos::new(cursor.line(), new_col)
            }
            Motion::Up => {
                if cursor.line() > 0 {
                    let new_col = editor.clamp_column(cursor.line() - 1, cursor.column(), false);
                    E::Pos::new(cursor.line() - 1, new_col)
                } else {
                    cursor
                }
            }
            Motion::Down => {
                if cursor.line() < line_count - 1 {
                    let new_col = editor.clamp_column(cursor.line() + 1, cursor.column(), false);
                    E::Pos::new(cursor.line() + 1, new_col)
                } else {
                    cursor
                }
            }
            Motion::LineStart => E::Pos::new(cursor.line(), 0),
            Motion::LineEnd => {
                let line_len = editor.line_len(cursor.line());
                E::Pos::new(cursor.line(), line_len.saturating_sub(1).max(0))
            }
            Motion::FirstNonBlank => {
                let line = editor.line_text(cursor.line());
                let col = line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
                E::Pos::new(cursor.line(), col)
            }
            Motion::LastNonBlank => {
                let line = editor.line_text(cursor.line());
                let col = line
                    .chars()
                    .rev()
                    .position(|c| !c.is_whitespace())
                    .map(|p| line.len() - 1 - p)
                    .unwrap_or(0);
                E::Pos::new(cursor.line(), col)
            }
            Motion::NextWordStart { ignore_punctuation } => {
                self.find_next_word_start(cursor, ignore_punctuation, editor)
            }
            Motion::NextWordEnd { ignore_punctuation } => {
                self.find_next_word_end(cursor, ignore_punctuation, editor)
            }
            Motion::PrevWordStart { ignore_punctuation } => {
                self.find_prev_word_start(cursor, ignore_punctuation, editor)
            }
            Motion::PrevWordEnd { ignore_punctuation } => {
                self.find_prev_word_end(cursor, ignore_punctuation, editor)
            }
            Motion::ParagraphForward => self.find_next_paragraph(cursor, editor),
            Motion::ParagraphBackward => self.find_prev_paragraph(cursor, editor),
            Motion::DocumentStart => E::Pos::new(0, 0),
            Motion::DocumentEnd { line } => {
                let target_line = line.unwrap_or(line_count - 1).min(line_count - 1);
                E::Pos::new(target_line, 0)
            }
            Motion::GoToLine { line } => {
                let target = line.min(line_count - 1);
                E::Pos::new(target, 0)
            }
            Motion::FindChar { char, before } => {
                self.find_char_forward(cursor, char, before, editor)
            }
            Motion::FindCharBackward { char, after } => {
                self.find_char_backward(cursor, char, after, editor)
            }
            Motion::MatchingBracket => self.find_matching_bracket(cursor, editor),
            Motion::PageDown => {
                let lines = editor.viewport_lines();
                let new_line = (cursor.line() + lines).min(line_count - 1);
                E::Pos::new(new_line, cursor.column())
            }
            Motion::PageUp => {
                let lines = editor.viewport_lines();
                let new_line = cursor.line().saturating_sub(lines);
                E::Pos::new(new_line, cursor.column())
            }
            Motion::HalfPageDown => {
                let lines = editor.viewport_lines() / 2;
                let new_line = (cursor.line() + lines).min(line_count - 1);
                E::Pos::new(new_line, cursor.column())
            }
            Motion::HalfPageUp => {
                let lines = editor.viewport_lines() / 2;
                let new_line = cursor.line().saturating_sub(lines);
                E::Pos::new(new_line, cursor.column())
            }
            Motion::WindowTop => {
                let visible = editor.visible_lines();
                E::Pos::new(visible.start, 0)
            }
            Motion::WindowMiddle => {
                let visible = editor.visible_lines();
                let middle = (visible.start + visible.end) / 2;
                E::Pos::new(middle, 0)
            }
            Motion::WindowBottom => {
                let visible = editor.visible_lines();
                E::Pos::new(visible.end.saturating_sub(1), 0)
            }
            _ => cursor, // Other motions not yet implemented
        };

        editor.set_cursor(new_pos);
    }

    // === Word motion helpers ===

    fn find_next_word_start<E: VimEditor>(
        &self,
        pos: E::Pos,
        ignore_punctuation: bool,
        editor: &E,
    ) -> E::Pos {
        let mut line = pos.line();
        let mut col = pos.column();
        let line_count = editor.line_count();

        // Get current character class
        let line_text = editor.line_text(line);
        let chars: Vec<char> = line_text.chars().collect();

        if chars.is_empty() {
            // Empty line, move to next line
            if line < line_count - 1 {
                return E::Pos::new(line + 1, 0);
            }
            return pos;
        }

        // Skip current word
        let start_class = if col < chars.len() {
            if ignore_punctuation {
                if chars[col].is_whitespace() {
                    CharClass::Whitespace
                } else {
                    CharClass::Word
                }
            } else {
                CharClass::of(chars[col])
            }
        } else {
            CharClass::Whitespace
        };

        // Skip rest of current word
        while col < chars.len() {
            let class = if ignore_punctuation {
                if chars[col].is_whitespace() {
                    CharClass::Whitespace
                } else {
                    CharClass::Word
                }
            } else {
                CharClass::of(chars[col])
            };
            if class != start_class {
                break;
            }
            col += 1;
        }

        // Skip whitespace
        while col < chars.len() && chars[col].is_whitespace() {
            col += 1;
        }

        if col < chars.len() {
            return E::Pos::new(line, col);
        }

        // Move to next line
        line += 1;
        while line < line_count {
            let text = editor.line_text(line);
            let chars: Vec<char> = text.chars().collect();
            if let Some(col) = chars.iter().position(|c| !c.is_whitespace()) {
                return E::Pos::new(line, col);
            }
            line += 1;
        }

        E::Pos::new(line_count - 1, editor.line_len(line_count - 1))
    }

    fn find_next_word_end<E: VimEditor>(
        &self,
        pos: E::Pos,
        ignore_punctuation: bool,
        editor: &E,
    ) -> E::Pos {
        let mut line = pos.line();
        let mut col = pos.column() + 1; // Move past current position
        let line_count = editor.line_count();

        loop {
            let line_text = editor.line_text(line);
            let chars: Vec<char> = line_text.chars().collect();

            // Skip whitespace
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }

            if col < chars.len() {
                // Found non-whitespace, find end of word
                let start_class = if ignore_punctuation {
                    CharClass::Word
                } else {
                    CharClass::of(chars[col])
                };

                while col + 1 < chars.len() {
                    let next_class = if ignore_punctuation {
                        if chars[col + 1].is_whitespace() {
                            CharClass::Whitespace
                        } else {
                            CharClass::Word
                        }
                    } else {
                        CharClass::of(chars[col + 1])
                    };
                    if next_class != start_class {
                        break;
                    }
                    col += 1;
                }
                return E::Pos::new(line, col);
            }

            // Move to next line
            line += 1;
            col = 0;
            if line >= line_count {
                return E::Pos::new(line_count - 1, editor.line_len(line_count - 1));
            }
        }
    }

    fn find_prev_word_start<E: VimEditor>(
        &self,
        pos: E::Pos,
        ignore_punctuation: bool,
        editor: &E,
    ) -> E::Pos {
        let mut line = pos.line();
        let mut col = pos.column();

        loop {
            let line_text = editor.line_text(line);
            let chars: Vec<char> = line_text.chars().collect();

            // Move back one if not at start
            if col > 0 {
                col -= 1;
            } else if line > 0 {
                line -= 1;
                let prev_len = editor.line_len(line);
                col = prev_len.saturating_sub(1);
                continue;
            } else {
                return E::Pos::new(0, 0);
            }

            // Skip whitespace going backward
            while col > 0 && chars.get(col).map(|c| c.is_whitespace()).unwrap_or(true) {
                col -= 1;
            }

            if chars.get(col).map(|c| c.is_whitespace()).unwrap_or(true) {
                if line > 0 {
                    line -= 1;
                    col = editor.line_len(line).saturating_sub(1);
                    continue;
                } else {
                    return E::Pos::new(0, 0);
                }
            }

            // Find start of word going backward
            let end_class = if ignore_punctuation {
                CharClass::Word
            } else {
                CharClass::of(chars[col])
            };

            while col > 0 {
                let prev_class = if ignore_punctuation {
                    if chars[col - 1].is_whitespace() {
                        CharClass::Whitespace
                    } else {
                        CharClass::Word
                    }
                } else {
                    CharClass::of(chars[col - 1])
                };
                if prev_class != end_class {
                    break;
                }
                col -= 1;
            }

            return E::Pos::new(line, col);
        }
    }

    fn find_prev_word_end<E: VimEditor>(
        &self,
        pos: E::Pos,
        ignore_punctuation: bool,
        editor: &E,
    ) -> E::Pos {
        // First move to previous word
        let prev_start = self.find_prev_word_start(pos, ignore_punctuation, editor);
        let mut line = prev_start.line();
        let mut col = prev_start.column();

        // If we didn't move, try again from one position back
        if prev_start == pos && (col > 0 || line > 0) {
            if col > 0 {
                col -= 1;
            } else {
                line -= 1;
                col = editor.line_len(line).saturating_sub(1);
            }
            let prev_start = self.find_prev_word_start(E::Pos::new(line, col), ignore_punctuation, editor);
            line = prev_start.line();
            col = prev_start.column();
        }

        // Now find end of that word
        let line_text = editor.line_text(line);
        let chars: Vec<char> = line_text.chars().collect();

        if col < chars.len() {
            let start_class = if ignore_punctuation {
                CharClass::Word
            } else {
                CharClass::of(chars[col])
            };

            while col + 1 < chars.len() {
                let next_class = if ignore_punctuation {
                    if chars[col + 1].is_whitespace() {
                        CharClass::Whitespace
                    } else {
                        CharClass::Word
                    }
                } else {
                    CharClass::of(chars[col + 1])
                };
                if next_class != start_class {
                    break;
                }
                col += 1;
            }
        }

        E::Pos::new(line, col)
    }

    // === Paragraph motion helpers ===

    fn find_next_paragraph<E: VimEditor>(&self, pos: E::Pos, editor: &E) -> E::Pos {
        let mut line = pos.line() + 1;
        let line_count = editor.line_count();

        // Skip non-blank lines
        while line < line_count && !editor.is_blank_line(line) {
            line += 1;
        }

        // Skip blank lines
        while line < line_count && editor.is_blank_line(line) {
            line += 1;
        }

        if line >= line_count {
            E::Pos::new(line_count - 1, 0)
        } else {
            E::Pos::new(line, 0)
        }
    }

    fn find_prev_paragraph<E: VimEditor>(&self, pos: E::Pos, editor: &E) -> E::Pos {
        let mut line = pos.line().saturating_sub(1);

        // Skip blank lines
        while line > 0 && editor.is_blank_line(line) {
            line -= 1;
        }

        // Skip non-blank lines
        while line > 0 && !editor.is_blank_line(line) {
            line -= 1;
        }

        E::Pos::new(line, 0)
    }

    // === Find character helpers ===

    fn find_char_forward<E: VimEditor>(
        &self,
        pos: E::Pos,
        target: char,
        before: bool,
        editor: &E,
    ) -> E::Pos {
        let line = editor.line_text(pos.line());
        let chars: Vec<char> = line.chars().collect();

        for i in (pos.column() + 1)..chars.len() {
            if chars[i] == target {
                let col = if before { i - 1 } else { i };
                return E::Pos::new(pos.line(), col);
            }
        }
        pos
    }

    fn find_char_backward<E: VimEditor>(
        &self,
        pos: E::Pos,
        target: char,
        after: bool,
        editor: &E,
    ) -> E::Pos {
        let line = editor.line_text(pos.line());
        let chars: Vec<char> = line.chars().collect();

        for i in (0..pos.column()).rev() {
            if chars[i] == target {
                let col = if after { i + 1 } else { i };
                return E::Pos::new(pos.line(), col);
            }
        }
        pos
    }

    // === Bracket matching ===

    fn find_matching_bracket<E: VimEditor>(&self, pos: E::Pos, editor: &E) -> E::Pos {
        let line = editor.line_text(pos.line());
        let chars: Vec<char> = line.chars().collect();

        if pos.column() >= chars.len() {
            return pos;
        }

        let current = chars[pos.column()];
        let (target, forward) = match current {
            '(' => (')', true),
            ')' => ('(', false),
            '[' => (']', true),
            ']' => ('[', false),
            '{' => ('}', true),
            '}' => ('{', false),
            '<' => ('>', true),
            '>' => ('<', false),
            _ => return pos,
        };

        let mut depth = 1;

        if forward {
            let mut line_num = pos.line();
            let mut col = pos.column() + 1;

            while line_num < editor.line_count() {
                let text = editor.line_text(line_num);
                let chars: Vec<char> = text.chars().collect();

                while col < chars.len() {
                    if chars[col] == current {
                        depth += 1;
                    } else if chars[col] == target {
                        depth -= 1;
                        if depth == 0 {
                            return E::Pos::new(line_num, col);
                        }
                    }
                    col += 1;
                }

                line_num += 1;
                col = 0;
            }
        } else {
            let mut line_num = pos.line();
            let mut col = pos.column();

            loop {
                if col == 0 {
                    if line_num == 0 {
                        break;
                    }
                    line_num -= 1;
                    col = editor.line_len(line_num);
                }
                col -= 1;

                let text = editor.line_text(line_num);
                let chars: Vec<char> = text.chars().collect();

                if col < chars.len() {
                    if chars[col] == current {
                        depth += 1;
                    } else if chars[col] == target {
                        depth -= 1;
                        if depth == 0 {
                            return E::Pos::new(line_num, col);
                        }
                    }
                }
            }
        }

        pos
    }

    // === Operator execution ===

    fn execute_operator_motion<E: VimEditor>(
        &mut self,
        op: Operator,
        motion: Motion,
        count: usize,
        editor: &mut E,
    ) {
        let start = editor.cursor();
        for _ in 0..count {
            self.move_cursor(motion.clone(), editor);
        }
        let end = editor.cursor();

        // Create range
        let (range_start, range_end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        // Adjust range based on motion kind
        let range = if motion.is_inclusive() {
            // Include the final character
            let adjusted_end = E::Pos::new(
                range_end.line(),
                (range_end.column() + 1).min(editor.line_len(range_end.line())),
            );
            TextRange::new(range_start, adjusted_end)
        } else if motion.is_linewise() {
            // Extend to full lines
            let line_start = E::Pos::new(range_start.line(), 0);
            let line_end = if range_end.line() < editor.line_count() - 1 {
                E::Pos::new(range_end.line() + 1, 0)
            } else {
                E::Pos::new(range_end.line(), editor.line_len(range_end.line()))
            };
            TextRange::new(line_start, line_end)
        } else {
            TextRange::new(range_start, range_end)
        };

        match op {
            Operator::Delete => {
                let text = self.get_range_text(&range, editor);
                self.state.store_register(text, true);
                editor.delete(range);
                editor.set_cursor(range.start);
            }
            Operator::Change => {
                let text = self.get_range_text(&range, editor);
                self.state.store_register(text, true);
                editor.delete(range);
                editor.set_cursor(range.start);
                self.state.enter_insert();
            }
            Operator::Yank => {
                let text = self.get_range_text(&range, editor);
                self.state.store_register(text, false);
                editor.set_cursor(range.start);
            }
            _ => {}
        }
    }

    fn get_range_text<E: VimEditor>(&self, range: &TextRange<E::Pos>, editor: &E) -> String {
        let mut result = String::new();

        if range.start.line() == range.end.line() {
            let line = editor.line_text(range.start.line());
            let start_col = range.start.column().min(line.len());
            let end_col = range.end.column().min(line.len());
            result.push_str(&line[start_col..end_col]);
        } else {
            // First line
            let first_line = editor.line_text(range.start.line());
            let start_col = range.start.column().min(first_line.len());
            result.push_str(&first_line[start_col..]);
            result.push('\n');

            // Middle lines
            for line_num in (range.start.line() + 1)..range.end.line() {
                result.push_str(editor.line_text(line_num));
                result.push('\n');
            }

            // Last line
            if range.end.line() < editor.line_count() {
                let last_line = editor.line_text(range.end.line());
                let end_col = range.end.column().min(last_line.len());
                result.push_str(&last_line[..end_col]);
            }
        }

        result
    }

    // === Text object execution ===

    fn execute_object<E: VimEditor>(
        &mut self,
        object: Object,
        around: bool,
        editor: &mut E,
    ) -> KeyResult {
        let cursor = editor.cursor();
        let range = self.find_object_range(object, around, cursor, editor);

        if let Some(range) = range {
            if let Some(op) = self.state.pop_operator() {
                match op {
                    Operator::Delete => {
                        let text = self.get_range_text(&range, editor);
                        self.state.store_register(text, true);
                        editor.delete(range);
                        editor.set_cursor(range.start);
                        self.state.reset_pending();
                        return KeyResult::TextChanged;
                    }
                    Operator::Change => {
                        let text = self.get_range_text(&range, editor);
                        self.state.store_register(text, true);
                        editor.delete(range);
                        editor.set_cursor(range.start);
                        self.state.enter_insert();
                        return KeyResult::ModeChanged(Mode::Insert);
                    }
                    Operator::Yank => {
                        let text = self.get_range_text(&range, editor);
                        self.state.store_register(text, false);
                        self.state.reset_pending();
                        return KeyResult::Handled;
                    }
                    _ => {}
                }
            }
        }

        self.state.reset_pending();
        KeyResult::Handled
    }

    fn find_object_range<E: VimEditor>(
        &self,
        object: Object,
        around: bool,
        pos: E::Pos,
        editor: &E,
    ) -> Option<TextRange<E::Pos>> {
        match object {
            Object::Word { ignore_punctuation } => {
                self.find_word_object(around, ignore_punctuation, pos, editor)
            }
            Object::Paragraph => self.find_paragraph_object(around, pos, editor),
            Object::SingleQuotes | Object::DoubleQuotes | Object::BackQuotes => {
                let delim = object.open_delimiter()?;
                self.find_quote_object(around, delim, pos, editor)
            }
            Object::Parentheses
            | Object::SquareBrackets
            | Object::CurlyBrackets
            | Object::AngleBrackets => {
                let open = object.open_delimiter()?;
                let close = object.close_delimiter()?;
                self.find_bracket_object(around, open, close, pos, editor)
            }
            _ => None,
        }
    }

    fn find_word_object<E: VimEditor>(
        &self,
        around: bool,
        ignore_punctuation: bool,
        pos: E::Pos,
        editor: &E,
    ) -> Option<TextRange<E::Pos>> {
        let line = editor.line_text(pos.line());
        let chars: Vec<char> = line.chars().collect();

        if pos.column() >= chars.len() {
            return None;
        }

        let col = pos.column();
        let current_class = if ignore_punctuation {
            if chars[col].is_whitespace() {
                CharClass::Whitespace
            } else {
                CharClass::Word
            }
        } else {
            CharClass::of(chars[col])
        };

        // Find word boundaries
        let mut start = col;
        while start > 0 {
            let prev_class = if ignore_punctuation {
                if chars[start - 1].is_whitespace() {
                    CharClass::Whitespace
                } else {
                    CharClass::Word
                }
            } else {
                CharClass::of(chars[start - 1])
            };
            if prev_class != current_class {
                break;
            }
            start -= 1;
        }

        let mut end = col;
        while end < chars.len() {
            let char_class = if ignore_punctuation {
                if chars[end].is_whitespace() {
                    CharClass::Whitespace
                } else {
                    CharClass::Word
                }
            } else {
                CharClass::of(chars[end])
            };
            if char_class != current_class {
                break;
            }
            end += 1;
        }

        if around {
            // Include trailing whitespace
            while end < chars.len() && chars[end].is_whitespace() {
                end += 1;
            }
        }

        Some(TextRange::new(
            E::Pos::new(pos.line(), start),
            E::Pos::new(pos.line(), end),
        ))
    }

    fn find_paragraph_object<E: VimEditor>(
        &self,
        around: bool,
        pos: E::Pos,
        editor: &E,
    ) -> Option<TextRange<E::Pos>> {
        let mut start_line = pos.line();
        let mut end_line = pos.line();
        let line_count = editor.line_count();

        // Find start of paragraph
        while start_line > 0 && !editor.is_blank_line(start_line - 1) {
            start_line -= 1;
        }

        // Find end of paragraph
        while end_line < line_count - 1 && !editor.is_blank_line(end_line + 1) {
            end_line += 1;
        }

        if around {
            // Include trailing blank lines
            while end_line < line_count - 1 && editor.is_blank_line(end_line + 1) {
                end_line += 1;
            }
        }

        let end_col = editor.line_len(end_line);
        Some(TextRange::new(
            E::Pos::new(start_line, 0),
            E::Pos::new(end_line, end_col),
        ))
    }

    fn find_quote_object<E: VimEditor>(
        &self,
        around: bool,
        delim: char,
        pos: E::Pos,
        editor: &E,
    ) -> Option<TextRange<E::Pos>> {
        let line = editor.line_text(pos.line());
        let chars: Vec<char> = line.chars().collect();
        let col = pos.column();

        // Find quotes on current line
        let mut quotes: Vec<usize> = Vec::new();
        for (i, &c) in chars.iter().enumerate() {
            if c == delim {
                quotes.push(i);
            }
        }

        // Find pair containing cursor
        for i in 0..quotes.len() / 2 {
            let start = quotes[i * 2];
            let end = quotes[i * 2 + 1];
            if start <= col && col <= end {
                let (start_col, end_col) = if around {
                    (start, end + 1)
                } else {
                    (start + 1, end)
                };
                return Some(TextRange::new(
                    E::Pos::new(pos.line(), start_col),
                    E::Pos::new(pos.line(), end_col),
                ));
            }
        }

        None
    }

    fn find_bracket_object<E: VimEditor>(
        &self,
        around: bool,
        open: char,
        close: char,
        pos: E::Pos,
        editor: &E,
    ) -> Option<TextRange<E::Pos>> {
        // Find opening bracket
        let mut start_line = pos.line();
        let mut start_col = pos.column();
        let mut depth = 0;

        // Search backward for opening bracket
        loop {
            let line = editor.line_text(start_line);
            let chars: Vec<char> = line.chars().collect();

            while start_col > 0 || (start_col == 0 && start_line == pos.line()) {
                if start_col == 0 {
                    break;
                }
                start_col -= 1;

                if start_col < chars.len() {
                    if chars[start_col] == close {
                        depth += 1;
                    } else if chars[start_col] == open {
                        if depth == 0 {
                            // Found opening bracket, now find closing
                            let mut end_line = start_line;
                            let mut end_col = start_col + 1;
                            depth = 1;

                            while end_line < editor.line_count() {
                                let line = editor.line_text(end_line);
                                let chars: Vec<char> = line.chars().collect();

                                while end_col < chars.len() {
                                    if chars[end_col] == open {
                                        depth += 1;
                                    } else if chars[end_col] == close {
                                        depth -= 1;
                                        if depth == 0 {
                                            let (s_col, e_col) = if around {
                                                (start_col, end_col + 1)
                                            } else {
                                                (start_col + 1, end_col)
                                            };
                                            return Some(TextRange::new(
                                                E::Pos::new(start_line, s_col),
                                                E::Pos::new(end_line, e_col),
                                            ));
                                        }
                                    }
                                    end_col += 1;
                                }

                                end_line += 1;
                                end_col = 0;
                            }

                            return None;
                        }
                        depth -= 1;
                    }
                }
            }

            if start_line == 0 {
                break;
            }
            start_line -= 1;
            start_col = editor.line_len(start_line);
        }

        None
    }

    // === Line operations ===

    fn delete_lines<E: VimEditor>(&mut self, editor: &mut E) -> KeyResult {
        let cursor = editor.cursor();
        let count = self.state.effective_count();
        let line_count = editor.line_count();

        let start_line = cursor.line();
        let end_line = (start_line + count).min(line_count);

        // Build text to yank
        let mut text = String::new();
        for line in start_line..end_line {
            text.push_str(editor.line_text(line));
            text.push('\n');
        }
        self.state.store_register(text, true);

        // Delete lines
        let start = E::Pos::new(start_line, 0);
        let end = if end_line >= line_count {
            E::Pos::new(line_count - 1, editor.line_len(line_count - 1))
        } else {
            E::Pos::new(end_line, 0)
        };

        editor.delete(TextRange::new(start, end));

        // Position cursor
        let new_line = start_line.min(editor.line_count().saturating_sub(1));
        let new_col = {
            let line = editor.line_text(new_line);
            line.chars().position(|c| !c.is_whitespace()).unwrap_or(0)
        };
        editor.set_cursor(E::Pos::new(new_line, new_col));

        self.state.reset_pending();
        KeyResult::TextChanged
    }

    fn change_lines<E: VimEditor>(&mut self, editor: &mut E) -> KeyResult {
        self.delete_lines(editor);
        self.state.enter_insert();
        KeyResult::ModeChanged(Mode::Insert)
    }

    fn yank_lines<E: VimEditor>(&mut self, editor: &mut E) -> KeyResult {
        let cursor = editor.cursor();
        let count = self.state.effective_count();
        let line_count = editor.line_count();

        let start_line = cursor.line();
        let end_line = (start_line + count).min(line_count);

        let mut text = String::new();
        for line in start_line..end_line {
            text.push_str(editor.line_text(line));
            text.push('\n');
        }
        self.state.store_register(text, false);

        self.state.reset_pending();
        KeyResult::Handled
    }

    fn delete_to_end_of_line<E: VimEditor>(&mut self, editor: &mut E) {
        let cursor = editor.cursor();
        let line_len = editor.line_len(cursor.line());

        if cursor.column() < line_len {
            let range = TextRange::new(cursor, E::Pos::new(cursor.line(), line_len));
            let text = self.get_range_text(&range, editor);
            self.state.store_register(text, true);
            editor.delete(range);
        }

        self.state.reset_pending();
    }

    // === Single character operations ===

    fn delete_char_forward<E: VimEditor>(&mut self, editor: &mut E) {
        let cursor = editor.cursor();
        let line_len = editor.line_len(cursor.line());
        let count = self.state.effective_count();

        let end_col = (cursor.column() + count).min(line_len);
        if cursor.column() < end_col {
            let range = TextRange::new(cursor, E::Pos::new(cursor.line(), end_col));
            let text = self.get_range_text(&range, editor);
            self.state.store_register(text, true);
            editor.delete(range);
        }

        self.state.reset_pending();
    }

    fn delete_char_backward<E: VimEditor>(&mut self, editor: &mut E) {
        let cursor = editor.cursor();
        let count = self.state.effective_count();

        let start_col = cursor.column().saturating_sub(count);
        if start_col < cursor.column() {
            let range = TextRange::new(E::Pos::new(cursor.line(), start_col), cursor);
            let text = self.get_range_text(&range, editor);
            self.state.store_register(text, true);
            editor.delete(range);
            editor.set_cursor(E::Pos::new(cursor.line(), start_col));
        }

        self.state.reset_pending();
    }

    fn replace_char<E: VimEditor>(&mut self, c: char, editor: &mut E) {
        let cursor = editor.cursor();
        let line_len = editor.line_len(cursor.line());
        let count = self.state.effective_count();

        let end_col = (cursor.column() + count).min(line_len);
        if cursor.column() < end_col {
            let range = TextRange::new(cursor, E::Pos::new(cursor.line(), end_col));
            let replacement: String = std::iter::repeat(c).take(count).collect();
            editor.replace(range, &replacement);
        }

        self.state.reset_pending();
    }

    fn toggle_case_char<E: VimEditor>(&mut self, editor: &mut E) {
        let cursor = editor.cursor();
        let line = editor.line_text(cursor.line());
        let chars: Vec<char> = line.chars().collect();

        if cursor.column() < chars.len() {
            let c = chars[cursor.column()];
            let toggled = if c.is_uppercase() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                c.to_uppercase().next().unwrap_or(c)
            };

            let range = TextRange::new(cursor, E::Pos::new(cursor.line(), cursor.column() + 1));
            editor.replace(range, &toggled.to_string());

            // Move cursor right
            let line_len = editor.line_len(cursor.line());
            if cursor.column() + 1 < line_len {
                editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + 1));
            }
        }
    }

    // === Paste operations ===

    fn paste_after<E: VimEditor>(&mut self, editor: &mut E) {
        if let Some(text) = self.state.get_register(self.state.selected_register).map(|s| s.to_string()) {
            let cursor = editor.cursor();

            if text.ends_with('\n') {
                // Linewise paste - paste below current line
                let line_len = editor.line_len(cursor.line());
                let pos = E::Pos::new(cursor.line(), line_len);
                editor.insert(pos, &format!("\n{}", text.trim_end_matches('\n')));
                editor.set_cursor(E::Pos::new(cursor.line() + 1, 0));
            } else {
                // Characterwise paste - paste after cursor
                let pos = E::Pos::new(cursor.line(), cursor.column() + 1);
                editor.insert(pos, &text);
                editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + text.len()));
            }
        }
        self.state.reset_pending();
    }

    fn paste_before<E: VimEditor>(&mut self, editor: &mut E) {
        if let Some(text) = self.state.get_register(self.state.selected_register).map(|s| s.to_string()) {
            let cursor = editor.cursor();

            if text.ends_with('\n') {
                // Linewise paste - paste above current line
                let pos = E::Pos::new(cursor.line(), 0);
                editor.insert(pos, &text);
                editor.set_cursor(E::Pos::new(cursor.line(), 0));
            } else {
                // Characterwise paste - paste before cursor
                editor.insert(cursor, &text);
                editor.set_cursor(E::Pos::new(cursor.line(), cursor.column() + text.len() - 1));
            }
        }
        self.state.reset_pending();
    }

    // === Other operations ===

    fn join_lines<E: VimEditor>(&mut self, editor: &mut E) {
        let cursor = editor.cursor();
        let count = self.state.effective_count();

        for _ in 0..count {
            if cursor.line() >= editor.line_count() - 1 {
                break;
            }

            let line_len = editor.line_len(cursor.line());
            let next_line = editor.line_text(cursor.line() + 1);
            let trimmed = next_line.trim_start();

            // Delete newline and leading whitespace of next line
            let range = TextRange::new(
                E::Pos::new(cursor.line(), line_len),
                E::Pos::new(cursor.line() + 1, next_line.len() - trimmed.len()),
            );
            editor.delete(range);

            // Add single space if needed
            if line_len > 0 {
                editor.insert(E::Pos::new(cursor.line(), line_len), " ");
            }
        }

        self.state.reset_pending();
    }

    // === Visual mode ===

    fn update_visual_selection<E: VimEditor>(&mut self, editor: &mut E) {
        if let Some(anchor) = self.state.visual_anchor {
            let cursor = editor.cursor();
            let anchor_pos = E::Pos::new(anchor.0, anchor.1);

            let range = match self.state.mode {
                Mode::VisualLine => {
                    let (start_line, end_line) = if anchor.0 <= cursor.line() {
                        (anchor.0, cursor.line())
                    } else {
                        (cursor.line(), anchor.0)
                    };
                    TextRange::new(
                        E::Pos::new(start_line, 0),
                        E::Pos::new(end_line, editor.line_len(end_line)),
                    )
                }
                _ => {
                    if anchor_pos <= cursor {
                        TextRange::new(anchor_pos, E::Pos::new(cursor.line(), cursor.column() + 1))
                    } else {
                        TextRange::new(cursor, E::Pos::new(anchor.0, anchor.1 + 1))
                    }
                }
            };

            editor.set_selection(Some(range));
        }
    }

    fn delete_selection<E: VimEditor>(&mut self, editor: &mut E) {
        if let Some(range) = editor.selection() {
            let text = self.get_range_text(&range, editor);
            self.state.store_register(text, true);
            editor.delete(range);
            editor.set_cursor(range.start);
            editor.set_selection(None);
        }
    }

    fn yank_selection<E: VimEditor>(&mut self, editor: &mut E) {
        if let Some(range) = editor.selection() {
            let text = self.get_range_text(&range, editor);
            self.state.store_register(text, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let handler = VimHandler::new();
        assert_eq!(handler.mode(), Mode::Normal);
    }
}
