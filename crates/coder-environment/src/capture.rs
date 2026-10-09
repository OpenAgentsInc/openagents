//! What a clean build may capture into its output image (ENV-04).
//!
//! A recipe's [`Capture`] declares paths the image must contain
//! (`required`), extra paths to leave out (`exclude`), and the explored
//! state it wants kept (`keep_explored`). Everything else follows standing
//! rules every recipe inherits and cannot relax:
//!
//! - [`LOGIN_PATHS`]: sign-ins and token stores (Claude Code, Codex, `gh`,
//!   npm, Cargo, Git, Docker, SSH) are always removed. A recipe can neither
//!   require nor keep one (`docs/cloud/claude-code-byo.md`).
//! - [`PRIVATE_MOUNTS`]: secret mounts are always removed.
//! - [`EXPLORED_PATHS`]: shell history, agent sessions, caches, and command
//!   records are explored state; they are removed unless the recipe keeps a
//!   path at or under one of them.
//!
//! Paths are `~/relative` (home), `/absolute`, or source-relative.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Sign-in and token stores, home-relative. Never captured.
pub const LOGIN_PATHS: &[&str] = &[
    "~/.claude/.credentials.json",
    "~/.claude.json",
    "~/.codex/auth.json",
    "~/.config/gh/hosts.yml",
    "~/.git-credentials",
    "~/.netrc",
    "~/.cargo/credentials",
    "~/.cargo/credentials.toml",
    "~/.docker/config.json",
    "~/.pypirc",
    "~/.ssh",
];
/// Files whose token lines are removed while the rest of the file stays.
pub const TOKEN_LINE_FILES: &[&str] = &["~/.npmrc", "~/.yarnrc.yml"];
/// Provider snapshot-exclusion files a base template may carry, removed
/// before capture. Boat skips every path named in `~/.boxignore` when it
/// saves a snapshot; the Coder runtime template lists `~/.cargo` and
/// `~/.rustup` there, which silently dropped the toolchains a recipe
/// installed from the saved image.
pub const SNAPSHOT_EXCLUSION_FILES: &[&str] = &["~/.boxignore"];
/// Secret mounts a builder machine may carry. Never captured.
pub const PRIVATE_MOUNTS: &[&str] = &["/run/secrets", "/var/run/secrets", "/mnt/oa-private"];
/// Explored state removed unless a recipe keeps a path under it.
pub const EXPLORED_PATHS: &[&str] = &[
    "~/.bash_history",
    "~/.zsh_history",
    "~/.python_history",
    "~/.lesshst",
    "~/.viminfo",
    "~/.cache",
    "~/.codex/sessions",
    "~/.codex/log",
    "~/.claude/projects",
    "~/.claude/todos",
    "/tmp/oa-commands",
    "/tmp/oa-services",
];
pub const MAX_CAPTURE_PATHS: usize = 64;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    /// Paths the output image must contain; checked after sanitization.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required: BTreeSet<String>,
    /// Further paths to remove before capture.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub exclude: BTreeSet<String>,
    /// Explored state to keep (at or under an [`EXPLORED_PATHS`] entry).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub keep_explored: BTreeSet<String>,
}

impl Capture {
    pub fn is_empty(&self) -> bool {
        self.required.is_empty() && self.exclude.is_empty() && self.keep_explored.is_empty()
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.required.len() + self.exclude.len() + self.keep_explored.len() > MAX_CAPTURE_PATHS {
            return Err("The capture policy declares too many paths.");
        }
        let all = self
            .required
            .iter()
            .chain(&self.exclude)
            .chain(&self.keep_explored);
        if !all.clone().all(|p| valid_path(p)) {
            return Err("Capture paths must be ~/home, /absolute, or source-relative paths.");
        }
        for p in self.required.iter().chain(&self.keep_explored) {
            if is_login(p) {
                return Err("A sign-in or token store is never captured.");
            }
            if PRIVATE_MOUNTS.iter().any(|m| within(p, m)) {
                return Err("A private mount is never captured.");
            }
            if self.exclude.iter().any(|e| within(p, e)) {
                return Err("A required or kept path is excluded.");
            }
        }
        for p in &self.required {
            if EXPLORED_PATHS.iter().any(|x| within(p, x))
                && !self.keep_explored.iter().any(|k| within(p, k))
            {
                return Err("A required path under explored state must also be kept.");
            }
        }
        if !self
            .keep_explored
            .iter()
            .all(|k| EXPLORED_PATHS.iter().any(|x| within(k, x)))
        {
            return Err("Only explored state can be kept explicitly.");
        }
        Ok(())
    }
}

/// Whether `path` is a sign-in or token store (or inside one).
pub fn is_login(path: &str) -> bool {
    LOGIN_PATHS.iter().any(|l| within(path, l))
        || path
            .rsplit('/')
            .next()
            .is_some_and(|name| name == ".credentials.json" || name == "auth.json")
}

/// Whether `path` is `root` or lies under it.
pub fn within(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `~/rel`, `/abs`, or `rel`, with no empty, `.`, or `..` segments, no
/// glob or control characters, at most 512 bytes.
pub fn valid_path(path: &str) -> bool {
    let body = path
        .strip_prefix("~/")
        .or_else(|| path.strip_prefix('/'))
        .unwrap_or(path);
    !body.is_empty()
        && path.len() <= 512
        && !path.chars().any(|c| c.is_control() || "*?[]\\".contains(c))
        && body
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
