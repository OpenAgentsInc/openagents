use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::mcp::{McpServerConfig, McpServerStatus};
use tokio::sync::mpsc;
use wgpui::components::atoms::{AgentStatus, AgentType};
use wgpui::components::molecules::{AgentProfileCard, AgentProfileInfo, SkillCard};
use wgpui::components::organisms::{EventInspector, InspectorView};

use super::agents::AgentCatalog;
use super::hooks::HookScriptCatalog;
use super::skills::SkillCatalog;
use super::{AgentCardAction, AgentCardEvent, HookLogEntry, SkillCardAction, SkillCardEvent};
use super::{AgentEntry, HookConfig, HookScriptEntry, HookSetting, SkillEntry};
use crate::app::chat::ChatState;
use crate::app::events::QueryControl;

pub(crate) struct CatalogState {
    pub(crate) agent_entries: Vec<AgentEntry>,
    pub(crate) agent_cards: Vec<AgentProfileCard>,
    pub(crate) agent_action_tx: Option<mpsc::UnboundedSender<AgentCardEvent>>,
    pub(crate) agent_action_rx: Option<mpsc::UnboundedReceiver<AgentCardEvent>>,
    pub(crate) active_agent: Option<String>,
    pub(crate) agent_project_path: Option<PathBuf>,
    pub(crate) agent_user_path: Option<PathBuf>,
    pub(crate) agent_load_error: Option<String>,
    pub(crate) skill_entries: Vec<SkillEntry>,
    pub(crate) codex_skill_entries: Vec<SkillEntry>,
    pub(crate) skill_cards: Vec<SkillCard>,
    pub(crate) skill_action_tx: Option<mpsc::UnboundedSender<SkillCardEvent>>,
    pub(crate) skill_action_rx: Option<mpsc::UnboundedReceiver<SkillCardEvent>>,
    pub(crate) skill_update_tx: Option<mpsc::UnboundedSender<SkillUpdate>>,
    pub(crate) skill_update_rx: Option<mpsc::UnboundedReceiver<SkillUpdate>>,
    pub(crate) skill_project_path: Option<PathBuf>,
    pub(crate) skill_user_path: Option<PathBuf>,
    pub(crate) skill_load_error: Option<String>,
    pub(crate) codex_skill_error: Option<String>,
    pub(crate) hook_config: HookConfig,
    pub(crate) hook_scripts: Vec<HookScriptEntry>,
    pub(crate) hook_project_path: Option<PathBuf>,
    pub(crate) hook_user_path: Option<PathBuf>,
    pub(crate) hook_load_error: Option<String>,
    pub(crate) hook_event_log: Vec<HookLogEntry>,
    pub(crate) hook_inspector: Option<EventInspector>,
    pub(crate) hook_inspector_view: InspectorView,
    pub(crate) hook_inspector_action_tx: Option<mpsc::UnboundedSender<InspectorView>>,
    pub(crate) hook_inspector_action_rx: Option<mpsc::UnboundedReceiver<InspectorView>>,
    pub(crate) mcp_project_servers: HashMap<String, McpServerConfig>,
    pub(crate) mcp_runtime_servers: HashMap<String, McpServerConfig>,
    pub(crate) mcp_disabled_servers: HashSet<String>,
    pub(crate) mcp_status: Vec<McpServerStatus>,
    pub(crate) mcp_project_error: Option<String>,
    pub(crate) mcp_status_error: Option<String>,
    pub(crate) mcp_project_path: Option<PathBuf>,
}

pub(crate) enum SkillUpdate {
    CodexSkillsLoaded {
        entries: Vec<SkillEntry>,
        error: Option<String>,
    },
    Error(String),
}

impl CatalogState {
    pub(crate) fn new(
        agent_catalog: AgentCatalog,
        skill_catalog: SkillCatalog,
        hook_config: HookConfig,
        hook_catalog: HookScriptCatalog,
        mcp_project_servers: HashMap<String, McpServerConfig>,
        mcp_project_error: Option<String>,
        mcp_project_path: Option<PathBuf>,
    ) -> Self {
        Self {
            agent_entries: agent_catalog.entries,
            agent_cards: Vec::new(),
            agent_action_tx: None,
            agent_action_rx: None,
            active_agent: None,
            agent_project_path: agent_catalog.project_path,
            agent_user_path: agent_catalog.user_path,
            agent_load_error: agent_catalog.error,
            skill_entries: skill_catalog.entries,
            codex_skill_entries: Vec::new(),
            skill_cards: Vec::new(),
            skill_action_tx: None,
            skill_action_rx: None,
            skill_update_tx: None,
            skill_update_rx: None,
            skill_project_path: skill_catalog.project_path,
            skill_user_path: skill_catalog.user_path,
            skill_load_error: skill_catalog.error,
            codex_skill_error: None,
            hook_config,
            hook_scripts: hook_catalog.entries,
            hook_project_path: hook_catalog.project_path,
            hook_user_path: hook_catalog.user_path,
            hook_load_error: hook_catalog.error,
            hook_event_log: Vec::new(),
            hook_inspector: None,
            hook_inspector_view: InspectorView::Summary,
            hook_inspector_action_tx: None,
            hook_inspector_action_rx: None,
            mcp_project_servers,
            mcp_runtime_servers: HashMap::new(),
            mcp_disabled_servers: HashSet::new(),
            mcp_status: Vec::new(),
            mcp_project_error,
            mcp_status_error: None,
            mcp_project_path,
        }
    }

    pub(crate) fn refresh_agent_cards(&mut self, is_thinking: bool) {
        let action_tx = self.agent_action_tx.clone();
        let active_agent = self.active_agent.clone();
        self.agent_cards = self
            .agent_entries
            .iter()
            .map(|entry| {
                let status = if active_agent
                    .as_ref()
                    .map(|name| name == &entry.name)
                    .unwrap_or(false)
                {
                    if is_thinking {
                        AgentStatus::Busy
                    } else {
                        AgentStatus::Online
                    }
                } else {
                    AgentStatus::Idle
                };
                let agent_type = match entry.source {
                    super::AgentSource::Project => AgentType::Sovereign,
                    super::AgentSource::User => AgentType::Custodial,
                };
                let created_at = entry
                    .created_at
                    .map(super::super::format_relative_time)
                    .unwrap_or_else(|| "unknown".to_string());
                let info = AgentProfileInfo::new(&entry.name, &entry.name, agent_type)
                    .status(status)
                    .description(entry.definition.description.clone())
                    .capabilities(super::super::agent_capabilities(entry))
                    .created_at(created_at);
                let mut card = AgentProfileCard::new(info);
                if let Some(tx) = action_tx.clone() {
                    let tx_view = tx.clone();
                    let agent_id_view = entry.name.clone();
                    let agent_id_action = entry.name.clone();
                    card = card
                        .on_view(move |_id| {
                            let _ = tx_view.send(AgentCardEvent {
                                action: AgentCardAction::Select,
                                agent_id: agent_id_view.clone(),
                            });
                        })
                        .on_action(move |_id| {
                            let _ = tx.send(AgentCardEvent {
                                action: AgentCardAction::ToggleActive,
                                agent_id: agent_id_action.clone(),
                            });
                        });
                }
                card
            })
            .collect();
    }

    pub(crate) fn refresh_skill_cards(&mut self) {
        let action_tx = self.skill_action_tx.clone();
        self.skill_cards = self
            .skill_entries
            .iter()
            .map(|entry| {
                let mut card = SkillCard::new(entry.info.clone());
                if let Some(tx) = action_tx.clone() {
                    let view_tx = tx.clone();
                    let skill_id_view = entry.info.id.clone();
                    let skill_id_action = entry.info.id.clone();
                    card = card
                        .on_view(move |_id| {
                            let _ = view_tx.send(SkillCardEvent {
                                action: SkillCardAction::View,
                                skill_id: skill_id_view.clone(),
                            });
                        })
                        .on_install(move |_id| {
                            let _ = tx.send(SkillCardEvent {
                                action: SkillCardAction::Install,
                                skill_id: skill_id_action.clone(),
                            });
                        });
                }
                card
            })
            .collect();
    }

    #[allow(dead_code)]
    pub(crate) fn merged_mcp_servers(&self) -> HashMap<String, McpServerConfig> {
        let mut servers = self.mcp_project_servers.clone();
        for (name, config) in &self.mcp_runtime_servers {
            servers.insert(name.clone(), config.clone());
        }
        for name in &self.mcp_disabled_servers {
            servers.remove(name);
        }
        servers
    }

    pub(crate) fn mcp_entries(&self) -> Vec<super::McpServerEntry> {
        let mut entries = Vec::new();
        let mut status_map = HashMap::new();
        for status in &self.mcp_status {
            status_map.insert(status.name.clone(), status.status.clone());
        }

        for (name, config) in &self.mcp_project_servers {
            entries.push(super::McpServerEntry {
                name: name.clone(),
                source: Some(super::McpServerSource::Project),
                config: Some(config.clone()),
                status: status_map.remove(name),
                disabled: self.mcp_disabled_servers.contains(name),
            });
        }

        for (name, config) in &self.mcp_runtime_servers {
            entries.push(super::McpServerEntry {
                name: name.clone(),
                source: Some(super::McpServerSource::Runtime),
                config: Some(config.clone()),
                status: status_map.remove(name),
                disabled: self.mcp_disabled_servers.contains(name),
            });
        }

        for (name, status) in status_map {
            entries.push(super::McpServerEntry {
                name,
                source: None,
                config: None,
                status: Some(status),
                disabled: false,
            });
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    pub(crate) fn add_runtime_mcp_server(&mut self, name: String, config: McpServerConfig) {
        self.mcp_runtime_servers.insert(name.clone(), config);
        self.mcp_disabled_servers.remove(&name);
    }

    pub(crate) fn remove_mcp_server(&mut self, name: &str) {
        self.mcp_runtime_servers.remove(name);
        self.mcp_disabled_servers.insert(name.to_string());
    }

    pub(crate) fn update_mcp_status(
        &mut self,
        servers: Vec<McpServerStatus>,
        error: Option<String>,
    ) {
        self.mcp_status = servers;
        self.mcp_status_error = error;
    }

    #[allow(dead_code)]
    pub(crate) fn mcp_status_summary(&self) -> Option<String> {
        let total = self.merged_mcp_servers().len();
        if total == 0 {
            return None;
        }
        if self.mcp_status_error.is_some() {
            return Some("mcp error".to_string());
        }
        if self.mcp_status.is_empty() {
            return Some(format!("mcp {}", total));
        }
        let connected = self
            .mcp_status
            .iter()
            .filter(|status| status.status.eq_ignore_ascii_case("connected"))
            .count();
        Some(format!("mcp {}/{}", connected, total))
    }

    pub(crate) fn reload_hooks(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = super::load_hook_scripts(&cwd);
        self.hook_scripts = catalog.entries;
        self.hook_project_path = catalog.project_path;
        self.hook_user_path = catalog.user_path;
        self.hook_load_error = catalog.error;
    }

    pub(crate) fn toggle_hook_setting(&mut self, setting: HookSetting) {
        match setting {
            HookSetting::ToolBlocker => {
                self.hook_config.tool_blocker = !self.hook_config.tool_blocker;
            }
            HookSetting::ToolLogger => {
                self.hook_config.tool_logger = !self.hook_config.tool_logger;
            }
            HookSetting::OutputTruncator => {
                self.hook_config.output_truncator = !self.hook_config.output_truncator;
            }
            HookSetting::ContextInjection => {
                self.hook_config.context_injection = !self.hook_config.context_injection;
            }
            HookSetting::TodoEnforcer => {
                self.hook_config.todo_enforcer = !self.hook_config.todo_enforcer;
            }
        }
        super::save_hook_config(&self.hook_config);
    }

    pub(crate) fn reload_agents(&mut self, chat: &mut ChatState) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = super::load_agent_entries(&cwd);
        self.agent_entries = catalog.entries;
        self.agent_project_path = catalog.project_path;
        self.agent_user_path = catalog.user_path;
        self.agent_load_error = catalog.error;
        if let Some(active) = self.active_agent.clone() {
            if !self.agent_entries.iter().any(|entry| entry.name == active) {
                self.active_agent = None;
                chat.push_system_message(format!("Active agent {} no longer available.", active));
            }
        }
        self.refresh_agent_cards(chat.is_thinking);
    }

    pub(crate) fn reload_skills(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = super::load_skill_entries(&cwd);
        self.skill_entries = self.merge_skill_entries(catalog.entries);
        self.skill_project_path = catalog.project_path;
        self.skill_user_path = catalog.user_path;
        self.skill_load_error = catalog.error;
        self.refresh_skill_cards();
    }

    pub(crate) fn update_codex_skills(&mut self, entries: Vec<SkillEntry>, error: Option<String>) {
        self.codex_skill_entries = entries;
        self.codex_skill_error = error;
        let local_entries: Vec<SkillEntry> = self
            .skill_entries
            .iter()
            .filter(|entry| entry.source != super::SkillSource::Codex)
            .cloned()
            .collect();
        self.skill_entries = self.merge_skill_entries(local_entries);
        self.refresh_skill_cards();
    }

    fn merge_skill_entries(&self, mut local_entries: Vec<SkillEntry>) -> Vec<SkillEntry> {
        local_entries.extend(self.codex_skill_entries.iter().cloned());
        local_entries.sort_by(|a, b| a.info.name.cmp(&b.info.name));
        local_entries
    }

    pub(crate) fn reload_mcp_project_servers(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let (servers, error) = super::load_mcp_project_servers(&cwd);
        self.mcp_project_servers = servers;
        self.mcp_project_error = error;
        self.mcp_project_path = Some(crate::app::config::mcp_project_file(&cwd));
    }

    pub(crate) fn request_mcp_status(&self, chat: &mut ChatState) {
        if let Some(tx) = &chat.query_control_tx {
            let _ = tx.send(QueryControl::FetchMcpStatus);
        } else {
            chat.push_system_message("No active session for MCP status.".to_string());
        }
    }
}
