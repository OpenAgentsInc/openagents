//! The Coder SDK's hooks on this crate's sessions (#11529): the Coder
//! Terminal turn's Claude Code or Codex session and the issue flow's
//! Microcoder turns run the session's registry before every tool: the
//! floors, held (nobody answers a delegated session from inside), the
//! worktree guard holding file writes to the turn's folder, and the
//! shared-checkout fence (#11532).
//!
//! A CLI session reaches the registry through the SDK's hook bridge,
//! installed as the CLI's own `PreToolUse` hook
//! ([`coder_delegate::delegate::HookBridge`]); the bridge program is the
//! binary running this turn (`coder coder-hook`, `openagents coder-hook`),
//! which holds the floors alone when the host is gone. The Microcoder loop
//! fires the registry itself ([`microcoder_loop::env::Hooked`]).

use std::path::Path;

/// A turn's registry in `workdir`: the held floors, the worktree guard
/// (refusing every write on a read-only turn) and the shared-checkout
/// fence.
#[must_use]
pub fn registry(workdir: &Path, read_only: bool) -> coder_sdk::Registry {
    microcoder_loop::env::held_hooks(workdir, !read_only).registry()
}

/// The host answering a CLI session's bridge, kept while it runs, and the
/// bridge to install. `None` when the socket or this program's path can't
/// be had; the session then runs as before, held by its boundary alone.
#[must_use]
pub fn bridge(
    workdir: &Path,
    read_only: bool,
) -> Option<(
    coder_delegate::delegate::HookBridge,
    coder_sdk::bridge::Host,
)> {
    let program = std::env::current_exe().ok()?;
    bridge_with(workdir, read_only, program)
}

/// [`bridge`] with the bridge program named.
#[must_use]
pub fn bridge_with(
    workdir: &Path,
    read_only: bool,
    program: std::path::PathBuf,
) -> Option<(
    coder_delegate::delegate::HookBridge,
    coder_sdk::bridge::Host,
)> {
    let runner = coder_sdk::Runner::new(registry(workdir, read_only), coder_sdk::Budget::default());
    let host =
        coder_sdk::bridge::Host::start(std::sync::Arc::new(std::sync::Mutex::new(runner))).ok()?;
    Some((coder_sdk::bridge::hook_bridge(&host, program), host))
}
