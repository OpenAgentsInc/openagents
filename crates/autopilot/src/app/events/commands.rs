use codex_agent_sdk::{ApprovalMode, SandboxMode};
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
    AgentBackends { selected: usize, model_selected: usize },
    SkillList { selected: usize },
    Hooks { view: HookModalView, selected: usize },
    ToolList { selected: usize },
    PermissionRules,
    Wallet,
    DvmProviders,
    Gateway,
    LmRouter,
    Nexus,
    SparkWallet,
    Nip90Jobs,
    Oanix,
    Directives,
    Issues,
    AutopilotIssues,
    Rlm,
    RlmTrace,
    PylonEarnings,
    PylonJobs,
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
    Plan,
    /// Autopilot mode - auto-approves all, default mode.
    #[default]
    Autopilot,
}

impl CoderMode {
    /// Convert to Codex sandbox mode.
    pub(crate) fn sandbox_mode(&self) -> SandboxMode {
        match self {
            CoderMode::BypassPermissions => SandboxMode::DangerFullAccess,
            CoderMode::Plan => SandboxMode::ReadOnly,
            CoderMode::Autopilot => SandboxMode::WorkspaceWrite,
        }
    }

    /// Convert to Codex approval mode.
    pub(crate) fn approval_mode(&self) -> ApprovalMode {
        match self {
            CoderMode::BypassPermissions => ApprovalMode::Never,
            CoderMode::Plan => ApprovalMode::Never,
            CoderMode::Autopilot => ApprovalMode::OnRequest,
        }
    }

    /// Label for UI/logging.
    pub(crate) fn mode_label(&self) -> &'static str {
        match self {
            CoderMode::BypassPermissions => "bypassPermissions",
            CoderMode::Plan => "plan",
            CoderMode::Autopilot => "autopilot",
        }
    }

    /// Whether this mode auto-approves all permissions.
    pub(crate) fn auto_approves_all(&self) -> bool {
        matches!(self, CoderMode::BypassPermissions | CoderMode::Autopilot)
    }
}
