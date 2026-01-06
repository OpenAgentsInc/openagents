//! Vim motions - cursor movements

/// How a motion affects selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionKind {
    /// Selection includes whole lines
    Linewise,
    /// Selection excludes the final character
    Exclusive,
    /// Selection includes the final character
    Inclusive,
}

/// Vim cursor motions
#[derive(Clone, Debug, PartialEq)]
pub enum Motion {
    // Basic character/line movement
    /// Move left (h)
    Left,
    /// Move right (l)
    Right,
    /// Move up (k)
    Up,
    /// Move down (j)
    Down,

    // Line position
    /// Go to start of line (0)
    LineStart,
    /// Go to end of line ($)
    LineEnd,
    /// Go to first non-blank character (^)
    FirstNonBlank,
    /// Go to last non-blank character (g_)
    LastNonBlank,

    // Word movement
    /// Move to next word start (w/W)
    NextWordStart {
        /// If true, use WORD (whitespace-delimited), not word
        ignore_punctuation: bool,
    },
    /// Move to next word end (e/E)
    NextWordEnd {
        /// If true, use WORD (whitespace-delimited), not word
        ignore_punctuation: bool,
    },
    /// Move to previous word start (b/B)
    PrevWordStart {
        /// If true, use WORD (whitespace-delimited), not word
        ignore_punctuation: bool,
    },
    /// Move to previous word end (ge/gE)
    PrevWordEnd {
        /// If true, use WORD (whitespace-delimited), not word
        ignore_punctuation: bool,
    },

    // Paragraph movement
    /// Move to next paragraph (})
    ParagraphForward,
    /// Move to previous paragraph ({)
    ParagraphBackward,

    // Sentence movement
    /// Move to next sentence ())
    SentenceForward,
    /// Move to previous sentence (()
    SentenceBackward,

    // Document movement
    /// Go to start of document (gg)
    DocumentStart,
    /// Go to end of document or specific line (G)
    DocumentEnd {
        /// If Some, go to this line number instead
        line: Option<usize>,
    },
    /// Go to specific line
    GoToLine {
        line: usize,
    },
    /// Go to percentage of file (%)
    GoToPercentage {
        percentage: usize,
    },

    // Find character
    /// Find character forward (f)
    FindChar {
        char: char,
        /// If true, stop before the character (t)
        before: bool,
    },
    /// Find character backward (F)
    FindCharBackward {
        char: char,
        /// If true, stop after the character (T)
        after: bool,
    },
    /// Repeat last find motion (;)
    RepeatFind,
    /// Repeat last find motion in reverse (,)
    RepeatFindReverse,

    // Matching
    /// Go to matching bracket (%)
    MatchingBracket,

    // Screen/window movement
    /// Go to top of visible window (H)
    WindowTop,
    /// Go to middle of visible window (M)
    WindowMiddle,
    /// Go to bottom of visible window (L)
    WindowBottom,

    // Scroll
    /// Page down (Ctrl-F)
    PageDown,
    /// Page up (Ctrl-B)
    PageUp,
    /// Half page down (Ctrl-D)
    HalfPageDown,
    /// Half page up (Ctrl-U)
    HalfPageUp,

    // Search
    /// Go to next search match (n)
    SearchNext,
    /// Go to previous search match (N)
    SearchPrev,
    /// Search forward (/)
    SearchForward {
        query: String,
    },
    /// Search backward (?)
    SearchBackward {
        query: String,
    },
    /// Search for word under cursor (*)
    SearchWordForward,
    /// Search for word under cursor backward (#)
    SearchWordBackward,

    // Marks
    /// Go to mark position (`)
    GoToMark {
        mark: char,
    },
    /// Go to mark line (')
    GoToMarkLine {
        mark: char,
    },

    // Other
    /// Current line (used for operators like dd, yy)
    CurrentLine,
    /// To end of line from cursor (used for D, C)
    ToEndOfLine,
}

impl Motion {
    /// Get the kind of this motion (how it affects selection)
    pub fn kind(&self) -> MotionKind {
        match self {
            // Linewise motions
            Motion::Up
            | Motion::Down
            | Motion::ParagraphForward
            | Motion::ParagraphBackward
            | Motion::DocumentStart
            | Motion::DocumentEnd { .. }
            | Motion::GoToLine { .. }
            | Motion::GoToPercentage { .. }
            | Motion::WindowTop
            | Motion::WindowMiddle
            | Motion::WindowBottom
            | Motion::CurrentLine
            | Motion::GoToMarkLine { .. } => MotionKind::Linewise,

            // Inclusive motions
            Motion::FindChar { .. }
            | Motion::FindCharBackward { .. }
            | Motion::RepeatFind
            | Motion::RepeatFindReverse
            | Motion::NextWordEnd { .. }
            | Motion::PrevWordEnd { .. }
            | Motion::LineEnd
            | Motion::LastNonBlank
            | Motion::ToEndOfLine
            | Motion::MatchingBracket
            | Motion::SearchNext
            | Motion::SearchPrev
            | Motion::SearchForward { .. }
            | Motion::SearchBackward { .. }
            | Motion::SearchWordForward
            | Motion::SearchWordBackward
            | Motion::GoToMark { .. } => MotionKind::Inclusive,

            // Exclusive motions (default)
            Motion::Left
            | Motion::Right
            | Motion::LineStart
            | Motion::FirstNonBlank
            | Motion::NextWordStart { .. }
            | Motion::PrevWordStart { .. }
            | Motion::SentenceForward
            | Motion::SentenceBackward
            | Motion::PageDown
            | Motion::PageUp
            | Motion::HalfPageDown
            | Motion::HalfPageUp => MotionKind::Exclusive,
        }
    }

    /// Check if this is a linewise motion
    pub fn is_linewise(&self) -> bool {
        self.kind() == MotionKind::Linewise
    }

    /// Check if this is an inclusive motion
    pub fn is_inclusive(&self) -> bool {
        self.kind() == MotionKind::Inclusive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motion_kinds() {
        assert_eq!(Motion::Down.kind(), MotionKind::Linewise);
        assert_eq!(Motion::LineEnd.kind(), MotionKind::Inclusive);
        assert_eq!(Motion::Left.kind(), MotionKind::Exclusive);
    }

    #[test]
    fn test_word_motions() {
        let w = Motion::NextWordStart {
            ignore_punctuation: false,
        };
        let big_w = Motion::NextWordStart {
            ignore_punctuation: true,
        };
        assert_eq!(w.kind(), MotionKind::Exclusive);
        assert_eq!(big_w.kind(), MotionKind::Exclusive);
    }
}
