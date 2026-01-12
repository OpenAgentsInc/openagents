use serde::{Deserialize, Serialize};

use crate::keybindings::Action as KeyAction;

use super::super::ui::ThemeSetting;

pub(crate) const LOCAL_OSS_MODEL: &str = "gpt-oss:20b";
pub(crate) const LOCAL_OSS_PROVIDER: &str = "ollama";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CoderSettings {
    #[serde(default)]
    pub(crate) theme: ThemeSetting,
    #[serde(default = "super::super::default_font_size")]
    pub(crate) font_size: f32,
    #[serde(default = "super::super::default_auto_scroll")]
    pub(crate) auto_scroll: bool,
    #[serde(default)]
    pub(crate) model_mode: ModelMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_thinking_tokens: Option<u32>,
    #[serde(default = "super::super::default_local_oss_base_url")]
    pub(crate) local_oss_base_url: String,
    #[serde(default = "super::super::default_session_auto_save")]
    pub(crate) session_auto_save: bool,
    #[serde(default = "super::super::default_session_history_limit")]
    pub(crate) session_history_limit: usize,
}

impl Default for CoderSettings {
    fn default() -> Self {
        Self {
            theme: ThemeSetting::default(),
            font_size: super::super::default_font_size(),
            auto_scroll: super::super::default_auto_scroll(),
            model_mode: ModelMode::default(),
            model: None,
            reasoning_effort: None,
            max_thinking_tokens: None,
            local_oss_base_url: super::super::default_local_oss_base_url(),
            session_auto_save: super::super::default_session_auto_save(),
            session_history_limit: super::super::default_session_history_limit(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ModelMode {
    Pro,
    Local,
}

impl Default for ModelMode {
    fn default() -> Self {
        ModelMode::Pro
    }
}

pub(crate) fn model_mode_label(mode: ModelMode) -> &'static str {
    match mode {
        ModelMode::Pro => "Pro",
        ModelMode::Local => "Local (GPT-OSS)",
    }
}

impl CoderSettings {
    pub(crate) fn is_local_mode(&self) -> bool {
        matches!(self.model_mode, ModelMode::Local)
    }

    pub(crate) fn local_oss_base_url_value(&self) -> String {
        let trimmed = self.local_oss_base_url.trim();
        if trimmed.is_empty() {
            super::super::default_local_oss_base_url()
        } else {
            trimmed.to_string()
        }
    }

    pub(crate) fn local_model_override(&self) -> Option<String> {
        if self.is_local_mode() {
            Some(LOCAL_OSS_MODEL.to_string())
        } else {
            None
        }
    }

    pub(crate) fn local_model_provider(&self) -> Option<String> {
        if self.is_local_mode() {
            Some(LOCAL_OSS_PROVIDER.to_string())
        } else {
            None
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
    ModelMode,
    DefaultModel,
    ReasoningEffort,
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
