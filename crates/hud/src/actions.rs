//! Actions: Graph editor keyboard shortcuts and commands
//!
//! Defines actions that can be triggered via keyboard shortcuts or menus.
//! Uses GPUI's action system for consistent handling.

use gpui::actions;

// Define graph editor actions
actions!(
    graph_editor,
    [
        // Selection
        SelectAll,
        DeselectAll,
        InvertSelection,
        // Clipboard
        Cut,
        Copy,
        Paste,
        Duplicate,
        // Editing
        Delete,
        Undo,
        Redo,
        // View
        ZoomIn,
        ZoomOut,
        ZoomToFit,
        ZoomToSelection,
        ResetView,
        // Arrangement
        AlignLeft,
        AlignRight,
        AlignTop,
        AlignBottom,
        AlignCenterH,
        AlignCenterV,
        DistributeH,
        DistributeV,
        // Graph
        AddNode,
        GroupSelection,
        UngroupSelection,
    ]
);

/// Key bindings for graph editor
pub mod bindings {
    /// Default key bindings as context-aware strings
    pub const DELETE: &str = "backspace";
    pub const DELETE_ALT: &str = "delete";
    pub const SELECT_ALL: &str = "cmd-a";
    pub const DESELECT: &str = "escape";
    pub const CUT: &str = "cmd-x";
    pub const COPY: &str = "cmd-c";
    pub const PASTE: &str = "cmd-v";
    pub const DUPLICATE: &str = "cmd-d";
    pub const UNDO: &str = "cmd-z";
    pub const REDO: &str = "cmd-shift-z";
    pub const ZOOM_IN: &str = "cmd-=";
    pub const ZOOM_OUT: &str = "cmd--";
    pub const ZOOM_FIT: &str = "cmd-0";
    pub const GROUP: &str = "cmd-g";
    pub const UNGROUP: &str = "cmd-shift-g";
}

/// Represents a clipboard entry for graph elements
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    /// Type of entry
    pub entry_type: ClipboardEntryType,
    /// Serialized data
    pub data: String,
}

/// Types of clipboard entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardEntryType {
    /// Single node
    Node,
    /// Multiple nodes with connections
    Subgraph,
    /// Connection specification
    Connection,
}

/// Clipboard for graph operations (cut/copy/paste)
#[derive(Debug, Clone, Default)]
pub struct GraphClipboard {
    /// Current clipboard contents
    entries: Vec<ClipboardEntry>,
    /// Paste offset (increments on each paste)
    paste_offset: (f32, f32),
}

impl GraphClipboard {
    /// Create a new empty clipboard
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the clipboard
    pub fn clear(&mut self) {
        self.entries.clear();
        self.paste_offset = (0.0, 0.0);
    }

    /// Check if clipboard is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Set clipboard contents
    pub fn set(&mut self, entries: Vec<ClipboardEntry>) {
        self.entries = entries;
        self.paste_offset = (20.0, 20.0); // Initial paste offset
    }

    /// Get clipboard contents
    pub fn get(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    /// Get current paste offset and increment for next paste
    pub fn next_paste_offset(&mut self) -> (f32, f32) {
        let offset = self.paste_offset;
        self.paste_offset.0 += 20.0;
        self.paste_offset.1 += 20.0;
        offset
    }

    /// Reset paste offset (e.g., after new copy)
    pub fn reset_paste_offset(&mut self) {
        self.paste_offset = (20.0, 20.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_empty() {
        let clipboard = GraphClipboard::new();
        assert!(clipboard.is_empty());
    }

    #[test]
    fn test_clipboard_set_get() {
        let mut clipboard = GraphClipboard::new();

        clipboard.set(vec![ClipboardEntry {
            entry_type: ClipboardEntryType::Node,
            data: "test".to_string(),
        }]);

        assert!(!clipboard.is_empty());
        assert_eq!(clipboard.get().len(), 1);
    }

    #[test]
    fn test_paste_offset() {
        let mut clipboard = GraphClipboard::new();
        clipboard.set(vec![]);

        let first = clipboard.next_paste_offset();
        assert_eq!(first, (20.0, 20.0));

        let second = clipboard.next_paste_offset();
        assert_eq!(second, (40.0, 40.0));

        clipboard.reset_paste_offset();
        let reset = clipboard.next_paste_offset();
        assert_eq!(reset, (20.0, 20.0));
    }
}
