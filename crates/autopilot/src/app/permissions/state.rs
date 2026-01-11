use std::collections::VecDeque;

use tokio::sync::mpsc;
use wgpui::components::organisms::PermissionDialog;
use super::request::{PermissionRequest, PermissionResult};

use super::{PermissionHistoryEntry, PermissionPending};
use super::rules::{
    add_unique, coder_mode_default_allow, extract_bash_command, permission_detail_for_request,
    pattern_matches, permission_type_for_request, remove_items, sanitize_tokens, save_permission_config,
    split_permission_tokens, PermissionConfig,
};
use crate::app::events::CoderMode;
use crate::app::session::SessionInfo;
use wgpui::components::atoms::PermissionAction;

struct AutoDecision {
    result: PermissionResult,
    label: &'static str,
}

pub(crate) struct PermissionState {
    pub(crate) coder_mode: CoderMode,
    pub(crate) permission_default_allow: bool,
    pub(crate) permission_allow_tools: Vec<String>,
    pub(crate) permission_deny_tools: Vec<String>,
    pub(crate) permission_allow_bash_patterns: Vec<String>,
    pub(crate) permission_deny_bash_patterns: Vec<String>,
    pub(crate) permission_requests_rx: Option<mpsc::UnboundedReceiver<PermissionPending>>,
    pub(crate) permission_action_tx: Option<mpsc::UnboundedSender<PermissionAction>>,
    pub(crate) permission_action_rx: Option<mpsc::UnboundedReceiver<PermissionAction>>,
    pub(crate) permission_dialog: Option<PermissionDialog>,
    pub(crate) permission_queue: VecDeque<PermissionPending>,
    pub(crate) permission_pending: Option<PermissionPending>,
    pub(crate) permission_history: Vec<PermissionHistoryEntry>,
    pub(crate) tools_allowed: Vec<String>,
    pub(crate) tools_disallowed: Vec<String>,
    pub(crate) output_style: Option<String>,
}

impl PermissionState {
    pub(crate) fn new(
        coder_mode: CoderMode,
        permission_default_allow: bool,
        allow_tools: Vec<String>,
        deny_tools: Vec<String>,
        allow_bash_patterns: Vec<String>,
        deny_bash_patterns: Vec<String>,
    ) -> Self {
        Self {
            coder_mode,
            permission_default_allow,
            permission_allow_tools: allow_tools,
            permission_deny_tools: deny_tools,
            permission_allow_bash_patterns: allow_bash_patterns,
            permission_deny_bash_patterns: deny_bash_patterns,
            permission_requests_rx: None,
            permission_action_tx: None,
            permission_action_rx: None,
            permission_dialog: None,
            permission_queue: VecDeque::new(),
            permission_pending: None,
            permission_history: Vec::new(),
            tools_allowed: Vec::new(),
            tools_disallowed: Vec::new(),
            output_style: None,
        }
    }

    pub(crate) fn enqueue_permission_prompt(&mut self, pending: PermissionPending) {
        if let Some(decision) = self.auto_decision_for_request(&pending.request) {
            let detail = permission_detail_for_request(&pending.request);
            self.record_permission_history(&pending.request, decision.label, detail);
            let _ = pending.respond_to.send(decision.result);
            return;
        }
        if self.permission_pending.is_some() || self.permission_dialog.is_some() {
            self.permission_queue.push_back(pending);
            return;
        }
        self.start_permission_prompt(pending);
    }

    fn auto_decision_for_request(&self, request: &PermissionRequest) -> Option<AutoDecision> {
        if matches!(self.coder_mode, CoderMode::BypassPermissions) {
            return Some(AutoDecision {
                result: PermissionResult::Allow {
                    updated_input: request.input.clone(),
                    updated_permissions: None,
                    tool_use_id: Some(request.tool_use_id.clone()),
                    accept_for_session: None,
                },
                label: "auto allow",
            });
        }

        if self.is_request_denied(request) {
            return Some(AutoDecision {
                result: PermissionResult::Deny {
                    message: "Permission denied by rules.".to_string(),
                    interrupt: None,
                    tool_use_id: Some(request.tool_use_id.clone()),
                },
                label: "auto deny",
            });
        }

        if self.is_request_allowed(request) {
            return Some(AutoDecision {
                result: PermissionResult::Allow {
                    updated_input: request.input.clone(),
                    updated_permissions: None,
                    tool_use_id: Some(request.tool_use_id.clone()),
                    accept_for_session: Some(false),
                },
                label: "auto allow",
            });
        }

        if self.permission_default_allow {
            return Some(AutoDecision {
                result: PermissionResult::Allow {
                    updated_input: request.input.clone(),
                    updated_permissions: None,
                    tool_use_id: Some(request.tool_use_id.clone()),
                    accept_for_session: None,
                },
                label: "auto allow",
            });
        }

        None
    }

    fn is_request_allowed(&self, request: &PermissionRequest) -> bool {
        self.matches_tool_rules(&request.tool_name, &self.permission_allow_tools)
            || self.matches_bash_patterns(request, &self.permission_allow_bash_patterns)
    }

    fn is_request_denied(&self, request: &PermissionRequest) -> bool {
        self.matches_tool_rules(&request.tool_name, &self.permission_deny_tools)
            || self.matches_bash_patterns(request, &self.permission_deny_bash_patterns)
    }

    fn matches_tool_rules(&self, tool_name: &str, rules: &[String]) -> bool {
        rules.iter().any(|rule| rule.eq_ignore_ascii_case(tool_name))
    }

    fn matches_bash_patterns(&self, request: &PermissionRequest, patterns: &[String]) -> bool {
        if !matches!(request.tool_name.as_str(), "Bash" | "KillBash") {
            return false;
        }
        let Some(command) = extract_bash_command(&request.input) else {
            return false;
        };
        patterns
            .iter()
            .any(|pattern| pattern_matches(pattern, &command))
    }

    fn start_permission_prompt(&mut self, pending: PermissionPending) {
        let Some(action_tx) = self.permission_action_tx.clone() else {
            let _ = pending
                .respond_to
                .send(PermissionResult::deny_and_interrupt(
                    "Permission prompt unavailable.",
                ));
            return;
        };
        let permission_type = permission_type_for_request(&pending.request);
        let dialog = PermissionDialog::new(permission_type).on_action(move |action| {
            let _ = action_tx.send(action);
        });
        self.permission_pending = Some(pending);
        self.permission_dialog = Some(dialog);
    }

    pub(crate) fn open_next_permission_prompt(&mut self) {
        if self.permission_pending.is_some() || self.permission_dialog.is_some() {
            return;
        }
        if let Some(next) = self.permission_queue.pop_front() {
            self.start_permission_prompt(next);
        }
    }

    pub(crate) fn handle_permission_action(&mut self, action: PermissionAction) {
        let Some(pending) = self.permission_pending.take() else {
            return;
        };

        let request = pending.request;
        let decision_label = match action {
            PermissionAction::Allow | PermissionAction::AllowAlways => "allow",
            PermissionAction::AllowOnce => "allow once",
            PermissionAction::Deny => "deny",
        };

        let result = match action {
            PermissionAction::Allow | PermissionAction::AllowAlways => {
                self.apply_permission_allow(&request);
                PermissionResult::Allow {
                    updated_input: request.input.clone(),
                    updated_permissions: request.suggestions.clone(),
                    tool_use_id: Some(request.tool_use_id.clone()),
                    accept_for_session: Some(false),
                }
            }
            PermissionAction::AllowOnce => PermissionResult::Allow {
                updated_input: request.input.clone(),
                updated_permissions: None,
                tool_use_id: Some(request.tool_use_id.clone()),
                accept_for_session: Some(true),
            },
            PermissionAction::Deny => PermissionResult::Deny {
                message: "User denied permission.".to_string(),
                interrupt: None,
                tool_use_id: Some(request.tool_use_id.clone()),
            },
        };

        let detail = permission_detail_for_request(&request);
        self.record_permission_history(&request, decision_label, detail);

        let _ = pending.respond_to.send(result);
        self.permission_dialog = None;
        self.open_next_permission_prompt();
    }

    fn apply_permission_allow(&mut self, request: &PermissionRequest) {
        if request.tool_name == "Bash" {
            if let Some(command) = extract_bash_command(&request.input) {
                add_unique(&mut self.permission_allow_bash_patterns, &[command.clone()]);
                remove_items(&mut self.permission_deny_bash_patterns, &[command]);
                self.persist_permission_config();
            }
            return;
        }
        add_unique(&mut self.permission_allow_tools, &[request.tool_name.clone()]);
        remove_items(&mut self.permission_deny_tools, &[request.tool_name.clone()]);
        self.persist_permission_config();
    }

    fn record_permission_history(
        &mut self,
        request: &PermissionRequest,
        decision: &str,
        detail: Option<String>,
    ) {
        const PERMISSION_HISTORY_LIMIT: usize = 50;
        self.permission_history.push(PermissionHistoryEntry {
            tool_name: request.tool_name.clone(),
            decision: decision.to_string(),
            timestamp: super::super::now_timestamp(),
            detail,
        });
        if self.permission_history.len() > PERMISSION_HISTORY_LIMIT {
            let overflow = self.permission_history.len() - PERMISSION_HISTORY_LIMIT;
            self.permission_history.drain(0..overflow);
        }
    }

    pub(crate) fn persist_permission_config(&self) {
        let config = PermissionConfig {
            coder_mode: self.coder_mode,
            default_allow: self.permission_default_allow,
            allow_tools: self.permission_allow_tools.clone(),
            deny_tools: self.permission_deny_tools.clone(),
            bash_allow_patterns: self.permission_allow_bash_patterns.clone(),
            bash_deny_patterns: self.permission_deny_bash_patterns.clone(),
        };
        save_permission_config(&config);
    }

    pub(crate) fn cycle_coder_mode(&mut self, session_info: &mut SessionInfo) {
        let next = match self.coder_mode {
            CoderMode::BypassPermissions => CoderMode::Plan,
            CoderMode::Plan => CoderMode::Autopilot,
            CoderMode::Autopilot => CoderMode::BypassPermissions,
        };
        self.set_coder_mode(next, session_info);
    }

    pub(crate) fn set_coder_mode(&mut self, mode: CoderMode, session_info: &mut SessionInfo) {
        self.coder_mode = mode;
        self.permission_default_allow = coder_mode_default_allow(mode, self.permission_default_allow);
        session_info.permission_mode = super::rules::coder_mode_label(mode).to_string();
        self.persist_permission_config();
    }

    pub(crate) fn add_permission_allow(&mut self, tools: Vec<String>) -> String {
        let tokens = sanitize_tokens(tools);
        if tokens.is_empty() {
            return "No tools provided to allow.".to_string();
        }
        let (tool_rules, bash_patterns) = split_permission_tokens(tokens);
        if tool_rules.is_empty() && bash_patterns.is_empty() {
            return "No valid tools or patterns provided to allow.".to_string();
        }
        add_unique(&mut self.permission_allow_tools, &tool_rules);
        remove_items(&mut self.permission_deny_tools, &tool_rules);
        add_unique(&mut self.permission_allow_bash_patterns, &bash_patterns);
        remove_items(&mut self.permission_deny_bash_patterns, &bash_patterns);
        self.persist_permission_config();

        let mut parts = Vec::new();
        if !tool_rules.is_empty() {
            parts.push(format!("tools: {}", tool_rules.join(", ")));
        }
        if !bash_patterns.is_empty() {
            parts.push(format!("bash patterns: {}", bash_patterns.join(", ")));
        }
        format!("Allowed {}.", parts.join("; "))
    }

    pub(crate) fn add_permission_deny(&mut self, tools: Vec<String>) -> String {
        let tokens = sanitize_tokens(tools);
        if tokens.is_empty() {
            return "No tools provided to deny.".to_string();
        }
        let (tool_rules, bash_patterns) = split_permission_tokens(tokens);
        if tool_rules.is_empty() && bash_patterns.is_empty() {
            return "No valid tools or patterns provided to deny.".to_string();
        }
        add_unique(&mut self.permission_deny_tools, &tool_rules);
        remove_items(&mut self.permission_allow_tools, &tool_rules);
        add_unique(&mut self.permission_deny_bash_patterns, &bash_patterns);
        remove_items(&mut self.permission_allow_bash_patterns, &bash_patterns);
        self.persist_permission_config();

        let mut parts = Vec::new();
        if !tool_rules.is_empty() {
            parts.push(format!("tools: {}", tool_rules.join(", ")));
        }
        if !bash_patterns.is_empty() {
            parts.push(format!("bash patterns: {}", bash_patterns.join(", ")));
        }
        format!("Denied {}.", parts.join("; "))
    }

    pub(crate) fn enable_tools(&mut self, tools: Vec<String>) -> String {
        let tools = sanitize_tokens(tools);
        if tools.is_empty() {
            return "No tools provided to enable.".to_string();
        }
        add_unique(&mut self.tools_allowed, &tools);
        remove_items(&mut self.tools_disallowed, &tools);
        format!("Enabled tools: {}.", tools.join(", "))
    }

    pub(crate) fn disable_tools(&mut self, tools: Vec<String>) -> String {
        let tools = sanitize_tokens(tools);
        if tools.is_empty() {
            return "No tools provided to disable.".to_string();
        }
        add_unique(&mut self.tools_disallowed, &tools);
        remove_items(&mut self.tools_allowed, &tools);
        format!("Disabled tools: {}.", tools.join(", "))
    }

    pub(crate) fn set_output_style(&mut self, style: Option<String>) -> String {
        self.output_style = style.clone();
        match style {
            Some(name) => format!("Output style set to {}.", name),
            None => "Output style cleared.".to_string(),
        }
    }
}
