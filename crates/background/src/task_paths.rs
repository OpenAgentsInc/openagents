//! Task-store paths shared by Coder and the Unix background scheduler.

use std::path::{Path, PathBuf};

/// Target slots per project that Coder keeps ([`crates/coder` targets]).
/// A slot numbered past this is left over and belongs to class 1.
pub const SLOTS: usize = 4;

/// Names another task store than `~/.openagents/tasks`; Coder reads the
/// same variable ([`crates/coder` `task::local::STORE_VAR`]).
pub const STORE_VAR: &str = "OPENAGENTS_TASKS";

/// The task store on this computer: `$OPENAGENTS_TASKS`, else
/// `HOME/.openagents/tasks`. Coder and the background runner both use it,
/// so they agree on where tasks, worktrees and target slots live.
#[must_use]
pub fn task_store(home: &Path) -> PathBuf {
    std::env::var_os(STORE_VAR)
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| home.join(".openagents/tasks"), PathBuf::from)
}

/// The folder Coder makes task worktrees in: beside the task store.
#[must_use]
pub fn task_worktrees(store: &Path) -> PathBuf {
    store.parent().unwrap_or(store).join("worktrees")
}

/// The folder Coder keeps Cargo target slots in: beside the task store.
#[must_use]
pub fn task_targets(store: &Path) -> PathBuf {
    store.parent().unwrap_or(store).join("targets")
}
