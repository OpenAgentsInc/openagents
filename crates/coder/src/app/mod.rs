pub(crate) mod agents;
pub(crate) mod autopilot;
pub(crate) mod autopilot_issues;
pub(crate) mod catalog;
pub(crate) mod chat;
pub(crate) mod config;
pub(crate) mod directives;
pub(crate) mod dspy;
pub(crate) mod dvm;
pub(crate) mod events;
pub(crate) mod gateway;
pub(crate) mod issues;
pub(crate) mod rlm;
pub(crate) mod pylon_earnings;
pub(crate) mod pylon_jobs;
pub(crate) mod pylon_paths;
pub(crate) mod lm_router;
pub(crate) mod nexus;
pub(crate) mod nip28;
pub(crate) mod nip90;
pub(crate) mod oanix;
pub(crate) mod parsing;
pub(crate) mod permissions;
pub(crate) mod session;
pub(crate) mod spark_wallet;
pub(crate) mod state;
pub(crate) mod tools;
pub(crate) mod ui;
pub(crate) mod utils;
pub(crate) mod wallet;

pub(crate) use state::AppState;
pub use events::CoderMode;
pub(crate) use catalog::{
    AgentCardAction, HookLogEntry, HookModalView, HookSetting, SkillCardAction,
};
pub(crate) use config::{settings_rows, ModelOption, SettingsInputMode, SettingsSnapshot};
pub(crate) use permissions::sanitize_tokens;
pub(crate) use session::SessionCardEvent;
pub(crate) use crate::app_entry::{CoderHookCallback, HookCallbackKind};
pub(crate) use utils::{
    agent_capabilities, build_checkpoint_entries, build_input, build_markdown_config,
    build_markdown_document, build_markdown_renderer, default_auto_scroll, default_font_size,
    default_session_auto_save, default_session_history_limit, format_relative_time, hook_event_label,
    now_timestamp, selection_point_cmp, strip_markdown_markers, truncate_bytes, truncate_preview,
};


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
