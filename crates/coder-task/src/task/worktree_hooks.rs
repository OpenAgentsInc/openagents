//! A repository's own worktree setup and teardown (#10297).
//!
//! A task works in a fresh worktree, which has the repository's tracked
//! files and nothing else: no `.env`, no installed dependencies, no local
//! configuration. A repository can say how to make one ready in a
//! committed `.openagents/worktree.json`:
//!
//! ```json
//! {
//!   "setup": ["cp \"$OPENAGENTS_SOURCE_CHECKOUT/.env\" .env", "npm ci"],
//!   "teardown": "docker compose down",
//!   "timeout_seconds": 900
//! }
//! ```
//!
//! `setup` and `teardown` are each a command or a list of commands, run
//! one after another with `/bin/sh -c` in the worktree, standard input
//! closed, stopping at the first that fails. Each sees:
//!
//! - `OPENAGENTS_SOURCE_CHECKOUT`: the checkout the worktree came from,
//!   to read local files from;
//! - `OPENAGENTS_WORKTREE`: the worktree;
//! - `OPENAGENTS_WORKTREE_PORT`: a port free when the worktree was set
//!   up, kept for that worktree (its teardown sees the same one).
//!
//! **When.** Setup runs once per task, after the task has its worktree
//! (a spare it took, or a fresh one) and before the engine starts, so
//! spares stay free of ignored files and a task's engine never sees a
//! half-made worktree. A follow-up turn reuses the worktree as it is.
//! Teardown runs before a worktree is removed ([`teardown`]); a teardown
//! that fails keeps the worktree, so nothing it would have stopped or
//! saved is lost.
//!
//! **Never the source checkout.** The commands run outside the engine's
//! boundary, as the person would run them, but under the worktree's
//! source guard ([`coder_boundary::source::Guard`]): they may read the
//! source checkout, never write it. Where the guard can't be enforced
//! (no `bwrap` on Linux, another platform) the hooks don't run, and the
//! task's record says why.
//!
//! **Trust.** Only the repository's own committed configuration runs:
//! the file as committed at the source checkout's `HEAD` (the commit the
//! person has checked out; uncommitted edits never run). A task whose
//! commit carries a different file, such as a pull request from a fork
//! that adds or changes its setup, is not trusted: its hooks are skipped
//! and the run says so.
//!
//! There is no time limit unless the repository sets `timeout_seconds`.

use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::local::git_out;

/// Where a repository commits its worktree hooks.
pub const FILE: &str = ".openagents/worktree.json";

/// Kept in the worktree's own Git administrative directory: its port.
const STATE: &str = "openagents-worktree.json";

/// How much of a failing command's output a refusal quotes.
const TAIL: usize = 2_000;

/// A command or a list of them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum Commands {
    One(String),
    Many(Vec<String>),
}

impl Commands {
    fn list(&self) -> Vec<String> {
        match self {
            Commands::One(one) => vec![one.clone()],
            Commands::Many(many) => many.clone(),
        }
        .into_iter()
        .map(|command| command.trim().to_owned())
        .filter(|command| !command.is_empty())
        .collect()
    }
}

/// `.openagents/worktree.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hooks {
    #[serde(default)]
    setup: Option<Commands>,
    #[serde(default)]
    teardown: Option<Commands>,
    /// No limit unless set.
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

impl Hooks {
    /// The setup commands, in order.
    #[must_use]
    pub fn setup(&self) -> Vec<String> {
        self.setup.as_ref().map(Commands::list).unwrap_or_default()
    }

    /// The teardown commands, in order.
    #[must_use]
    pub fn teardown(&self) -> Vec<String> {
        self.teardown
            .as_ref()
            .map(Commands::list)
            .unwrap_or_default()
    }

    fn timeout(&self) -> Option<Duration> {
        self.timeout_seconds.map(Duration::from_secs)
    }
}

/// Which hooks run for a worktree, and why none do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trusted {
    /// The repository commits no hooks.
    None,
    /// The committed hooks, the same at the task's commit.
    Run(Hooks),
    /// Not run, with the reason in a sentence.
    Skipped(String),
}

/// The hooks for a worktree of `checkout` at `commit`: the file committed
/// at the checkout's `HEAD`, when `commit` carries the same file.
#[must_use]
pub fn trusted(checkout: &Path, commit: &str) -> Trusted {
    let blob = |rev: &str| {
        git_out(
            checkout,
            &["rev-parse", "--verify", "-q", &format!("{rev}:{FILE}")],
        )
        .ok()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
    };
    let ours = blob("HEAD");
    let theirs = blob(commit);
    let Some(ours) = ours else {
        return match theirs {
            None => Trusted::None,
            Some(_) => Trusted::Skipped(format!(
                "This task's commit adds {FILE}, which the checked-out branch doesn't have, so its \
                 worktree hooks didn't run."
            )),
        };
    };
    if theirs.as_deref() != Some(ours.as_str()) {
        return Trusted::Skipped(format!(
            "This task's commit has a different {FILE} from the checked-out branch, so its \
             worktree hooks didn't run."
        ));
    }
    let bytes = match git_out(checkout, &["cat-file", "blob", &ours]) {
        Ok(text) => text,
        Err(why) => return Trusted::Skipped(format!("Coder could not read {FILE}: {why}")),
    };
    match serde_json::from_str::<Hooks>(&bytes) {
        Ok(hooks) if hooks.setup().is_empty() && hooks.teardown().is_empty() => Trusted::None,
        Ok(hooks) => Trusted::Run(hooks),
        Err(error) => Trusted::Skipped(format!("{FILE} is not worktree hooks: {error}")),
    }
}

/// What a setup did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ran {
    /// Commands run.
    pub commands: usize,
    /// The worktree's port.
    pub port: Option<u16>,
    /// Why the hooks didn't run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    pub ms: u64,
}

/// Run the repository's setup in `worktree`, made from `checkout` at
/// `commit`, writing its output to `log`.
///
/// Where the source guard can't be enforced, the hooks don't run and
/// [`Ran::skipped`] says why.
///
/// # Errors
/// A setup command failed or ran past the repository's time limit: a
/// sentence naming the command, with the end of its output.
pub fn setup(checkout: &Path, worktree: &Path, commit: &str, log: &Path) -> Result<Ran, String> {
    let began = Instant::now();
    let hooks = match trusted(checkout, commit) {
        Trusted::None => return Ok(Ran::default()),
        Trusted::Skipped(why) => {
            return Ok(Ran {
                skipped: Some(why),
                ms: millis(began),
                ..Ran::default()
            });
        }
        Trusted::Run(hooks) => hooks,
    };
    let commands = hooks.setup();
    if commands.is_empty() {
        return Ok(Ran::default());
    }
    // Unguarded, the hooks don't run: the start goes on and says why.
    let guard = match guard(worktree) {
        Ok(guard) => guard,
        Err(why) => {
            return Ok(Ran {
                skipped: Some(why),
                ms: millis(began),
                ..Ran::default()
            });
        }
    };
    let port = port(worktree)?;
    run(
        &guard,
        "setup",
        &commands,
        hooks.timeout(),
        checkout,
        worktree,
        port,
        log,
    )?;
    Ok(Ran {
        commands: commands.len(),
        port: Some(port),
        skipped: None,
        ms: millis(began),
    })
}

/// Run the repository's teardown in `worktree`, made from `checkout`,
/// before it is removed, writing its output to `log`. Call it before any
/// removal; keep the worktree when it fails.
///
/// The hooks are the ones committed at the checkout's `HEAD` and the
/// worktree's `HEAD` alike, so an untrusted worktree's teardown never
/// runs either. A worktree with no teardown, or one that never had a
/// port, has nothing to do.
///
/// # Errors
/// A teardown command failed or ran past the time limit.
pub fn teardown(checkout: &Path, worktree: &Path, log: &Path) -> Result<(), String> {
    let Ok(head) = git_out(worktree, &["rev-parse", "HEAD"]) else {
        return Ok(());
    };
    let Trusted::Run(hooks) = trusted(checkout, head.trim()) else {
        return Ok(());
    };
    let commands = hooks.teardown();
    if commands.is_empty() {
        return Ok(());
    }
    let Ok(guard) = guard(worktree) else {
        return Ok(());
    };
    let port = match kept_port(worktree) {
        Some(port) => port,
        None => port(worktree)?,
    };
    run(
        &guard,
        "teardown",
        &commands,
        hooks.timeout(),
        checkout,
        worktree,
        port,
        log,
    )
}

#[allow(clippy::too_many_arguments)]
fn run(
    guard: &coder_boundary::source::Guard,
    stage: &str,
    commands: &[String],
    timeout: Option<Duration>,
    checkout: &Path,
    worktree: &Path,
    port: u16,
    log: &Path,
) -> Result<(), String> {
    if let Some(parent) = log.parent() {
        let _ = crate::private::create_dir_all(parent);
    }
    let began = Instant::now();
    for command in commands {
        let output = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|error| format!("Coder could not write {}: {error}", log.display()))?;
        let errors = output
            .try_clone()
            .map_err(|error| format!("Coder could not write {}: {error}", log.display()))?;
        let mut shell = guard.command("/bin/sh", &[]);
        shell
            .arg("-c")
            .arg(command)
            .current_dir(coder_boundary::plain_path(worktree))
            .env("OPENAGENTS_SOURCE_CHECKOUT", checkout)
            .env("OPENAGENTS_WORKTREE", worktree)
            .env("OPENAGENTS_WORKTREE_PORT", port.to_string())
            .envs(guard.environment())
            .stdin(Stdio::null())
            .stdout(output)
            .stderr(errors);
        let start = format!("$ {command}\n");
        let _ = append(log, &start);
        let mut child = shell.spawn().map_err(|error| {
            format!("The worktree {stage} `{command}` could not start: {error}")
        })?;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                break status;
            }
            if timeout.is_some_and(|limit| began.elapsed() >= limit) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "The worktree {stage} `{command}` ran past the repository's {} s limit \
                     ({FILE}). Its output is in {}.",
                    timeout.map_or(0, |limit| limit.as_secs()),
                    log.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        if !status.success() {
            let code = status
                .code()
                .map_or_else(|| "was stopped".to_owned(), |code| format!("exited {code}"));
            return Err(format!(
                "The worktree {stage} `{command}` {code} ({FILE}).{}\nIts output is in {}.",
                tail(log)
                    .map(|tail| format!("\n{tail}"))
                    .unwrap_or_default(),
                log.display()
            ));
        }
    }
    Ok(())
}

/// The worktree's source guard, which must hold for any hook to run.
fn guard(worktree: &Path) -> Result<coder_boundary::source::Guard, String> {
    let refuse = |why: String| {
        format!(
            "The repository's worktree hooks ({FILE}) didn't run: Coder can't keep them from \
             writing the source checkout here ({why})."
        )
    };
    if !coder_boundary::source::APPLIES {
        return Err(refuse(format!("{} has no guard", std::env::consts::OS)));
    }
    let guard = coder_boundary::source::Guard::for_worktree(worktree)
        .map_err(|error| refuse(error.to_string()))?
        .ok_or_else(|| refuse("it is not a linked worktree".into()))?;
    guard
        .enforceable()
        .map_err(|error| refuse(error.to_string()))?;
    Ok(guard)
}

/// Where the worktree's state lives: its Git administrative directory,
/// which goes with it when it is removed.
fn state_path(worktree: &Path) -> Option<PathBuf> {
    let dir = git_out(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-dir"],
    )
    .ok()?;
    Some(PathBuf::from(dir.trim()).join(STATE))
}

#[derive(Serialize, Deserialize)]
struct State {
    port: u16,
}

fn kept_port(worktree: &Path) -> Option<u16> {
    let bytes = std::fs::read(state_path(worktree)?).ok()?;
    serde_json::from_slice::<State>(&bytes)
        .ok()
        .map(|state| state.port)
}

/// The worktree's port: the one kept for it, or a free one, kept.
fn port(worktree: &Path) -> Result<u16, String> {
    if let Some(port) = kept_port(worktree) {
        return Ok(port);
    }
    let port = TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| format!("Coder could not find a free port for the worktree: {error}"))?;
    if let Some(path) = state_path(worktree) {
        let bytes = serde_json::to_vec(&State { port }).map_err(|error| error.to_string())?;
        std::fs::write(&path, bytes)
            .map_err(|error| format!("Coder could not keep the worktree's port: {error}"))?;
    }
    Ok(port)
}

fn append(log: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?
        .write_all(text.as_bytes())
}

fn tail(log: &Path) -> Option<String> {
    let mut text = String::new();
    std::fs::File::open(log)
        .ok()?
        .read_to_string(&mut text)
        .ok()?;
    let text = text.trim_end();
    if text.is_empty() {
        return None;
    }
    let start = text.len().saturating_sub(TAIL);
    let start = (start..=text.len()).find(|at| text.is_char_boundary(*at))?;
    Some(text[start..].to_owned())
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "worktree_hooks_tests.rs"]
mod tests;
