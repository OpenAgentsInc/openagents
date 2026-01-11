use std::fs;

use super::request::PermissionRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wgpui::components::organisms::PermissionType;

use crate::app::config::{config_dir, permission_config_file};
use crate::app::events::CoderMode;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PermissionConfig {
    pub(crate) coder_mode: CoderMode,
    pub(crate) default_allow: bool,
    pub(crate) allow_tools: Vec<String>,
    pub(crate) deny_tools: Vec<String>,
    pub(crate) bash_allow_patterns: Vec<String>,
    pub(crate) bash_deny_patterns: Vec<String>,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            coder_mode: CoderMode::Autopilot,
            default_allow: false,
            allow_tools: Vec::new(),
            deny_tools: Vec::new(),
            bash_allow_patterns: Vec::new(),
            bash_deny_patterns: Vec::new(),
        }
    }
}

pub(crate) fn load_permission_config() -> PermissionConfig {
    let path = permission_config_file();
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(config) = serde_json::from_str::<PermissionConfig>(&content) {
            return config;
        }
    }
    PermissionConfig::default()
}

pub(crate) fn save_permission_config(config: &PermissionConfig) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(json) = serde_json::to_string_pretty(config) {
            let _ = fs::write(permission_config_file(), json);
        }
    }
}

pub(crate) fn parse_coder_mode(input: &str) -> Option<CoderMode> {
    let normalized = input
        .trim()
        .to_ascii_lowercase()
        .replace('-', "")
        .replace('_', "")
        .replace('\'', "");
    match normalized.as_str() {
        "bypasspermissions" | "bypass" => Some(CoderMode::BypassPermissions),
        "plan" => Some(CoderMode::Plan),
        "autopilot" | "auto" => Some(CoderMode::Autopilot),
        _ => None,
    }
}

pub(crate) fn coder_mode_default_allow(mode: CoderMode, fallback: bool) -> bool {
    if mode.auto_approves_all() {
        true
    } else {
        fallback
    }
}

pub(crate) fn coder_mode_label(mode: CoderMode) -> &'static str {
    match mode {
        CoderMode::BypassPermissions => "bypass",
        CoderMode::Plan => "plan",
        CoderMode::Autopilot => "autopilot",
    }
}

pub(crate) fn permission_type_for_request(request: &PermissionRequest) -> PermissionType {
    let tool = request.tool_name.as_str();
    if matches!(tool, "Read" | "Grep" | "Glob") {
        let path = request
            .blocked_path
            .clone()
            .or_else(|| extract_input_string(&request.input, &["path", "file_path", "filePath"]));
        if let Some(path) = path {
            return PermissionType::FileRead(super::super::truncate_preview(&path, 120));
        }
    }
    if matches!(tool, "Edit" | "Write" | "NotebookEdit") {
        let path = request
            .blocked_path
            .clone()
            .or_else(|| extract_input_string(&request.input, &["path", "file_path", "filePath"]));
        if let Some(path) = path {
            return PermissionType::FileWrite(super::super::truncate_preview(&path, 120));
        }
    }
    if matches!(tool, "Bash" | "KillBash") {
        if let Some(command) = extract_bash_command(&request.input) {
            return PermissionType::Execute(super::super::truncate_preview(&command, 120));
        }
    }
    if matches!(tool, "WebSearch" | "WebFetch" | "Browser") {
        if let Some(target) = extract_input_string(&request.input, &["url", "uri", "query"]) {
            return PermissionType::Network(super::super::truncate_preview(&target, 120));
        }
    }

    let mut desc = format!("Tool: {}", tool);
    if let Some(reason) = &request.decision_reason {
        if !reason.trim().is_empty() {
            desc.push_str(" (");
            desc.push_str(reason.trim());
            desc.push(')');
        }
    }
    PermissionType::Custom(super::super::truncate_preview(&desc, 160))
}

pub(crate) fn permission_detail_for_request(request: &PermissionRequest) -> Option<String> {
    let detail = permission_type_for_request(request).description();
    if detail.trim().is_empty() {
        None
    } else {
        Some(super::super::truncate_preview(&detail, 120))
    }
}

pub(crate) fn extract_bash_command(input: &Value) -> Option<String> {
    extract_input_string(input, &["command"])
}

fn extract_input_string(input: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = input.get(*key).and_then(|val| val.as_str()) {
            if !value.trim().is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(crate) fn pattern_matches(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut remainder = text;
    let mut first_match = true;
    for part in parts.iter().filter(|part| !part.is_empty()) {
        if first_match && !pattern.starts_with('*') {
            if let Some(rest) = remainder.strip_prefix(*part) {
                remainder = rest;
            } else {
                return false;
            }
        } else if let Some(idx) = remainder.find(*part) {
            remainder = &remainder[idx + part.len()..];
        } else {
            return false;
        }
        first_match = false;
    }

    if !pattern.ends_with('*') {
        if let Some(last) = parts.iter().rev().find(|part| !part.is_empty()) {
            return text.ends_with(last);
        }
    }
    true
}

pub(crate) fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Read" | "Grep" | "Glob" | "WebSearch" | "Search" | "WebFetch"
            | "AskUserQuestion" | "Task" | "ExitPlanMode" | "LSP"
    )
}

pub(crate) fn sanitize_tokens(tokens: Vec<String>) -> Vec<String> {
    tokens
        .into_iter()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

pub(crate) fn split_permission_tokens(tokens: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut tools = Vec::new();
    let mut bash_patterns = Vec::new();
    for token in tokens {
        if let Some(pattern) = parse_bash_pattern(&token) {
            bash_patterns.push(pattern);
        } else {
            tools.push(token);
        }
    }
    (tools, bash_patterns)
}

fn parse_bash_pattern(token: &str) -> Option<String> {
    let trimmed = token.trim();
    let rest = trimmed
        .strip_prefix("Bash(")
        .or_else(|| trimmed.strip_prefix("bash("))?;
    let inner = rest.strip_suffix(')')?.trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

pub(crate) fn add_unique(target: &mut Vec<String>, items: &[String]) {
    for item in items {
        if !target.iter().any(|entry| entry == item) {
            target.push(item.clone());
        }
    }
}

pub(crate) fn remove_items(target: &mut Vec<String>, items: &[String]) {
    target.retain(|entry| !items.iter().any(|item| item == entry));
}
