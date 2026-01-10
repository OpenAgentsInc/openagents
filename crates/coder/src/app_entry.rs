//! Main application state and event handling.

use crate::app::AppState;

#[path = "app_entry/application.rs"]
mod application;
#[path = "app_entry/coder_actions.rs"]
mod coder_actions;
#[path = "app_entry/commands.rs"]
mod commands;
#[path = "app_entry/hooks.rs"]
mod hooks;
#[path = "app_entry/settings.rs"]
mod settings;
#[path = "app_entry/state_actions.rs"]
mod state_actions;

pub(crate) use hooks::{CoderHookCallback, HookCallbackKind};

mod command_palette_ids {
    pub const HELP: &str = "help.open";
    pub const SETTINGS: &str = "settings.open";
    pub const MODEL_PICKER: &str = "model.open";
    pub const SESSION_LIST: &str = "session.list";
    pub const SESSION_FORK: &str = "session.fork";
    pub const SESSION_EXPORT: &str = "session.export";
    pub const CLEAR_CONVERSATION: &str = "session.clear";
    pub const UNDO_LAST: &str = "session.undo";
    pub const COMPACT_CONTEXT: &str = "context.compact";
    pub const INTERRUPT_REQUEST: &str = "request.interrupt";
    pub const PERMISSION_RULES: &str = "permissions.rules";
    pub const MODE_CYCLE: &str = "mode.cycle";
    pub const MODE_BYPASS: &str = "mode.bypass";
    pub const MODE_PLAN: &str = "mode.plan";
    pub const MODE_AUTOPILOT: &str = "mode.autopilot";
    pub const TOOLS_LIST: &str = "tools.list";
    pub const MCP_CONFIG: &str = "mcp.open";
    pub const MCP_RELOAD: &str = "mcp.reload";
    pub const MCP_STATUS: &str = "mcp.status";
    pub const AGENTS_LIST: &str = "agents.list";
    pub const AGENT_CLEAR: &str = "agents.clear";
    pub const AGENT_RELOAD: &str = "agents.reload";
    pub const WALLET_OPEN: &str = "wallet.open";
    pub const DSPY_OPEN: &str = "dspy.open";
    pub const SKILLS_LIST: &str = "skills.list";
    pub const SKILLS_RELOAD: &str = "skills.reload";
    pub const HOOKS_OPEN: &str = "hooks.open";
    pub const HOOKS_RELOAD: &str = "hooks.reload";
    pub const SIDEBAR_LEFT: &str = "sidebar.left";
    pub const SIDEBAR_RIGHT: &str = "sidebar.right";
    pub const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub const BUG_REPORT: &str = "bug.report";
    pub const KITCHEN_SINK: &str = "dev.kitchen_sink";
}



/// Main application
pub struct CoderApp {
    state: Option<AppState>,
    runtime_handle: tokio::runtime::Handle,
}

impl CoderApp {
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            state: None,
            runtime_handle,
        }
    }
}
