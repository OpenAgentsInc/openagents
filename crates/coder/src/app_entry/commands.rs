use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

use serde_json::Value;
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamedKey};

use crate::app::catalog::{
    expand_env_vars_in_value, parse_mcp_server_config, save_hook_config, McpServerEntry,
};
use crate::app::config::{SettingsItem, SettingsTab};
use crate::app::events::{
    convert_key_for_binding, convert_modifiers, CommandAction, CoderMode, ModalState,
};
use crate::app::permissions::{coder_mode_default_allow, coder_mode_label, parse_coder_mode};
use crate::app::ui::ThemeSetting;
use crate::app::{
    settings_rows, HookModalView, HookSetting, ModelOption, SettingsInputMode,
    SettingsSnapshot,
};
use crate::app::AppState;
use crate::commands::Command;
use crate::keybindings::{default_keybindings, Keybinding};

use super::settings::{clamp_font_size, save_keybindings};

const BUG_REPORT_URL: &str = "https://github.com/OpenAgentsInc/openagents/issues/new";

pub(super) fn handle_command(state: &mut AppState, command: Command) -> CommandAction {
    match command {
        Command::Help => {
            state.open_command_palette();
            CommandAction::None
        }
        Command::Clear => {
            state.clear_conversation();
            CommandAction::None
        }
        Command::Compact => {
            if state.chat.is_thinking {
                state.push_system_message("Cannot compact during an active request.".to_string());
                CommandAction::None
            } else {
                CommandAction::SubmitPrompt("/compact".to_string())
            }
        }
        Command::Model => {
            state.open_model_picker();
            CommandAction::None
        }
        Command::Undo => {
            state.undo_last_exchange();
            CommandAction::None
        }
        Command::Cancel => {
            state.interrupt_query();
            CommandAction::None
        }
        Command::Bug => {
            match open_url(BUG_REPORT_URL) {
                Ok(()) => state.push_system_message("Opened bug report in browser.".to_string()),
                Err(err) => state.push_system_message(format!(
                    "Failed to open browser: {} (URL: {}).",
                    err, BUG_REPORT_URL
                )),
            }
            CommandAction::None
        }
        Command::SessionList => {
            state.open_session_list();
            CommandAction::None
        }
        Command::SessionResume(id) => {
            state.begin_session_resume(id);
            CommandAction::None
        }
        Command::SessionFork => {
            state.begin_session_fork();
            CommandAction::None
        }
        Command::SessionExport => {
            state.export_session();
            CommandAction::None
        }
        Command::PermissionMode(mode) => {
            match parse_coder_mode(&mode) {
                Some(parsed) => state
                    .permissions
                    .set_coder_mode(parsed, &mut state.session.session_info),
                None => state.push_system_message(format!(
                    "Unknown mode: {}. Valid modes: bypass, plan, autopilot",
                    mode
                )),
            }
            CommandAction::None
        }
        Command::PermissionRules => {
            state.open_permission_rules();
            CommandAction::None
        }
        Command::PermissionAllow(tools) => {
            let message = state.permissions.add_permission_allow(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::PermissionDeny(tools) => {
            let message = state.permissions.add_permission_deny(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::ToolsList => {
            state.open_tool_list();
            CommandAction::None
        }
        Command::ToolsEnable(tools) => {
            let message = state.permissions.enable_tools(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::ToolsDisable(tools) => {
            let message = state.permissions.disable_tools(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::Config => {
            state.open_config();
            CommandAction::None
        }
        Command::OutputStyle(style) => {
            let trimmed = style.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
                let message = state.permissions.set_output_style(None);
                state.push_system_message(message);
                return CommandAction::None;
            }

            match resolve_output_style(trimmed) {
                Ok(Some(_path)) => {
                    let message = state
                        .permissions
                        .set_output_style(Some(trimmed.to_string()));
                    state.push_system_message(message);
                }
                Ok(None) => state.push_system_message(format!(
                    "Output style not found: {}.",
                    trimmed
                )),
                Err(err) => state.push_system_message(format!(
                    "Failed to load output style: {}.",
                    err
                )),
            }
            CommandAction::None
        }
        Command::Mcp => {
            state.open_mcp_config();
            CommandAction::None
        }
        Command::McpReload => {
            state.reload_mcp_project_servers();
            if let Some(err) = &state.catalogs.mcp_project_error {
                state.push_system_message(format!("MCP config reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded MCP project config.".to_string());
            }
            CommandAction::None
        }
        Command::McpStatus => {
            state.request_mcp_status();
            CommandAction::None
        }
        Command::McpAdd { name, config } => {
            let trimmed_name = name.trim();
            if trimmed_name.is_empty() {
                state.push_system_message("MCP add requires a server name.".to_string());
                return CommandAction::None;
            }
            let config_text = config.trim();
            if config_text.is_empty() {
                state.push_system_message("MCP add requires a JSON config.".to_string());
                return CommandAction::None;
            }
            match serde_json::from_str::<Value>(config_text) {
                Ok(value) => {
                    let expanded = expand_env_vars_in_value(&value);
                    match parse_mcp_server_config(trimmed_name, &expanded) {
                        Ok(server) => {
                            state.catalogs.add_runtime_mcp_server(trimmed_name.to_string(), server);
                            state.push_system_message(format!(
                                "Added MCP server {} (applies next request).",
                                trimmed_name
                            ));
                        }
                        Err(err) => state.push_system_message(format!(
                            "Failed to add MCP server {}: {}",
                            trimmed_name, err
                        )),
                    }
                }
                Err(err) => state.push_system_message(format!(
                    "Failed to parse MCP server JSON: {}",
                    err
                )),
            }
            CommandAction::None
        }
        Command::McpRemove(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                state.push_system_message("MCP remove requires a server name.".to_string());
                return CommandAction::None;
            }
            state.catalogs.remove_mcp_server(trimmed);
            state.push_system_message(format!(
                "Disabled MCP server {} (applies next request).",
                trimmed
            ));
            CommandAction::None
        }
        Command::Agents => {
            state.open_agent_list();
            CommandAction::None
        }
        Command::AgentSelect(name) => {
            state.set_active_agent_by_name(&name);
            CommandAction::None
        }
        Command::AgentClear => {
            state.clear_active_agent();
            CommandAction::None
        }
        Command::AgentReload => {
            state.reload_agents();
            if let Some(err) = &state.catalogs.agent_load_error {
                state.push_system_message(format!("Agent reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded agents from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Skills => {
            state.open_skill_list();
            CommandAction::None
        }
        Command::SkillsReload => {
            state.reload_skills();
            if let Some(err) = &state.catalogs.skill_load_error {
                state.push_system_message(format!("Skill reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded skills from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Hooks => {
            state.open_hooks();
            CommandAction::None
        }
        Command::HooksReload => {
            state.reload_hooks();
            if let Some(err) = &state.catalogs.hook_load_error {
                state.push_system_message(format!("Hook reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded hook scripts from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Wallet => {
            state.open_wallet();
            CommandAction::None
        }
        Command::WalletRefresh => {
            state.request_wallet_refresh();
            state.open_wallet();
            CommandAction::None
        }
        Command::Dvm => {
            state.open_dvm();
            CommandAction::None
        }
        Command::DvmRefresh => {
            state.refresh_dvm();
            state.open_dvm();
            CommandAction::None
        }
        Command::DvmConnect(relay_url) => {
            if relay_url.trim().is_empty() {
                state.push_system_message("DVM relay URL is required.".to_string());
            } else {
                state.connect_dvm(Some(relay_url));
                state.open_dvm();
            }
            CommandAction::None
        }
        Command::DvmKind(kind) => {
            state.set_dvm_job_kind(kind);
            state.open_dvm();
            CommandAction::None
        }
        Command::Gateway => {
            state.open_gateway();
            CommandAction::None
        }
        Command::GatewayRefresh => {
            state.refresh_gateway();
            state.open_gateway();
            CommandAction::None
        }
        Command::LmRouter => {
            state.open_lm_router();
            CommandAction::None
        }
        Command::LmRouterRefresh => {
            state.refresh_lm_router();
            state.open_lm_router();
            CommandAction::None
        }
        Command::Nexus => {
            state.open_nexus();
            CommandAction::None
        }
        Command::NexusRefresh => {
            state.refresh_nexus();
            state.open_nexus();
            CommandAction::None
        }
        Command::NexusConnect(stats_url) => {
            if stats_url.trim().is_empty() {
                state.push_system_message("Nexus stats URL is required.".to_string());
            } else {
                state.connect_nexus(stats_url);
                state.open_nexus();
            }
            CommandAction::None
        }
        Command::SparkWallet => {
            state.open_spark_wallet();
            CommandAction::None
        }
        Command::SparkWalletRefresh => {
            state.refresh_spark_wallet();
            state.open_spark_wallet();
            CommandAction::None
        }
        Command::Nip90 => {
            state.open_nip90();
            CommandAction::None
        }
        Command::Nip90Refresh => {
            state.refresh_nip90();
            state.open_nip90();
            CommandAction::None
        }
        Command::Nip90Connect(relay_url) => {
            if relay_url.trim().is_empty() {
                state.push_system_message("NIP-90 relay URL is required.".to_string());
            } else {
                state.connect_nip90(Some(relay_url));
                state.open_nip90();
            }
            CommandAction::None
        }
        Command::Oanix => {
            state.open_oanix();
            CommandAction::None
        }
        Command::OanixRefresh => {
            state.refresh_oanix();
            state.open_oanix();
            CommandAction::None
        }
        Command::Directives => {
            state.open_directives();
            CommandAction::None
        }
        Command::DirectivesRefresh => {
            state.refresh_directives();
            state.open_directives();
            CommandAction::None
        }
        Command::Issues => {
            state.open_issues();
            CommandAction::None
        }
        Command::IssuesRefresh => {
            state.refresh_issues();
            state.open_issues();
            CommandAction::None
        }
        Command::AutopilotIssues => {
            state.open_issue_tracker();
            CommandAction::None
        }
        Command::AutopilotIssuesRefresh => {
            state.refresh_issue_tracker();
            state.open_issue_tracker();
            CommandAction::None
        }
        Command::Rlm => {
            state.open_rlm();
            CommandAction::None
        }
        Command::RlmRefresh => {
            state.refresh_rlm();
            state.open_rlm();
            CommandAction::None
        }
        Command::RlmTrace(run_id) => {
            state.open_rlm_trace(run_id);
            CommandAction::None
        }
        Command::Dspy => {
            state.open_dspy();
            CommandAction::None
        }
        Command::DspyRefresh => {
            state.refresh_dspy_snapshot();
            state.push_system_message("DSPy status refreshed.".to_string());
            CommandAction::None
        }
        Command::DspyAuto(enabled) => {
            state.set_dspy_auto_optimizer_enabled(enabled);
            CommandAction::None
        }
        Command::DspyBackground(enabled) => {
            state.set_dspy_background_optimization(enabled);
            CommandAction::None
        }
        Command::Nip28 => {
            state.open_nip28();
            CommandAction::None
        }
        Command::Nip28Refresh => {
            state.refresh_nip28();
            state.open_nip28();
            CommandAction::None
        }
        Command::Nip28Connect(relay_url) => {
            if relay_url.trim().is_empty() {
                state.push_system_message("NIP-28 relay URL is required.".to_string());
            } else {
                state.connect_nip28(Some(relay_url));
                state.open_nip28();
            }
            CommandAction::None
        }
        Command::Nip28Channel(channel) => {
            if channel.trim().is_empty() {
                state.push_system_message("NIP-28 channel name or id is required.".to_string());
            } else {
                state.set_nip28_channel(channel);
                state.open_nip28();
            }
            CommandAction::None
        }
        Command::Nip28Send(message) => {
            if message.trim().is_empty() {
                state.push_system_message("NIP-28 message is empty.".to_string());
            } else {
                state.send_nip28_message(message);
                state.open_nip28();
            }
            CommandAction::None
        }
        Command::Custom(name, args) => {
            if state.chat.is_thinking {
                state.push_system_message(
                    "Cannot run custom commands during an active request.".to_string(),
                );
                return CommandAction::None;
            }

            match load_custom_command(&name) {
                Ok(Some(template)) => {
                    let prompt = apply_custom_command_args(&template, &args);
                    CommandAction::SubmitPrompt(prompt)
                }
                Ok(None) => {
                    let mut message = format!("Unknown command: /{}", name);
                    if !args.is_empty() {
                        message.push(' ');
                        message.push_str(&args.join(" "));
                    }
                    state.push_system_message(message);
                    CommandAction::None
                }
                Err(err) => {
                    state.push_system_message(format!(
                        "Failed to load custom command /{}: {}.",
                        name, err
                    ));
                    CommandAction::None
                }
            }
        }
    }
}

pub(super) fn handle_modal_input(state: &mut AppState, key: &WinitKey) -> bool {
    let empty_entries: Vec<McpServerEntry> = Vec::new();
    let mcp_entries = if matches!(state.modal_state, ModalState::McpConfig { .. }) {
        Some(state.catalogs.mcp_entries())
    } else {
        None
    };
    let settings_snapshot = SettingsSnapshot::from_state(state);
    match &mut state.modal_state {
        ModalState::ModelPicker { selected } => {
            let selected = *selected;
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    let models = ModelOption::all();
                    state.update_selected_model(models[selected]);
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if selected > 0 {
                        state.modal_state = ModalState::ModelPicker { selected: selected - 1 };
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if selected + 1 < ModelOption::all().len() {
                        state.modal_state = ModalState::ModelPicker { selected: selected + 1 };
                    }
                }
                WinitKey::Character(c) => {
                    match c.as_str() {
                        "1" => {
                            state.settings.selected_model = ModelOption::Opus;
                        }
                        "2" => {
                            state.settings.selected_model = ModelOption::Sonnet;
                        }
                        "3" => {
                            state.settings.selected_model = ModelOption::Haiku;
                        }
                        _ => {}
                    }
                    if matches!(c.as_str(), "1" | "2" | "3") {
                        state.update_selected_model(state.settings.selected_model);
                        state.modal_state = ModalState::None;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::SessionList { selected } => {
            let session_count = state.session.session_index.len();
            if session_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= session_count {
                *selected = session_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    if let Some(entry) = state.session.session_index.get(*selected).cloned() {
                        state.begin_session_resume(entry.id);
                    }
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < session_count {
                        *selected += 1;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::AgentList { selected } => {
            let agent_count = state.catalogs.agent_entries.len();
            if agent_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_agents();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= agent_count {
                *selected = agent_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    let selected_name = state.catalogs.agent_entries
                        .get(*selected)
                        .map(|entry| entry.name.clone());
                    if let Some(name) = selected_name {
                        state.set_active_agent_by_name(&name);
                    }
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < agent_count {
                        *selected += 1;
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_agents();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::SkillList { selected } => {
            let skill_count = state.catalogs.skill_entries.len();
            if skill_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_skills();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= skill_count {
                *selected = skill_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < skill_count {
                        *selected += 1;
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_skills();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Hooks { view, selected } => {
            let mut sync_index = None;
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Tab) => {
                    *view = match *view {
                        HookModalView::Config => HookModalView::Events,
                        HookModalView::Events => HookModalView::Config,
                    };
                    if *view == HookModalView::Events {
                        *selected = 0;
                        sync_index = Some(*selected);
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_hooks();
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("c") => {
                    if *view == HookModalView::Events {
                        state.clear_hook_log();
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *view == HookModalView::Events && !state.catalogs.hook_event_log.is_empty() {
                        if *selected > 0 {
                            *selected -= 1;
                            sync_index = Some(*selected);
                        }
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *view == HookModalView::Events && !state.catalogs.hook_event_log.is_empty() {
                        if *selected + 1 < state.catalogs.hook_event_log.len() {
                            *selected += 1;
                            sync_index = Some(*selected);
                        }
                    }
                }
                WinitKey::Character(c) if *view == HookModalView::Config => match c.as_str() {
                    "1" => state.toggle_hook_setting(HookSetting::ToolBlocker),
                    "2" => state.toggle_hook_setting(HookSetting::ToolLogger),
                    "3" => state.toggle_hook_setting(HookSetting::OutputTruncator),
                    "4" => state.toggle_hook_setting(HookSetting::ContextInjection),
                    "5" => state.toggle_hook_setting(HookSetting::TodoEnforcer),
                    _ => {}
                },
                _ => {}
            }
            if let Some(index) = sync_index {
                state.sync_hook_inspector(index);
            }
            state.window.request_redraw();
            true
        }
        ModalState::ToolList { selected } => {
            let tool_count = state.session.session_info.tools.len();
            if tool_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= tool_count {
                *selected = tool_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < tool_count {
                        *selected += 1;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::McpConfig { selected } => {
            let entries = mcp_entries.as_ref().unwrap_or(&empty_entries);
            if entries.is_empty() {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_mcp_project_servers();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= entries.len() {
                *selected = entries.len() - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < entries.len() {
                        *selected += 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::Delete | WinitNamedKey::Backspace) => {
                    if let Some(entry) = entries.get(*selected) {
                        state.catalogs.remove_mcp_server(&entry.name);
                    }
                }
                WinitKey::Character(c) => match c.as_str() {
                    "r" | "R" => {
                        state.reload_mcp_project_servers();
                    }
                    "s" | "S" => {
                        state.request_mcp_status();
                    }
                    _ => {}
                },
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::PermissionRules => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Wallet => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.request_wallet_refresh();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::DvmProviders => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_dvm();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Gateway => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_gateway();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::LmRouter => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_lm_router();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Nexus => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_nexus();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::SparkWallet => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_spark_wallet();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Nip90Jobs => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_nip90();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Oanix => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_oanix();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Directives => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_directives();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Issues => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_issues();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::AutopilotIssues => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_issue_tracker();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Rlm => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_rlm();
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("t") => {
                    state.open_rlm_trace(None);
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::RlmTrace => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_rlm_trace();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Nip28Chat => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    if let Err(err) = state.nip28.send_message() {
                        state.nip28.status_message = Some(err);
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowLeft) => {
                    state.nip28.move_cursor_left();
                }
                WinitKey::Named(WinitNamedKey::ArrowRight) => {
                    state.nip28.move_cursor_right();
                }
                WinitKey::Named(WinitNamedKey::Home) => {
                    state.nip28.move_cursor_home();
                }
                WinitKey::Named(WinitNamedKey::End) => {
                    state.nip28.move_cursor_end();
                }
                WinitKey::Named(WinitNamedKey::Backspace) => {
                    state.nip28.backspace();
                }
                WinitKey::Named(WinitNamedKey::Delete) => {
                    state.nip28.delete();
                }
                WinitKey::Character(c)
                    if c.eq_ignore_ascii_case("r") && state.modifiers.control_key() =>
                {
                    state.refresh_nip28();
                }
                WinitKey::Character(c) => {
                    if !state.modifiers.control_key()
                        && !state.modifiers.super_key()
                        && !state.modifiers.alt_key()
                    {
                        state.nip28.insert_text(c);
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Dspy => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_dspy_snapshot();
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("e") => {
                    let enabled = !state.dspy.snapshot.auto_optimizer.config.enabled;
                    state.set_dspy_auto_optimizer_enabled(enabled);
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("b") => {
                    let enabled = !state.dspy.snapshot.auto_optimizer.config.background_optimization;
                    state.set_dspy_background_optimization(enabled);
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Help => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter | WinitNamedKey::F1) => {
                    state.modal_state = ModalState::None;
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Config {
            tab,
            selected,
            search,
            input_mode,
        } => {
            let rows = settings_rows(&settings_snapshot, *tab, search);
            if rows.is_empty() {
                *selected = 0;
            } else if *selected >= rows.len() {
                *selected = rows.len().saturating_sub(1);
            }
            let current_item = rows.get(*selected).map(|row| row.item);
            let shift = state.modifiers.shift_key();
            let ctrl = state.modifiers.control_key();

            let mut change_tab = |forward: bool| {
                let tabs = SettingsTab::all();
                let current_index = tabs.iter().position(|entry| entry == tab).unwrap_or(0);
                let next_index = if forward {
                    (current_index + 1) % tabs.len()
                } else {
                    (current_index + tabs.len() - 1) % tabs.len()
                };
                *tab = tabs[next_index];
                *selected = 0;
            };

            match input_mode {
                SettingsInputMode::Search => match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        *input_mode = SettingsInputMode::Normal;
                    }
                    WinitKey::Named(WinitNamedKey::Backspace) => {
                        search.pop();
                        *selected = 0;
                    }
                    WinitKey::Character(c) => {
                        search.push_str(c.as_str());
                        *selected = 0;
                    }
                    WinitKey::Named(WinitNamedKey::Tab) => {
                        *input_mode = SettingsInputMode::Normal;
                        change_tab(!shift);
                    }
                    _ => {}
                },
                SettingsInputMode::Capture(action) => match key {
                    WinitKey::Named(WinitNamedKey::Escape) => {
                        *input_mode = SettingsInputMode::Normal;
                    }
                    WinitKey::Named(WinitNamedKey::Backspace | WinitNamedKey::Delete) => {
                        state.settings.keybindings.retain(|binding| binding.action != *action);
                        save_keybindings(&state.settings.keybindings);
                        *input_mode = SettingsInputMode::Normal;
                    }
                    _ => {
                        if let Some(binding_key) = convert_key_for_binding(key) {
                            let modifiers = convert_modifiers(&state.modifiers);
                            state.settings.keybindings.retain(|binding| {
                                binding.action != *action
                                    && !(binding.key == binding_key && binding.modifiers == modifiers)
                            });
                            state.settings.keybindings.push(Keybinding {
                                key: binding_key,
                                modifiers,
                                action: *action,
                            });
                            save_keybindings(&state.settings.keybindings);
                        }
                        *input_mode = SettingsInputMode::Normal;
                    }
                },
                SettingsInputMode::Normal => match key {
                    WinitKey::Named(WinitNamedKey::Escape) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Named(WinitNamedKey::Tab) => {
                        change_tab(!shift);
                    }
                    WinitKey::Named(WinitNamedKey::ArrowUp) => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                    WinitKey::Named(WinitNamedKey::ArrowDown) => {
                        if *selected + 1 < rows.len() {
                            *selected += 1;
                        }
                    }
                    WinitKey::Character(c) if (ctrl && c.eq_ignore_ascii_case("f")) || c == "/" => {
                        *input_mode = SettingsInputMode::Search;
                    }
                    WinitKey::Named(WinitNamedKey::ArrowLeft)
                    | WinitKey::Named(WinitNamedKey::ArrowRight)
                    | WinitKey::Named(WinitNamedKey::Enter) => {
                        let forward = !matches!(key, WinitKey::Named(WinitNamedKey::ArrowLeft));
                        if let Some(item) = current_item {
                            match item {
                                SettingsItem::Theme => {
                                    state.settings.coder_settings.theme = if state.settings.coder_settings.theme == ThemeSetting::Dark {
                                        ThemeSetting::Light
                                    } else {
                                        ThemeSetting::Dark
                                    };
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::FontSize => {
                                    let delta = if forward { 1.0 } else { -1.0 };
                                    state.settings.coder_settings.font_size =
                                        clamp_font_size(state.settings.coder_settings.font_size + delta);
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::AutoScroll => {
                                    state.settings.coder_settings.auto_scroll = !state.settings.coder_settings.auto_scroll;
                                    state.persist_settings();
                                }
                                SettingsItem::DefaultModel => {
                                    let next = cycle_model(state.settings.selected_model, forward);
                                    state.update_selected_model(next);
                                }
                                SettingsItem::MaxThinkingTokens => {
                                    const THINKING_STEP: u32 = 256;
                                    const THINKING_MAX: u32 = 8192;
                                    let current = state.settings.coder_settings.max_thinking_tokens.unwrap_or(0);
                                    let next = if forward {
                                        let value = current.saturating_add(THINKING_STEP).min(THINKING_MAX);
                                        Some(value)
                                    } else if current <= THINKING_STEP {
                                        None
                                    } else {
                                        Some(current - THINKING_STEP)
                                    };
                                    state.settings.coder_settings.max_thinking_tokens = next;
                                    state.persist_settings();
                                }
                                SettingsItem::PermissionMode => {
                                    let next = cycle_coder_mode_standalone(state.permissions.coder_mode, forward);
                                    state.permissions.coder_mode = next;
                                    state.permissions.permission_default_allow =
                                        coder_mode_default_allow(next, state.permissions.permission_default_allow);
                                    state.session.session_info.permission_mode =
                                        coder_mode_label(next).to_string();
                                    state.permissions.persist_permission_config();
                                }
                                SettingsItem::PermissionDefaultAllow => {
                                    state.permissions.permission_default_allow = !state.permissions.permission_default_allow;
                                    state.permissions.persist_permission_config();
                                }
                                SettingsItem::PermissionRules
                                | SettingsItem::PermissionAllowList
                                | SettingsItem::PermissionDenyList
                                | SettingsItem::PermissionBashAllowList
                                | SettingsItem::PermissionBashDenyList => {
                                    state.open_permission_rules();
                                }
                                SettingsItem::SessionAutoSave => {
                                    state.settings.coder_settings.session_auto_save = !state.settings.coder_settings.session_auto_save;
                                    state.persist_settings();
                                    if state.settings.coder_settings.session_auto_save {
                                        state.apply_session_history_limit();
                                    }
                                }
                                SettingsItem::SessionHistoryLimit => {
                                    const HISTORY_STEP: usize = 10;
                                    const HISTORY_MAX: usize = 500;
                                    let current = state.settings.coder_settings.session_history_limit;
                                    let next = if forward {
                                        if current == 0 {
                                            HISTORY_STEP
                                        } else {
                                            (current + HISTORY_STEP).min(HISTORY_MAX)
                                        }
                                    } else if current <= HISTORY_STEP {
                                        0
                                    } else {
                                        current - HISTORY_STEP
                                    };
                                    state.settings.coder_settings.session_history_limit = next;
                                    state.persist_settings();
                                    state.apply_session_history_limit();
                                }
                                SettingsItem::SessionStoragePath | SettingsItem::McpSummary => {}
                                SettingsItem::McpOpenConfig => {
                                    state.open_mcp_config();
                                }
                                SettingsItem::McpReloadProject => {
                                    state.reload_mcp_project_servers();
                                    if let Some(err) = &state.catalogs.mcp_project_error {
                                        state.push_system_message(format!(
                                            "MCP reload warning: {}",
                                            err
                                        ));
                                    } else {
                                        state.push_system_message(
                                            "Reloaded MCP project config.".to_string(),
                                        );
                                    }
                                }
                                SettingsItem::McpRefreshStatus => {
                                    state.request_mcp_status();
                                }
                                SettingsItem::HookToolBlocker => {
                                    state.toggle_hook_setting(HookSetting::ToolBlocker);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookToolLogger => {
                                    state.toggle_hook_setting(HookSetting::ToolLogger);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookOutputTruncator => {
                                    state.toggle_hook_setting(HookSetting::OutputTruncator);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookContextInjection => {
                                    state.toggle_hook_setting(HookSetting::ContextInjection);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookTodoEnforcer => {
                                    state.toggle_hook_setting(HookSetting::TodoEnforcer);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookOpenPanel => {
                                    state.open_hooks();
                                }
                                SettingsItem::Keybinding(action) => {
                                    *input_mode = SettingsInputMode::Capture(action);
                                }
                                SettingsItem::KeybindingReset => {
                                    state.settings.keybindings = default_keybindings();
                                    save_keybindings(&state.settings.keybindings);
                                }
                            }
                        }
                    }
                    _ => {}
                },
            }
            state.window.request_redraw();
            true
        }
        ModalState::None => false,
    }
}

fn cycle_model(current: ModelOption, forward: bool) -> ModelOption {
    let models = ModelOption::all();
    let idx = models
        .iter()
        .position(|model| *model == current)
        .unwrap_or(0);
    let next = if forward {
        (idx + 1) % models.len()
    } else {
        (idx + models.len() - 1) % models.len()
    };
    models[next]
}

fn cycle_coder_mode_standalone(current: CoderMode, forward: bool) -> CoderMode {
    let modes = [
        CoderMode::BypassPermissions,
        CoderMode::Plan,
        CoderMode::Autopilot,
    ];
    let idx = match current {
        CoderMode::BypassPermissions => 0,
        CoderMode::Plan => 1,
        CoderMode::Autopilot => 2,
    };
    let next = if forward {
        (idx + 1) % modes.len()
    } else {
        (idx + modes.len() - 1) % modes.len()
    };
    modes[next]
}


fn resolve_output_style(name: &str) -> io::Result<Option<PathBuf>> {
    if name.trim().is_empty() {
        return Ok(None);
    }

    let file_name = if name.ends_with(".md") {
        name.to_string()
    } else {
        format!("{}.md", name)
    };

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".claude").join("output-styles").join(&file_name));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude").join("output-styles").join(&file_name));
    }

    for path in candidates {
        if path.is_file() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn resolve_custom_command_path(name: &str) -> io::Result<Option<PathBuf>> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let file_name = if trimmed.ends_with(".md") {
        trimmed.to_string()
    } else {
        format!("{}.md", trimmed)
    };

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".claude").join("commands").join(&file_name));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude").join("commands").join(&file_name));
    }

    for path in candidates {
        if path.is_file() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn load_custom_command(name: &str) -> io::Result<Option<String>> {
    let Some(path) = resolve_custom_command_path(name)? else {
        return Ok(None);
    };
    let content = fs::read_to_string(path)?;
    Ok(Some(content))
}

fn apply_custom_command_args(template: &str, args: &[String]) -> String {
    if args.is_empty() {
        return template.to_string();
    }
    let joined = args.join(" ");
    if template.contains("{{args}}") {
        template.replace("{{args}}", &joined)
    } else {
        format!("{}\n\n{}", template.trim_end(), joined)
    }
}

fn open_url(url: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = ProcessCommand::new("open")
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let _ = ProcessCommand::new("cmd")
            .args(["/C", "start", url])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let _ = ProcessCommand::new("xdg-open")
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    Err(io::Error::new(
        io::ErrorKind::Other,
        "open_url not supported on this platform",
    ))
}
