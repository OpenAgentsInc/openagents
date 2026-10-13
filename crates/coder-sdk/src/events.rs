//! The normalized hook events, named as Claude Code names them, and the
//! call a hook sees.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One hook event. Serialized under its Claude Code name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Event {
    /// A session starts or resumes.
    SessionStart,
    /// A message enters the session.
    UserPromptSubmit,
    /// Before a tool runs.
    PreToolUse,
    /// After a tool returns.
    PostToolUse,
    /// After a batch of parallel tools.
    PostToolBatch,
    /// The agent says it is done.
    Stop,
    PreCompact,
    PostCompact,
    SubagentStart,
    SubagentStop,
    /// A file in the workspace changed.
    FileChanged,
    /// The session is over.
    SessionEnd,
}

impl Event {
    /// Every event, in `coder_delegate::session::HOOK_EVENTS` order.
    pub const ALL: [Event; 12] = [
        Event::SessionStart,
        Event::UserPromptSubmit,
        Event::PreToolUse,
        Event::PostToolUse,
        Event::PostToolBatch,
        Event::Stop,
        Event::PreCompact,
        Event::PostCompact,
        Event::SubagentStart,
        Event::SubagentStop,
        Event::FileChanged,
        Event::SessionEnd,
    ];

    /// The event's Claude Code name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Event::SessionStart => "SessionStart",
            Event::UserPromptSubmit => "UserPromptSubmit",
            Event::PreToolUse => "PreToolUse",
            Event::PostToolUse => "PostToolUse",
            Event::PostToolBatch => "PostToolBatch",
            Event::Stop => "Stop",
            Event::PreCompact => "PreCompact",
            Event::PostCompact => "PostCompact",
            Event::SubagentStart => "SubagentStart",
            Event::SubagentStop => "SubagentStop",
            Event::FileChanged => "FileChanged",
            Event::SessionEnd => "SessionEnd",
        }
    }

    /// The event with this Claude Code name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Event> {
        Event::ALL.into_iter().find(|event| event.name() == name)
    }

    /// Whether a hook on this event can rewrite the input.
    #[must_use]
    pub fn rewrites(self) -> bool {
        self == Event::PreToolUse
    }
}

/// One firing of an event: what a hook sees.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub event: Event,
    /// The tool, for the tool events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// The event's payload: a tool's input, a prompt, a changed path.
    #[serde(default)]
    pub input: Value,
}

impl Call {
    #[must_use]
    pub fn new(event: Event, input: Value) -> Self {
        Call {
            event,
            tool: None,
            input,
        }
    }

    /// A tool event on `tool`.
    #[must_use]
    pub fn tool(event: Event, tool: &str, input: Value) -> Self {
        Call {
            event,
            tool: Some(tool.to_string()),
            input,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_events_carry_the_claude_code_names_both_ways() {
        let names: Vec<&str> = Event::ALL.iter().map(|event| event.name()).collect();
        assert_eq!(names, coder_delegate::session::HOOK_EVENTS);
        for event in Event::ALL {
            assert_eq!(Event::from_name(event.name()), Some(event));
            assert_eq!(serde_json::json!(event), serde_json::json!(event.name()));
        }
        assert_eq!(Event::from_name("Mystery"), None);
        assert!(Event::PreToolUse.rewrites() && !Event::PostToolUse.rewrites());
    }
}
