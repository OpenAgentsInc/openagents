//! Sign in to Claude inside an environment (#11294).
//!
//! A run on a saved environment starts on a fresh computer, which has no
//! Claude login. So that a person can use their own Claude plan there
//! through Anthropic's own sign-in (`docs/cloud/claude-code-byo.md` rules
//! 1 to 4), each environment gets one computer of its own for Claude, its
//! [`Home`]: made from the saved version's image the first time the person
//! signs in, then stopped between runs with its disk kept. That stopped
//! disk is the person's own checkpoint of the environment; the login lives
//! only there (`~/.claude` in its home), never in the shared image, and
//! the person signs in once per environment version.
//!
//! The sign-in is the unmodified `claude auth login`, run under a terminal
//! on that computer (`script`, with its input from a pipe). The web shows
//! what it prints ([`View::screen`], with Anthropic's sign-in page as a
//! link) and sends what the person types ([`Sessions::send`]): the code
//! Anthropic shows goes straight into that terminal. Nothing here keeps
//! it: the screen lives in memory only, input is written to the computer
//! and removed at once, and the terminal's log is deleted when sign-in
//! ends. The only record kept is which computer is the environment's and
//! whether it is signed in.
//!
//! Computers that are no longer the person's own copy are deleted (#11296):
//! the computer of an older saved version once a newer one replaces it,
//! and a computer that never finished signing in once it has sat stopped
//! longer than the retention window ([`DEFAULT_RETENTION_DAYS`], the
//! studio's `home_retention_days`). [`sweep`] does it at studio start and
//! every hour. A computer that is still running is never deleted, and
//! neither is the signed-in computer for the environment's current saved
//! version: that is the person's own checkpoint.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where the terminal's pipe and log live on the computer, removed when
/// sign-in ends.
const DIR: &str = "/tmp/oa-claude-sign-in";
/// The longest a sign-in may stay open.
pub const SIGN_IN_SECONDS: u64 = 15 * 60;
/// The longest a computer made for sign-in may run before it stops.
const MACHINE_TTL_SECONDS: i64 = 30 * 60;
/// How much of the screen the web shows.
const SCREEN_LINES: usize = 40;
const SCREEN_BYTES: usize = 8 * 1024;
/// The most one line of input may carry.
pub const MAX_INPUT: usize = 2048;

/// Starts `claude auth login` under a terminal, reading its input from a
/// pipe that stays open, and notes its exit code when it ends.
pub const LAUNCH: &str = r#"d=/tmp/oa-claude-sign-in
rm -rf "$d" && mkdir -p "$d" && chmod 700 "$d" && mkfifo "$d/in" || exit 1
cat >"$d/run" <<'RUN'
d=/tmp/oa-claude-sign-in
PATH="$HOME/.local/bin:$HOME/bin:$PATH"; export PATH
cd "$HOME" 2>/dev/null || true
exec 3<>"$d/in"
if script --version 2>/dev/null | grep -q util-linux; then
  TERM=xterm-256color script -qfec "stty cols 400 rows 40 2>/dev/null; exec claude auth login" "$d/out" <&3 >/dev/null 2>&1
  echo $? >"$d/exit"
else
  # No util-linux script (tests): Enter arrives as a carriage return.
  mkfifo "$d/lines"
  awk 'BEGIN { RS = "\r" } { print; fflush() }' <&3 >"$d/lines" &
  a=$!
  claude auth login <"$d/lines" >"$d/out" 2>&1
  echo $? >"$d/exit"
  kill "$a" 2>/dev/null
fi
RUN
if command -v setsid >/dev/null 2>&1; then s=setsid; else s=; fi
$s nohup sh "$d/run" >/dev/null 2>&1 </dev/null &
echo started
"#;

/// Whether the terminal ended (`exit:N`) or still runs, then its log.
pub const TICK: &str = r#"d=/tmp/oa-claude-sign-in
if [ -f "$d/exit" ]; then printf 'exit:%s\n' "$(cat "$d/exit")"; else echo running; fi
cat "$d/out" 2>/dev/null || true
"#;

/// Feeds what was written to `key` into the terminal, then removes it.
pub const FEED: &str = r#"d=/tmp/oa-claude-sign-in
[ -f "$d/key" ] && cat "$d/key" >"$d/in"; rm -f "$d/key"
"#;

/// Claude Code's own answer about its login, then the terminal's files
/// removed.
pub const STATUS: &str = r#"rm -rf /tmp/oa-claude-sign-in
PATH="$HOME/.local/bin:$HOME/bin:$PATH" claude auth status 2>/dev/null || true
"#;

/// The computers a sign-in drives: Boat in production, local shells in
/// tests. Each call either finishes or says why not.
#[allow(async_fn_in_trait)]
pub trait Machines {
    /// A new computer from `image`, ready for commands.
    async fn create(&self, image: &str, size: &str) -> Result<String, String>;
    /// Start a stopped computer again and wait until it is ready.
    async fn resume(&self, id: &str) -> Result<(), String>;
    /// Run `script`; its output when it exits 0.
    async fn run(&self, id: &str, script: &str) -> Result<String, String>;
    /// Put `bytes` at `path` on the computer.
    async fn put(&self, id: &str, path: &str, bytes: &[u8]) -> Result<(), String>;
    /// Stop the computer, keeping its disk.
    async fn stop(&self, id: &str) -> Result<(), String>;
    /// The computer's state (`stopped`, `stopping`, `provisioned`,
    /// `ready`), or `None` when it no longer exists.
    async fn state(&self, id: &str) -> Result<Option<String>, String>;
    /// Delete the computer and its disk.
    async fn delete(&self, id: &str) -> Result<(), String>;
}

/// How long a stopped computer that is not the person's own signed-in
/// copy is kept, by default.
pub const DEFAULT_RETENTION_DAYS: u64 = 7;

/// Delete computer `id` if it is stopped (or already gone). `Ok(true)` when
/// it no longer exists; `Ok(false)` when it is running and was kept.
pub async fn delete_stopped<M: Machines>(machines: &M, id: &str) -> Result<bool, String> {
    match machines.state(id).await? {
        None => Ok(true),
        Some(state) if state == "stopped" => machines.delete(id).await.map(|()| true),
        Some(_) => Ok(false),
    }
}

/// Delete the computers no longer anyone's own copy (see the module
/// notes). `current` names each environment's current saved version
/// (`None` when it has none or is gone); `busy` says a sign-in is open
/// there. Returns how many computers were deleted.
pub async fn sweep<M: Machines>(
    machines: &M,
    homes: &Homes,
    current: &dyn Fn(&str) -> Option<u64>,
    busy: &dyn Fn(&str) -> bool,
    now_ms: u64,
    retention_ms: u64,
) -> usize {
    let mut deleted = 0;
    for (env, mut home) in homes.all() {
        if busy(&env) {
            continue;
        }
        let before = home.clone();
        let mut kept = Vec::new();
        for old in std::mem::take(&mut home.retired) {
            match delete_stopped(machines, &old).await {
                Ok(true) => deleted += 1,
                _ => kept.push(old),
            }
        }
        home.retired = kept;
        let own_copy = home.signed_in && current(&env) == Some(home.version);
        let replaced = current(&env) != Some(home.version);
        let expired = now_ms.saturating_sub(home.updated_ms) > retention_ms;
        if !own_copy && (replaced || expired) {
            if let Ok(true) = delete_stopped(machines, &home.sandbox).await {
                deleted += 1;
                if home.retired.is_empty() {
                    homes.forget(&env);
                    continue;
                }
                // The record stays only to remember what is left to delete.
                home.signed_in = false;
                home.sandbox.clear();
            }
        }
        if home != before {
            let _ = homes.save(&env, &home);
        }
    }
    deleted
}

/// An environment's own computer for Claude.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Home {
    /// The Boat sandbox.
    pub sandbox: String,
    /// The saved version it was made from.
    pub version: u64,
    /// Claude Code said it is signed in when sign-in ended.
    pub signed_in: bool,
    pub updated_ms: u64,
    /// Older computers of this environment still to delete (#11296): kept
    /// here only while they could not be deleted yet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retired: Vec<String>,
}

/// Where each environment's [`Home`] is recorded.
#[derive(Clone, Debug)]
pub struct Homes {
    root: PathBuf,
}

impl Homes {
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, environment: &str) -> PathBuf {
        self.root.join(format!("{environment}.json"))
    }

    pub fn read(&self, environment: &str) -> Option<Home> {
        let bytes = std::fs::read(self.path(environment)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Every environment's record.
    pub fn all(&self) -> Vec<(String, Home)> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut all: Vec<(String, Home)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let env = name.strip_suffix(".json")?.to_owned();
                let home = self.read(&env)?;
                Some((env, home))
            })
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }

    /// Remove an environment's record.
    pub fn forget(&self, environment: &str) {
        let _ = std::fs::remove_file(self.path(environment));
    }

    /// The environment's signed-in computer for `version`, if there is one.
    pub fn signed_in(&self, environment: &str, version: u64) -> Option<Home> {
        self.read(environment)
            .filter(|home| home.signed_in && home.version == version)
    }

    pub fn save(&self, environment: &str, home: &Home) -> Result<(), String> {
        std::fs::create_dir_all(&self.root)
            .map_err(|_| "The sign-in record couldn't be saved.".to_owned())?;
        let path = self.path(environment);
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(home).map_err(|e| e.to_string())?;
        std::fs::write(&temporary, bytes)
            .and_then(|()| std::fs::rename(&temporary, &path))
            .map_err(|_| "The sign-in record couldn't be saved.".to_owned())
    }
}

/// The record directory under the studio state.
pub fn root(state: &Path) -> PathBuf {
    state.join("environment-claude-sign-in")
}

/// Where a sign-in is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The computer is starting.
    Starting,
    /// Claude Code's sign-in is open in the terminal.
    Open,
    /// Signed in; the computer is stopped with the login kept.
    SignedIn,
    /// It ended without a login: why, in plain words.
    Failed(String),
}

impl Phase {
    pub fn finished(&self) -> bool {
        matches!(self, Self::SignedIn | Self::Failed(_))
    }
}

/// A sign-in as the web shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub phase: Phase,
    /// What the terminal shows, plain text, its last lines.
    pub screen: String,
    /// Anthropic's sign-in page, when the terminal printed it.
    pub link: Option<String>,
}

struct Session {
    view: View,
    input: Option<mpsc::Sender<Vec<u8>>>,
}

/// The sign-ins open in this process, one per environment.
#[derive(Clone, Default)]
pub struct Sessions {
    open: Arc<Mutex<BTreeMap<String, Session>>>,
}

impl std::fmt::Debug for Sessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sessions")
    }
}

impl Sessions {
    /// The environment's sign-in, if one was started in this process.
    pub fn view(&self, environment: &str) -> Option<View> {
        self.open
            .lock()
            .ok()?
            .get(environment)
            .map(|s| s.view.clone())
    }

    /// Type `text` and Enter into the environment's open sign-in.
    pub fn send(&self, environment: &str, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.len() > MAX_INPUT || text.chars().any(char::is_control) {
            return Err("That can't be typed into the sign-in.".into());
        }
        let open = self.open.lock().map_err(|_| "Sign-in isn't open.")?;
        let session = open
            .get(environment)
            .filter(|s| s.view.phase == Phase::Open)
            .ok_or("Sign-in isn't open. Start it again.")?;
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(b'\r');
        session
            .input
            .as_ref()
            .ok_or("Sign-in isn't open. Start it again.")?
            .send(bytes)
            .map_err(|_| "Sign-in isn't open. Start it again.".into())
    }

    /// Claim the environment for a new sign-in, unless one is running.
    /// The returned receiver carries what the person types.
    pub fn begin(&self, environment: &str) -> Result<mpsc::Receiver<Vec<u8>>, String> {
        let mut open = self.open.lock().map_err(|_| "Sign-in couldn't start.")?;
        if open
            .get(environment)
            .is_some_and(|s| !s.view.phase.finished())
        {
            return Err("Sign-in is already open for this environment.".into());
        }
        let (tx, rx) = mpsc::channel();
        open.insert(
            environment.to_owned(),
            Session {
                view: View {
                    phase: Phase::Starting,
                    screen: String::new(),
                    link: None,
                },
                input: Some(tx),
            },
        );
        Ok(rx)
    }

    pub(crate) fn set(&self, environment: &str, change: impl FnOnce(&mut View)) {
        if let Ok(mut open) = self.open.lock()
            && let Some(session) = open.get_mut(environment)
        {
            change(&mut session.view);
            if session.view.phase.finished() {
                session.input = None;
            }
        }
    }
}

/// What one sign-in needs to know.
#[derive(Clone, Debug)]
pub struct Job {
    pub environment: String,
    /// The saved version's image and number.
    pub image: String,
    pub version: u64,
    pub size: String,
}

/// Drive one sign-in to its end: start (or restart) the environment's
/// computer, open `claude auth login` there, pass what the person types,
/// and when it ends ask Claude Code whether it is signed in, then stop the
/// computer with its disk kept.
pub async fn drive<M: Machines>(
    machines: &M,
    homes: &Homes,
    sessions: &Sessions,
    job: &Job,
    input: mpsc::Receiver<Vec<u8>>,
    tick: Duration,
) {
    let env = job.environment.as_str();
    let fail = |why: &str| {
        let why = why.to_owned();
        sessions.set(env, |v| v.phase = Phase::Failed(why));
    };
    let previous = homes.read(env);
    let mut retired = previous
        .as_ref()
        .map(|home| home.retired.clone())
        .unwrap_or_default();
    let reused = match previous.as_ref() {
        Some(home) if home.version == job.version && !home.sandbox.is_empty() => {
            machines.resume(&home.sandbox).await.is_ok()
        }
        _ => false,
    };
    let sandbox = if reused {
        previous
            .as_ref()
            .map(|home| home.sandbox.clone())
            .unwrap_or_default()
    } else {
        match machines.create(&job.image, &job.size).await {
            Ok(id) => id,
            Err(_) => return fail(START_FAILED),
        }
    };
    // The computer this one replaces (an older version's, or one that
    // didn't start again) is deleted now, or later by the sweep (#11296).
    if let Some(old) = previous
        .map(|home| home.sandbox)
        .filter(|old| !old.is_empty() && *old != sandbox)
    {
        retired.push(old);
    }
    let mut kept = Vec::new();
    for old in retired {
        if !matches!(delete_stopped(machines, &old).await, Ok(true)) {
            kept.push(old);
        }
    }
    let mut home = Home {
        sandbox: sandbox.clone(),
        version: job.version,
        signed_in: false,
        updated_ms: crate::now_ms(),
        retired: kept,
    };
    if homes.save(env, &home).is_err() {
        let _ = machines.stop(&sandbox).await;
        return fail(START_FAILED);
    }
    if machines.run(&sandbox, LAUNCH).await.is_err() {
        let _ = machines.stop(&sandbox).await;
        return fail(START_FAILED);
    }
    sessions.set(env, |v| v.phase = Phase::Open);
    let started = Instant::now();
    let ended = loop {
        while let Ok(mut bytes) = input.try_recv() {
            let fed = match machines.put(&sandbox, &format!("{DIR}/key"), &bytes).await {
                Ok(()) => machines.run(&sandbox, FEED).await.map(|_| ()),
                Err(e) => Err(e),
            };
            // What was typed is kept nowhere here.
            bytes.fill(0);
            if fed.is_err() {
                sessions.set(env, |v| {
                    v.screen
                        .push_str("\n[That didn't reach the computer. Send it again.]");
                });
            }
        }
        if let Ok(out) = machines.run(&sandbox, TICK).await {
            let (state, log) = out.split_once('\n').unwrap_or((out.as_str(), ""));
            let screen = screen(log.as_bytes());
            let link = link(&screen);
            sessions.set(env, |v| {
                v.screen = screen;
                v.link = link;
            });
            if let Some(code) = state.strip_prefix("exit:") {
                break Some(code.trim().to_owned());
            }
        }
        if started.elapsed() >= Duration::from_secs(SIGN_IN_SECONDS) {
            break None;
        }
        tokio::time::sleep(tick).await;
    };
    let status = machines.run(&sandbox, STATUS).await.unwrap_or_default();
    let signed_in = logged_in(&status);
    let _ = machines.stop(&sandbox).await;
    home.signed_in = signed_in;
    home.updated_ms = crate::now_ms();
    let saved = homes.save(env, &home);
    if signed_in && saved.is_ok() {
        sessions.set(env, |v| v.phase = Phase::SignedIn);
    } else if ended.is_none() {
        fail("Sign-in timed out. Start it again.");
    } else {
        fail("Claude isn't signed in yet. Start sign-in again and finish it in the terminal.");
    }
}

const START_FAILED: &str =
    "Your computer for this environment didn't start. Try again in a minute.";

/// Whether `claude auth status` said it is logged in.
pub fn logged_in(status: &str) -> bool {
    status
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .chain(serde_json::from_str::<serde_json::Value>(status.trim()).ok())
        .any(|v| v["loggedIn"].as_bool() == Some(true))
}

/// A terminal log as plain text: escape sequences dropped, carriage
/// returns and backspaces applied, its last lines.
pub fn screen(log: &[u8]) -> String {
    let text = String::from_utf8_lossy(log);
    let mut lines: Vec<String> = vec![String::new()];
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' => {
                if chars.peek() != Some(&'\n')
                    && let Some(line) = lines.last_mut()
                {
                    line.clear();
                }
            }
            '\n' => lines.push(String::new()),
            '\u{8}' => {
                if let Some(line) = lines.last_mut() {
                    line.pop();
                }
            }
            '\t' => lines.last_mut().into_iter().for_each(|l| l.push(' ')),
            c if c.is_control() => {}
            c => lines.last_mut().into_iter().for_each(|l| l.push(c)),
        }
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    let start = lines.len().saturating_sub(SCREEN_LINES);
    let mut out = lines[start..]
        .iter()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    if out.len() > SCREEN_BYTES {
        let mut cut = out.len() - SCREEN_BYTES;
        while !out.is_char_boundary(cut) {
            cut += 1;
        }
        out = out[cut..].to_owned();
    }
    out
}

/// Anthropic's sign-in page in `screen`, if it shows one.
pub fn link(screen: &str) -> Option<String> {
    const HOSTS: [&str; 4] = [
        "https://claude.ai/",
        "https://claude.com/",
        "https://console.anthropic.com/",
        "https://platform.claude.com/",
    ];
    screen
        .split_whitespace()
        .find(|word| HOSTS.iter().any(|host| word.starts_with(host)))
        .map(|word| word.trim_end_matches(['.', ',', ')', '"', '\'']).to_owned())
        .filter(|url| url.len() <= 4096)
}

/// Boat, as the sign-in uses it.
pub struct Boat {
    pub client: boat::Client,
}

impl Machines for Boat {
    async fn create(&self, image: &str, size: &str) -> Result<String, String> {
        use boat::models::*;
        let reply = self
            .client
            .create(&CreateParams {
                body: Some(CreateSandboxRequest {
                    type_: Some(size.to_owned()),
                    ttl_seconds: boat::Nullable::Value(MACHINE_TTL_SECONDS),
                    no_env: Some(true),
                    env: Some(BTreeMap::new()),
                    from_: Some(image.to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let id = reply.sandbox.id;
        self.ready(&id).await?;
        Ok(id)
    }

    async fn resume(&self, id: &str) -> Result<(), String> {
        use boat::models::*;
        self.client
            .resume(&ResumeParams {
                sandbox_id: id.to_owned(),
                body: Some(ResumeRequest {
                    no_env: Some(true),
                    env: Some(BTreeMap::new()),
                    ttl_seconds: boat::Nullable::Value(MACHINE_TTL_SECONDS),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        self.ready(id).await
    }

    async fn run(&self, id: &str, script: &str) -> Result<String, String> {
        use boat::models::*;
        match self
            .client
            .command(&CommandParams {
                sandbox_id: id.to_owned(),
                body: CommandRequest {
                    command: script.to_owned(),
                    timeout_seconds: Some(60),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?
        {
            CommandResponseBody::Finished(reply) if reply.exit_code == Some(0) => Ok(reply.stdout),
            _ => Err("The command failed.".into()),
        }
    }

    async fn put(&self, id: &str, path: &str, bytes: &[u8]) -> Result<(), String> {
        self.client
            .write_bytes(id, path, bytes)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn stop(&self, id: &str) -> Result<(), String> {
        self.client
            .stop(&boat::models::StopParams {
                sandbox_id: id.to_owned(),
                ..Default::default()
            })
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn state(&self, id: &str) -> Result<Option<String>, String> {
        match self
            .client
            .get(&boat::models::GetParams {
                sandbox_id: id.to_owned(),
                ..Default::default()
            })
            .await
        {
            Ok(reply) => Ok(Some(reply.sandbox.state)),
            Err(boat::Error::Api(api)) if api.status.as_u16() == 404 => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    async fn delete(&self, id: &str) -> Result<(), String> {
        self.client
            .delete_sandbox(&boat::models::DeleteSandboxParams {
                sandbox_id: id.to_owned(),
                x_ascii_confirm_delete: id.to_owned(),
                ..Default::default()
            })
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

impl Boat {
    async fn ready(&self, id: &str) -> Result<(), String> {
        let opts = boat::WaitOptions {
            timeout: Duration::from_secs(900),
            ..Default::default()
        };
        self.client
            .wait_until_ready(id, &opts)
            .await
            .map_err(|e| e.to_string())?;
        self.client
            .wait_until_hydrated(id, &opts)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Claude Code as the tests need it: `auth login` prints a sign-in
    /// page and takes a code (only `good-code` signs in), `auth status`
    /// answers from the login file, and `-p` runs only when signed in.
    const FIXTURE: &str = r#"#!/bin/sh
case "$1 $2" in
"auth login")
  echo "Opening your browser to sign in."
  echo "Browser didn't open? Use the url below to sign in:"
  echo ""
  echo "https://claude.ai/oauth/authorize?code=true&client_id=fixture&state=abc"
  echo ""
  printf 'Paste code here if prompted > '
  read code
  if [ "$code" = "good-code" ]; then
    mkdir -p "$HOME/.claude" && echo '{"claudeAiOauth":{}}' >"$HOME/.claude/.credentials.json"
    echo "Login successful."
    exit 0
  fi
  echo "That code didn't work."
  exit 1;;
"auth status")
  if [ -f "$HOME/.claude/.credentials.json" ]; then
    echo '{"loggedIn":true,"authMethod":"claude.ai"}'
  else
    echo '{"loggedIn":false}'
    exit 1
  fi
  exit 0;;
esac
if [ "$1" = "-p" ]; then
  [ -f "$HOME/.claude/.credentials.json" ] || { echo "Not logged in. Please run /login"; exit 1; }
  echo "ran: $2"
  exit 0
fi
exit 2
"#;

    /// Each computer is a directory used as its home; the sign-in's
    /// fixed folder is moved inside it so tests don't share one.
    struct Local {
        root: PathBuf,
        bin: PathBuf,
        log: Mutex<Vec<String>>,
        running: Mutex<Vec<String>>,
        gone: Mutex<Vec<String>>,
    }

    impl Local {
        fn new(root: &Path) -> Self {
            let bin = root.join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            let claude = bin.join("claude");
            std::fs::write(&claude, FIXTURE).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            Self {
                root: root.to_owned(),
                bin,
                log: Mutex::new(Vec::new()),
                running: Mutex::new(Vec::new()),
                gone: Mutex::new(Vec::new()),
            }
        }
        fn home(&self, id: &str) -> PathBuf {
            self.root.join(id)
        }
        fn local(&self, id: &str, text: &str) -> String {
            text.replace(DIR, &self.home(id).join("sign-in").display().to_string())
        }
        fn note(&self, what: String) {
            self.log.lock().unwrap().push(what);
        }
        fn shell(&self, id: &str, script: &str) -> Result<String, String> {
            let out = Command::new("sh")
                .arg("-c")
                .arg(self.local(id, script))
                .env("HOME", self.home(id))
                .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
                .output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                Err("failed".into())
            }
        }
    }

    impl Machines for Local {
        async fn create(&self, image: &str, _size: &str) -> Result<String, String> {
            let id = format!("bx_{}", self.log.lock().unwrap().len());
            std::fs::create_dir_all(self.home(&id)).unwrap();
            self.note(format!("create {id} from {image}"));
            Ok(id)
        }
        async fn resume(&self, id: &str) -> Result<(), String> {
            self.note(format!("resume {id}"));
            Ok(())
        }
        async fn run(&self, id: &str, script: &str) -> Result<String, String> {
            self.shell(id, script)
        }
        async fn put(&self, id: &str, path: &str, bytes: &[u8]) -> Result<(), String> {
            std::fs::write(self.local(id, path), bytes).map_err(|e| e.to_string())
        }
        async fn stop(&self, id: &str) -> Result<(), String> {
            self.note(format!("stop {id}"));
            Ok(())
        }
        async fn state(&self, id: &str) -> Result<Option<String>, String> {
            if self.gone.lock().unwrap().iter().any(|g| g == id) {
                return Ok(None);
            }
            Ok(Some(
                if self.running.lock().unwrap().iter().any(|r| r == id) {
                    "ready"
                } else {
                    "stopped"
                }
                .into(),
            ))
        }
        async fn delete(&self, id: &str) -> Result<(), String> {
            self.note(format!("delete {id}"));
            self.gone.lock().unwrap().push(id.to_owned());
            Ok(())
        }
    }

    fn job_at(version: u64) -> Job {
        Job {
            environment: "env-1".into(),
            image: format!("env-1-v{version}"),
            version,
            size: "small".into(),
        }
    }

    async fn sign_in(machines: &Local, homes: &Homes, sessions: &Sessions, code: &str) {
        sign_in_at(machines, homes, sessions, code, 2).await;
    }

    async fn sign_in_at(
        machines: &Local,
        homes: &Homes,
        sessions: &Sessions,
        code: &str,
        version: u64,
    ) {
        let input = sessions.begin("env-1").unwrap();
        let typing = async {
            for _ in 0..400 {
                if sessions
                    .view("env-1")
                    .is_some_and(|v| v.link.is_some() && v.screen.contains("Paste code here"))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            let view = sessions.view("env-1").unwrap();
            assert_eq!(view.phase, Phase::Open);
            assert_eq!(
                view.link.as_deref(),
                Some("https://claude.ai/oauth/authorize?code=true&client_id=fixture&state=abc")
            );
            assert!(view.screen.contains("Paste code here"), "{}", view.screen);
            assert!(sessions.send("env-1", "two\nlines").is_err());
            sessions.send("env-1", code).unwrap();
        };
        let job = job_at(version);
        tokio::join!(
            drive(
                machines,
                homes,
                sessions,
                &job,
                input,
                Duration::from_millis(25)
            ),
            typing
        );
    }

    #[tokio::test]
    async fn after_signing_in_inside_the_environment_the_held_message_runs_there() {
        let dir = tempfile::tempdir().unwrap();
        let machines = Local::new(&dir.path().join("machines"));
        let homes = Homes::under(dir.path().join("homes"));
        let sessions = Sessions::default();
        assert!(homes.signed_in("env-1", 2).is_none());

        sign_in(&machines, &homes, &sessions, "good-code").await;
        assert_eq!(sessions.view("env-1").unwrap().phase, Phase::SignedIn);
        let home = homes.signed_in("env-1", 2).expect("signed in");
        // A new version needs signing in again.
        assert!(homes.signed_in("env-1", 3).is_none());
        // Made from the saved version's image, then stopped with its disk.
        assert_eq!(
            *machines.log.lock().unwrap(),
            [
                format!("create {} from env-1-v2", home.sandbox),
                format!("stop {}", home.sandbox)
            ]
        );
        // The login is only in that computer's home; the terminal's files
        // (and what was typed) are gone; the record holds no secret.
        let computer = machines.home(&home.sandbox);
        assert!(computer.join(".claude/.credentials.json").is_file());
        assert!(!computer.join("sign-in").exists());
        let record = std::fs::read_to_string(dir.path().join("homes/env-1.json")).unwrap();
        assert!(!record.contains("good-code") && !record.contains("claudeAiOauth"));
        // Typing after it ended is refused.
        assert!(sessions.send("env-1", "late").is_err());

        // The held message runs on that computer with no key at all.
        let reply = machines
            .shell(&home.sandbox, "claude -p 'Fix the failing test'")
            .unwrap();
        assert_eq!(reply.trim(), "ran: Fix the failing test");

        // Signing in again (an expired login) starts the same computer.
        sign_in(&machines, &homes, &sessions, "good-code").await;
        let log = machines.log.lock().unwrap().clone();
        assert_eq!(
            log[2..],
            [
                format!("resume {}", home.sandbox),
                format!("stop {}", home.sandbox)
            ]
        );
    }

    #[tokio::test]
    async fn a_wrong_code_leaves_the_environment_signed_out() {
        let dir = tempfile::tempdir().unwrap();
        let machines = Local::new(&dir.path().join("machines"));
        let homes = Homes::under(dir.path().join("homes"));
        let sessions = Sessions::default();
        sign_in(&machines, &homes, &sessions, "bad-code").await;
        assert!(matches!(
            sessions.view("env-1").unwrap().phase,
            Phase::Failed(_)
        ));
        assert!(homes.signed_in("env-1", 2).is_none());
        let home = homes.read("env-1").unwrap();
        assert!(machines.log.lock().unwrap().last().unwrap() == &format!("stop {}", home.sandbox));
    }

    /// #11296: a newer saved version's sign-in deletes the older version's
    /// stopped computer; a running one is kept until the sweep finds it
    /// stopped; the person's signed-in copy of the current version stays.
    #[tokio::test]
    async fn an_older_versions_computer_is_deleted_once_a_newer_one_replaces_it() {
        let dir = tempfile::tempdir().unwrap();
        let machines = Local::new(&dir.path().join("machines"));
        let homes = Homes::under(dir.path().join("homes"));
        let sessions = Sessions::default();
        sign_in_at(&machines, &homes, &sessions, "good-code", 2).await;
        let v2 = homes.signed_in("env-1", 2).unwrap().sandbox;

        // Version 2's computer is somehow still running: kept for now.
        machines.running.lock().unwrap().push(v2.clone());
        sign_in_at(&machines, &homes, &sessions, "good-code", 3).await;
        let v3 = homes.signed_in("env-1", 3).unwrap();
        assert_ne!(v3.sandbox, v2);
        assert_eq!(v3.retired, [v2.clone()]);
        assert!(
            !machines
                .log
                .lock()
                .unwrap()
                .contains(&format!("delete {v2}"))
        );

        // Once it stops, the sweep deletes it; version 3's stays.
        machines.running.lock().unwrap().clear();
        let current = |_: &str| Some(3);
        let idle = |_: &str| false;
        let now = crate::now_ms();
        let week = DEFAULT_RETENTION_DAYS * 86_400_000;
        assert_eq!(
            sweep(&machines, &homes, &current, &idle, now, week).await,
            1
        );
        assert!(
            machines
                .log
                .lock()
                .unwrap()
                .contains(&format!("delete {v2}"))
        );
        let home = homes.signed_in("env-1", 3).unwrap();
        assert!(home.retired.is_empty());
        // The person's own copy outlives the retention window.
        let later = now + 30 * 86_400_000;
        assert_eq!(
            sweep(&machines, &homes, &current, &idle, later, week).await,
            0
        );
        assert!(homes.signed_in("env-1", 3).is_some());

        // A newer version saved without signing in again: version 3's
        // computer is no longer anyone's copy and goes, record and all.
        let newer = |_: &str| Some(4);
        assert_eq!(sweep(&machines, &homes, &newer, &idle, now, week).await, 1);
        assert!(homes.read("env-1").is_none());

        // A second sign-in straight away deletes the replaced computer.
        sign_in_at(&machines, &homes, &sessions, "good-code", 4).await;
        let v4 = homes.signed_in("env-1", 4).unwrap().sandbox;
        sign_in_at(&machines, &homes, &sessions, "good-code", 5).await;
        assert!(
            machines
                .log
                .lock()
                .unwrap()
                .contains(&format!("delete {v4}"))
        );
        assert!(homes.signed_in("env-1", 5).unwrap().retired.is_empty());
    }

    /// #11296: a computer whose sign-in never finished goes after the
    /// retention window, never while running or while a sign-in is open.
    #[tokio::test]
    async fn an_unfinished_sign_ins_computer_goes_after_the_retention_window() {
        let dir = tempfile::tempdir().unwrap();
        let machines = Local::new(&dir.path().join("machines"));
        let homes = Homes::under(dir.path().join("homes"));
        let sessions = Sessions::default();
        sign_in(&machines, &homes, &sessions, "bad-code").await;
        let home = homes.read("env-1").unwrap();
        let current = |_: &str| Some(2);
        let idle = |_: &str| false;
        let week = DEFAULT_RETENTION_DAYS * 86_400_000;
        let now = home.updated_ms;
        assert_eq!(
            sweep(&machines, &homes, &current, &idle, now, week).await,
            0
        );
        let later = now + week + 1;
        let open = |_: &str| true;
        assert_eq!(
            sweep(&machines, &homes, &current, &open, later, week).await,
            0
        );
        machines.running.lock().unwrap().push(home.sandbox.clone());
        assert_eq!(
            sweep(&machines, &homes, &current, &idle, later, week).await,
            0
        );
        assert!(homes.read("env-1").is_some());
        machines.running.lock().unwrap().clear();
        assert_eq!(
            sweep(&machines, &homes, &current, &idle, later, week).await,
            1
        );
        assert!(homes.read("env-1").is_none());
    }

    #[test]
    fn the_screen_is_plain_text_with_the_sign_in_page() {
        let log = b"\x1b[?25l\x1b[1mWelcome\x1b[0m\r\nWorking.\rDone.   \r\n\x1b]0;title\x07Visit https://claude.ai/oauth/authorize?x=1.\r\nab\x08c\r\n";
        let screen = screen(log);
        assert_eq!(
            screen,
            "Welcome\nDone.\nVisit https://claude.ai/oauth/authorize?x=1.\nac"
        );
        assert_eq!(
            link(&screen).as_deref(),
            Some("https://claude.ai/oauth/authorize?x=1")
        );
        assert_eq!(link("see https://example.com/claude.ai/"), None);
        assert!(logged_in("{\"loggedIn\":true}"));
        assert!(logged_in("note\n{\"loggedIn\": true}\n"));
        assert!(!logged_in("{\"loggedIn\":false}"));
        assert!(!logged_in(""));
    }
}
