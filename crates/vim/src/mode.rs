//! Vim mode definitions

/// Vim editing modes
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Normal mode - navigation and commands
    #[default]
    Normal,
    /// Insert mode - text entry
    Insert,
    /// Replace mode - overwrite text
    Replace,
    /// Visual mode - character-wise selection
    Visual,
    /// Visual line mode - line-wise selection
    VisualLine,
    /// Visual block mode - rectangular selection
    VisualBlock,
}

impl Mode {
    /// Check if this is any visual mode
    pub fn is_visual(&self) -> bool {
        matches!(self, Mode::Visual | Mode::VisualLine | Mode::VisualBlock)
    }

    /// Check if this is an insert-like mode
    pub fn is_insert(&self) -> bool {
        matches!(self, Mode::Insert | Mode::Replace)
    }

    /// Get the display label for this mode
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Replace => "REPLACE",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "V-LINE",
            Mode::VisualBlock => "V-BLOCK",
        }
    }

    /// Short label for status bar
    pub fn short_label(&self) -> &'static str {
        match self {
            Mode::Normal => "NOR",
            Mode::Insert => "INS",
            Mode::Replace => "REP",
            Mode::Visual => "VIS",
            Mode::VisualLine => "V-L",
            Mode::VisualBlock => "V-B",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_default() {
        assert_eq!(Mode::default(), Mode::Normal);
    }

    #[test]
    fn test_is_visual() {
        assert!(!Mode::Normal.is_visual());
        assert!(!Mode::Insert.is_visual());
        assert!(Mode::Visual.is_visual());
        assert!(Mode::VisualLine.is_visual());
        assert!(Mode::VisualBlock.is_visual());
    }

    #[test]
    fn test_is_insert() {
        assert!(!Mode::Normal.is_insert());
        assert!(Mode::Insert.is_insert());
        assert!(Mode::Replace.is_insert());
        assert!(!Mode::Visual.is_insert());
    }
}
