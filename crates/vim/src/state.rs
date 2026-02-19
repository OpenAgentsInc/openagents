//! Vim state machine

use std::collections::HashMap;

use crate::{Mode, Motion, Operator};

/// Action that was recorded for dot-repeat or macros
#[derive(Clone, Debug)]
pub struct RecordedAction {
    /// The keys that were pressed
    pub keys: String,
    /// Register used (if any)
    pub register: Option<char>,
}

/// Core vim state machine
#[derive(Clone, Debug)]
pub struct VimState {
    /// Current vim mode
    pub mode: Mode,

    /// Stack of pending operators (e.g., for "d" waiting for motion)
    /// Using a stack allows composable operators like "y2d3w"
    pub operator_stack: Vec<Operator>,

    /// Count prefix (e.g., "3" in "3j")
    pub count: Option<usize>,

    /// Selected register (after ")
    pub selected_register: Option<char>,

    /// Last find motion for ; and , repeat
    pub last_find: Option<Motion>,

    /// Pending key sequence (for multi-key commands like gg, gc, etc.)
    pub pending_keys: String,

    /// Visual mode anchor (where selection started)
    pub visual_anchor: Option<(usize, usize)>,

    // Registers
    /// Named registers (a-z, A-Z appends to a-z)
    pub registers: HashMap<char, String>,

    /// Unnamed register (")
    pub unnamed_register: String,

    /// Small delete register (-)
    pub small_delete_register: String,

    /// Numbered registers (0-9) - 0 is yank, 1-9 are delete history
    pub numbered_registers: [String; 10],

    /// Last search register (/)
    pub last_search: Option<String>,

    // Recording
    /// Register being recorded to (if any)
    pub recording_register: Option<char>,

    /// Actions recorded in current macro
    pub recorded_actions: Vec<RecordedAction>,

    /// Last change for dot-repeat
    pub last_change: Option<RecordedAction>,

    // Marks
    /// Local marks (a-z) - (line, column)
    pub local_marks: HashMap<char, (usize, usize)>,

    /// Jump list for Ctrl-O/Ctrl-I
    pub jump_list: Vec<(usize, usize)>,
    pub jump_list_index: usize,
}

impl Default for VimState {
    fn default() -> Self {
        Self::new()
    }
}

impl VimState {
    /// Create a new vim state in normal mode
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            operator_stack: Vec::new(),
            count: None,
            selected_register: None,
            last_find: None,
            pending_keys: String::new(),
            visual_anchor: None,
            registers: HashMap::new(),
            unnamed_register: String::new(),
            small_delete_register: String::new(),
            numbered_registers: Default::default(),
            last_search: None,
            recording_register: None,
            recorded_actions: Vec::new(),
            last_change: None,
            local_marks: HashMap::new(),
            jump_list: Vec::new(),
            jump_list_index: 0,
        }
    }

    /// Push an operator onto the stack
    pub fn push_operator(&mut self, op: Operator) {
        self.operator_stack.push(op);
    }

    /// Pop an operator from the stack
    pub fn pop_operator(&mut self) -> Option<Operator> {
        self.operator_stack.pop()
    }

    /// Get the active (most recent) operator
    pub fn active_operator(&self) -> Option<&Operator> {
        self.operator_stack.last()
    }

    /// Check if there's a pending operator
    pub fn has_pending_operator(&self) -> bool {
        !self.operator_stack.is_empty()
    }

    /// Get the effective count (defaults to 1)
    pub fn effective_count(&self) -> usize {
        self.count.unwrap_or(1)
    }

    /// Accumulate a digit into the count
    pub fn push_count_digit(&mut self, digit: u8) {
        let current = self.count.unwrap_or(0);
        self.count = Some(current * 10 + digit as usize);
    }

    /// Clear count
    pub fn clear_count(&mut self) {
        self.count = None;
    }

    /// Reset pending state (after executing command)
    pub fn reset_pending(&mut self) {
        self.count = None;
        self.operator_stack.clear();
        self.pending_keys.clear();
        self.selected_register = None;
    }

    /// Enter insert mode
    pub fn enter_insert(&mut self) {
        self.mode = Mode::Insert;
        self.reset_pending();
        self.visual_anchor = None;
    }

    /// Enter replace mode
    pub fn enter_replace(&mut self) {
        self.mode = Mode::Replace;
        self.reset_pending();
        self.visual_anchor = None;
    }

    /// Return to normal mode
    pub fn enter_normal(&mut self) {
        self.mode = Mode::Normal;
        self.reset_pending();
        self.visual_anchor = None;
    }

    /// Enter visual mode with anchor at current position
    pub fn enter_visual(&mut self, anchor: (usize, usize)) {
        self.mode = Mode::Visual;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }

    /// Enter visual line mode with anchor at current line
    pub fn enter_visual_line(&mut self, anchor: (usize, usize)) {
        self.mode = Mode::VisualLine;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }

    /// Enter visual block mode with anchor at current position
    pub fn enter_visual_block(&mut self, anchor: (usize, usize)) {
        self.mode = Mode::VisualBlock;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }

    /// Get the visual selection range (anchor, current) normalized to (start, end)
    pub fn visual_range(
        &self,
        current: (usize, usize),
    ) -> Option<((usize, usize), (usize, usize))> {
        self.visual_anchor.map(|anchor| {
            if anchor.0 < current.0 || (anchor.0 == current.0 && anchor.1 <= current.1) {
                (anchor, current)
            } else {
                (current, anchor)
            }
        })
    }

    /// Store text in a register
    pub fn store_register(&mut self, text: String, is_delete: bool) {
        let register = self.selected_register.unwrap_or('"');

        // Store in unnamed register
        self.unnamed_register = text.clone();

        // Handle register by type
        match register {
            '"' => {
                // Unnamed register - also update numbered if delete
                if is_delete {
                    self.shift_numbered_registers(text);
                } else {
                    self.numbered_registers[0] = text;
                }
            }
            '0' => {
                // Yank register
                self.numbered_registers[0] = text;
            }
            '1'..='9' => {
                // Numbered registers - explicit set
                let idx = (register as u8 - b'0') as usize;
                self.numbered_registers[idx] = text;
            }
            'a'..='z' => {
                // Named register - set
                self.registers.insert(register, text);
            }
            'A'..='Z' => {
                // Named register - append
                let lower = register.to_ascii_lowercase();
                let existing = self.registers.get(&lower).cloned().unwrap_or_default();
                self.registers.insert(lower, existing + &text);
            }
            '-' => {
                // Small delete register
                self.small_delete_register = text;
            }
            '_' => {
                // Black hole register - discard
            }
            '+' | '*' => {
                // System clipboard - handled by editor
                // Store locally as well
                self.registers.insert(register, text);
            }
            _ => {
                // Unknown register, use unnamed
            }
        }
    }

    /// Get text from a register
    pub fn get_register(&self, register: Option<char>) -> Option<&str> {
        let register = register.unwrap_or('"');

        match register {
            '"' => Some(&self.unnamed_register),
            '0'..='9' => {
                let idx = (register as u8 - b'0') as usize;
                Some(&self.numbered_registers[idx])
            }
            'a'..='z' | 'A'..='Z' => {
                let lower = register.to_ascii_lowercase();
                self.registers.get(&lower).map(|s| s.as_str())
            }
            '-' => Some(&self.small_delete_register),
            '+' | '*' => self.registers.get(&register).map(|s| s.as_str()),
            _ => None,
        }
    }

    /// Shift numbered registers (for delete operations)
    fn shift_numbered_registers(&mut self, new_text: String) {
        // Shift 1-8 to 2-9
        for i in (1..9).rev() {
            self.numbered_registers[i + 1] = self.numbered_registers[i].clone();
        }
        self.numbered_registers[1] = new_text;
    }

    /// Set a local mark
    pub fn set_mark(&mut self, mark: char, position: (usize, usize)) {
        if mark.is_ascii_lowercase() {
            self.local_marks.insert(mark, position);
        }
    }

    /// Get a local mark
    pub fn get_mark(&self, mark: char) -> Option<(usize, usize)> {
        if mark.is_ascii_lowercase() {
            self.local_marks.get(&mark).copied()
        } else {
            None
        }
    }

    /// Add position to jump list
    pub fn push_jump(&mut self, position: (usize, usize)) {
        // Truncate jump list at current position
        if self.jump_list_index < self.jump_list.len() {
            self.jump_list.truncate(self.jump_list_index);
        }
        self.jump_list.push(position);
        self.jump_list_index = self.jump_list.len();

        // Limit jump list size
        if self.jump_list.len() > 100 {
            self.jump_list.remove(0);
            self.jump_list_index = self.jump_list_index.saturating_sub(1);
        }
    }

    /// Go to previous position in jump list (Ctrl-O)
    pub fn jump_back(&mut self) -> Option<(usize, usize)> {
        if self.jump_list_index > 0 {
            self.jump_list_index -= 1;
            self.jump_list.get(self.jump_list_index).copied()
        } else {
            None
        }
    }

    /// Go to next position in jump list (Ctrl-I)
    pub fn jump_forward(&mut self) -> Option<(usize, usize)> {
        if self.jump_list_index < self.jump_list.len().saturating_sub(1) {
            self.jump_list_index += 1;
            self.jump_list.get(self.jump_list_index).copied()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let state = VimState::new();
        assert_eq!(state.mode, Mode::Normal);
        assert!(state.operator_stack.is_empty());
        assert_eq!(state.count, None);
    }

    #[test]
    fn test_mode_transitions() {
        let mut state = VimState::new();

        state.enter_insert();
        assert_eq!(state.mode, Mode::Insert);

        state.enter_normal();
        assert_eq!(state.mode, Mode::Normal);

        state.enter_visual((0, 0));
        assert_eq!(state.mode, Mode::Visual);
        assert_eq!(state.visual_anchor, Some((0, 0)));
    }

    #[test]
    fn test_count() {
        let mut state = VimState::new();
        state.push_count_digit(1);
        state.push_count_digit(2);
        state.push_count_digit(3);
        assert_eq!(state.effective_count(), 123);
    }

    #[test]
    fn test_operator_stack() {
        let mut state = VimState::new();
        state.push_operator(Operator::Delete);
        assert!(state.has_pending_operator());
        assert_eq!(state.active_operator(), Some(&Operator::Delete));
        assert_eq!(state.pop_operator(), Some(Operator::Delete));
        assert!(!state.has_pending_operator());
    }

    #[test]
    fn test_registers() {
        let mut state = VimState::new();

        // Yank to register a
        state.selected_register = Some('a');
        state.store_register("hello".to_string(), false);
        assert_eq!(state.get_register(Some('a')), Some("hello"));

        // Append to register A
        state.selected_register = Some('A');
        state.store_register(" world".to_string(), false);
        assert_eq!(state.get_register(Some('a')), Some("hello world"));
    }
}
