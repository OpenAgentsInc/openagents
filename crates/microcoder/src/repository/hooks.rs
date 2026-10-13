//! Microcoder's sessions on the Coder SDK (#11527): every session a
//! repository turn starts runs one registry before each tool, composed by
//! one runner: the floors (`coder_sdk::floors`), held, since nobody answers
//! a repository turn from inside, the worktree guard
//! (`coder_sdk::worktree`) holding the file tools' writes to the task's
//! worktree, and the shared-checkout fence (`coder_sdk::checkout_guard`).
//!
//! - **The loop** fires it on every command ([`loop_hooks`],
//!   `microcoder_loop::env::Hooked`).
//! - **Claude Code and Codex CLI sessions** run it through the SDK's hook
//!   bridge, installed as the CLI's own `PreToolUse` hook ([`install`]);
//!   this binary's `coder-hook` ([`main`]) is the bridge program, holding
//!   the floors alone when the host is gone.
//! - **ACP agents** (Devin, OpenCode, Grok Build) answer their asks through
//!   it, and reach it before their own tools where they can
//!   ([`coder_sdk::acp::gate`]).
//! - **The `claude/sdk:` seat** answers Claude Code's permission callback
//!   through its own registry, whose gate asks the person about anything a
//!   floor names or the worktree guard refuses ([`super::claude_sdk`]).

use std::path::Path;

use coder_delegate::delegate::Cli;

/// The bridge's subcommand: `microcoder coder-hook --format
/// claude|codex|acp`.
pub const COMMAND: &str = coder_sdk::bridge::COMMAND;

/// A repository session's registry in `workspace`: the held floors, the
/// worktree guard on its writes (also admitted under `writable`), and the
/// shared-checkout fence (#11532).
#[must_use]
pub fn registry(workspace: &Path, writable: &[std::path::PathBuf]) -> coder_sdk::Registry {
    let guard = writable.iter().fold(
        coder_sdk::worktree::Guard::writes(workspace),
        |guard, dir| guard.writing(dir),
    );
    let mut registry = coder_sdk::floors::held_registry(Some(guard));
    let _ = registry.register(coder_sdk::checkout_guard::hook(workspace.to_path_buf()));
    registry
}

/// The loop's hooks in `workspace`.
#[must_use]
pub fn loop_hooks(workspace: &Path) -> coder_sdk::Hooks {
    microcoder_loop::env::held_hooks(workspace, true)
}

/// An ACP agent's hooks in `workspace`, `agent` by its catalog ID and
/// `name` as people read it, its writes also admitted under `writable`
/// (a boundary's scratch). An ask a hook would put to the person is held.
#[must_use]
pub fn acp(
    workspace: &Path,
    writable: &[std::path::PathBuf],
    agent: &str,
    name: &str,
) -> coder_sdk::Hooks {
    coder_sdk::acp::hooks(
        registry(workspace, writable),
        coder_sdk::Budget::default(),
        agent,
        name,
    )
    .0
}

/// Starts the ACP agent `agent` so its own tools reach `hooks` first, where
/// it can ([`coder_sdk::acp::gate`]), with this program as the bridge.
///
/// # Errors
/// The bridge can't be set up: the agent would run unchecked.
pub fn gate(
    agent: &str,
    name: &str,
    hooks: &coder_sdk::Hooks,
    arguments: &mut Vec<String>,
    environment: &mut Vec<(String, String)>,
) -> Result<coder_sdk::acp::Gate, String> {
    coder_sdk::acp::gate(
        agent,
        name,
        hooks,
        std::env::current_exe(),
        arguments,
        environment,
    )
}

/// Starts the host for `cli`'s session in `workspace` and installs the
/// bridge (this program's [`COMMAND`]) as the CLI's own hook. Keep the
/// host while the session runs. `None`, with the CLI unchanged, when the
/// host or this program's path can't be had.
pub fn install(cli: &mut Cli, workspace: &Path) -> Option<coder_sdk::bridge::Host> {
    let program = std::env::current_exe().ok()?;
    coder_sdk::bridge::install(cli, registry(workspace, &[]), program)
}

/// The bridge program: reads one hook input, asks the host, prints the
/// reply; with no host, the held floors answer alone.
#[must_use]
pub fn main(args: &[String]) -> i32 {
    coder_sdk::bridge::main(args, coder_sdk::floors::held_decision)
}
