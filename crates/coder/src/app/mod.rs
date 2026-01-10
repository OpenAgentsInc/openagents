pub(crate) mod autopilot;
pub(crate) mod catalog;
pub(crate) mod chat;
pub(crate) mod config;
pub(crate) mod events;
pub(crate) mod parsing;
pub(crate) mod permissions;
pub(crate) mod session;
pub(crate) mod state;
pub(crate) mod tools;
pub(crate) mod ui;
pub(crate) mod utils;

pub(crate) use state::AppState;
pub use events::CoderMode;
pub(crate) use config::{settings_rows, ModelOption, SettingsInputMode, SettingsSnapshot};
pub(crate) use permissions::sanitize_tokens;
pub(crate) use session::SessionCardEvent;
pub(crate) use crate::app_entry::{CoderHookCallback, HookCallbackKind};
pub(crate) use utils::{
    agent_capabilities, build_checkpoint_entries, build_input, build_markdown_config,
    build_markdown_document, build_markdown_renderer, default_auto_scroll, default_font_size,
    default_session_auto_save, default_session_history_limit, format_relative_time, hook_event_label,
    now_timestamp, selection_point_cmp, truncate_bytes, truncate_preview,
};

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

pub(crate) const HOOK_SCRIPT_TIMEOUT_SECS: u64 = 12;
pub(crate) const TOOL_HISTORY_LIMIT: usize = 100;

impl Drop for AppState {
    fn drop(&mut self) {
        // Kill auto-started llama-server process
        if let Some(mut child) = self.llama_server_process.take() {
            tracing::info!("Stopping llama-server (PID {})...", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
