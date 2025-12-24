//! ACP atom components - simple, single-purpose UI elements.

mod tool_icon;
mod tool_status_badge;
mod permission_button;
mod mode_badge;
mod model_badge;
mod thinking_toggle;
mod checkpoint_badge;
mod feedback_button;
mod content_type_icon;
mod entry_marker;
mod keybinding_hint;
mod streaming_indicator;

pub use tool_icon::{tool_icon, ToolKind};
pub use tool_status_badge::{tool_status_badge, ToolStatus};
pub use permission_button::{permission_button, PermissionKind};
pub use mode_badge::{mode_badge, AgentMode};
pub use model_badge::model_badge;
pub use thinking_toggle::{thinking_toggle, ThinkingState};
pub use checkpoint_badge::checkpoint_badge;
pub use feedback_button::{feedback_button, FeedbackKind, FeedbackState};
pub use content_type_icon::{content_type_icon, ContentType};
pub use entry_marker::{entry_marker, EntryKind};
pub use keybinding_hint::keybinding_hint;
pub use streaming_indicator::streaming_indicator;
