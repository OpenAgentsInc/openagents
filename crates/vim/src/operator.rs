//! Vim operators (actions that require a motion or text object)

use crate::Mode;

/// Vim operators that act on text ranges
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operator {
    /// Delete text (d)
    Delete,
    /// Change text - delete and enter insert mode (c)
    Change,
    /// Yank/copy text (y)
    Yank,

    /// Text object selection with inner/around modifier
    Object {
        /// If true, include surrounding whitespace/delimiters (a)
        /// If false, only inner content (i)
        around: bool,
    },

    /// Find character forward (f)
    FindForward {
        /// If true, stop before the character (t)
        before: bool,
    },
    /// Find character backward (F)
    FindBackward {
        /// If true, stop after the character (T)
        after: bool,
    },

    /// Add surrounds (ys)
    AddSurrounds {
        /// Target character for surround
        target: Option<char>,
    },
    /// Change surrounds (cs)
    ChangeSurrounds {
        /// Original surround character
        from: Option<char>,
        /// New surround character
        to: Option<char>,
    },
    /// Delete surrounds (ds)
    DeleteSurrounds,

    /// Convert to lowercase (gu)
    Lowercase,
    /// Convert to uppercase (gU)
    Uppercase,
    /// Toggle case (g~)
    ToggleCase,

    /// Indent right (>)
    Indent,
    /// Indent left (<)
    Outdent,

    /// Replace character (r)
    Replace,

    /// Select register (")
    Register,

    /// Go to mark (')
    Mark,

    /// Record macro (q)
    RecordMacro,
}

impl Operator {
    /// Check if this operator is waiting for additional input
    pub fn is_waiting(&self, mode: Mode) -> bool {
        match self {
            Operator::Object { .. } => true,
            Operator::FindForward { .. } | Operator::FindBackward { .. } => true,
            Operator::AddSurrounds { target } => target.is_none(),
            Operator::ChangeSurrounds { from, to } => from.is_none() || to.is_none(),
            Operator::DeleteSurrounds => true,
            Operator::Replace => true,
            Operator::Register => true,
            Operator::Mark => true,
            Operator::RecordMacro => true,
            // These operators need a motion in normal mode
            Operator::Delete | Operator::Change | Operator::Yank => !mode.is_visual(),
            Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => !mode.is_visual(),
            Operator::Indent | Operator::Outdent => !mode.is_visual(),
        }
    }

    /// Get the display string for this operator
    pub fn display(&self) -> &'static str {
        match self {
            Operator::Delete => "d",
            Operator::Change => "c",
            Operator::Yank => "y",
            Operator::Object { around: true } => "a",
            Operator::Object { around: false } => "i",
            Operator::FindForward { before: false } => "f",
            Operator::FindForward { before: true } => "t",
            Operator::FindBackward { after: false } => "F",
            Operator::FindBackward { after: true } => "T",
            Operator::AddSurrounds { .. } => "ys",
            Operator::ChangeSurrounds { .. } => "cs",
            Operator::DeleteSurrounds => "ds",
            Operator::Lowercase => "gu",
            Operator::Uppercase => "gU",
            Operator::ToggleCase => "g~",
            Operator::Indent => ">",
            Operator::Outdent => "<",
            Operator::Replace => "r",
            Operator::Register => "\"",
            Operator::Mark => "'",
            Operator::RecordMacro => "q",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_waiting() {
        assert!(Operator::Delete.is_waiting(Mode::Normal));
        assert!(!Operator::Delete.is_waiting(Mode::Visual));

        assert!(Operator::Object { around: true }.is_waiting(Mode::Normal));
        assert!(Operator::FindForward { before: false }.is_waiting(Mode::Normal));
    }
}
