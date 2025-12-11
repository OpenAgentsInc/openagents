//! Editor module - Full IDE workspace with multiple panels
//!
//! Components:
//! - FileTree: Project file browser with OANIX namespace
//! - CodeEditor: Syntax-highlighted code editing area
//! - PreviewPanel: Live app preview
//! - TerminalPanel: OANIX terminal integration
//! - AgentPanel: AI agent task feed and controls

mod file_tree;
mod code_editor;
mod preview_panel;
mod terminal_panel;
mod agent_panel;
mod workspace;

pub use file_tree::render_file_tree;
pub use code_editor::render_code_editor;
pub use preview_panel::render_preview_panel;
pub use terminal_panel::render_terminal_panel;
pub use agent_panel::render_agent_panel;
pub use workspace::render_editor_workspace;
