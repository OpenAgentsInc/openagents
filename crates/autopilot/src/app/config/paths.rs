use std::path::{Path, PathBuf};

/// Get the config directory path.
pub(crate) fn config_dir() -> PathBuf {
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot");
    if base.exists() {
        return base;
    }

    let legacy = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("coder");
    if legacy.exists() {
        return legacy;
    }

    base
}

/// Get the config file path.
pub(crate) fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub(crate) fn keybindings_file() -> PathBuf {
    config_dir().join("keybindings.json")
}

pub(crate) fn permission_config_file() -> PathBuf {
    config_dir().join("permissions.json")
}

pub(crate) fn hook_config_file() -> PathBuf {
    config_dir().join("hooks.json")
}

pub(crate) fn sessions_dir() -> PathBuf {
    config_dir().join("sessions")
}

pub(crate) fn session_index_file() -> PathBuf {
    sessions_dir().join("index.json")
}

pub(crate) fn session_messages_dir(session_id: &str) -> PathBuf {
    sessions_dir().join(session_id)
}

pub(crate) fn session_messages_file(session_id: &str) -> PathBuf {
    session_messages_dir(session_id).join("messages.jsonl")
}

pub(crate) fn mcp_project_file(cwd: &Path) -> PathBuf {
    cwd.join(".mcp.json")
}
