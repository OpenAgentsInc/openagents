use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use claude_agent_sdk::{AgentDefinition, SettingSource};
use tokio::sync::mpsc;
use wgpui::components::hud::Command as PaletteCommand;
use wgpui::components::molecules::SessionAction;
use wgpui::components::organisms::EventInspector;

use crate::app::catalog::SkillSource;
use crate::app::chat::MessageRole;
use crate::app::config::{config_dir, SettingsTab};
use crate::app::events::{keybinding_labels, ModalState};
use crate::app::{
    build_input, build_markdown_config, build_markdown_renderer, now_timestamp,
    AgentCardAction, HookLogEntry, HookModalView, HookSetting, ModelOption, SettingsInputMode,
    SkillCardAction,
};
use crate::app::AppState;
use crate::keybindings::Action as KeyAction;

use super::command_palette_ids;
use super::hooks::hook_log_event_data;
use super::settings::{normalize_settings, save_settings, update_settings_model};

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
            "Configure Coder preferences",
            "Navigation",
            Some(settings_keys),
        );

        push_command(
            command_palette_ids::MODEL_PICKER,
            "Select Model",
            "Choose the model for this session",
            "Navigation",
            None,
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
        let wallet_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::OpenWallet, "Ctrl+Shift+W");
        push_command(
            command_palette_ids::WALLET_OPEN,
            "Open Wallet",
            "View wallet status and configuration",
            "Wallet",
            Some(wallet_keys),
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
            command_palette_ids::MODE_CYCLE,
            "Cycle Mode",
            "Rotate through modes (Bypass/Plan/Autopilot)",
            "Mode",
            Some("Shift+Tab".to_string()),
        );
        push_command(
            command_palette_ids::MODE_BYPASS,
            "Mode: Bypass Permissions",
            "Auto-approve all tool use",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::MODE_PLAN,
            "Mode: Plan",
            "Read-only mode, deny write operations",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::MODE_AUTOPILOT,
            "Mode: Autopilot",
            "Use DSPy/Adjutant for autonomous execution",
            "Mode",
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
        self.modal_state = ModalState::None;
        if self.chat.chat_context_menu.is_open() {
            self.chat.chat_context_menu.close();
            self.chat.chat_context_menu_target = None;
        }
        self.command_palette.set_commands(self.build_command_palette_commands());
        self.command_palette.open();
    }

    pub(super) fn open_model_picker(&mut self) {
        let current_idx = ModelOption::all()
            .iter()
            .position(|m| *m == self.settings.selected_model)
            .unwrap_or(0);
        self.modal_state = ModalState::ModelPicker { selected: current_idx };
    }

    pub(super) fn open_session_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (checkpoint_tx, checkpoint_rx) = mpsc::unbounded_channel();
        self.session.session_action_tx = Some(action_tx);
        self.session.session_action_rx = Some(action_rx);
        self.session.checkpoint_action_tx = Some(checkpoint_tx);
        self.session.checkpoint_action_rx = Some(checkpoint_rx);
        self.session.refresh_session_cards(self.chat.is_thinking);
        self.session.refresh_checkpoint_restore(&self.chat.messages);
        let selected = self.session.session_index
            .iter()
            .position(|entry| entry.id == self.session.session_info.session_id)
            .unwrap_or(0);
        self.modal_state = ModalState::SessionList { selected };
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

    pub(super) fn open_skill_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.skill_action_tx = Some(action_tx);
        self.catalogs.skill_action_rx = Some(action_rx);
        self.catalogs.refresh_skill_cards();
        self.modal_state = ModalState::SkillList { selected: 0 };
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

    pub(super) fn request_wallet_refresh(&mut self) {
        self.refresh_wallet_snapshot();
        self.request_oanix_refresh();
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
        self.session.session_info.model = self.settings.selected_model.model_id().to_string();
        update_settings_model(&mut self.settings.coder_settings, self.settings.selected_model);
        self.persist_settings();
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

    pub(super) fn agent_definitions_for_query(&self) -> HashMap<String, AgentDefinition> {
        let mut agents = HashMap::new();
        for entry in &self.catalogs.agent_entries {
            agents.insert(entry.name.clone(), entry.definition.clone());
        }
        agents
    }

    pub(super) fn setting_sources_for_query(&self) -> Vec<SettingSource> {
        let mut sources = Vec::new();
        if self.catalogs.skill_entries
            .iter()
            .any(|entry| entry.source == SkillSource::Project)
        {
            sources.push(SettingSource::Project);
        }
        if self.catalogs.skill_entries
            .iter()
            .any(|entry| entry.source == SkillSource::User)
        {
            sources.push(SettingSource::User);
        }
        sources
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

    pub(super) fn begin_session_fork_from(&mut self, session_id: String) {
        self.session
            .begin_session_fork_from(session_id, &mut self.chat, &mut self.tools);
    }

    pub(super) fn attach_user_message_id(&mut self, uuid: String) {
        self.chat.attach_user_message_id(uuid, &mut self.session);
    }

    pub(super) fn request_rewind_files(&mut self, user_message_id: String) {
        self.chat.request_rewind_files(user_message_id);
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

    writeln!(file, "# Coder Session {}", session_id)?;
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
        }
    }

    Ok(path)
}
