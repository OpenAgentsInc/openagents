//! BlackBox organism components.

mod agent_line;
mod hour_divider;
mod lifecycle_line;
mod mcp_line;
mod phase_line;
mod question_line;
mod recall_line;
mod styles;
mod subagent_line;
mod time_marker;
mod tool_line;
mod user_line;

pub use agent_line::AgentLine;
pub use hour_divider::hour_divider;
pub use lifecycle_line::{LifecycleEvent, lifecycle_line};
pub use mcp_line::McpLine;
pub use phase_line::phase_line;
pub use question_line::QuestionLine;
pub use recall_line::RecallLine;
pub use subagent_line::SubagentLine;
pub use time_marker::time_marker;
pub use tool_line::ToolLine;
pub use user_line::UserLine;
