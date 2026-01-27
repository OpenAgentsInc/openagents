use std::env;
use std::path::PathBuf;

use crate::types::WorkspaceEntry;

pub(crate) fn resolve_workspace_codex_home(
    _entry: &WorkspaceEntry,
    _parent_path: Option<&str>,
) -> Option<PathBuf> {
    resolve_default_codex_home()
}

pub(crate) fn resolve_default_codex_home() -> Option<PathBuf> {
    if let Ok(value) = env::var("CODEX_HOME")
        && !value.trim().is_empty() {
            return Some(PathBuf::from(value.trim()));
        }
    resolve_home_dir().map(|home| home.join(".codex"))
}

fn resolve_home_dir() -> Option<PathBuf> {
    if let Ok(value) = env::var("HOME")
        && !value.trim().is_empty() {
            return Some(PathBuf::from(value));
        }
    if let Ok(value) = env::var("USERPROFILE")
        && !value.trim().is_empty() {
            return Some(PathBuf::from(value));
        }
    None
}
