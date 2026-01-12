use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc;
use wgpui::components::hud::Command as PaletteCommand;
use wgpui::components::molecules::SessionAction;
use wgpui::components::organisms::EventInspector;

use crate::app::agents::AgentBackendsStatus;
use crate::app::chat::{ChatMessage, MessageRole};
use crate::app::codex_app_server as app_server;
use crate::app::codex_runtime::{CodexRuntime, CodexRuntimeConfig};
use crate::app::config::{config_dir, SettingsTab};
use crate::app::events::{keybinding_labels, ModalState};
use crate::app::nip28::Nip28ConnectionStatus;
use crate::app::nip90::Nip90ConnectionStatus;
use crate::app::dvm::DvmStatus;
use crate::app::{
    build_input, build_markdown_config, build_markdown_document, build_markdown_renderer,
    now_timestamp, AgentCardAction, HookLogEntry, HookModalView, HookSetting, ModelOption,
    SettingsInputMode, SkillCardAction,
};
use crate::app::session::{SessionEntry, SessionUpdate};
use crate::app::config::SettingsUpdate;
use crate::app::catalog::{SkillEntry, SkillUpdate};
use crate::app::tools::ToolVisualization;
use crate::app::tools::parsing::{format_tool_input, tool_type_for_name};
use crate::app::workspaces::{ConversationItem, ConversationRole, ReviewState, ToolItemData};
use crate::app::AppState;
use crate::keybindings::Action as KeyAction;

use super::command_palette_ids;
use super::hooks::hook_log_event_data;
use super::settings::{normalize_settings, save_settings};
use super::COMMAND_PALETTE_ENABLED;

const HOOK_LOG_LIMIT: usize = 200;

impl AppState {
    pub(super) fn build_command_palette_commands(&self) -> Vec<PaletteCommand> {
        let mut commands = Vec::new();
        let mut push_command = |id: &str,
                                label: &str,
                                description: &str,
                                category: &str,
                                keybinding: Option<String>| {
            let mut command = PaletteCommand::new(id, label)
                .description(description)
                .category(category);
            if let Some(keys) = keybinding {
                command = command.keybinding(keys);
            }
            commands.push(command);
        };

        let interrupt_keys = keybinding_labels(&self.settings.keybindings, KeyAction::Interrupt, "Ctrl+C");
        push_command(
            command_palette_ids::INTERRUPT_REQUEST,
            "Interrupt Request",
            "Stop the active response stream",
            "Request",
            Some(interrupt_keys),
        );

        push_command(
            command_palette_ids::HELP,
            "Open Help",
            "Show hotkeys and feature overview",
            "Navigation",
            Some("F1".to_string()),
        );

        let settings_keys = keybinding_labels(&self.settings.keybindings, KeyAction::OpenSettings, "Ctrl+,");
        push_command(
            command_palette_ids::SETTINGS,
            "Open Settings",
            "Configure Autopilot preferences",
            "Navigation",
            Some(settings_keys),
        );

        push_command(
            command_palette_ids::SESSION_LIST,
            "Open Session List",
            "Resume or fork previous sessions",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::AGENTS_LIST,
            "Open Agents",
            "Browse available agents",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::AGENT_BACKENDS_OPEN,
            "Open Agent Backends",
            "View CLI backend status and models",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::SKILLS_LIST,
            "Open Skills",
            "Browse available skills",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::HOOKS_OPEN,
            "Open Hooks",
            "Manage hook configuration",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::TOOLS_LIST,
            "Open Tool List",
            "Review available tools",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::MCP_CONFIG,
            "Open MCP Servers",
            "Manage MCP configuration",
            "Navigation",
            None,
        );
        let dvm_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenDvm, "Ctrl+Shift+P");
        push_command(
            command_palette_ids::DVM_OPEN,
            "Open DVM Providers",
            "Discover NIP-89 DVM providers",
            "Marketplace",
            Some(dvm_keys),
        );
        let gateway_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenGateway, "Ctrl+Shift+G");
        push_command(
            command_palette_ids::GATEWAY_OPEN,
            "Open Gateway",
            "View gateway health and models",
            "System",
            Some(gateway_keys),
        );
        let lm_router_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenLmRouter, "Ctrl+Shift+L");
        push_command(
            command_palette_ids::LM_ROUTER_OPEN,
            "Open LM Router",
            "View LM router backends and models",
            "System",
            Some(lm_router_keys),
        );
        let nexus_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenNexus, "Ctrl+Shift+X");
        push_command(
            command_palette_ids::NEXUS_OPEN,
            "Open Nexus Stats",
            "View Nexus relay stats",
            "Nostr",
            Some(nexus_keys),
        );
        let nip90_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenNip90, "Ctrl+Shift+J");
        push_command(
            command_palette_ids::NIP90_OPEN,
            "Open NIP-90 Jobs",
            "Monitor NIP-90 job requests, results, and feedback",
            "Nostr",
            Some(nip90_keys),
        );
        let oanix_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenOanix, "Ctrl+Shift+O");
        push_command(
            command_palette_ids::OANIX_OPEN,
            "Open OANIX",
            "View the environment discovery manifest",
            "System",
            Some(oanix_keys),
        );
        let directives_keys = keybinding_labels(
            &self.settings.keybindings,
            KeyAction::OpenDirectives,
            "Ctrl+Shift+T",
        );
        push_command(
            command_palette_ids::DIRECTIVES_OPEN,
            "Open Directives",
            "Review workspace directives from .openagents",
            "Workspace",
            Some(directives_keys),
        );
        let issues_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenIssues, "Ctrl+Shift+I");
        push_command(
            command_palette_ids::ISSUES_OPEN,
            "Open Issues",
            "Review workspace issues from .openagents",
            "Workspace",
            Some(issues_keys),
        );
        let tracker_keys = keybinding_labels(
            &self.settings.keybindings,
            KeyAction::OpenIssueTracker,
            "Ctrl+Shift+A",
        );
        push_command(
            command_palette_ids::ISSUE_TRACKER_OPEN,
            "Open Issue Tracker",
            "Review issues from .openagents/autopilot.db",
            "Workspace",
            Some(tracker_keys),
        );
        let nip28_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenNip28, "Ctrl+Shift+N");
        push_command(
            command_palette_ids::NIP28_OPEN,
            "Open NIP-28 Chat",
            "Join public Nostr chat channels",
            "Nostr",
            Some(nip28_keys),
        );

        push_command(
            command_palette_ids::CLEAR_CONVERSATION,
            "Clear Conversation",
            "Reset the current chat history",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::UNDO_LAST,
            "Undo Last Exchange",
            "Remove the most recent exchange",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::COMPACT_CONTEXT,
            "Compact Context",
            "Summarize older context into a shorter prompt",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::SESSION_FORK,
            "Fork Session",
            "Create a new branch of this session",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::SESSION_EXPORT,
            "Export Session",
            "Export conversation to markdown",
            "Session",
            None,
        );

        push_command(
            command_palette_ids::PERMISSION_RULES,
            "Open Permission Rules",
            "Manage tool allow/deny rules",
            "Permissions",
            None,
        );

        let left_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleLeftSidebar, "Ctrl+[");
        let right_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleRightSidebar, "Ctrl+]");
        let toggle_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleSidebars, "Ctrl+\\");
        push_command(
            command_palette_ids::SIDEBAR_LEFT,
            "Open Left Sidebar",
            "Show the left sidebar",
            "Layout",
            Some(left_keys),
        );
        push_command(
            command_palette_ids::SIDEBAR_RIGHT,
            "Open Right Sidebar",
            "Show the right sidebar",
            "Layout",
            Some(right_keys),
        );
        push_command(
            command_palette_ids::SIDEBAR_TOGGLE,
            "Toggle Sidebars",
            "Show or hide both sidebars",
            "Layout",
            Some(toggle_keys),
        );

        push_command(
            command_palette_ids::MCP_RELOAD,
            "Reload MCP Config",
            "Reload MCP servers from project config",
            "MCP",
            None,
        );
        push_command(
            command_palette_ids::MCP_STATUS,
            "Refresh MCP Status",
            "Fetch MCP server status",
            "MCP",
            None,
        );

        push_command(
            command_palette_ids::AGENT_CLEAR,
            "Clear Active Agent",
            "Stop using the active agent",
            "Agents",
            None,
        );
        push_command(
            command_palette_ids::AGENT_RELOAD,
            "Reload Agents",
            "Reload agent definitions from disk",
            "Agents",
            None,
        );

        push_command(
            command_palette_ids::SKILLS_RELOAD,
            "Reload Skills",
            "Reload skills from disk",
            "Skills",
            None,
        );

        push_command(
            command_palette_ids::HOOKS_RELOAD,
            "Reload Hooks",
            "Reload hook scripts from disk",
            "Hooks",
            None,
        );

        push_command(
            command_palette_ids::BUG_REPORT,
            "Report a Bug",
            "Open the issue tracker",
            "Diagnostics",
            None,
        );

        push_command(
            command_palette_ids::KITCHEN_SINK,
            "Kitchen Sink",
            "Show all UI component variations",
            "Developer",
            None,
        );

        commands
    }

    pub(super) fn open_command_palette(&mut self) {
        if !COMMAND_PALETTE_ENABLED {
            if self.command_palette.is_open() {
                self.command_palette.close();
            }
            return;
        }
        self.modal_state = ModalState::None;
        if self.chat.chat_context_menu.is_open() {
            self.chat.chat_context_menu.close();
            self.chat.chat_context_menu_target = None;
        }
        self.command_palette.set_commands(self.build_command_palette_commands());
        self.command_palette.open();
    }

    pub(super) fn open_model_picker(&mut self) {
        let current_model_id = self
            .settings
            .coder_settings
            .model
            .as_deref()
            .unwrap_or_else(|| self.settings.selected_model.model_id());
        let models = crate::app::config::app_server_model_entries(&self.settings.app_server_models);
        let current_idx = models
            .iter()
            .position(|model| model.id == current_model_id)
            .unwrap_or(0);
        self.modal_state = ModalState::ModelPicker { selected: current_idx };

        let (update_tx, update_rx) = mpsc::unbounded_channel();
        self.settings.settings_update_tx = Some(update_tx.clone());
        self.settings.settings_update_rx = Some(update_rx);

        if should_fetch_codex_sessions() {
            let cwd = std::env::current_dir().unwrap_or_default();
            let models_tx = update_tx.clone();
            let models_cwd = cwd.clone();
            tokio::spawn(async move {
                match fetch_codex_models(models_cwd).await {
                    Ok(models) => {
                        let _ = models_tx.send(SettingsUpdate::ModelsLoaded(models));
                    }
                    Err(err) => {
                        let _ = models_tx.send(SettingsUpdate::Error(format!(
                            "Failed to load Codex models: {}",
                            err
                        )));
                    }
                }
            });
            let config_tx = update_tx;
            let config_cwd = cwd;
            tokio::spawn(async move {
                match fetch_codex_config_snapshot(config_cwd).await {
                    Ok((model, reasoning_effort)) => {
                        let _ = config_tx.send(SettingsUpdate::ConfigLoaded {
                            model,
                            reasoning_effort,
                        });
                    }
                    Err(err) => {
                        let _ = config_tx.send(SettingsUpdate::Error(format!(
                            "Failed to read Codex config: {}",
                            err
                        )));
                    }
                }
            });
        }
    }

    pub(super) fn open_session_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (checkpoint_tx, checkpoint_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        self.session.session_action_tx = Some(action_tx);
        self.session.session_action_rx = Some(action_rx);
        self.session.session_update_tx = Some(update_tx.clone());
        self.session.session_update_rx = Some(update_rx);
        self.session.checkpoint_action_tx = Some(checkpoint_tx);
        self.session.checkpoint_action_rx = Some(checkpoint_rx);
        self.session.refresh_session_cards(self.chat.is_thinking);
        self.session.refresh_checkpoint_restore(&self.chat.messages);
        let selected = self.session.session_index
            .iter()
            .position(|entry| entry.id == self.session.session_info.session_id)
            .unwrap_or(0);
        self.modal_state = ModalState::SessionList { selected };

        if should_fetch_codex_sessions() {
            let cwd = std::env::current_dir().unwrap_or_default();
            tokio::spawn(async move {
                match fetch_codex_session_entries(cwd).await {
                    Ok(entries) => {
                        let _ = update_tx.send(SessionUpdate::MergeEntries(entries));
                    }
                    Err(err) => {
                        let _ = update_tx.send(SessionUpdate::Error(format!(
                            "Failed to load Codex sessions: {}",
                            err
                        )));
                    }
                }
            });
        }
    }

    pub(super) fn open_agent_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.agent_action_tx = Some(action_tx);
        self.catalogs.agent_action_rx = Some(action_rx);
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
        let selected = self.catalogs.active_agent
            .as_ref()
            .and_then(|name| {
                self.catalogs.agent_entries
                    .iter()
                    .position(|entry| entry.name == *name)
            })
            .unwrap_or(0);
        self.modal_state = ModalState::AgentList { selected };
    }

    pub(super) fn open_agent_backends(&mut self) {
        if self.agent_backends.snapshot.is_none()
            || matches!(self.agent_backends.status, AgentBackendsStatus::Error(_))
        {
            self.agent_backends.refresh();
        }
        let (selected, model_selected) = self.agent_backends.selection_indices();
        self.modal_state = ModalState::AgentBackends {
            selected,
            model_selected,
        };
    }

    pub(super) fn open_skill_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.skill_action_tx = Some(action_tx);
        self.catalogs.skill_action_rx = Some(action_rx);
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        self.catalogs.skill_update_tx = Some(update_tx.clone());
        self.catalogs.skill_update_rx = Some(update_rx);
        self.catalogs.refresh_skill_cards();
        self.modal_state = ModalState::SkillList { selected: 0 };

        if should_fetch_codex_sessions() {
            let cwd = std::env::current_dir().unwrap_or_default();
            tokio::spawn(async move {
                match fetch_codex_skills(cwd, false).await {
                    Ok((entries, error)) => {
                        let _ = update_tx.send(SkillUpdate::CodexSkillsLoaded { entries, error });
                    }
                    Err(err) => {
                        let _ = update_tx.send(SkillUpdate::Error(format!(
                            "Failed to load Codex skills: {}",
                            err
                        )));
                    }
                }
            });
        }
    }

    pub(super) fn open_tool_list(&mut self) {
        self.modal_state = ModalState::ToolList { selected: 0 };
    }

    pub(super) fn open_permission_rules(&mut self) {
        self.modal_state = ModalState::PermissionRules;
    }

    pub(super) fn open_config(&mut self) {
        self.modal_state = ModalState::Config {
            tab: SettingsTab::General,
            selected: 0,
            search: String::new(),
            input_mode: SettingsInputMode::Normal,
        };

        let (update_tx, update_rx) = mpsc::unbounded_channel();
        self.settings.settings_update_tx = Some(update_tx.clone());
        self.settings.settings_update_rx = Some(update_rx);

        if should_fetch_codex_sessions() {
            let cwd = std::env::current_dir().unwrap_or_default();
            let models_tx = update_tx.clone();
            let models_cwd = cwd.clone();
            tokio::spawn(async move {
                match fetch_codex_models(models_cwd).await {
                    Ok(models) => {
                        let _ = models_tx.send(SettingsUpdate::ModelsLoaded(models));
                    }
                    Err(err) => {
                        let _ = models_tx.send(SettingsUpdate::Error(format!(
                            "Failed to load Codex models: {}",
                            err
                        )));
                    }
                }
            });
            let config_tx = update_tx;
            let config_cwd = cwd;
            tokio::spawn(async move {
                match fetch_codex_config_snapshot(config_cwd).await {
                    Ok((model, reasoning_effort)) => {
                        let _ = config_tx.send(SettingsUpdate::ConfigLoaded {
                            model,
                            reasoning_effort,
                        });
                    }
                    Err(err) => {
                        let _ = config_tx.send(SettingsUpdate::Error(format!(
                            "Failed to read Codex config: {}",
                            err
                        )));
                    }
                }
            });
        }
    }

    pub(super) fn open_wallet(&mut self) {
        if self.autopilot.oanix_manifest.is_none() && self.autopilot.oanix_manifest_rx.is_none() {
            self.request_oanix_refresh();
        }
        self.refresh_wallet_snapshot();
        self.modal_state = ModalState::Wallet;
    }

    pub(super) fn refresh_wallet_snapshot(&mut self) {
        self.wallet.refresh(self.autopilot.oanix_manifest.as_ref());
    }

    pub(super) fn refresh_agent_backends(&mut self) {
        self.agent_backends.refresh();
    }

    pub(super) fn request_wallet_refresh(&mut self) {
        self.refresh_wallet_snapshot();
        self.request_oanix_refresh();
    }

    pub(super) fn open_dvm(&mut self) {
        if self.dvm.providers.is_empty()
            || matches!(self.dvm.status, DvmStatus::Error(_))
        {
            self.dvm.refresh();
        }
        self.modal_state = ModalState::DvmProviders;
    }

    pub(super) fn refresh_dvm(&mut self) {
        self.dvm.refresh();
    }

    pub(super) fn open_gateway(&mut self) {
        self.refresh_gateway();
        self.modal_state = ModalState::Gateway;
    }

    pub(super) fn refresh_gateway(&mut self) {
        self.gateway.refresh();
    }

    pub(super) fn open_lm_router(&mut self) {
        self.refresh_lm_router();
        self.modal_state = ModalState::LmRouter;
    }

    pub(super) fn refresh_lm_router(&mut self) {
        self.lm_router.refresh();
    }

    pub(super) fn open_nexus(&mut self) {
        self.refresh_nexus();
        self.modal_state = ModalState::Nexus;
    }

    pub(super) fn refresh_nexus(&mut self) {
        self.nexus.refresh();
    }

    pub(super) fn connect_nexus(&mut self, stats_url: String) {
        self.nexus.set_stats_url(stats_url);
        self.refresh_nexus();
    }

    pub(super) fn open_spark_wallet(&mut self) {
        self.refresh_spark_wallet();
        self.modal_state = ModalState::SparkWallet;
    }

    pub(super) fn refresh_spark_wallet(&mut self) {
        self.spark_wallet.refresh();
    }

    pub(super) fn connect_dvm(&mut self, relay_url: Option<String>) {
        if let Some(url) = relay_url {
            self.dvm.connect_to(url);
        } else {
            self.dvm.refresh();
        }
    }

    pub(super) fn set_dvm_job_kind(&mut self, kind: u16) {
        self.dvm.set_job_kind(kind);
        self.push_system_message(format!("DVM job kind set to {}.", kind));
    }

    pub(super) fn open_nip90(&mut self) {
        if matches!(
            self.nip90.status,
            Nip90ConnectionStatus::Disconnected | Nip90ConnectionStatus::Error(_)
        ) {
            self.nip90.connect();
        }
        self.modal_state = ModalState::Nip90Jobs;
    }

    pub(super) fn connect_nip90(&mut self, relay_url: Option<String>) {
        if let Some(url) = relay_url {
            self.nip90.connect_to(url);
        } else {
            self.nip90.connect();
        }
    }

    pub(super) fn refresh_nip90(&mut self) {
        self.nip90.connect();
    }

    pub(super) fn open_oanix(&mut self) {
        if self.autopilot.oanix_manifest.is_none() && self.autopilot.oanix_manifest_rx.is_none() {
            self.request_oanix_refresh();
        }
        self.modal_state = ModalState::Oanix;
    }

    pub(super) fn refresh_oanix(&mut self) {
        self.request_oanix_refresh();
    }

    pub(super) fn open_directives(&mut self) {
        if self.autopilot.oanix_manifest.is_none() && self.autopilot.oanix_manifest_rx.is_none() {
            self.request_oanix_refresh();
        }
        self.modal_state = ModalState::Directives;
    }

    pub(super) fn refresh_directives(&mut self) {
        self.request_oanix_refresh();
    }

    pub(super) fn open_issues(&mut self) {
        if self.autopilot.oanix_manifest.is_none() && self.autopilot.oanix_manifest_rx.is_none() {
            self.request_oanix_refresh();
        }
        self.modal_state = ModalState::Issues;
    }

    pub(super) fn refresh_issues(&mut self) {
        self.request_oanix_refresh();
    }

    pub(super) fn open_issue_tracker(&mut self) {
        if self.autopilot.oanix_manifest.is_none() && self.autopilot.oanix_manifest_rx.is_none() {
            self.request_oanix_refresh();
        }
        self.refresh_issue_tracker();
        self.modal_state = ModalState::AutopilotIssues;
    }

    pub(super) fn refresh_issue_tracker(&mut self) {
        let workspace_root = self
            .autopilot
            .oanix_manifest
            .as_ref()
            .and_then(|manifest| manifest.workspace.as_ref())
            .map(|workspace| workspace.root.as_path());
        self.autopilot_issues.refresh(workspace_root);
    }

    pub(super) fn open_rlm(&mut self) {
        self.refresh_rlm();
        self.modal_state = ModalState::Rlm;
    }

    pub(super) fn refresh_rlm(&mut self) {
        self.rlm.refresh();
    }

    pub(super) fn open_rlm_trace(&mut self, run_id: Option<String>) {
        self.rlm_trace.refresh(run_id);
        self.modal_state = ModalState::RlmTrace;
    }

    pub(super) fn refresh_rlm_trace(&mut self) {
        self.rlm_trace.refresh_selected();
    }

    pub(super) fn open_pylon_earnings(&mut self) {
        self.refresh_pylon_earnings();
        self.modal_state = ModalState::PylonEarnings;
    }

    pub(super) fn refresh_pylon_earnings(&mut self) {
        self.pylon_earnings.refresh();
    }

    pub(super) fn open_pylon_jobs(&mut self) {
        self.refresh_pylon_jobs();
        self.modal_state = ModalState::PylonJobs;
    }

    pub(super) fn refresh_pylon_jobs(&mut self) {
        self.pylon_jobs.refresh();
    }

    pub(super) fn open_dspy(&mut self) {
        self.refresh_dspy_snapshot();
        self.modal_state = ModalState::Dspy;
    }

    pub(super) fn refresh_dspy_snapshot(&mut self) {
        self.dspy.refresh();
    }

    pub(super) fn set_dspy_auto_optimizer_enabled(&mut self, enabled: bool) {
        let result = self
            .dspy
            .update_auto_optimizer(|config| config.enabled = enabled);
        match result {
            Ok(()) => self.push_system_message(format!(
                "DSPy auto-optimizer {}.",
                if enabled { "enabled" } else { "disabled" }
            )),
            Err(err) => self.push_system_message(format!(
                "Failed to update DSPy auto-optimizer: {}.",
                err
            )),
        }
    }

    pub(super) fn set_dspy_background_optimization(&mut self, enabled: bool) {
        let result = self
            .dspy
            .update_auto_optimizer(|config| config.background_optimization = enabled);
        match result {
            Ok(()) => self.push_system_message(format!(
                "DSPy background optimization {}.",
                if enabled { "enabled" } else { "disabled" }
            )),
            Err(err) => self.push_system_message(format!(
                "Failed to update DSPy background optimization: {}.",
                err
            )),
        }
    }

    pub(super) fn open_nip28(&mut self) {
        if matches!(
            self.nip28.status,
            Nip28ConnectionStatus::Disconnected | Nip28ConnectionStatus::Error(_)
        ) {
            self.nip28.connect();
        }
        self.modal_state = ModalState::Nip28Chat;
    }

    pub(super) fn connect_nip28(&mut self, relay_url: Option<String>) {
        if let Some(url) = relay_url {
            self.nip28.connect_to(url);
        } else {
            self.nip28.connect();
        }
    }

    pub(super) fn set_nip28_channel(&mut self, channel: String) {
        self.nip28.set_channel(channel);
        self.push_system_message("NIP-28 channel updated.".to_string());
    }

    pub(super) fn send_nip28_message(&mut self, message: String) {
        let channel = match &self.nip28.channel_id {
            Some(channel) => channel.clone(),
            None => {
                self.push_system_message("NIP-28 channel not set.".to_string());
                return;
            }
        };
        if message.trim().is_empty() {
            self.push_system_message("NIP-28 message is empty.".to_string());
            return;
        }
        self.nip28.runtime.publish_chat_message(&channel, message.trim());
        self.push_system_message("NIP-28 message sent.".to_string());
    }

    pub(super) fn refresh_nip28(&mut self) {
        self.nip28.connect();
        self.nip28.request_channel_setup();
    }

    pub(super) fn request_oanix_refresh(&mut self) {
        if self.autopilot.oanix_manifest_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::unbounded_channel();
        self.autopilot.oanix_manifest_rx = Some(rx);
        tokio::spawn(async move {
            match oanix::boot().await {
                Ok(manifest) => {
                    let _ = tx.send(manifest);
                }
                Err(err) => {
                    tracing::warn!("OANIX refresh failed: {}", err);
                }
            }
        });
    }

    pub(super) fn persist_settings(&self) {
        save_settings(&self.settings.coder_settings);
    }

    pub(super) fn apply_settings(&mut self) {
        normalize_settings(&mut self.settings.coder_settings);
        let current_value = self.input.get_value().to_string();
        let focused = self.input.is_focused();
        self.input = build_input(&self.settings.coder_settings);
        self.input.set_value(current_value);
        if focused {
            self.input.focus();
        }
        self.chat.markdown_renderer = build_markdown_renderer(&self.settings.coder_settings);
        self.chat.streaming_markdown.set_markdown_config(build_markdown_config(&self.settings.coder_settings));
    }

    pub(super) fn update_selected_model(&mut self, model: ModelOption) {
        self.settings.selected_model = model;
        let model_id = self.settings.selected_model.model_id().to_string();
        self.session.session_info.model = model_id.clone();
        self.settings.coder_settings.model = Some(model_id);
        self.persist_settings();
        self.persist_codex_config_value("model".to_string(), Value::String(self.session.session_info.model.clone()));
    }

    pub(super) fn update_selected_model_id(&mut self, model_id: String) {
        self.settings.selected_model = ModelOption::from_id(&model_id);
        self.settings.coder_settings.model = Some(model_id.clone());
        self.session.session_info.model = model_id;
        self.persist_settings();
        self.persist_codex_config_value(
            "model".to_string(),
            Value::String(self.session.session_info.model.clone()),
        );
    }

    pub(super) fn persist_codex_config_value(&mut self, key_path: String, value: Value) {
        if !should_fetch_codex_sessions() {
            return;
        }
        let cwd = std::env::current_dir().unwrap_or_default();
        let update_tx = self.settings.settings_update_tx.clone();
        tokio::spawn(async move {
            if let Err(err) = write_codex_config_value(cwd, key_path, value).await {
                if let Some(tx) = update_tx {
                    let _ = tx.send(SettingsUpdate::Error(format!(
                        "Failed to persist Codex config: {}",
                        err
                    )));
                } else {
                    tracing::warn!(error = %err, "Failed to persist Codex config");
                }
            }
        });
    }

    pub(super) fn toggle_left_sidebar(&mut self) {
        self.left_sidebar_open = !self.left_sidebar_open;
    }

    pub(super) fn toggle_right_sidebar(&mut self) {
        self.right_sidebar_open = !self.right_sidebar_open;
    }

    pub(super) fn toggle_sidebars(&mut self) {
        let should_open = !(self.left_sidebar_open && self.right_sidebar_open);
        self.left_sidebar_open = should_open;
        self.right_sidebar_open = should_open;
    }

    pub(super) fn apply_session_history_limit(&mut self) {
        self.session.apply_history_limit(
            self.settings.coder_settings.session_history_limit,
            self.chat.is_thinking,
        );
    }

    pub(super) fn open_mcp_config(&mut self) {
        self.modal_state = ModalState::McpConfig { selected: 0 };
    }

    pub(super) fn open_help(&mut self) {
        self.help_scroll_offset = 0.0;
        self.modal_state = ModalState::Help;
    }

    pub(super) fn open_hooks(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.hook_inspector_action_tx = Some(action_tx);
        self.catalogs.hook_inspector_action_rx = Some(action_rx);
        self.modal_state = ModalState::Hooks {
            view: HookModalView::Config,
            selected: 0,
        };
    }

    pub(super) fn reload_hooks(&mut self) {
        self.catalogs.reload_hooks();
    }

    pub(super) fn toggle_hook_setting(&mut self, setting: HookSetting) {
        self.catalogs.toggle_hook_setting(setting);
    }

    pub(super) fn clear_hook_log(&mut self) {
        self.catalogs.hook_event_log.clear();
        self.catalogs.hook_inspector = None;
        if let ModalState::Hooks { view, selected } = &mut self.modal_state {
            if *view == HookModalView::Events {
                *selected = 0;
            }
        }
    }

    pub(super) fn reload_agents(&mut self) {
        self.catalogs.reload_agents(&mut self.chat);
    }

    pub(super) fn reload_skills(&mut self) {
        self.catalogs.reload_skills();

        if should_fetch_codex_sessions() {
            let Some(update_tx) = self.catalogs.skill_update_tx.clone() else {
                return;
            };
            let cwd = std::env::current_dir().unwrap_or_default();
            tokio::spawn(async move {
                match fetch_codex_skills(cwd, true).await {
                    Ok((entries, error)) => {
                        let _ = update_tx.send(SkillUpdate::CodexSkillsLoaded { entries, error });
                    }
                    Err(err) => {
                        let _ = update_tx.send(SkillUpdate::Error(format!(
                            "Failed to reload Codex skills: {}",
                            err
                        )));
                    }
                }
            });
        }
    }

    pub(super) fn reload_mcp_project_servers(&mut self) {
        self.catalogs.reload_mcp_project_servers();
    }

    pub(super) fn request_mcp_status(&mut self) {
        self.catalogs.request_mcp_status(&mut self.chat);
    }

    pub(super) fn handle_session_card_action(&mut self, action: SessionAction, session_id: String) {
        self.session.handle_session_card_action(
            action,
            session_id,
            &mut self.chat,
            &mut self.tools,
            &mut self.modal_state,
        );
    }

    pub(super) fn handle_agent_card_action(&mut self, action: AgentCardAction, agent_id: String) {
        match action {
            AgentCardAction::Select => {
                self.set_active_agent_by_name(&agent_id);
                self.modal_state = ModalState::None;
            }
            AgentCardAction::ToggleActive => {
                if self.catalogs.active_agent.as_deref() == Some(agent_id.as_str()) {
                    self.clear_active_agent();
                } else {
                    self.set_active_agent_by_name(&agent_id);
                }
            }
        }
    }

    pub(super) fn handle_skill_card_action(&mut self, action: SkillCardAction, skill_id: String) {
        match action {
            SkillCardAction::View => {
                if let Some(index) = self.catalogs.skill_entries
                    .iter()
                    .position(|entry| entry.info.id == skill_id)
                {
                    if matches!(self.modal_state, ModalState::SkillList { .. }) {
                        self.modal_state = ModalState::SkillList { selected: index };
                    }
                }
            }
            SkillCardAction::Install => {
                if let Some(entry) = self.catalogs.skill_entries
                    .iter()
                    .find(|entry| entry.info.id == skill_id)
                {
                    self.push_system_message(format!(
                        "Skill {} is already installed at {}.",
                        entry.info.name,
                        entry.path.display()
                    ));
                }
            }
        }
    }

    pub(super) fn set_active_agent_by_name(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.push_system_message("Agent name is required.".to_string());
            return;
        }
        if let Some(entry) = self.catalogs.agent_entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(trimmed))
        {
            self.set_active_agent(Some(entry.name.clone()));
        } else {
            self.push_system_message(format!("Unknown agent: {}.", trimmed));
        }
    }

    pub(super) fn clear_active_agent(&mut self) {
        self.set_active_agent(None);
    }

    pub(super) fn set_active_agent(&mut self, agent: Option<String>) {
        let next = agent.and_then(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        if next == self.catalogs.active_agent {
            return;
        }
        self.catalogs.active_agent = next.clone();
        if let Some(name) = next {
            self.push_system_message(format!("Active agent set to {}.", name));
        } else {
            self.push_system_message("Active agent cleared.".to_string());
        }
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
    }

    pub(super) fn push_hook_log(&mut self, entry: HookLogEntry) {
        self.catalogs.hook_event_log.insert(0, entry);
        if self.catalogs.hook_event_log.len() > HOOK_LOG_LIMIT {
            self.catalogs.hook_event_log.truncate(HOOK_LOG_LIMIT);
        }
        if let ModalState::Hooks {
            view: HookModalView::Events,
            selected,
        } = &mut self.modal_state
        {
            *selected = 0;
            self.sync_hook_inspector(0);
        }
    }

    pub(crate) fn sync_hook_inspector(&mut self, selected: usize) {
        let Some(entry) = self.catalogs.hook_event_log.get(selected) else {
            self.catalogs.hook_inspector = None;
            return;
        };

        let event = hook_log_event_data(entry);
        let view = self.catalogs.hook_inspector_view;
        let mut inspector = EventInspector::new(event).view(view);
        if let Some(tx) = self.catalogs.hook_inspector_action_tx.clone() {
            inspector = inspector.on_view_change(move |view| {
                let _ = tx.send(view);
            });
        }
        self.catalogs.hook_inspector = Some(inspector);
    }

    pub(super) fn handle_checkpoint_restore(&mut self, index: usize) {
        self.session
            .handle_checkpoint_restore(index, &mut self.chat);
    }

    pub(super) fn attach_user_message_id(&mut self, uuid: String) {
        self.chat.attach_user_message_id(uuid, &mut self.session);
    }

    pub(super) fn clear_conversation(&mut self) {
        self.session
            .clear_conversation(&mut self.chat, &mut self.tools);
    }

    pub(super) fn start_new_session(&mut self) {
        self.session
            .start_new_session(&mut self.chat, &mut self.tools);
    }

    pub(super) fn undo_last_exchange(&mut self) {
        self.session.undo_last_exchange(&mut self.chat);
    }

    pub(super) fn interrupt_query(&mut self) {
        self.chat.interrupt_query();
    }

    #[allow(dead_code)]
    pub(super) fn abort_query(&mut self) {
        self.chat.abort_query();
    }

    pub(super) fn begin_session_resume(&mut self, session_id: String) {
        self.session
            .begin_session_resume(session_id, &mut self.chat, &mut self.tools);
    }

    pub(super) fn begin_session_fork(&mut self) {
        self.session.begin_session_fork(&mut self.chat);
    }

    pub(super) fn export_session(&mut self) {
        if self.chat.messages.is_empty() {
            self.push_system_message("No messages to export yet.".to_string());
            return;
        }
        match export_session_markdown(self) {
            Ok(path) => self.push_system_message(format!(
                "Exported session to {}.",
                path.display()
            )),
            Err(err) => self.push_system_message(format!(
                "Failed to export session: {}.",
                err
            )),
        }
    }

    pub(super) fn push_system_message(&mut self, message: String) {
        self.chat.push_system_message(message);
    }
}

fn export_session_markdown(state: &AppState) -> io::Result<PathBuf> {
    let export_dir = config_dir().join("exports");
    fs::create_dir_all(&export_dir)?;
    let session_id = if state.session.session_info.session_id.is_empty() {
        "session".to_string()
    } else {
        state.session.session_info.session_id.clone()
    };
    let filename = format!("{}-{}.md", session_id, now_timestamp());
    let path = export_dir.join(filename);
    let mut file = fs::File::create(&path)?;

    writeln!(file, "# Autopilot Session {}", session_id)?;
    if !state.session.session_info.model.is_empty() {
        writeln!(file, "- Model: {}", state.session.session_info.model)?;
    }
    writeln!(file, "- Exported: {}", now_timestamp())?;
    writeln!(file)?;

    for message in &state.chat.messages {
        match message.role {
            MessageRole::User => {
                for line in message.content.lines() {
                    writeln!(file, "> {}", line)?;
                }
                writeln!(file)?;
            }
            MessageRole::Assistant => {
                writeln!(file, "{}", message.content)?;
                writeln!(file)?;
            }
            MessageRole::AssistantThought => {
                writeln!(file, "_Thought:_ {}", message.content)?;
                writeln!(file)?;
            }
        }
    }

    Ok(path)
}

fn should_fetch_codex_sessions() -> bool {
    CodexRuntime::is_available()
}

async fn fetch_codex_session_entries(cwd: PathBuf) -> Result<Vec<SessionEntry>> {
    let runtime = CodexRuntime::spawn(CodexRuntimeConfig {
        cwd: Some(cwd),
        wire_log: None,
    })
    .await?;
    let CodexRuntime { client, .. } = runtime;

    let mut entries = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let response = match client
            .thread_list(app_server::ThreadListParams {
                cursor: cursor.clone(),
                limit: Some(50),
                model_providers: None,
            })
            .await
        {
            Ok(response) => response,
            Err(err) => {
                let _ = client.shutdown().await;
                return Err(err);
            }
        };
        for thread in response.data {
            entries.push(thread_summary_to_entry(thread));
        }
        if response.next_cursor.is_none() {
            break;
        }
        cursor = response.next_cursor;
    }

    let _ = client.shutdown().await;
    Ok(entries)
}

fn thread_summary_to_entry(thread: app_server::ThreadSummary) -> SessionEntry {
    let created_at = if thread.created_at < 0 {
        0
    } else {
        thread.created_at as u64
    };
    let model = if thread.model_provider.trim().is_empty() {
        "codex".to_string()
    } else {
        thread.model_provider.clone()
    };
    SessionEntry {
        id: thread.id.clone(),
        codex_thread_id: Some(thread.id),
        created_at,
        updated_at: created_at,
        last_message: thread.preview,
        message_count: 0,
        model,
    }
}

fn codex_skill_entry(skill: app_server::SkillMetadata) -> SkillEntry {
    let description = skill
        .short_description
        .clone()
        .unwrap_or_else(|| skill.description.clone());
    let id = format!("codex:{}:{}", skill.scope, skill.name);
    let info = wgpui::components::molecules::SkillInfo::new(id, skill.name.clone(), description)
        .status(wgpui::components::molecules::SkillInstallStatus::Installed)
        .category(wgpui::components::molecules::SkillCategory::Other)
        .author(skill.scope.clone())
        .version("codex".to_string());
    SkillEntry {
        info,
        source: crate::app::catalog::SkillSource::Codex,
        path: PathBuf::from(skill.path),
    }
}

async fn fetch_codex_models(cwd: PathBuf) -> Result<Vec<app_server::ModelInfo>> {
    let runtime = CodexRuntime::spawn(CodexRuntimeConfig {
        cwd: Some(cwd),
        wire_log: None,
    })
    .await?;
    let CodexRuntime { client, .. } = runtime;

    let mut cursor: Option<String> = None;
    let mut models = Vec::new();
    loop {
        let response = client
            .model_list(app_server::ModelListParams {
                cursor: cursor.clone(),
                limit: Some(50),
            })
            .await?;
        models.extend(response.data);
        if response.next_cursor.is_none() {
            break;
        }
        cursor = response.next_cursor;
    }

    let _ = client.shutdown().await;
    Ok(models)
}

async fn fetch_codex_config_snapshot(
    cwd: PathBuf,
) -> Result<(Option<String>, Option<app_server::ReasoningEffort>)> {
    let runtime = CodexRuntime::spawn(CodexRuntimeConfig {
        cwd: Some(cwd),
        wire_log: None,
    })
    .await?;
    let CodexRuntime { client, .. } = runtime;

    let response = client
        .config_read(app_server::ConfigReadParams {
            include_layers: false,
        })
        .await?;

    let model = response
        .config
        .get("model")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let effort = response
        .config
        .get("model_reasoning_effort")
        .and_then(|value| serde_json::from_value(value.clone()).ok());

    let _ = client.shutdown().await;
    Ok((model, effort))
}

async fn fetch_codex_skills(
    cwd: PathBuf,
    force_reload: bool,
) -> Result<(Vec<SkillEntry>, Option<String>)> {
    let runtime = CodexRuntime::spawn(CodexRuntimeConfig {
        cwd: Some(cwd.clone()),
        wire_log: None,
    })
    .await?;
    let CodexRuntime { client, .. } = runtime;

    let response = client
        .skills_list(app_server::SkillsListParams {
            cwds: vec![cwd.to_string_lossy().to_string()],
            force_reload,
        })
        .await?;

    let mut entries = Vec::new();
    let mut errors = Vec::new();
    for list in response.data {
        for skill in list.skills {
            entries.push(codex_skill_entry(skill));
        }
        for err in list.errors {
            errors.push(format!("{}: {}", err.path, err.message));
        }
    }

    let _ = client.shutdown().await;
    let error = if errors.is_empty() {
        None
    } else {
        Some(errors.join(" | "))
    };
    Ok((entries, error))
}

impl AppState {
    pub(super) fn sync_workspace_timeline_view(&mut self) {
        if self.workspaces.active_workspace_id.is_none() {
            return;
        }
        let Some(thread_id) = self.workspaces.active_thread_id() else {
            self.clear_workspace_view();
            return;
        };
        let Some(timeline) = self.workspaces.timelines_by_thread.get(&thread_id) else {
            self.clear_workspace_view();
            return;
        };

        let mut tool_cache: HashMap<String, ToolVisualization> = self
            .tools
            .tool_history
            .drain(..)
            .map(|tool| (tool.tool_use_id.clone(), tool))
            .collect();

        let mut messages = Vec::new();
        let mut tool_history = Vec::new();

        for item in &timeline.items {
            match item {
                ConversationItem::Message { role, text, .. } => {
                    let (chat_role, document) = match role {
                        ConversationRole::User => (MessageRole::User, None),
                        ConversationRole::Assistant => {
                            (MessageRole::Assistant, Some(build_markdown_document(text)))
                        }
                    };
                    messages.push(ChatMessage {
                        role: chat_role,
                        content: text.clone(),
                        document,
                        uuid: None,
                        metadata: None,
                    });
                }
                ConversationItem::Reasoning { summary, content, .. } => {
                    let mut combined = String::new();
                    if !summary.trim().is_empty() {
                        combined.push_str(summary);
                    }
                    if !content.trim().is_empty() {
                        if !combined.is_empty() {
                            combined.push_str("\n\n");
                        }
                        combined.push_str(content);
                    }
                    let combined = if combined.is_empty() {
                        "Reasoning".to_string()
                    } else {
                        combined
                    };
                    messages.push(ChatMessage {
                        role: MessageRole::AssistantThought,
                        content: combined,
                        document: None,
                        uuid: None,
                        metadata: None,
                    });
                }
                ConversationItem::Review { state, text, .. } => {
                    let label = match state {
                        ReviewState::Started => "Review started",
                        ReviewState::Completed => "Review completed",
                    };
                    let content = if text.trim().is_empty() {
                        label.to_string()
                    } else {
                        format!("{}: {}", label, text)
                    };
                    messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content,
                        document: None,
                        uuid: None,
                        metadata: None,
                    });
                }
                ConversationItem::Tool { id, data } => {
                    let msg_index = messages.len();
                    let title = if data.title.is_empty() {
                        data.tool_name.clone()
                    } else {
                        data.title.clone()
                    };
                    messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: title,
                        document: None,
                        uuid: None,
                        metadata: None,
                    });

                    let mut tool = tool_cache.remove(id).unwrap_or_else(|| {
                        ToolVisualization::new(
                            id.clone(),
                            data.tool_name.clone(),
                            tool_type_for_name(&data.tool_name),
                            msg_index,
                        )
                    });

                    tool.message_index = msg_index;
                    sync_tool_from_item(&mut tool, data);
                    tool_history.push(tool);
                }
            }
        }

        self.chat.messages = messages;
        self.chat.streaming_markdown.reset();
        self.chat.chat_selection = None;
        self.chat.is_thinking = self
            .workspaces
            .thread_status_by_id
            .get(&thread_id)
            .map(|status| status.is_processing)
            .unwrap_or(false);

        self.tools.tool_history = tool_history;
        self.tools.dspy_stages.clear();
        self.tools.current_tool_name = None;
        self.tools.current_tool_input.clear();
        self.tools.current_tool_use_id = None;
    }

    fn clear_workspace_view(&mut self) {
        self.chat.messages.clear();
        self.chat.streaming_markdown.reset();
        self.chat.chat_selection = None;
        self.chat.is_thinking = false;
        self.tools.tool_history.clear();
        self.tools.dspy_stages.clear();
        self.tools.current_tool_name = None;
        self.tools.current_tool_input.clear();
        self.tools.current_tool_use_id = None;
    }
}

fn sync_tool_from_item(tool: &mut ToolVisualization, data: &ToolItemData) {
    tool.name = data.tool_name.clone();
    tool.tool_type = tool_type_for_name(&data.tool_name);
    tool.status = tool_status_from_item(data.status.as_deref(), data.output.as_str());
    tool.output = if data.output.trim().is_empty() {
        None
    } else {
        Some(data.output.clone())
    };
    tool.output_value = data.output_value.clone();

    if let Some(value) = data.input_value.as_ref() {
        if let Ok(json) = serde_json::to_string(value) {
            tool.input = Some(format_tool_input(&data.tool_name, &json));
        }
        tool.input_value = Some(value.clone());
    } else if !data.detail.trim().is_empty() {
        tool.input = Some(data.detail.clone());
        tool.input_value = None;
    } else {
        tool.input = None;
        tool.input_value = None;
    }

    tool.refresh_components();
}

fn tool_status_from_item(
    status: Option<&str>,
    output: &str,
) -> wgpui::components::atoms::ToolStatus {
    let status = status.unwrap_or("").trim().to_ascii_lowercase();
    match status.as_str() {
        "failed" | "declined" | "error" => wgpui::components::atoms::ToolStatus::Error,
        "completed" | "complete" | "succeeded" | "success" => {
            wgpui::components::atoms::ToolStatus::Success
        }
        "running" | "started" | "pending" => wgpui::components::atoms::ToolStatus::Running,
        _ => {
            if output.trim().is_empty() {
                wgpui::components::atoms::ToolStatus::Running
            } else {
                wgpui::components::atoms::ToolStatus::Success
            }
        }
    }
}

async fn write_codex_config_value(cwd: PathBuf, key_path: String, value: Value) -> Result<()> {
    let runtime = CodexRuntime::spawn(CodexRuntimeConfig {
        cwd: Some(cwd),
        wire_log: None,
    })
    .await?;
    let CodexRuntime { client, .. } = runtime;

    let params = app_server::ConfigValueWriteParams {
        key_path,
        value,
        merge_strategy: app_server::MergeStrategy::Replace,
        file_path: None,
        expected_version: None,
    };
    let result = client.config_value_write(params).await;
    let _ = client.shutdown().await;
    result.map(|_| ())
}
