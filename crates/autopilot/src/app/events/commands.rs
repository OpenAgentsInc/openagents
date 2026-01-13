use serde::{Deserialize, Serialize};

use crate::commands::ReviewCommand;

use super::super::config::SettingsTab;
use super::super::{HookModalView, SettingsInputMode};

pub(crate) enum CommandAction {
    None,
    SubmitPrompt(String),
    StartReview(ReviewCommand),
    StartChainViz(String),
}

/// Modal state for slash commands.
#[derive(Clone)]
pub(crate) enum ModalState {
    None,
    /// Bootloader modal shown on startup.
    Bootloader,
    ModelPicker {
        selected: usize,
    },
    SessionList {
        selected: usize,
    },
    AgentList {
        selected: usize,
    },
    AgentBackends {
        selected: usize,
        model_selected: usize,
    },
    SkillList {
        selected: usize,
    },
    Hooks {
        view: HookModalView,
        selected: usize,
    },
    ToolList {
        selected: usize,
    },
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
    ChainViz,
    Nip28Chat,
    Config {
        tab: SettingsTab,
        selected: usize,
        search: String,
        input_mode: SettingsInputMode,
    },
    Help,
    McpConfig {
        selected: usize,
    },
    /// Issue validation in progress
    ValidatingIssue {
        issue_number: u32,
        title: String,
    },
    /// Issue validation failed - show warning
    IssueValidationFailed {
        issue_number: u32,
        title: String,
        reason: String,
    },
}

/// Internal mode representation for Coder UI.
/// Maps to app-server approval and sandbox policies.
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
