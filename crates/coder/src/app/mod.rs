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

pub(crate) use state::AppState;
pub use events::CoderMode;
pub(crate) use permissions::sanitize_tokens;
pub(crate) use crate::app_entry::{CoderHookCallback, HookCallbackKind};

use std::time::{SystemTime, UNIX_EPOCH};

use claude_agent_sdk::{AgentModel, HookEvent};
use serde_json::Value;
use wgpui::markdown::{MarkdownConfig, MarkdownDocument, MarkdownRenderer as MdRenderer, StreamingMarkdown};
use wgpui::TextInput;

use crate::keybindings::{Action as KeyAction, Keybinding};
use crate::app::catalog::{AgentEntry, HookConfig};
use crate::app::chat::{ChatMessage, ChatSelectionPoint, MessageRole};
use crate::app::config::{CoderSettings, SettingsItem, SettingsRow, SettingsTab};
use crate::app::events::format_keybinding;
use crate::app::session::CheckpointEntry;
use crate::app::ui::{palette_for, theme_label};

pub(crate) const HOOK_SCRIPT_TIMEOUT_SECS: u64 = 12;
pub(crate) const TOOL_HISTORY_LIMIT: usize = 100;

pub(crate) fn selection_point_cmp(
    a: &ChatSelectionPoint,
    b: &ChatSelectionPoint,
) -> std::cmp::Ordering {
    match a.message_index.cmp(&b.message_index) {
        std::cmp::Ordering::Equal => a.offset.cmp(&b.offset),
        ordering => ordering,
    }
}

pub(crate) fn truncate_preview(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.len() <= max_chars {
        return trimmed;
    }
    let mut result = trimmed.chars().take(max_chars.saturating_sub(3)).collect::<String>();
    result.push_str("...");
    result
}

pub(crate) fn default_font_size() -> f32 {
    14.0
}

pub(crate) fn default_auto_scroll() -> bool {
    true
}

pub(crate) fn default_session_auto_save() -> bool {
    true
}

pub(crate) fn default_session_history_limit() -> usize {
    50
}

pub(crate) fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn build_markdown_document(source: &str) -> MarkdownDocument {
    let mut parser = StreamingMarkdown::new();
    parser.append(source);
    parser.complete();
    parser.document().clone()
}

pub(crate) fn build_markdown_config(settings: &CoderSettings) -> MarkdownConfig {
    let palette = palette_for(settings.theme);
    let mut config = MarkdownConfig::default();
    config.base_font_size = settings.font_size;
    config.text_color = palette.text_primary;
    config.header_color = palette.text_primary;
    config.code_background = palette.code_bg;
    config.inline_code_background = palette.inline_code_bg;
    config.link_color = palette.link;
    config.blockquote_color = palette.blockquote;
    config
}

pub(crate) fn build_markdown_renderer(settings: &CoderSettings) -> MdRenderer {
    MdRenderer::with_config(build_markdown_config(settings))
}

pub(crate) fn build_input(settings: &CoderSettings) -> TextInput {
    let palette = palette_for(settings.theme);
    let mut input = TextInput::new()
        .with_id(1)
        .font_size(settings.font_size)
        .padding(28.0, 10.0)
        .background(palette.background)
        .border_color(palette.input_border)
        .border_color_focused(palette.input_border_focused)
        .text_color(palette.text_primary)
        .placeholder_color(palette.text_dim)
        .cursor_color(palette.text_primary)
        .mono(true);
    input.focus();
    input
}

pub(crate) fn format_relative_time(timestamp: u64) -> String {
    let now = now_timestamp();
    if timestamp >= now {
        return "just now".to_string();
    }
    let delta = now - timestamp;
    if delta < 60 {
        format!("{}s ago", delta)
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86_400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86_400)
    }
}

pub(crate) fn build_checkpoint_entries(messages: &[ChatMessage]) -> Vec<CheckpointEntry> {
    let mut entries = Vec::new();
    for (idx, message) in messages.iter().enumerate() {
        if matches!(message.role, MessageRole::User) {
            if let Some(uuid) = &message.uuid {
                let label = format!("{}: {}", idx + 1, truncate_preview(&message.content, 32));
                entries.push(CheckpointEntry {
                    user_message_id: uuid.clone(),
                    label,
                });
            }
        }
    }
    entries
}

fn agent_model_label(model: AgentModel) -> &'static str {
    match model {
        AgentModel::Opus => "opus",
        AgentModel::Sonnet => "sonnet",
        AgentModel::Haiku => "haiku",
        AgentModel::Inherit => "inherit",
    }
}

pub(crate) fn agent_capabilities(entry: &AgentEntry) -> Vec<String> {
    let mut caps = Vec::new();
    if let Some(model) = entry.definition.model {
        caps.push(format!("model {}", agent_model_label(model)));
    }
    if let Some(tools) = &entry.definition.tools {
        caps.extend(tools.clone());
    } else if let Some(disallowed) = &entry.definition.disallowed_tools {
        caps.extend(disallowed.iter().map(|tool| format!("no {}", tool)));
    }
    if caps.is_empty() {
        caps.push("all tools".to_string());
    }
    caps
}

pub(crate) fn truncate_bytes(input: String, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input;
    }
    let mut truncated = input.as_bytes()[..max_bytes].to_vec();
    while !truncated.is_empty() && std::str::from_utf8(&truncated).is_err() {
        truncated.pop();
    }
    let mut result = String::from_utf8_lossy(&truncated).to_string();
    result.push_str("\n... [truncated]");
    result
}

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

pub(crate) fn hook_event_label(event: HookEvent) -> &'static str {
    event.as_str()
}

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
