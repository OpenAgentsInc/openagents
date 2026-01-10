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
pub(crate) use permissions::sanitize_tokens;
pub(crate) use crate::app_entry::{CoderHookCallback, HookCallbackKind};
pub(crate) use utils::{
    agent_capabilities, build_checkpoint_entries, build_input, build_markdown_config,
    build_markdown_document, build_markdown_renderer, default_auto_scroll, default_font_size,
    default_session_auto_save, default_session_history_limit, format_relative_time, hook_event_label,
    now_timestamp, selection_point_cmp, truncate_bytes, truncate_preview,
};

use claude_agent_sdk::HookEvent;
use serde_json::Value;

use crate::keybindings::{Action as KeyAction, Keybinding};
use crate::app::catalog::HookConfig;
use crate::app::config::{CoderSettings, SettingsItem, SettingsRow, SettingsTab};
use crate::app::events::format_keybinding;
use crate::app::ui::theme_label;

#[derive(Clone, Debug)]
pub(crate) struct SessionCardEvent {
    pub(crate) action: wgpui::components::molecules::SessionAction,
    pub(crate) session_id: String,
}

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ModelOption {
    Opus,
    Sonnet,
    Haiku,
}

impl ModelOption {
    pub(crate) fn all() -> [ModelOption; 3] {
        [ModelOption::Opus, ModelOption::Sonnet, ModelOption::Haiku]
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            ModelOption::Opus => "Default (recommended)",
            ModelOption::Sonnet => "Sonnet",
            ModelOption::Haiku => "Haiku",
        }
    }

    pub(crate) fn model_id(&self) -> &'static str {
        match self {
            ModelOption::Opus => "claude-opus-4-5-20251101",
            ModelOption::Sonnet => "claude-sonnet-4-5-20250929",
            ModelOption::Haiku => "claude-haiku-4-5-20251001",
        }
    }

    pub(crate) fn from_id(id: &str) -> ModelOption {
        match id {
            "claude-opus-4-5-20251101" => ModelOption::Opus,
            "claude-sonnet-4-5-20250929" => ModelOption::Sonnet,
            "claude-haiku-4-5-20251001" => ModelOption::Haiku,
            _ => ModelOption::Opus, // Default fallback
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        match self {
            ModelOption::Opus => "Opus 4.5 · Most capable for complex work",
            ModelOption::Sonnet => "Sonnet 4.5 · Best for everyday tasks",
            ModelOption::Haiku => "Haiku 4.5 · Fastest for quick answers",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsInputMode {
    Normal,
    Search,
    Capture(KeyAction),
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
#[derive(Clone)]
pub(crate) struct SettingsSnapshot {
    pub(crate) settings: CoderSettings,
    pub(crate) selected_model: ModelOption,
    pub(crate) coder_mode: CoderMode,
    pub(crate) permission_default_allow: bool,
    pub(crate) permission_allow_count: usize,
    pub(crate) permission_deny_count: usize,
    pub(crate) permission_bash_allow_count: usize,
    pub(crate) permission_bash_deny_count: usize,
    pub(crate) mcp_project_count: usize,
    pub(crate) mcp_runtime_count: usize,
    pub(crate) mcp_disabled_count: usize,
    pub(crate) hook_config: HookConfig,
    pub(crate) keybindings: Vec<Keybinding>,
}

impl SettingsSnapshot {
    pub(crate) fn from_state(state: &AppState) -> Self {
        Self {
            settings: state.settings.coder_settings.clone(),
            selected_model: state.settings.selected_model,
            coder_mode: state.permissions.coder_mode,
            permission_default_allow: state.permissions.permission_default_allow,
            permission_allow_count: state.permissions.permission_allow_tools.len(),
            permission_deny_count: state.permissions.permission_deny_tools.len(),
            permission_bash_allow_count: state.permissions.permission_allow_bash_patterns.len(),
            permission_bash_deny_count: state.permissions.permission_deny_bash_patterns.len(),
            mcp_project_count: state.catalogs.mcp_project_servers.len(),
            mcp_runtime_count: state.catalogs.mcp_runtime_servers.len(),
            mcp_disabled_count: state.catalogs.mcp_disabled_servers.len(),
            hook_config: state.catalogs.hook_config.clone(),
            keybindings: state.settings.keybindings.clone(),
        }
    }
}

pub(crate) fn settings_rows(
    snapshot: &SettingsSnapshot,
    tab: SettingsTab,
    search: &str,
) -> Vec<SettingsRow> {
    let mut rows = Vec::new();
    match tab {
        SettingsTab::General => {
            rows.push(SettingsRow {
                item: SettingsItem::Theme,
                label: "Theme".to_string(),
                value: theme_label(snapshot.settings.theme).to_string(),
                hint: Some("Enter/Left/Right to cycle".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::FontSize,
                label: "Chat font size".to_string(),
                value: format!("{:.0}px", snapshot.settings.font_size),
                hint: Some("Left/Right to adjust".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::AutoScroll,
                label: "Auto-scroll".to_string(),
                value: if snapshot.settings.auto_scroll {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Scroll on new output".to_string()),
            });
        }
        SettingsTab::Model => {
            rows.push(SettingsRow {
                item: SettingsItem::DefaultModel,
                label: "Default model".to_string(),
                value: snapshot.selected_model.name().to_string(),
                hint: Some("Left/Right to cycle".to_string()),
            });
            let thinking_value = snapshot
                .settings
                .max_thinking_tokens
                .map(|tokens| tokens.to_string())
                .unwrap_or_else(|| "Auto".to_string());
            rows.push(SettingsRow {
                item: SettingsItem::MaxThinkingTokens,
                label: "Max thinking tokens".to_string(),
                value: thinking_value,
                hint: Some("Left/Right to adjust".to_string()),
            });
        }
        SettingsTab::Permissions => {
            let mode_text = crate::app::permissions::coder_mode_label(snapshot.coder_mode).to_string();
            rows.push(SettingsRow {
                item: SettingsItem::PermissionMode,
                label: "Mode".to_string(),
                value: mode_text,
                hint: Some("Left/Right to cycle (Bypass/Plan/Autopilot)".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::PermissionDefaultAllow,
                label: "Default allow".to_string(),
                value: if snapshot.permission_default_allow {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Enter to toggle".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::PermissionAllowList,
                label: "Allowed tools".to_string(),
                value: format!("{} tools", snapshot.permission_allow_count),
                hint: Some("Use /permission allow".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::PermissionDenyList,
                label: "Denied tools".to_string(),
                value: format!("{} tools", snapshot.permission_deny_count),
                hint: Some("Use /permission deny".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::PermissionBashAllowList,
                label: "Bash allow patterns".to_string(),
                value: format!("{} patterns", snapshot.permission_bash_allow_count),
                hint: Some("Use /permission allow".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::PermissionBashDenyList,
                label: "Bash deny patterns".to_string(),
                value: format!("{} patterns", snapshot.permission_bash_deny_count),
                hint: Some("Use /permission deny".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::PermissionRules,
                label: "Permission rules".to_string(),
                value: "Open rules".to_string(),
                hint: Some("Enter to open".to_string()),
            });
        }
        SettingsTab::Sessions => {
            rows.push(SettingsRow {
                item: SettingsItem::SessionAutoSave,
                label: "Auto-save sessions".to_string(),
                value: if snapshot.settings.session_auto_save {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Enter to toggle".to_string()),
            });
            let history_value = if snapshot.settings.session_history_limit == 0 {
                "Unlimited".to_string()
            } else {
                snapshot.settings.session_history_limit.to_string()
            };
            rows.push(SettingsRow {
                item: SettingsItem::SessionHistoryLimit,
                label: "History limit".to_string(),
                value: history_value,
                hint: Some("0 = unlimited".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::SessionStoragePath,
                label: "Session storage".to_string(),
                value: crate::app::config::sessions_dir().display().to_string(),
                hint: None,
            });
        }
        SettingsTab::Keybindings => {
            rows.push(SettingsRow {
                item: SettingsItem::Keybindings,
                label: "Configure keybindings".to_string(),
                value: "Edit shortcuts".to_string(),
                hint: Some("Enter to edit".to_string()),
            });
        }
    }

    if search.trim().is_empty() {
        rows
    } else {
        let needle = search.to_ascii_lowercase();
        rows
            .into_iter()
            .filter(|row| {
                row.label.to_ascii_lowercase().contains(&needle)
                    || row.value.to_ascii_lowercase().contains(&needle)
                    || row
                        .hint
                        .as_ref()
                        .map(|hint| hint.to_ascii_lowercase().contains(&needle))
                        .unwrap_or(false)
            })
            .collect()
    }
}

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
