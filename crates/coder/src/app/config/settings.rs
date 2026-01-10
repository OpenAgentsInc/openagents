use serde::{Deserialize, Serialize};

use crate::keybindings::Action as KeyAction;

use super::super::ui::ThemeSetting;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CoderSettings {
    #[serde(default)]
    pub(crate) theme: ThemeSetting,
    #[serde(default = "super::super::default_font_size")]
    pub(crate) font_size: f32,
    #[serde(default = "super::super::default_auto_scroll")]
    pub(crate) auto_scroll: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_thinking_tokens: Option<u32>,
    #[serde(default = "super::super::default_session_auto_save")]
    pub(crate) session_auto_save: bool,
    #[serde(default = "super::super::default_session_history_limit")]
    pub(crate) session_history_limit: usize,
}

impl Default for CoderSettings {
    fn default() -> Self {
        Self {
            theme: ThemeSetting::Dark,
            font_size: super::super::default_font_size(),
            auto_scroll: super::super::default_auto_scroll(),
            model: None,
            max_thinking_tokens: None,
            session_auto_save: super::super::default_session_auto_save(),
            session_history_limit: super::super::default_session_history_limit(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    General,
    Model,
    Permissions,
    Sessions,
    Mcp,
    Hooks,
    Keyboard,
}

impl SettingsTab {
    pub(crate) fn all() -> &'static [SettingsTab] {
        &[
            SettingsTab::General,
            SettingsTab::Model,
            SettingsTab::Permissions,
            SettingsTab::Sessions,
            SettingsTab::Mcp,
            SettingsTab::Hooks,
            SettingsTab::Keyboard,
        ]
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Model => "Model",
            SettingsTab::Permissions => "Permissions",
            SettingsTab::Sessions => "Sessions",
            SettingsTab::Mcp => "MCP",
            SettingsTab::Hooks => "Hooks",
            SettingsTab::Keyboard => "Keyboard",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsItem {
    Theme,
    FontSize,
    AutoScroll,
    DefaultModel,
    MaxThinkingTokens,
    PermissionMode,
    PermissionDefaultAllow,
    PermissionRules,
    PermissionAllowList,
    PermissionDenyList,
    PermissionBashAllowList,
    PermissionBashDenyList,
    SessionAutoSave,
    SessionHistoryLimit,
    SessionStoragePath,
    McpSummary,
    McpOpenConfig,
    McpReloadProject,
    McpRefreshStatus,
    HookToolBlocker,
    HookToolLogger,
    HookOutputTruncator,
    HookContextInjection,
    HookTodoEnforcer,
    HookOpenPanel,
    Keybinding(KeyAction),
    KeybindingReset,
}

pub(crate) struct SettingsRow {
    pub(crate) item: SettingsItem,
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) hint: Option<String>,
}
