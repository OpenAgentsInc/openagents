use std::fs;
use std::path::{Path, PathBuf};

use super::types::HookEvent;
use serde::{Deserialize, Serialize};

use super::super::config::{config_dir, hook_config_file};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HookScriptSource {
    Project,
    User,
}

#[derive(Clone, Debug)]
pub(crate) struct HookScriptEntry {
    pub(crate) event: HookEvent,
    pub(crate) matcher: Option<String>,
    pub(crate) source: HookScriptSource,
    pub(crate) path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct HookConfig {
    pub(crate) tool_blocker: bool,
    pub(crate) tool_logger: bool,
    pub(crate) output_truncator: bool,
    pub(crate) context_injection: bool,
    pub(crate) todo_enforcer: bool,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            tool_blocker: true,
            tool_logger: false,
            output_truncator: true,
            context_injection: true,
            todo_enforcer: false,
        }
    }
}

pub(crate) struct HookScriptCatalog {
    pub(crate) entries: Vec<HookScriptEntry>,
    pub(crate) error: Option<String>,
    pub(crate) project_path: Option<PathBuf>,
    pub(crate) user_path: Option<PathBuf>,
}

fn hook_project_dir(cwd: &Path) -> PathBuf {
    cwd.join(".openagents").join("hooks")
}

fn hook_user_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".openagents").join("hooks"))
}

fn normalize_hook_name(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .replace('_', "")
        .replace('-', "")
}

fn parse_hook_event_name(name: &str) -> Option<HookEvent> {
    match normalize_hook_name(name).as_str() {
        "pretooluse" => Some(HookEvent::PreToolUse),
        "posttooluse" => Some(HookEvent::PostToolUse),
        "posttoolusefailure" => Some(HookEvent::PostToolUseFailure),
        "notification" => Some(HookEvent::Notification),
        "userpromptsubmit" => Some(HookEvent::UserPromptSubmit),
        "sessionstart" => Some(HookEvent::SessionStart),
        "sessionend" => Some(HookEvent::SessionEnd),
        "stop" => Some(HookEvent::Stop),
        "subagentstart" => Some(HookEvent::SubagentStart),
        "subagentstop" => Some(HookEvent::SubagentStop),
        "precompact" => Some(HookEvent::PreCompact),
        "permissionrequest" => Some(HookEvent::PermissionRequest),
        _ => None,
    }
}

fn parse_hook_script_name(stem: &str) -> (String, Option<String>) {
    if let Some((event, matcher)) = stem.split_once("__") {
        (event.to_string(), Some(matcher.to_string()))
    } else {
        (stem.to_string(), None)
    }
}

pub(crate) fn load_hook_scripts(cwd: &Path) -> HookScriptCatalog {
    let project_dir = hook_project_dir(cwd);
    let user_dir = hook_user_dir();
    let mut errors = Vec::new();
    let mut entries = Vec::new();

    if let Some(user_dir) = user_dir.as_ref() {
        entries.extend(load_hook_script_dir(user_dir, HookScriptSource::User, &mut errors));
    }
    entries.extend(load_hook_script_dir(
        &project_dir,
        HookScriptSource::Project,
        &mut errors,
    ));

    entries.sort_by(|a, b| a.path.cmp(&b.path));

    HookScriptCatalog {
        entries,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join(" | "))
        },
        project_path: Some(project_dir),
        user_path: user_dir,
    }
}

fn load_hook_script_dir(
    dir: &Path,
    source: HookScriptSource,
    errors: &mut Vec<String>,
) -> Vec<HookScriptEntry> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            errors.push(format!("Failed to read {}: {}", dir.display(), err));
            return Vec::new();
        }
    };

    let mut scripts = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                errors.push(format!("Failed to read hook entry: {}", err));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match parse_hook_script_entry(&path, source) {
            Ok(Some(script)) => scripts.push(script),
            Ok(None) => {}
            Err(err) => errors.push(err),
        }
    }
    scripts
}

fn parse_hook_script_entry(path: &Path, source: HookScriptSource) -> Result<Option<HookScriptEntry>, String> {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(None);
    };
    let (event_name, matcher) = parse_hook_script_name(stem);
    let Some(event) = parse_hook_event_name(&event_name) else {
        return Ok(None);
    };

    Ok(Some(HookScriptEntry {
        event,
        matcher,
        source,
        path: path.to_path_buf(),
    }))
}

pub(crate) fn load_hook_config() -> HookConfig {
    let path = hook_config_file();
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(config) = serde_json::from_str::<HookConfig>(&content) {
            return config;
        }
    }
    HookConfig::default()
}

pub(crate) fn save_hook_config(config: &HookConfig) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(content) = serde_json::to_string_pretty(config) {
            let _ = fs::write(hook_config_file(), content);
        }
    }
}
