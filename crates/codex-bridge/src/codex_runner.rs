//! Codex CLI process management for the bridge.
//!
//! Spawns the Codex CLI with a JSON output mode, exposes stdin/stdout/stderr
//! handles for streaming, and respawns lightweight children for subsequent
//! prompts when the previous stdin has been closed.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::info;

use crate::Opts;

/// Wrapper for a spawned Codex child process with I/O handles.
pub struct ChildWithIo {
    pub pid: u32,
    pub stdin: Option<tokio::process::ChildStdin>,
    pub stdout: Option<tokio::process::ChildStdout>,
    pub stderr: Option<tokio::process::ChildStderr>,
}

/// Spawn the long‑lived Codex process used for initial bootstrap and broadcast.
pub async fn spawn_codex(opts: &Opts) -> Result<(ChildWithIo, tokio::sync::broadcast::Sender<String>)> {
    let (bin, args) = build_bin_and_args(opts)?;
    let workdir = detect_repo_root(None);
    info!("bin" = %bin.display(), "args" = ?args, "workdir" = %workdir.display(), "msg" = "spawning codex");
    let mut command = Command::new(&bin);
    command.current_dir(&workdir).args(&args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe { command.pre_exec(|| { let res = libc::setpgid(0, 0); if res != 0 { return Err(std::io::Error::last_os_error()); } Ok(()) }); }
    let mut child = command.spawn().context("failed to spawn codex")?;
    let pid = child.id().context("child pid missing")?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, _rx) = tokio::sync::broadcast::channel::<String>(1024);
    tokio::spawn(async move { match child.wait().await { Ok(status) => tracing::info!(?status, "codex exited"), Err(e) => tracing::error!(?e, "codex wait failed"), } });
    Ok((ChildWithIo { pid, stdin, stdout, stderr }, tx))
}

/// Spawn a short‑lived Codex child for one prompt, optionally resuming a thread.
pub async fn spawn_codex_child_only_with_dir(opts: &Opts, workdir_override: Option<PathBuf>, resume_id: Option<&str>) -> Result<ChildWithIo> {
    let (bin, mut args) = build_bin_and_args(opts)?;
    if let Some(rid) = resume_id {
        let supports = cli_supports_resume(&bin);
        if supports {
            if rid == "last" { args.push("resume".into()); args.push("--last".into()); } else { args.push("resume".into()); args.push(rid.into()); }
        }
    }
    let workdir = workdir_override.unwrap_or_else(|| detect_repo_root(None));
    info!("bin" = %bin.display(), "args" = ?args, "workdir" = %workdir.display(), "msg" = "respawn codex for new prompt");
    let mut command = Command::new(&bin);
    command.current_dir(&workdir).args(&args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe { command.pre_exec(|| { let res = libc::setpgid(0, 0); if res != 0 { return Err(std::io::Error::last_os_error()); } Ok(()) }); }
    let mut child = command.spawn().context("failed to spawn codex")?;
    let pid = child.id().context("child pid missing")?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tokio::spawn(async move { match child.wait().await { Ok(status) => tracing::info!(?status, "codex exited"), Err(e) => tracing::error!(?e, "codex wait failed"), } });
    Ok(ChildWithIo { pid, stdin, stdout, stderr })
}

/// Resolve the codex binary and build default arguments for exec/json mode.
fn build_bin_and_args(opts: &Opts) -> Result<(PathBuf, Vec<String>)> {
    let bin = resolve_codex_bin(opts)?;
    let mut args: Vec<String> = Vec::new();
    // Exec mode with JSON lines to stdout
    let mut cli_args = opts
        .codex_args
        .clone()
        .unwrap_or_else(|| "exec --json".to_string());
    // Inject defaults unless already present
    let defaults = [
        "--dangerously-bypass-approvals-and-sandbox",
        "-s",
        "danger-full-access",
        "-m",
        "gpt-5",
        "-c",
        "model_reasoning_effort=high",
    ];
    for def in defaults { if !cli_args.contains(def) { args.push(def.into()); } }
    args.extend(cli_args.split_whitespace().map(|s| s.to_string()));
    // Dash to read stdin for prompt
    args.push("-".into());
    Ok((bin, args))
}

/// Locate the codex binary via options, environment, or PATH.
fn resolve_codex_bin(opts: &Opts) -> Result<PathBuf> {
    if let Some(bin) = opts.codex_bin.clone() { return Ok(bin); }
    if let Ok(env) = std::env::var("CODEX_BIN") { return Ok(PathBuf::from(env)); }
    which::which("codex").map_err(|e| anyhow::anyhow!("codex binary not found in PATH: {e}"))
}

/// Probe whether the CLI supports the `resume` subcommand.
fn cli_supports_resume(bin: &PathBuf) -> bool {
    use std::process::Command as StdCommand;
    let out = StdCommand::new(bin).arg("--help").output();
    match out { Ok(o) => String::from_utf8_lossy(&o.stdout).contains("resume"), Err(_) => false }
}

/// Detect the repository root to set Codex working directory appropriately.
fn detect_repo_root(start: Option<PathBuf>) -> PathBuf {
    fn is_repo_root(p: &Path) -> bool { p.join("expo").is_dir() && p.join("crates").is_dir() }
    let mut cur = start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let original = cur.clone();
    loop { if is_repo_root(&cur) { return cur; } if !cur.pop() { return original; } }
}

