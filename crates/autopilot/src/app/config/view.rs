use crate::app::AppState;
use crate::app::catalog::HookConfig;
use crate::app::events::{CoderMode, format_keybinding};
use crate::app::permissions;
use crate::app::ui::theme_label;
use crate::keybindings::{Action as KeyAction, Keybinding};

use super::models::ModelOption;
use super::{CoderSettings, SettingsItem, SettingsRow, SettingsTab};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsInputMode {
    Normal,
    Search,
    Capture(KeyAction),
}

#[derive(Clone)]
pub(crate) struct SettingsSnapshot {
    pub(crate) settings: CoderSettings,
    pub(crate) selected_model: ModelOption,
    pub(crate) selected_model_label: String,
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
    pub(crate) keybindings: Vec<KeybindingSummary>,
}

#[derive(Clone)]
pub(crate) struct KeybindingSummary {
    pub(crate) action: KeyAction,
    pub(crate) label: String,
    pub(crate) keys: Vec<String>,
}

impl SettingsSnapshot {
    pub(crate) fn from_state(state: &AppState) -> Self {
        let current_model_id = state
            .settings
            .coder_settings
            .model
            .as_deref()
            .unwrap_or_else(|| state.settings.selected_model.model_id());
        let models = super::models::app_server_model_entries(&state.settings.app_server_models);
        let selected_label = models
            .iter()
            .find(|model| model.id == current_model_id)
            .map(|model| model.name.clone())
            .unwrap_or_else(|| state.settings.selected_model.name().to_string());
        Self {
            settings: state.settings.coder_settings.clone(),
            selected_model: state.settings.selected_model,
            selected_model_label: selected_label,
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
            keybindings: summarize_keybindings(&state.settings.keybindings),
        }
    }
}

fn summarize_keybindings(bindings: &[Keybinding]) -> Vec<KeybindingSummary> {
    let mut summaries = Vec::new();
    for action in KeyAction::all() {
        let mut keys: Vec<String> = bindings
            .iter()
            .filter(|binding| binding.action == *action)
            .map(format_keybinding)
            .collect();
        keys.sort();
        keys.dedup();
        if keys.is_empty() {
            keys.push("Unbound".to_string());
        }
        summaries.push(KeybindingSummary {
            action: *action,
            label: action.label().to_string(),
            keys,
        });
    }
    summaries
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
                value: snapshot.selected_model_label.clone(),
                hint: Some("Left/Right to cycle".to_string()),
            });
            let effort_value = snapshot
                .settings
                .reasoning_effort
                .clone()
                .unwrap_or_else(|| "auto".to_string());
            rows.push(SettingsRow {
                item: SettingsItem::ReasoningEffort,
                label: "Reasoning effort".to_string(),
                value: effort_value,
                hint: Some("Left/Right to cycle".to_string()),
            });
        }
        SettingsTab::Permissions => {
            let mode_text = permissions::coder_mode_label(snapshot.coder_mode).to_string();
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
                item: SettingsItem::PermissionRules,
                label: "Permission rules".to_string(),
                value: "Open rules".to_string(),
                hint: Some("Enter to open".to_string()),
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
        SettingsTab::Mcp => {
            let summary = format!(
                "project {}, runtime {}, disabled {}",
                snapshot.mcp_project_count, snapshot.mcp_runtime_count, snapshot.mcp_disabled_count
            );
            rows.push(SettingsRow {
                item: SettingsItem::McpSummary,
                label: "MCP servers".to_string(),
                value: summary,
                hint: Some("Configured servers".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::McpOpenConfig,
                label: "Open MCP config".to_string(),
                value: "Edit project file".to_string(),
                hint: Some("Enter to open".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::McpReloadProject,
                label: "Reload MCP project".to_string(),
                value: "Reload config".to_string(),
                hint: Some("Enter to reload".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::McpRefreshStatus,
                label: "Refresh MCP status".to_string(),
                value: "Check server health".to_string(),
                hint: Some("Enter to refresh".to_string()),
            });
        }
        SettingsTab::Hooks => {
            rows.push(SettingsRow {
                item: SettingsItem::HookToolBlocker,
                label: "Tool blocker".to_string(),
                value: if snapshot.hook_config.tool_blocker {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Block tool use".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::HookToolLogger,
                label: "Tool logger".to_string(),
                value: if snapshot.hook_config.tool_logger {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Log tool calls".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::HookOutputTruncator,
                label: "Output truncator".to_string(),
                value: if snapshot.hook_config.output_truncator {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Truncate long output".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::HookContextInjection,
                label: "Context injection".to_string(),
                value: if snapshot.hook_config.context_injection {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Inject rules into prompts".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::HookTodoEnforcer,
                label: "Todo enforcer".to_string(),
                value: if snapshot.hook_config.todo_enforcer {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                hint: Some("Require TODO list".to_string()),
            });
            rows.push(SettingsRow {
                item: SettingsItem::HookOpenPanel,
                label: "Open hook panel".to_string(),
                value: "Inspect hook events".to_string(),
                hint: Some("Enter to open".to_string()),
            });
        }
        SettingsTab::Keyboard => {
            for binding in &snapshot.keybindings {
                rows.push(SettingsRow {
                    item: SettingsItem::Keybinding(binding.action),
                    label: binding.label.clone(),
                    value: binding
                        .keys
                        .iter()
                        .map(|key| key.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    hint: Some("Enter to rebind".to_string()),
                });
            }
            rows.push(SettingsRow {
                item: SettingsItem::KeybindingReset,
                label: "Reset keybindings".to_string(),
                value: "Restore defaults".to_string(),
                hint: Some("Enter to reset".to_string()),
            });
        }
    }

    if search.trim().is_empty() {
        rows
    } else {
        let needle = search.to_ascii_lowercase();
        rows.into_iter()
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
