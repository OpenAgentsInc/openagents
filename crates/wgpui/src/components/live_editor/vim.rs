//! Vim mode state machine for LiveEditor

use super::Cursor;

/// Vim modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
}

impl VimMode {
    pub fn label(&self) -> &'static str {
        match self {
            VimMode::Normal => "NORMAL",
            VimMode::Insert => "INSERT",
            VimMode::Visual => "VISUAL",
            VimMode::VisualLine => "V-LINE",
        }
    }
}

/// Pending operator waiting for a motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingOperator {
    Delete, // d
    Change, // c
    Yank,   // y
}

/// Vim state machine
#[derive(Debug, Clone, Default)]
pub struct VimState {
    /// Current vim mode
    pub mode: VimMode,

    /// Count prefix (e.g., "3" in "3j")
    pub count: Option<usize>,

    /// Pending operator (e.g., "d" waiting for motion)
    pub pending_operator: Option<PendingOperator>,

    /// Vim-specific register (for yanked text)
    pub register: Option<String>,

    /// Visual mode anchor (where visual selection started)
    pub visual_anchor: Option<Cursor>,

    /// Pending 'g' key for gg motion
    pub pending_g: bool,
}

impl VimState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset operator/count state (after executing an operation)
    pub fn reset_pending(&mut self) {
        self.count = None;
        self.pending_operator = None;
        self.pending_g = false;
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

    /// Check if we're waiting for a motion after an operator
    pub fn awaiting_motion(&self) -> bool {
        self.pending_operator.is_some()
    }

    /// Enter insert mode
    pub fn enter_insert(&mut self) {
        self.mode = VimMode::Insert;
        self.reset_pending();
    }

    /// Return to normal mode
    pub fn enter_normal(&mut self) {
        self.mode = VimMode::Normal;
        self.reset_pending();
        self.visual_anchor = None;
    }

    /// Enter visual mode
    pub fn enter_visual(&mut self, anchor: Cursor) {
        self.mode = VimMode::Visual;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }

    /// Enter visual line mode
    pub fn enter_visual_line(&mut self, anchor: Cursor) {
        self.mode = VimMode::VisualLine;
        self.visual_anchor = Some(anchor);
        self.reset_pending();
    }
}
