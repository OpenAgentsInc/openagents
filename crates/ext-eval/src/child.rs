//! One `coder -p` turn: its environment, its spawn inside the boundary,
//! and its end.
//!
//! The child starts from an empty environment. The harness sets `HOME`,
//! `OPENAGENTS_HOME`, `TMPDIR`, `TMP`, and `CODER_TRACE_DIR` inside the run
//! directory, a fixed `PATH` and locale, and then the pinned door
//! (`CODER_DOOR_URL`, `CODER_DOOR_KEY`, `CODER_MODEL`) as the run's door
//! proxy and its token, never the door's key. Nothing of the operator's
//! shell is inherited: no `XDG_*`, no `CODER_*`, no credential. The
//! subject arm adds `CODER_PROGRAMS` and `CODER_PROGRAM_EFFECTS`; either
//! arm may add `CODER_GUIDANCE` and its digest, and the case's `OA_EVAL_*`
//! variables.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::case::Grant;
use crate::proxy::Proxy;
use crate::sandbox::Sandbox;
use crate::signal::Cancel;

/// How long a stopped child gets between `SIGTERM` and `SIGKILL`.
pub const GRACE: Duration = Duration::from_secs(2);
/// How often the harness looks at a live child.
const POLL: Duration = Duration::from_millis(25);

/// The variable `coder` reads appended guidance from.
pub const GUIDANCE_ENV: &str = "CODER_GUIDANCE";
/// The variable naming the guidance file's digest, which `coder` checks.
pub const GUIDANCE_DIGEST_ENV: &str = "CODER_GUIDANCE_DIGEST";

/// What a child's environment is built from.
#[derive(Debug)]
pub struct ChildSpec<'a> {
    /// The run directory.
    pub sandbox: &'a Sandbox,
    /// The chat door's proxy.
    pub door: &'a Proxy,
    /// The model the door runs.
    pub model: &'a str,
    /// The decision door's proxy, when the child classifies its turns.
    pub decision: Option<&'a Proxy>,
    /// `CODER_PROGRAMS` and `CODER_PROGRAM_EFFECTS`, for the subject arm.
    pub programs: Option<(String, String)>,
    /// The guidance file's digest, when the arm appends guidance.
    pub guidance: Option<String>,
    /// The operator's grants for the case.
    pub grants: &'a BTreeSet<Grant>,
    /// The case's `OA_EVAL_*` variables, already checked.
    pub env: &'a BTreeMap<String, String>,
}

/// The directories on the child's `PATH`: the system's, and Homebrew's
/// where it exists.
#[must_use]
pub fn path() -> String {
    let mut dirs = vec!["/usr/bin", "/bin", "/usr/sbin", "/sbin"];
    for extra in ["/opt/homebrew/bin", "/usr/local/bin"] {
        if Path::new(extra).is_dir() {
            dirs.push(extra);
        }
    }
    dirs.join(":")
}

/// The child's whole environment, in order.
#[must_use]
pub fn environment(spec: &ChildSpec<'_>) -> Vec<(String, String)> {
    let sandbox = spec.sandbox;
    let show = |path: PathBuf| path.display().to_string();
    let mut env: Vec<(String, String)> = vec![
        ("HOME".into(), show(sandbox.home())),
        ("OPENAGENTS_HOME".into(), show(sandbox.openagents())),
        ("TMPDIR".into(), show(sandbox.tmp())),
        ("TMP".into(), show(sandbox.tmp())),
        ("CODER_TRACE_DIR".into(), show(sandbox.trace_dir())),
        ("PATH".into(), path()),
        ("LANG".into(), "en_US.UTF-8".into()),
        ("USER".into(), "eval".into()),
        ("LOGNAME".into(), "eval".into()),
        ("TERM".into(), "dumb".into()),
        ("NO_COLOR".into(), "1".into()),
        (
            "GIT_CEILING_DIRECTORIES".into(),
            show(sandbox.root().to_path_buf()),
        ),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("CODER_DOOR_URL".into(), spec.door.url()),
        (
            "CODER_DOOR_KEY".into(),
            spec.door.token().expose().to_string(),
        ),
        ("CODER_MODEL".into(), spec.model.to_string()),
        ("CODER_DELEGATE".into(), "off".into()),
    ];
    if !spec.grants.contains(&Grant::Exec) {
        env.push(("CODER_SHELL".into(), "off".into()));
    }
    if let Some(decision) = spec.decision {
        env.push(("TYPESAFE_BASE_URL".into(), decision.url()));
        env.push((
            "TYPESAFE_API_KEY".into(),
            decision.token().expose().to_string(),
        ));
    }
    match &spec.programs {
        Some((programs, effects)) => {
            env.push(("CODER_PROGRAMS".into(), programs.clone()));
            env.push(("CODER_PROGRAM_EFFECTS".into(), effects.clone()));
        }
        None => env.push(("CODER_PROGRAMS".into(), "none".into())),
    }
    if let Some(digest) = &spec.guidance {
        env.push((GUIDANCE_ENV.into(), show(sandbox.guidance())));
        env.push((GUIDANCE_DIGEST_ENV.into(), digest.clone()));
    }
    for (key, value) in spec.env {
        env.push((key.clone(), value.clone()));
    }
    env
}

/// How a child ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// It exited with this code, or by a signal when `None`.
    Exited(Option<i32>),
    /// The deadline passed and the harness stopped it.
    TimedOut,
    /// The operator stopped the run.
    Cancelled,
}

/// A finished child.
#[derive(Clone, Debug)]
pub struct Finished {
    /// How it ended.
    pub ended: Ended,
    /// Wall seconds from spawn to reaping.
    pub seconds: f64,
}

/// The arguments `coder` takes for one headless turn.
#[must_use]
pub fn arguments(sandbox: &Sandbox) -> Vec<String> {
    vec![
        "--prompt-file".into(),
        sandbox.root().join("prompt.md").display().to_string(),
        "--trace".into(),
        sandbox.trajectory().display().to_string(),
        "--json".into(),
    ]
}

/// Spawns `command` (already wrapped by the boundary) in its own process
/// group with `env` alone, its output to `out/stdout.jsonl` and
/// `out/stderr.txt`, and waits for it under `deadline` and `cancel`.
///
/// # Errors
///
/// Returns the I/O error when the child can't be spawned.
pub fn run(
    mut command: Command,
    sandbox: &Sandbox,
    env: &[(String, String)],
    deadline: Duration,
    cancel: &Cancel,
) -> std::io::Result<Finished> {
    let stdout = std::fs::File::create(sandbox.out().join("stdout.jsonl"))?;
    let stderr = std::fs::File::create(sandbox.out().join("stderr.txt"))?;
    command
        .env_clear()
        .envs(
            env.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .current_dir(sandbox.cwd())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    // Windows has no process group to signal: the child gets no console
    // window, and a stop ends the child alone.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    let started = Instant::now();
    let mut child = command.spawn()?;
    let group = i32::try_from(child.id()).unwrap_or(0);
    let ended = loop {
        if let Some(status) = child.try_wait()? {
            break Ended::Exited(status.code());
        }
        if cancel.cancelled() {
            stop(&mut child, group);
            break Ended::Cancelled;
        }
        if started.elapsed() >= deadline {
            stop(&mut child, group);
            break Ended::TimedOut;
        }
        std::thread::sleep(POLL);
    };
    Ok(Finished {
        ended,
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// Stops a child's whole process group: `SIGTERM`, a short grace, then
/// `SIGKILL`, and reaps it.
#[cfg(unix)]
fn stop(child: &mut std::process::Child, group: i32) {
    if group > 0 {
        // SAFETY: `killpg` takes a process group id and a signal number.
        unsafe { libc::killpg(group, libc::SIGTERM) };
    }
    let until = Instant::now() + GRACE;
    while Instant::now() < until {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(POLL);
    }
    if group > 0 {
        // SAFETY: as above; the group may already be gone, which is fine.
        unsafe { libc::killpg(group, libc::SIGKILL) };
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Stops a child on Windows, which has no signal that asks a windowless
/// program to stop: it is ended and reaped at once.
#[cfg(not(unix))]
fn stop(child: &mut std::process::Child, _group: i32) {
    let _ = child.kill();
    let _ = child.wait();
}

/// The last line of a `--json` stream that parses as the summary object.
#[must_use]
pub fn summary(stdout: &[u8]) -> Option<serde_json::Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .find(|value| value.get("outcome").is_some())
}
