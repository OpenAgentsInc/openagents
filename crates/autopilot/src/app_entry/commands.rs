use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

use rfd::FileDialog;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamedKey};

use crate::app::AppState;
use crate::app::agents::AgentKind;
use crate::app::catalog::{
    McpServerEntry, expand_env_vars_in_value, parse_mcp_server_config, save_hook_config,
};
use crate::app::codex_app_server as app_server;
use crate::app::codex_runtime::{CodexRuntime, CodexRuntimeConfig};
use crate::app::config::{
    AgentKindConfig, ModelMode, ModelPickerEntry, SettingsItem, SettingsTab,
    app_server_model_entries,
};
use crate::app::events::{
    CoderMode, CommandAction, ModalState, ResponseEvent, convert_key_for_binding, convert_modifiers,
};
use crate::app::permissions::{coder_mode_default_allow, coder_mode_label, parse_coder_mode};
use crate::app::ui::ThemeSetting;
use crate::app::{
    HookModalView, HookSetting, ModelOption, SettingsInputMode, SettingsSnapshot, settings_rows,
};
use crate::commands::Command;
use crate::keybindings::{Keybinding, default_keybindings};

use super::settings::{
    apply_codex_oss_env, clamp_font_size, rate_limits_from_snapshot, save_keybindings,
};

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
        Command::Backend => {
            // Toggle between backends (Codex only for now).
            tracing::info!("Backend switched to: Codex");
            state.agent_selection.agent = AgentKindConfig::Codex;
            CommandAction::None
        }
        Command::BackendSet(name) => {
            let lower = name.to_lowercase();
            match lower.as_str() {
                "codex" | "openai" => {
                    state.agent_selection.agent = AgentKindConfig::Codex;
                    tracing::info!("Backend set to: Codex");
                }
                _ => {
                    state.push_system_message(format!(
                        "Unknown backend: {}. Available: codex",
                        name
                    ));
                }
            }
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
        Command::WorkspaceList => {
            let summary = state.workspaces.list_workspace_summary();
            state.push_system_message(summary);
            CommandAction::None
        }
        Command::WorkspaceAdd => {
            match FileDialog::new().pick_folder() {
                Some(path) => {
                    state.workspaces.runtime.add_workspace(path, None);
                    state.push_system_message("Adding workspace...".to_string());
                }
                None => {
                    state.push_system_message("Workspace selection canceled.".to_string());
                }
            }
            CommandAction::None
        }
        Command::WorkspaceConnect(hint) => {
            if let Some(workspace_id) = state.workspaces.connect_by_hint(&hint) {
                state.workspaces.runtime.connect_workspace(workspace_id);
                state.push_system_message("Connecting workspace...".to_string());
            } else {
                state.push_system_message(format!("Workspace not found for: {}.", hint.trim()));
            }
            CommandAction::None
        }
        Command::WorkspaceRefresh => {
            state.workspaces.runtime.reload();
            for workspace in &state.workspaces.workspaces {
                if workspace.connected {
                    state.workspaces.runtime.list_threads(workspace.id.clone());
                }
            }
            state.push_system_message("Refreshing workspaces...".to_string());
            CommandAction::None
        }
        Command::Review(review) => {
            let invalid = match &review.target {
                crate::commands::ReviewTarget::BaseBranch { branch } => branch.trim().is_empty(),
                crate::commands::ReviewTarget::Commit { sha, .. } => sha.trim().is_empty(),
                crate::commands::ReviewTarget::Custom { instructions } => {
                    instructions.trim().is_empty()
                }
                crate::commands::ReviewTarget::UncommittedChanges => false,
            };
            if invalid {
                state.push_system_message(
                    "Review target is missing required arguments.".to_string(),
                );
                CommandAction::None
            } else if state.chat.is_thinking {
                state.push_system_message(
                    "Cannot start review during an active request.".to_string(),
                );
                CommandAction::None
            } else {
                CommandAction::StartReview(review)
            }
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
                Ok(None) => {
                    state.push_system_message(format!("Output style not found: {}.", trimmed))
                }
                Err(err) => {
                    state.push_system_message(format!("Failed to load output style: {}.", err))
                }
            }
            CommandAction::None
        }
        Command::AccountStatus => {
            if let Some(tx) = start_app_server_task(state, "Account status") {
                tokio::spawn(async move {
                    let Some((client, _channels)) = init_app_server_client(&tx).await else {
                        return;
                    };
                    match client
                        .account_read(app_server::GetAccountParams {
                            refresh_token: false,
                        })
                        .await
                    {
                        Ok(response) => {
                            let message = format_account_status(&response);
                            let _ = tx.send(ResponseEvent::SystemMessage(message));
                        }
                        Err(err) => {
                            let _ = tx.send(ResponseEvent::Error(format!(
                                "Failed to read account status: {}",
                                err
                            )));
                            let _ = client.shutdown().await;
                            return;
                        }
                    }
                    let _ = client.shutdown().await;
                    let _ = tx.send(ResponseEvent::Complete { metadata: None });
                });
            }
            CommandAction::None
        }
        Command::AccountLoginApiKey(api_key) => {
            let trimmed = api_key.trim();
            if trimmed.is_empty() {
                state.push_system_message("API key is required.".to_string());
                return CommandAction::None;
            }
            if let Some(tx) = start_app_server_task(state, "Account login") {
                let api_key = trimmed.to_string();
                tokio::spawn(async move {
                    let Some((client, _channels)) = init_app_server_client(&tx).await else {
                        return;
                    };
                    let response = client
                        .account_login_start(app_server::LoginAccountParams::ApiKey { api_key })
                        .await;
                    match response {
                        Ok(_) => {
                            let _ =
                                tx.send(ResponseEvent::SystemMessage("API key saved.".to_string()));
                        }
                        Err(err) => {
                            let _ = tx.send(ResponseEvent::Error(format!(
                                "Failed to login with API key: {}",
                                err
                            )));
                            let _ = client.shutdown().await;
                            return;
                        }
                    }
                    let _ = client.shutdown().await;
                    let _ = tx.send(ResponseEvent::Complete { metadata: None });
                });
            }
            CommandAction::None
        }
        Command::AccountLoginChatgpt => {
            if let Some(tx) = start_app_server_task(state, "Account login") {
                tokio::spawn(async move {
                    let Some((client, mut channels)) = init_app_server_client(&tx).await else {
                        return;
                    };
                    let response = client
                        .account_login_start(app_server::LoginAccountParams::Chatgpt)
                        .await;
                    let (login_id, auth_url) = match response {
                        Ok(app_server::LoginAccountResponse::Chatgpt { login_id, auth_url }) => {
                            (login_id, auth_url)
                        }
                        Ok(_) => {
                            let _ = tx.send(ResponseEvent::Error(
                                "Unexpected login response.".to_string(),
                            ));
                            let _ = client.shutdown().await;
                            return;
                        }
                        Err(err) => {
                            let _ = tx.send(ResponseEvent::Error(format!(
                                "Failed to start ChatGPT login: {}",
                                err
                            )));
                            let _ = client.shutdown().await;
                            return;
                        }
                    };
                    let _ = tx.send(ResponseEvent::SystemMessage(format!(
                        "ChatGPT login started (id {}).",
                        login_id
                    )));
                    if let Err(err) = open_url(&auth_url) {
                        let _ = tx.send(ResponseEvent::SystemMessage(format!(
                            "Open this URL to login: {} (error: {}).",
                            auth_url, err
                        )));
                    } else {
                        let _ = tx.send(ResponseEvent::SystemMessage(format!(
                            "Opened login URL: {}",
                            auth_url
                        )));
                    }

                    let timeout = time::sleep(Duration::from_secs(300));
                    tokio::pin!(timeout);
                    loop {
                        tokio::select! {
                            _ = &mut timeout => {
                                let _ = tx.send(ResponseEvent::SystemMessage(
                                    "Login still pending. Check /account status or cancel.".to_string(),
                                ));
                                break;
                            }
                            Some(notification) = channels.notifications.recv() => {
                                match notification.method.as_str() {
                                    "account/login/completed" => {
                                        if let Some(params) = notification.params {
                                            if let Ok(event) = serde_json::from_value::<app_server::AccountLoginCompletedNotification>(params) {
                                                if event.login_id.as_deref() == Some(login_id.as_str()) || event.login_id.is_none() {
                                                    if event.success {
                                                        let _ = tx.send(ResponseEvent::SystemMessage(
                                                            "Account login completed.".to_string(),
                                                        ));
                                                    } else {
                                                        let error = event.error.unwrap_or_else(|| "unknown error".to_string());
                                                        let _ = tx.send(ResponseEvent::SystemMessage(format!(
                                                            "Account login failed: {}",
                                                            error
                                                        )));
                                                    }
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    "account/updated" => {
                                        if let Some(params) = notification.params {
                                            if let Ok(event) = serde_json::from_value::<app_server::AccountUpdatedNotification>(params) {
                                                let message = match event.auth_mode {
                                                    Some(app_server::AuthMode::ApiKey) => {
                                                        "Account updated: API key auth.".to_string()
                                                    }
                                                    Some(app_server::AuthMode::Chatgpt) => {
                                                        "Account updated: ChatGPT auth.".to_string()
                                                    }
                                                    None => "Account signed out.".to_string(),
                                                };
                                                let _ = tx.send(ResponseEvent::SystemMessage(message));
                                            }
                                        }
                                    }
                                    "account/rateLimits/updated" => {
                                        if let Some(params) = notification.params {
                                            if let Ok(event) = serde_json::from_value::<app_server::AccountRateLimitsUpdatedNotification>(params) {
                                                let limits = rate_limits_from_snapshot(event.rate_limits);
                                                let _ = tx.send(ResponseEvent::RateLimitsUpdated { limits });
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            else => break,
                        }
                    }

                    let _ = client.shutdown().await;
                    let _ = tx.send(ResponseEvent::Complete { metadata: None });
                });
            }
            CommandAction::None
        }
        Command::AccountLoginCancel(login_id) => {
            let trimmed = login_id.trim();
            if trimmed.is_empty() {
                state.push_system_message("Login id is required.".to_string());
                return CommandAction::None;
            }
            if let Some(tx) = start_app_server_task(state, "Account login cancel") {
                let login_id = trimmed.to_string();
                tokio::spawn(async move {
                    let Some((client, _channels)) = init_app_server_client(&tx).await else {
                        return;
                    };
                    match client
                        .account_login_cancel(app_server::CancelLoginAccountParams { login_id })
                        .await
                    {
                        Ok(response) => {
                            let _ = tx.send(ResponseEvent::SystemMessage(format!(
                                "Login canceled: {}",
                                response.status
                            )));
                        }
                        Err(err) => {
                            let _ = tx.send(ResponseEvent::Error(format!(
                                "Failed to cancel login: {}",
                                err
                            )));
                            let _ = client.shutdown().await;
                            return;
                        }
                    }
                    let _ = client.shutdown().await;
                    let _ = tx.send(ResponseEvent::Complete { metadata: None });
                });
            }
            CommandAction::None
        }
        Command::AccountLogout => {
            if let Some(tx) = start_app_server_task(state, "Account logout") {
                tokio::spawn(async move {
                    let Some((client, _channels)) = init_app_server_client(&tx).await else {
                        return;
                    };
                    match client.account_logout().await {
                        Ok(_) => {
                            let _ = tx.send(ResponseEvent::SystemMessage(
                                "Account logged out.".to_string(),
                            ));
                        }
                        Err(err) => {
                            let _ =
                                tx.send(ResponseEvent::Error(format!("Failed to logout: {}", err)));
                            let _ = client.shutdown().await;
                            return;
                        }
                    }
                    let _ = client.shutdown().await;
                    let _ = tx.send(ResponseEvent::Complete { metadata: None });
                });
            }
            CommandAction::None
        }
        Command::AccountRateLimits => {
            if let Some(tx) = start_app_server_task(state, "Rate limits") {
                tokio::spawn(async move {
                    let Some((client, _channels)) = init_app_server_client(&tx).await else {
                        return;
                    };
                    match client.account_rate_limits_read().await {
                        Ok(response) => {
                            let limits = rate_limits_from_snapshot(response.rate_limits);
                            let _ = tx.send(ResponseEvent::RateLimitsUpdated { limits });
                            let _ = tx.send(ResponseEvent::SystemMessage(
                                "Rate limits refreshed.".to_string(),
                            ));
                        }
                        Err(err) => {
                            let _ = tx.send(ResponseEvent::Error(format!(
                                "Failed to read rate limits: {}",
                                err
                            )));
                            let _ = client.shutdown().await;
                            return;
                        }
                    }
                    let _ = client.shutdown().await;
                    let _ = tx.send(ResponseEvent::Complete { metadata: None });
                });
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
        Command::McpLogin(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                state.push_system_message("MCP login requires a server name.".to_string());
                return CommandAction::None;
            }
            if let Some(tx) = start_app_server_task(state, "MCP login") {
                let name = trimmed.to_string();
                tokio::spawn(async move {
                    let Some((client, mut channels)) = init_app_server_client(&tx).await else {
                        return;
                    };
                    let response = client
                        .mcp_server_oauth_login(app_server::McpServerOauthLoginParams {
                            name: name.clone(),
                            scopes: None,
                            timeout_secs: None,
                        })
                        .await;
                    let auth_url = match response {
                        Ok(response) => response.authorization_url,
                        Err(err) => {
                            let _ = tx.send(ResponseEvent::Error(format!(
                                "Failed to start MCP login: {}",
                                err
                            )));
                            let _ = client.shutdown().await;
                            return;
                        }
                    };
                    let _ = tx.send(ResponseEvent::SystemMessage(format!(
                        "MCP login started for {}.",
                        name
                    )));
                    if let Err(err) = open_url(&auth_url) {
                        let _ = tx.send(ResponseEvent::SystemMessage(format!(
                            "Open this URL to login: {} (error: {}).",
                            auth_url, err
                        )));
                    } else {
                        let _ = tx.send(ResponseEvent::SystemMessage(format!(
                            "Opened login URL: {}",
                            auth_url
                        )));
                    }

                    let timeout = time::sleep(Duration::from_secs(300));
                    tokio::pin!(timeout);
                    loop {
                        tokio::select! {
                            _ = &mut timeout => {
                                let _ = tx.send(ResponseEvent::SystemMessage(
                                    "MCP login still pending. Run /mcp status to refresh.".to_string(),
                                ));
                                break;
                            }
                            Some(notification) = channels.notifications.recv() => {
                                if notification.method.as_str() == "mcpServer/oauthLogin/completed" {
                                    if let Some(params) = notification.params {
                                        if let Ok(event) = serde_json::from_value::<app_server::McpServerOauthLoginCompletedNotification>(params) {
                                            if event.name == name {
                                                if event.success {
                                                    let _ = tx.send(ResponseEvent::SystemMessage(format!(
                                                        "MCP login completed for {}.",
                                                        name
                                                    )));
                                                } else {
                                                    let error = event.error.unwrap_or_else(|| "unknown error".to_string());
                                                    let _ = tx.send(ResponseEvent::SystemMessage(format!(
                                                        "MCP login failed for {}: {}",
                                                        name, error
                                                    )));
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            else => break,
                        }
                    }

                    let _ = client.shutdown().await;
                    let _ = tx.send(ResponseEvent::Complete { metadata: None });
                });
            }
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
                            state
                                .catalogs
                                .add_runtime_mcp_server(trimmed_name.to_string(), server);
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
                Err(err) => {
                    state.push_system_message(format!("Failed to parse MCP server JSON: {}", err))
                }
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
        Command::AgentBackends => {
            state.open_agent_backends();
            CommandAction::None
        }
        Command::AgentBackendsRefresh => {
            state.refresh_agent_backends();
            state.open_agent_backends();
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
        Command::PylonEarnings => {
            state.open_pylon_earnings();
            CommandAction::None
        }
        Command::PylonEarningsRefresh => {
            state.refresh_pylon_earnings();
            state.open_pylon_earnings();
            CommandAction::None
        }
        Command::PylonJobs => {
            state.open_pylon_jobs();
            CommandAction::None
        }
        Command::PylonJobsRefresh => {
            state.refresh_pylon_jobs();
            state.open_pylon_jobs();
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
        Command::ChainViz(prompt) => {
            if prompt.trim().is_empty() {
                state.push_system_message(
                    "Chain visualizer prompt is required. Usage: /chainviz <prompt>".to_string(),
                );
                CommandAction::None
            } else if state.chat.is_thinking {
                state.push_system_message(
                    "Cannot start chain visualizer while a request is active.".to_string(),
                );
                CommandAction::None
            } else {
                CommandAction::StartChainViz(prompt)
            }
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
            let models = model_picker_entries(state);
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    if let Some(entry) = models.get(selected) {
                        state.update_selected_model_id(entry.id.clone());
                    }
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if selected > 0 {
                        state.modal_state = ModalState::ModelPicker {
                            selected: selected - 1,
                        };
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if selected + 1 < models.len() {
                        state.modal_state = ModalState::ModelPicker {
                            selected: selected + 1,
                        };
                    }
                }
                WinitKey::Character(c) => {
                    if let Ok(index) = c.parse::<usize>() {
                        if index > 0 && index <= models.len() {
                            if let Some(entry) = models.get(index - 1) {
                                state.update_selected_model_id(entry.id.clone());
                                state.modal_state = ModalState::None;
                            }
                        }
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
                    let selected_name = state
                        .catalogs
                        .agent_entries
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
        ModalState::AgentBackends {
            selected,
            model_selected,
        } => {
            let kinds = state.agent_backends.kinds();
            if kinds.is_empty() {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.refresh_agent_backends();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= kinds.len() {
                *selected = kinds.len().saturating_sub(1);
            }
            let selected_kind = kinds.get(*selected).copied().unwrap_or(AgentKind::Codex);
            let models = state.agent_backends.models_for_kind(selected_kind);
            let max_model_index = models.len();
            if *model_selected > max_model_index {
                *model_selected = max_model_index;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    let model_id = if *model_selected == 0 {
                        None
                    } else {
                        models
                            .get(model_selected.saturating_sub(1))
                            .map(|model| model.id.clone())
                    };
                    state
                        .agent_backends
                        .set_selection(selected_kind, model_id.clone());
                    state.agent_selection = state.agent_backends.settings.selected.clone();
                    if let Some(model_id) = model_id {
                        state.update_selected_model_id(model_id);
                    } else {
                        state.update_selected_model(ModelOption::Default);
                    }
                    state.push_system_message(format!(
                        "Agent backend set to {}.",
                        state.agent_backends.settings.selected.display_name()
                    ));
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                        let next_kind = kinds.get(*selected).copied().unwrap_or(AgentKind::Codex);
                        *model_selected = state.agent_backends.model_index_for_kind(next_kind);
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < kinds.len() {
                        *selected += 1;
                        let next_kind = kinds.get(*selected).copied().unwrap_or(AgentKind::Codex);
                        *model_selected = state.agent_backends.model_index_for_kind(next_kind);
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowLeft) => {
                    if *model_selected > 0 {
                        *model_selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowRight) => {
                    if *model_selected < max_model_index {
                        *model_selected += 1;
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_agent_backends();
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("d") => {
                    *model_selected = 0;
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
        ModalState::ChainViz => {
            if matches!(key, WinitKey::Named(WinitNamedKey::Escape)) {
                state.modal_state = ModalState::None;
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
        ModalState::PylonEarnings => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_pylon_earnings();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::PylonJobs => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.refresh_pylon_jobs();
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
                    let enabled = !state
                        .dspy
                        .snapshot
                        .auto_optimizer
                        .config
                        .background_optimization;
                    state.set_dspy_background_optimization(enabled);
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Help => {
            match key {
                WinitKey::Named(
                    WinitNamedKey::Escape | WinitNamedKey::Enter | WinitNamedKey::F1,
                ) => {
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
                        state
                            .settings
                            .keybindings
                            .retain(|binding| binding.action != *action);
                        save_keybindings(&state.settings.keybindings);
                        *input_mode = SettingsInputMode::Normal;
                    }
                    _ => {
                        if let Some(binding_key) = convert_key_for_binding(key) {
                            let modifiers = convert_modifiers(&state.modifiers);
                            state.settings.keybindings.retain(|binding| {
                                binding.action != *action
                                    && !(binding.key == binding_key
                                        && binding.modifiers == modifiers)
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
                                    state.settings.coder_settings.theme =
                                        match state.settings.coder_settings.theme {
                                            ThemeSetting::System => ThemeSetting::Dark,
                                            ThemeSetting::Dark => ThemeSetting::Light,
                                            ThemeSetting::Light => ThemeSetting::System,
                                        };
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::FontSize => {
                                    let delta = if forward { 1.0 } else { -1.0 };
                                    state.settings.coder_settings.font_size = clamp_font_size(
                                        state.settings.coder_settings.font_size + delta,
                                    );
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::AutoScroll => {
                                    state.settings.coder_settings.auto_scroll =
                                        !state.settings.coder_settings.auto_scroll;
                                    state.persist_settings();
                                }
                                SettingsItem::ModelMode => {
                                    let next =
                                        match (state.settings.coder_settings.model_mode, forward) {
                                            (ModelMode::Pro, true) => ModelMode::Local,
                                            (ModelMode::Local, true) => ModelMode::Pro,
                                            (ModelMode::Pro, false) => ModelMode::Local,
                                            (ModelMode::Local, false) => ModelMode::Pro,
                                        };
                                    if state.settings.coder_settings.model_mode != next {
                                        state.settings.coder_settings.model_mode = next;
                                        state.persist_settings();
                                        apply_codex_oss_env(&state.settings.coder_settings);
                                        let message = match next {
                                            ModelMode::Local => {
                                                "Local GPT-OSS mode enabled. Reconnect workspaces to apply.".to_string()
                                            }
                                            ModelMode::Pro => {
                                                "Pro mode enabled. Reconnect workspaces to apply.".to_string()
                                            }
                                        };
                                        state.push_system_message(message);
                                    }
                                }
                                SettingsItem::DefaultModel => {
                                    if state.settings.coder_settings.is_local_mode() {
                                        state.push_system_message(
                                            "Local mode forces gpt-oss:20b. Switch to Pro to change models.".to_string(),
                                        );
                                    } else {
                                        let models = model_picker_entries(state);
                                        let ids: Vec<String> =
                                            models.iter().map(|model| model.id.clone()).collect();
                                        let current = state
                                            .settings
                                            .coder_settings
                                            .model
                                            .clone()
                                            .unwrap_or_else(|| {
                                                state.settings.selected_model.model_id().to_string()
                                            });
                                        let next_id = cycle_string_option(&ids, &current, forward);
                                        state.update_selected_model_id(next_id);
                                    }
                                }
                                SettingsItem::ReasoningEffort => {
                                    if state.settings.coder_settings.is_local_mode() {
                                        state.push_system_message(
                                            "Local mode uses chat completions. Reasoning effort is unavailable.".to_string(),
                                        );
                                    } else {
                                        let options = available_reasoning_efforts(state);
                                        let current = state
                                            .settings
                                            .coder_settings
                                            .reasoning_effort
                                            .clone()
                                            .unwrap_or_else(|| "auto".to_string());
                                        let next = cycle_string_option(&options, &current, forward);
                                        state.settings.coder_settings.reasoning_effort =
                                            if next == "auto" { None } else { Some(next) };
                                        state.persist_settings();
                                        let value = state
                                            .settings
                                            .coder_settings
                                            .reasoning_effort
                                            .clone()
                                            .map(Value::String)
                                            .unwrap_or(Value::Null);
                                        state.persist_codex_config_value(
                                            "model_reasoning_effort".to_string(),
                                            value,
                                        );
                                    }
                                }
                                SettingsItem::PermissionMode => {
                                    let next = cycle_coder_mode_standalone(
                                        state.permissions.coder_mode,
                                        forward,
                                    );
                                    state.permissions.coder_mode = next;
                                    state.permissions.permission_default_allow =
                                        coder_mode_default_allow(
                                            next,
                                            state.permissions.permission_default_allow,
                                        );
                                    state.session.session_info.permission_mode =
                                        coder_mode_label(next).to_string();
                                    state.permissions.persist_permission_config();
                                }
                                SettingsItem::PermissionDefaultAllow => {
                                    state.permissions.permission_default_allow =
                                        !state.permissions.permission_default_allow;
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
                                    state.settings.coder_settings.session_auto_save =
                                        !state.settings.coder_settings.session_auto_save;
                                    state.persist_settings();
                                    if state.settings.coder_settings.session_auto_save {
                                        state.apply_session_history_limit();
                                    }
                                }
                                SettingsItem::SessionHistoryLimit => {
                                    const HISTORY_STEP: usize = 10;
                                    const HISTORY_MAX: usize = 500;
                                    let current =
                                        state.settings.coder_settings.session_history_limit;
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

fn model_picker_entries(state: &AppState) -> Vec<ModelPickerEntry> {
    app_server_model_entries(&state.settings.app_server_models)
}

fn cycle_string_option(options: &[String], current: &str, forward: bool) -> String {
    if options.is_empty() {
        return current.to_string();
    }
    let idx = options
        .iter()
        .position(|value| value == current)
        .unwrap_or(0);
    let next = if forward {
        (idx + 1) % options.len()
    } else {
        (idx + options.len() - 1) % options.len()
    };
    options[next].clone()
}

fn available_reasoning_efforts(state: &AppState) -> Vec<String> {
    let mut options = Vec::new();
    options.push("auto".to_string());

    let selected_model = state
        .settings
        .coder_settings
        .model
        .as_deref()
        .unwrap_or_else(|| state.settings.selected_model.model_id());

    if !state.settings.app_server_models.is_empty() {
        if let Some(model) = state
            .settings
            .app_server_models
            .iter()
            .find(|model| model.id == selected_model || model.model == selected_model)
        {
            for effort in &model.supported_reasoning_efforts {
                options.push(reasoning_effort_id(effort.reasoning_effort));
            }
        }
    }

    if options.len() == 1 {
        options.extend(
            ["none", "minimal", "low", "medium", "high", "xhigh"]
                .iter()
                .map(|value| value.to_string()),
        );
    }

    options
}

fn reasoning_effort_id(effort: crate::app::codex_app_server::ReasoningEffort) -> String {
    match effort {
        crate::app::codex_app_server::ReasoningEffort::None => "none",
        crate::app::codex_app_server::ReasoningEffort::Minimal => "minimal",
        crate::app::codex_app_server::ReasoningEffort::Low => "low",
        crate::app::codex_app_server::ReasoningEffort::Medium => "medium",
        crate::app::codex_app_server::ReasoningEffort::High => "high",
        crate::app::codex_app_server::ReasoningEffort::XHigh => "xhigh",
    }
    .to_string()
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
        candidates.push(
            cwd.join(".openagents")
                .join("output-styles")
                .join(&file_name),
        );
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join(".openagents")
                .join("output-styles")
                .join(&file_name),
        );
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
        candidates.push(cwd.join(".openagents").join("commands").join(&file_name));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".openagents").join("commands").join(&file_name));
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

fn start_app_server_task(
    state: &mut AppState,
    label: &str,
) -> Option<mpsc::UnboundedSender<ResponseEvent>> {
    if state.chat.is_thinking || state.chat.response_rx.is_some() {
        state.push_system_message(format!(
            "Cannot start {} while another request is active.",
            label
        ));
        return None;
    }
    let (tx, rx) = mpsc::unbounded_channel();
    state.chat.response_rx = Some(rx);
    state.chat.query_control_tx = None;
    state.chat.is_thinking = true;
    state.chat.streaming_markdown.reset();
    state.catalogs.refresh_agent_cards(state.chat.is_thinking);
    state.window.request_redraw();
    Some(tx)
}

async fn init_app_server_client(
    tx: &mpsc::UnboundedSender<ResponseEvent>,
) -> Option<(app_server::AppServerClient, app_server::AppServerChannels)> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let runtime = match CodexRuntime::spawn(CodexRuntimeConfig {
        cwd: Some(cwd),
        wire_log: None,
    })
    .await
    {
        Ok(runtime) => runtime,
        Err(err) => {
            let _ = tx.send(ResponseEvent::Error(format!(
                "Failed to start codex app-server: {}",
                err
            )));
            return None;
        }
    };
    let CodexRuntime {
        client, channels, ..
    } = runtime;
    Some((client, channels))
}

fn format_account_status(response: &app_server::GetAccountResponse) -> String {
    let auth_requirement = if response.requires_openai_auth {
        "OpenAI auth required."
    } else {
        "OpenAI auth not required."
    };
    match &response.account {
        None => format!("No account configured. {}", auth_requirement),
        Some(app_server::AccountInfo::ApiKey) => {
            format!("Signed in with API key. {}", auth_requirement)
        }
        Some(app_server::AccountInfo::Chatgpt { email, plan_type }) => {
            let plan = format_plan_type(*plan_type);
            format!(
                "Signed in as {} (plan {}). {}",
                email, plan, auth_requirement
            )
        }
    }
}

fn format_plan_type(plan: app_server::PlanType) -> &'static str {
    match plan {
        app_server::PlanType::Free => "free",
        app_server::PlanType::Plus => "plus",
        app_server::PlanType::Pro => "pro",
        app_server::PlanType::Team => "team",
        app_server::PlanType::Business => "business",
        app_server::PlanType::Enterprise => "enterprise",
        app_server::PlanType::Edu => "edu",
        app_server::PlanType::Unknown => "unknown",
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
