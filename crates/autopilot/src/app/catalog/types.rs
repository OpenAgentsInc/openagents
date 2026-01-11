use claude_agent_sdk::HookEvent;
use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct AgentCardEvent {
    pub(crate) action: AgentCardAction,
    pub(crate) agent_id: String,
}

#[derive(Clone, Debug)]
pub(crate) enum AgentCardAction {
    Select,
    ToggleActive,
}

#[derive(Clone, Debug)]
pub(crate) struct SkillCardEvent {
    pub(crate) action: SkillCardAction,
    pub(crate) skill_id: String,
}

#[derive(Clone, Debug)]
pub(crate) enum SkillCardAction {
    View,
    Install,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookModalView {
    Config,
    Events,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookSetting {
    ToolBlocker,
    ToolLogger,
    OutputTruncator,
    ContextInjection,
    TodoEnforcer,
}

#[derive(Clone, Debug)]
pub(crate) struct HookLogEntry {
    pub(crate) id: String,
    pub(crate) event: HookEvent,
    pub(crate) timestamp: u64,
    pub(crate) summary: String,
    pub(crate) tool_name: Option<String>,
    pub(crate) matcher: Option<String>,
    pub(crate) input: Value,
    pub(crate) output: Option<Value>,
    pub(crate) error: Option<String>,
    pub(crate) sources: Vec<String>,
}
