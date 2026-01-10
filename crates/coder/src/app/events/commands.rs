use claude_agent_sdk::protocol::PermissionMode;
use serde::{Deserialize, Serialize};

use super::super::config::SettingsTab;
use super::super::{HookModalView, SettingsInputMode};

pub(crate) enum CommandAction {
    None,
    SubmitPrompt(String),
}

/// Modal state for slash commands.
#[derive(Clone)]
pub(crate) enum ModalState {
    None,
    ModelPicker { selected: usize },
    SessionList { selected: usize },
    AgentList { selected: usize },
    SkillList { selected: usize },
    Hooks { view: HookModalView, selected: usize },
    ToolList { selected: usize },
    PermissionRules,
    Wallet,
    DvmProviders,
    Gateway,
    LmRouter,
    Nexus,
    Nip90Jobs,
    Oanix,
    Dspy,
    Nip28Chat,
    Config {
        tab: SettingsTab,
        selected: usize,
        search: String,
        input_mode: SettingsInputMode,
    },
    Help,
    McpConfig { selected: usize },
}

/// Internal mode representation for Coder UI.
/// Maps to PermissionMode for SDK calls, with Autopilot as a special case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoderMode {
    /// Auto-approve all tool use (maps to PermissionMode::BypassPermissions).
    BypassPermissions,
    /// Read-only mode, deny write operations (maps to PermissionMode::Plan).
    #[default]
    Plan,
    /// Autopilot mode - bypasses Claude SDK, uses DSPy/Adjutant (placeholder).
    Autopilot,
}

impl CoderMode {
    /// Convert to SDK PermissionMode (returns BypassPermissions for Autopilot since it auto-approves).
    pub(crate) fn to_sdk_permission_mode(&self) -> PermissionMode {
        match self {
            CoderMode::BypassPermissions => PermissionMode::BypassPermissions,
            CoderMode::Plan => PermissionMode::Plan,
            CoderMode::Autopilot => PermissionMode::BypassPermissions, // Auto-approve when SDK is used
        }
    }

    /// Whether this mode auto-approves all permissions.
    pub(crate) fn auto_approves_all(&self) -> bool {
        matches!(self, CoderMode::BypassPermissions | CoderMode::Autopilot)
    }
}
