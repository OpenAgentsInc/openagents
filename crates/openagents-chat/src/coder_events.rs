//! What a Coder task does, as one typed stream of events.
//!
//! A Coder task records everything it does as an ATIF trajectory, one
//! file per turn (`<task>.<turn>.atif.jsonl`). This module turns those
//! steps into [`CoderEvent`]s, the one stream every surface shows: the
//! `openagents` CLI prints it (text, or NDJSON under `--json`), and the
//! desktop and the phone render the same events in their own views. The
//! mapping lives here, beside the chat service they all share, so a step
//! reads the same everywhere.
//!
//! The events, in the order a turn produces them:
//!
//! - [`CoderEvent::CoderStarted`]: the task, its project and worktree, and
//!   the provider chosen, with why.
//! - [`CoderEvent::Step`]: one thing Coder did or said, a
//!   [`StepKind`] (thinking, a command, a tool call, an observation, its
//!   reply as it is written, a note), naming its ATIF step.
//! - [`CoderEvent::Output`]: a command's output, bounded.
//! - [`CoderEvent::ProviderSwitched`]: a provider refused for a usage or
//!   rate limit and the run moved on.
//! - [`CoderEvent::Progress`]: the loop's step, the time so far, and
//!   Jev's estimate of how much of the task is complete. A run has no step
//!   or time budget, so there is no bound to show.
//! - A turn ends with exactly one of [`CoderEvent::Result`],
//!   [`CoderEvent::Question`], [`CoderEvent::Approval`],
//!   [`CoderEvent::Failure`], or [`CoderEvent::Stopped`].
//!
//! [`Line`] is the envelope a stream carries: a sequence number, the task,
//! and the thread that started it. Nothing here reads a file, runs a
//! command, or holds a key; the caller feeds the steps and knows what
//! changed in the worktree.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The version of the event stream a [`Line`] belongs to.
pub const SCHEMA: &str = "openagents.coder.events.v1";
/// The most bytes of a command's output an [`Output`] event carries.
pub const MAX_OUTPUT: usize = 4 * 1024;
/// The most bytes of text a [`Step`] event carries.
pub const MAX_TEXT: usize = 2 * 1024;
/// What a person reads when a turn ended because its owner process died
/// and the task store ended the run for it (#10248).
/// What a turn's end says when the person stopped it (#10331).
pub const STOPPED_AS_ASKED: &str = "Stopped, as you asked.";

pub const OWNER_ENDED_MESSAGE: &str = "Coder's process ended unexpectedly.";

/// One thing a Coder task did, said, or became.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CoderEvent {
    CoderStarted(Started),
    Step(Step),
    Output(Output),
    ProviderSwitched(Switched),
    Question(Asked),
    Approval(Asked),
    Progress(Progress),
    /// What the run is doing while it has nothing else to show yet: its
    /// engine starting, connected, or thinking. A status line, never a
    /// row of the transcript.
    Status(Status),
    Result(Finished),
    Failure(Failure),
    Stopped(Stopped),
}

impl CoderEvent {
    /// The event's name, as `event` spells it.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            CoderEvent::CoderStarted(_) => "coder_started",
            CoderEvent::Step(_) => "step",
            CoderEvent::Output(_) => "output",
            CoderEvent::ProviderSwitched(_) => "provider_switched",
            CoderEvent::Question(_) => "question",
            CoderEvent::Approval(_) => "approval",
            CoderEvent::Progress(_) => "progress",
            CoderEvent::Status(_) => "status",
            CoderEvent::Result(_) => "result",
            CoderEvent::Failure(_) => "failure",
            CoderEvent::Stopped(_) => "stopped",
        }
    }

    /// Whether the event ends a turn.
    #[must_use]
    pub fn ends_turn(&self) -> bool {
        matches!(
            self,
            CoderEvent::Question(_)
                | CoderEvent::Approval(_)
                | CoderEvent::Result(_)
                | CoderEvent::Failure(_)
                | CoderEvent::Stopped(_)
        )
    }
}

/// What the run is doing now, from a step the engine's adapter recorded:
/// "Starting Grok Build…", "Grok Build connected · grok-4.7",
/// "Thinking…".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub turn: usize,
    pub step_id: u64,
    pub text: String,
}

/// A turn started: where, and on which provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Started {
    /// The turn, from one; an answer starts the next.
    pub turn: usize,
    /// The project's name: its checkout's folder name.
    pub project: String,
    /// The person's checkout, which Coder never writes in.
    pub checkout: String,
    /// Coder's own worktree of the checkout, where it works.
    pub worktree: String,
    /// The commit the worktree started from.
    pub base: String,
    /// The provider's word: `codex`, `claude`, `grok`, `opencode`, or `devin`.
    pub provider: String,
    pub model: String,
    /// Why this provider, in a sentence ("Codex is signed in and has
    /// capacity", "Codex has reached its usage limit until …; using Claude
    /// Code").
    pub reason: String,
    /// The routes the run fails over to, `provider:model`, in order.
    pub fallbacks: Vec<String>,
    /// `local` when this computer started it for the person at it, `host`
    /// when a host's auto-start did.
    pub via: String,
    /// Who runs the turn and why, as the same prediction the offer showed
    /// ([`Runner`]). Absent from a start recorded before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<Runner>,
}

/// Which coding agent a Coder run on this computer will use, predicted
/// before it runs from what the run itself reads: which agents are signed
/// in here, the capacity book's refusals, and a fresh usage reading. The
/// chat shows it beside an offer to run Coder and on `coder_started`, so
/// every surface says the same thing with [`Runner::text`]. It names
/// providers, models, percents, and reset times; never a credential or an
/// account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Runner {
    /// `provider` will do the work. The routes before it in preference
    /// order were passed over, each with why.
    Runs {
        /// The provider's word: `codex`, `claude`, `grok`, `opencode`, or `devin`.
        provider: String,
        model: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        passed: Vec<Passed>,
        /// The provider the person asked for (#10076), as the router's
        /// typed `engine` reading named it on the offer, when they asked
        /// for one: first in `passed`, with why, when it does not run.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requested: Option<String>,
    },
    /// None of the coding agents a run here may use (`providers`, in
    /// preference order) is signed in on this computer.
    NotSignedIn {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        providers: Vec<String>,
    },
    /// Every agent signed in here has a refusal that still holds; the
    /// earliest ends at `until`, Unix seconds.
    NoCapacity { until: Option<u64> },
}

/// A route passed over before the one that runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passed {
    /// The provider's word: `codex`, `claude`, `grok`, `opencode`, or `devin`.
    pub provider: String,
    #[serde(flatten)]
    pub why: PassedOver,
}

/// Why a route was passed over.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum PassedOver {
    /// Its agent is not signed in on this computer.
    NotSignedIn,
    /// It refused for a usage or rate limit (`kind`, as the capacity book
    /// names it) that holds until `until`, Unix seconds.
    Refused { kind: String, until: u64 },
    /// A fresh usage reading puts its fullest window at `used_percent`,
    /// at or above the threshold.
    NearLimit { used_percent: u8 },
    /// The person asked for it, and the owner's Coder settings do not
    /// list it among the engines a run here may use (#10076).
    NotAllowed,
}

impl PassedOver {
    /// Whether this is a provider's own usage window (a refusal or a
    /// reading near it). Those are the provider's, not ours: a run fails
    /// over past them silently, and no surface names them (#10120). The
    /// record keeps them.
    #[must_use]
    pub fn capacity(&self) -> bool {
        matches!(
            self,
            PassedOver::Refused { .. } | PassedOver::NearLimit { .. }
        )
    }
}

impl Passed {
    /// `Codex is not signed in here`, without a full stop.
    #[must_use]
    pub fn text(&self) -> String {
        let who = provider_name(&Value::String(self.provider.clone()));
        format!("{who} {}", self.clause())
    }

    /// Why it was passed over, without its name: `is not signed in here`.
    /// A provider's usage window is never named: it `isn't available right
    /// now` (#10120).
    #[must_use]
    pub fn clause(&self) -> String {
        match &self.why {
            PassedOver::NotSignedIn => "is not signed in here".into(),
            PassedOver::Refused { .. } | PassedOver::NearLimit { .. } => {
                "isn't available right now".into()
            }
            PassedOver::NotAllowed => "is turned off in your Coder settings".into(),
        }
    }

    /// Whether a surface says why this route was passed over: not for a
    /// provider's usage window, which a run fails over past silently.
    #[must_use]
    pub fn shown(&self) -> bool {
        !self.why.capacity()
    }
}

/// The reason a run names when the person asked for an engine (#10076):
/// "You asked for Claude Code; it isn't available right now, so Codex
/// {then}" when it does not run, "You asked for Claude Code; it {runs}"
/// when it does. No provider's usage window is named (#10120). `None` when they asked for none. `then` is the
/// verb phrase of the surface: "will do this." on an offer, "is running."
/// on a start.
#[must_use]
pub fn requested_reason(
    provider: &str,
    passed: &[Passed],
    requested: Option<&str>,
    runs: &str,
    then: &str,
) -> Option<String> {
    let requested = requested?;
    let asked = provider_name(&Value::String(requested.to_owned()));
    if requested == provider {
        return Some(format!("You asked for {asked}; it {runs}"));
    }
    let who = provider_name(&Value::String(provider.to_owned()));
    let mut why: Vec<String> = Vec::new();
    match passed.iter().find(|p| p.provider == requested) {
        Some(own) => why.push(format!("it {}", own.clause())),
        None => why.push("it can't run here now".into()),
    }
    why.extend(
        passed
            .iter()
            .filter(|p| p.provider != requested && p.shown())
            .map(Passed::text),
    );
    Some(format!(
        "You asked for {asked}; {}, so {who} {then}",
        why.join("; ")
    ))
}

impl Started {
    /// The line every surface shows when the turn starts: "Grok Build is
    /// working." (#10115). No task ID, no worktree path: those stay in
    /// `--json` and an export.
    #[must_use]
    pub fn line(&self) -> String {
        working(&self.provider)
    }

    /// Why this engine runs, only when that says something new: another
    /// engine than the one asked for, or one passed over for a reason a
    /// surface names ([`Runner::plain`]). It is drawn from the typed
    /// prediction, never the host's `reason` text, which a host from
    /// before #10120 wrote with a provider's usage limit in it.
    #[must_use]
    pub fn news(&self) -> Option<String> {
        self.runner
            .as_ref()
            .filter(|runner| !runner.plain())
            .map(|runner| runner.started("is running."))
            .filter(|news| !news.is_empty())
    }
}

/// "Grok Build is working.": a run on `provider` (its word) started.
#[must_use]
pub fn working(provider: &str) -> String {
    format!(
        "{} is working.",
        provider_name(&Value::String(provider.to_owned()))
    )
}

/// "Starting Grok Build…": a run on `provider` (its word) is starting.
#[must_use]
pub fn starting(provider: &str) -> String {
    format!(
        "Starting {}…",
        provider_name(&Value::String(provider.to_owned()))
    )
}

impl Runner {
    /// Whether saying who runs would only repeat what the person knows:
    /// the engine asked for (or the first one) runs and none was passed
    /// over.
    #[must_use]
    pub fn plain(&self) -> bool {
        match self {
            Runner::Runs {
                provider,
                passed,
                requested,
                ..
            } => {
                passed.iter().all(|p| !p.shown())
                    && requested.as_ref().is_none_or(|asked| asked == provider)
            }
            _ => false,
        }
    }

    /// Why a run started where it did: "Codex {plain}" when nothing was
    /// passed over that a surface names, "Codex is not signed in here;
    /// using Claude Code." when something was, or the person's request
    /// ([`requested_reason`]). A provider's usage window is never named:
    /// the run fails over past it silently (#10120).
    #[must_use]
    pub fn started(&self, plain: &str) -> String {
        let Runner::Runs {
            provider,
            passed,
            requested,
            ..
        } = self
        else {
            return self.text();
        };
        if let Some(reason) =
            requested_reason(provider, passed, requested.as_deref(), plain, "is running.")
        {
            return reason;
        }
        let name = provider_name(&Value::String(provider.clone()));
        let why: Vec<String> = passed
            .iter()
            .filter(|p| p.shown())
            .map(Passed::text)
            .collect();
        if why.is_empty() {
            format!("{name} {plain}")
        } else {
            format!("{}; using {name}.", why.join("; "))
        }
    }

    /// The provider that will run, when one will.
    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        match self {
            Runner::Runs { provider, .. } => Some(provider),
            _ => None,
        }
    }

    /// The sentence every surface shows: "Codex will do this.", "Codex is
    /// not signed in here; Claude Code will do this.", or why nothing can
    /// run here. A provider's usage window is never named (#10120).
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Runner::Runs {
                provider,
                passed,
                requested,
                ..
            } => {
                if let Some(text) = requested_reason(
                    provider,
                    passed,
                    requested.as_deref(),
                    "will do this.",
                    "will do this.",
                ) {
                    return text;
                }
                let who = provider_name(&Value::String(provider.clone()));
                let why: Vec<String> = passed
                    .iter()
                    .filter(|p| p.shown())
                    .map(Passed::text)
                    .collect();
                if why.is_empty() {
                    format!("{who} will do this.")
                } else {
                    format!("{}; {who} will do this.", why.join("; "))
                }
            }
            Runner::NotSignedIn { providers } => not_signed_in(providers),
            Runner::NoCapacity { until } => format!(
                "No coding agent signed in on this computer is available right now{}.",
                until
                    .map(|at| format!("; try again after {}", utc(at)))
                    .unwrap_or_default()
            ),
        }
    }
}

/// What a [`Step`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// The person's message that started the turn.
    Message,
    /// Coder's own note on what it does next and why.
    Thinking,
    /// A shell command Coder runs.
    Command,
    /// A tool an agent calls (Devin, OpenCode).
    ToolCall,
    /// What a command or tool returned, in a line.
    Observation,
    /// Coder's reply to the person, as it is written.
    Reply,
    /// Something the run says about itself, such as running without Jev.
    Note,
}

/// One thing Coder did or said, from one ATIF step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub turn: usize,
    /// The ATIF step it came from, in the turn's trajectory.
    pub step_id: u64,
    pub kind: StepKind,
    /// `user`, `agent`, or `system`, as ATIF names the source.
    pub source: String,
    /// At most [`MAX_TEXT`] bytes.
    pub text: String,
    /// For a command or a tool call, what it did and to what, typed
    /// (#10117): the shared grouping ([`crate::tool_groups`]) reads this,
    /// never `text`. Absent from other steps and from lines written before
    /// it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<Call>,
    /// For a plan update, the whole plan as it now stands; empty when the
    /// engine cleared it ([`crate::plan`]). Absent from other steps and
    /// from lines written before it existed, so older readers skip it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Vec<crate::plan::Item>>,
}

/// What a command or a tool call did, and to what (#10117).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub verb: Verb,
    /// The file, directory, pattern, URL, or command it acted on; for
    /// [`Verb::Other`], the agent's own words for the call.
    pub target: String,
    /// What the agent said a command does, when it said (Grok Build's
    /// `description`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    /// The agent reported the call failed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
}

/// What a [`Call`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    Read,
    Search,
    List,
    Fetch,
    Edit,
    Delete,
    Move,
    Run,
    Other,
}

/// A command's output, bounded to [`MAX_OUTPUT`] bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub turn: usize,
    pub step_id: u64,
    pub command: String,
    /// `None` when a signal or the deadline ended it.
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub seconds: f64,
    pub text: String,
    /// The output was longer and was cut.
    pub truncated: bool,
}

/// The run moved off a provider that refused for a usage or rate limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switched {
    pub turn: usize,
    pub step_id: u64,
    /// `provider:model`.
    pub from: String,
    /// `provider:model`, or `None` when no admitted route had capacity.
    pub to: Option<String>,
    pub reason: String,
    /// When the refusing provider said its limit resets, Unix seconds.
    pub resets_at: Option<u64>,
}

impl Switched {
    /// What a surface shows: only which engine runs now, "Claude Code is
    /// working.", or that none is available. The provider's refusal stays
    /// in `reason` and the record, never on screen (#10120).
    #[must_use]
    pub fn line(&self) -> String {
        match &self.to {
            Some(to) => working(
                to.split_once(':')
                    .map_or(to.as_str(), |(provider, _)| provider),
            ),
            None => unavailable(self.resets_at),
        }
    }
}

/// "No coding agent is available right now; try again after …": every
/// engine a run may use is out of its provider's window. It names no
/// limit (#10120).
#[must_use]
pub fn unavailable(until: Option<u64>) -> String {
    format!(
        "No coding agent is available right now{}.",
        until
            .map(|at| format!("; try again after {}", utc(at)))
            .unwrap_or_default()
    )
}

/// Coder asked the person and waits. `answer` is how to reply from the
/// terminal; the apps answer from their own composer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asked {
    pub turn: usize,
    pub text: String,
    pub answer: Option<String>,
}

/// Where the loop is. A run has no step or time budget (#10103); an
/// older line's `max_steps` is ignored when read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub turn: usize,
    /// The loop's step, from one.
    pub step: usize,
    /// Seconds since the turn started.
    pub seconds: f64,
    /// Jev's probability that the task is done, when Jev judged.
    pub done: Option<f64>,
    /// Jev's estimate of how much of the task is complete, from 0 to 1
    /// (its `complete` score), when Jev judged. Absent from lines written
    /// before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complete: Option<f64>,
}

impl Progress {
    /// The running line every surface shows: "Working · step 5
    /// · ≈40% done · 9s". The share is Jev's estimate of how much of the
    /// task is complete, left out until Jev has given one; there is never
    /// a step budget ("of N").
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "Working · step {}{} · {:.0}s",
            self.step,
            self.complete
                .map(|complete| format!(" · ≈{:.0}% done", complete.clamp(0.0, 1.0) * 100.0))
                .unwrap_or_default(),
            self.seconds
        )
    }
}

/// One file the turn changed in the worktree.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    /// `added`, `modified`, `deleted`, or `renamed`.
    pub status: String,
    /// Lines added and removed; `None` for a binary file.
    pub added: Option<u64>,
    pub removed: Option<u64>,
    /// The file's unified diff from its first hunk on, at most
    /// [`PATCH_LINES`] lines; `None` when no patch was read. Older
    /// clients ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    /// Lines of the patch left out to keep the event small.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub patch_cut: u64,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// The most lines of one file's patch a [`FileChange`] carries.
pub const PATCH_LINES: usize = 400;
/// The most patch lines one turn's [`FileChange`]s carry together.
pub const PATCH_LINES_TOTAL: usize = 2_000;

/// Gives each of `changes` its patch from `diff`, a whole unified diff
/// (`git diff`) of the same change: the lines from the file's first hunk
/// on, cut at [`PATCH_LINES`] per file and [`PATCH_LINES_TOTAL`] in all,
/// with the count of lines left out. A file `diff` does not name, or a
/// binary file, keeps no patch.
pub fn attach_patches(changes: &mut [FileChange], diff: &str) {
    let patches = split_diff(diff);
    let mut left = PATCH_LINES_TOTAL;
    for change in changes.iter_mut() {
        let Some(lines) = patches
            .iter()
            .find(|(path, _)| *path == change.path)
            .map(|(_, lines)| lines)
        else {
            continue;
        };
        if lines.is_empty() {
            continue;
        }
        let keep = lines.len().min(PATCH_LINES).min(left);
        left -= keep;
        change.patch = (keep > 0).then(|| lines[..keep].join("\n"));
        change.patch_cut = (lines.len() - keep) as u64;
    }
}

/// A whole unified diff split by file: each file's new path (its old one
/// when deleted) and its lines from the first hunk header on.
fn split_diff(diff: &str) -> Vec<(String, Vec<&str>)> {
    let mut out: Vec<(String, Vec<&str>)> = Vec::new();
    let mut in_hunks = false;
    for line in diff.lines() {
        if let Some(header) = line.strip_prefix("diff --git ") {
            let path = header
                .rsplit_once(" b/")
                .map_or(header, |(_, path)| path)
                .trim_matches('"');
            out.push((path.to_owned(), Vec::new()));
            in_hunks = false;
            continue;
        }
        let Some((_, lines)) = out.last_mut() else {
            continue;
        };
        if line.starts_with("@@") {
            in_hunks = true;
        }
        if in_hunks {
            lines.push(line);
        }
    }
    out
}

/// Where software one shell command line installs lands (#10336), or
/// `None` when the line installs nothing outside the task's worktree. A
/// bounded parse of the agent's own commands, never of what the person
/// asked.
///
/// A full-access run points the usual per-user destinations at its own
/// prefix (`pip install --user`, `npm install -g`, `cargo install`, `gem
/// install`, `go install`), so those stay with the run. A system package
/// manager, `sudo`, or `--break-system-packages` still changes the
/// computer.
#[must_use]
pub fn install_reach(line: &str) -> Option<InstallReach> {
    let words: Vec<&str> = line
        .split(|c: char| c.is_whitespace() || c == ';' || c == '&' || c == '|')
        .filter(|word| !word.is_empty())
        .collect();
    let has = |word: &str| words.contains(&word);
    let program = |name: &str| {
        words
            .iter()
            .any(|word| word.rsplit('/').next() == Some(name))
    };
    let pip = (program("pip") || program("pip3") || (has("-m") && has("pip"))) && has("install");
    let system = (program("brew") && has("install"))
        || ((program("apt") || program("apt-get") || program("dnf") || program("yum"))
            && has("install"))
        || (has("sudo") && has("install"))
        || (pip && has("--break-system-packages"));
    if system {
        return Some(InstallReach::Computer);
    }
    let run = (pip && has("--user"))
        || (program("npm") && has("install") && (has("-g") || has("--global")))
        || (program("cargo") && has("install"))
        || (program("gem") && has("install"))
        || (program("go") && has("install"));
    run.then_some(InstallReach::Run)
}

/// Where an install landed: in the run's own prefix, or on the computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallReach {
    Run,
    Computer,
}

/// The turn finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finished {
    pub turn: usize,
    /// Coder's reply: what it did and found.
    pub summary: String,
    pub files_changed: Vec<FileChange>,
    pub insertions: u64,
    pub deletions: u64,
    /// Where the changes are: Coder's worktree.
    pub worktree: String,
    /// The remote-tracking branch that contains the completed change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_to: Option<String>,
    /// The turn's ATIF trajectory file.
    pub trajectory: String,
    /// The GitHub issue the run worked, and how it landed, when the run
    /// was the issue flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueLink>,
    /// What the turn cost in micro-dollars, the engine and Jev together,
    /// when the whole of it is known. Information only, never a limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
}

/// The GitHub issue an issue-flow run worked, for a result card: its
/// link, and what the run did with it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueLink {
    /// `owner/name`.
    pub repository: String,
    pub number: u64,
    pub url: String,
    pub title: String,
    /// `landed` (pushed to the default branch), `pull_request`, `queued`
    /// (handed to the landing queue, which closes the issue when it lands),
    /// `unchanged`, `failed`, or `stopped`.
    pub outcome: String,
    /// The commits the run pushed, newest last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<String>,
    /// The pull request the run opened, in pull-request mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<String>,
    /// Whether the run closed the issue.
    pub closed: bool,
    /// Why a committed change did not land, when the outcome is `failed`
    /// at landing: `conflict` (the change conflicts with the newer
    /// branch), `push_refused` (the remote kept refusing the push), or
    /// `checks_failed` (the checks failed on the rebased change).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_landed: Option<String>,
}

impl IssueLink {
    /// The issue and what happened to it, in a line.
    #[must_use]
    pub fn line(&self) -> String {
        let what = match self.outcome.as_str() {
            "landed" => match self.commits.last() {
                Some(commit) => format!(
                    "landed {} on the default branch{}",
                    &commit[..commit.len().min(10)],
                    if self.closed { " and closed" } else { "" }
                ),
                None => "landed".to_owned(),
            },
            "pull_request" => match &self.pull_request {
                Some(url) => format!("pull request {url}"),
                None => "pull request".to_owned(),
            },
            "queued" => match self.commits.last() {
                Some(commit) => format!(
                    "queued {} to land; closes when it lands",
                    &commit[..commit.len().min(10)]
                ),
                None => "queued to land; closes when it lands".to_owned(),
            },
            "unchanged" => "nothing changed; left open".to_owned(),
            "stopped" => "stopped; left open".to_owned(),
            _ => "not landed; left open with a comment".to_owned(),
        };
        format!("Issue #{} ({}): {what}", self.number, self.url)
    }
}

/// The turn ended without finishing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub turn: usize,
    pub message: String,
    /// The task's ending, such as `no_capacity` or `loop_incomplete`.
    pub ending: Option<String>,
    /// When a provider has capacity again, for `no_capacity`.
    pub resets_at: Option<u64>,
    /// The GitHub issue the run worked, when the run was the issue flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueLink>,
}

impl Failure {
    /// What a surface shows: for `no_capacity`, only that no coding agent
    /// is available, whatever words a host from before #10120 recorded;
    /// otherwise the message.
    #[must_use]
    pub fn shown(&self) -> String {
        if self.ending.as_deref() == Some("no_capacity") {
            unavailable(self.resets_at)
        } else {
            self.message.clone()
        }
    }
}

/// The turn was stopped, by the person or the host's deadline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stopped {
    pub turn: usize,
    pub message: String,
}

/// One event as a stream carries it: `{"event": ..., "seq": ..., "task":
/// ..., "thread": ..., fields}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// From one, in the order the task produced them; a replay produces
    /// the same numbers.
    pub seq: u64,
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(flatten)]
    pub event: CoderEvent,
}

/// Maps a turn's ATIF steps, in order, to events. One mapper follows one
/// turn; [`Mapper::end`] makes the event that ends it.
#[derive(Clone, Debug, Default)]
pub struct Mapper {
    turn: usize,
    /// How to answer a question from the terminal, when there is a way.
    answer: Option<String>,
    /// The reply as streamed so far this step.
    streamed: String,
    /// The last whole reply Coder wrote this turn.
    reply: String,
    /// The last loop ending the transcript named, with its detail.
    ending: Option<(String, Option<String>)>,
    /// What a whole coding agent's turn (Devin, OpenCode, Grok Build) says
    /// about how it stopped, from the adapter's summary.
    stopped: Option<String>,
    /// The host's own fault that ended the turn, from the closing record
    /// (#10993): what a `host_fault` ending names.
    fault: Option<String>,
    started: Option<u64>,
    /// The last command this turn that ran past its deadline, and its
    /// seconds: what a `process_cleanup_unknown` ending names (#10281).
    timed_out: Option<(String, f64)>,
    /// The last line the task's own checks printed when they all passed:
    /// the answer of a turn the loop ended because its checks passed
    /// before the agent wrote one (#10331).
    checked: Option<String>,
    /// Commands this turn ran that install software outside the task's
    /// worktree, and where it landed (#10336).
    installs: Vec<(String, InstallReach)>,
}

impl Mapper {
    /// A mapper for `turn`; `answer` is the command that answers a
    /// question (`openagents chat answer --thread ID TEXT`), if any.
    #[must_use]
    pub fn new(turn: usize, answer: Option<String>) -> Self {
        Mapper {
            turn,
            answer,
            ..Mapper::default()
        }
    }

    /// Coder's last whole reply this turn.
    #[must_use]
    pub fn reply(&self) -> &str {
        &self.reply
    }

    /// Notes each line of `command` that installs software outside the
    /// worktree, once (#10336).
    fn note_installs(installs: &mut Vec<(String, InstallReach)>, command: &str) {
        for line in command.lines() {
            let line = line.trim();
            if let Some(reach) = install_reach(line)
                && !installs.iter().any(|(seen, _)| seen == line)
            {
                installs.push((line.to_owned(), reach));
            }
        }
    }

    /// The turn's answer: Coder's reply, or, when the loop ended the turn
    /// because the task's checks passed before the agent wrote one, what
    /// the checks said (#10331).
    fn answer_or_checks(&self) -> String {
        let answer = if self.reply.trim().is_empty() {
            match &self.checked {
                Some(line) if !line.is_empty() => format!("The task's checks pass: {line}"),
                Some(_) => "The task's checks pass.".to_owned(),
                None => String::new(),
            }
        } else {
            self.reply.clone()
        };
        if self.installs.is_empty() {
            return answer;
        }
        // What the turn installed and where, so the person knows (#10336).
        let listed = |reach: InstallReach| {
            self.installs
                .iter()
                .filter(|(_, held)| *held == reach)
                .map(|(command, _)| format!("`{command}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut notes = Vec::new();
        let computer = listed(InstallReach::Computer);
        if !computer.is_empty() {
            notes.push(format!(
                "Outside its worktree, this turn installed software on this computer: {computer}."
            ));
        }
        let run = listed(InstallReach::Run);
        if !run.is_empty() {
            notes.push(format!(
                "This turn installed software just for this run, gone when it ends: {run}."
            ));
        }
        let note = notes.join(" ");
        if answer.trim().is_empty() {
            note
        } else {
            format!("{}\n\n{note}", answer.trim_end())
        }
    }

    /// The events one step makes, in the ATIF document form
    /// (`crates/atif`'s `document`) or the log's record form: `extra` or
    /// `extensions` carry what Coder noted.
    pub fn step(&mut self, step: &Value) -> Vec<CoderEvent> {
        let id = step["step_id"].as_u64().unwrap_or(0);
        let source = step["source"]
            .as_str()
            .unwrap_or("system")
            .to_ascii_lowercase();
        let extra = step
            .get("extra")
            .filter(|extra| extra.is_object())
            .or_else(|| step.get("extensions"))
            .cloned()
            .unwrap_or(Value::Null);
        let at = step["timestamp"]
            .as_str()
            .and_then(parse_iso)
            .or_else(|| step["at"].as_u64());
        if self.started.is_none() {
            self.started = at;
        }
        let seconds = |fallback: Option<f64>| {
            fallback.unwrap_or_else(|| {
                self.started
                    .zip(at)
                    .map_or(0.0, |(start, at)| at.saturating_sub(start) as f64 / 1000.0)
            })
        };
        let make = |kind: StepKind, text: &str| {
            // The loop records the model's words and commands as its own
            // observations; they are the agent's.
            let source = match kind {
                StepKind::Thinking | StepKind::Command | StepKind::ToolCall | StepKind::Reply => {
                    "agent".to_owned()
                }
                StepKind::Message => source.clone(),
                StepKind::Observation | StepKind::Note => {
                    if source == "agent" {
                        "system".to_owned()
                    } else {
                        source.clone()
                    }
                }
            };
            CoderEvent::Step(Step {
                turn: self.turn,
                step_id: id,
                kind,
                source,
                text: bounded(text, MAX_TEXT).0,
                call: None,
                plan: None,
            })
        };
        let with_call = |event: CoderEvent, call: Call| match event {
            CoderEvent::Step(step) => CoderEvent::Step(Step {
                call: Some(call),
                ..step
            }),
            other => other,
        };
        // A plan update: one note naming the count, carrying the whole plan.
        let planned = |items: Vec<crate::plan::Item>| match make(
            StepKind::Note,
            &crate::plan::Summary::of(&items).line(),
        ) {
            CoderEvent::Step(step) => CoderEvent::Step(Step {
                plan: Some(items),
                ..step
            }),
            other => other,
        };
        let mut events = Vec::new();
        if extra
            .pointer("/admission/grant/adapter_configuration")
            .is_some_and(Value::is_object)
        {
            return events;
        }
        if let Some(summary) = extra.get("adapter_summary").and_then(Value::as_object) {
            self.stopped = summary
                .values()
                .filter(|agent| agent.get("engine").is_some())
                .find_map(|agent| agent.get("stopped").and_then(Value::as_str))
                .map(str::to_owned);
            self.fault = extra
                .get("host_fault")
                .and_then(Value::as_str)
                .map(str::to_owned);
            return events;
        }
        if source == "user" {
            if let Some(text) = step["message"].as_str() {
                events.push(make(StepKind::Message, text));
            }
            return events;
        }
        if let Some(switch) = extra
            .get("route_switch")
            .or_else(|| extra.get("route_exhausted"))
        {
            let to = extra.get("route_switch").map(|_| route(&switch["to"]));
            let refusal = &switch["refusal"];
            let resets_at = refusal["resets_at"]
                .as_u64()
                .or_else(|| switch["resets_at"].as_u64())
                .or_else(|| refusal["until"].as_u64());
            let kind = refusal["kind"]
                .as_str()
                .unwrap_or("limit")
                .replace('_', " ");
            let from = route(&switch["from"]);
            let reason = format!(
                "{} refused for a {kind}{}",
                provider_name(&switch["from"]["provider"]),
                resets_at
                    .map(|at| format!(" until {}", utc(at)))
                    .unwrap_or_default()
            );
            events.push(CoderEvent::ProviderSwitched(Switched {
                turn: self.turn,
                step_id: id,
                from,
                to,
                reason,
                resets_at,
            }));
            return events;
        }
        // An Agent Client Protocol engine's plan, as its adapter noted it.
        if let Some(items) = crate::plan::noted(&extra) {
            events.push(planned(items));
            return events;
        }
        if extra.get("decision_unavailable").is_some() || extra.get("routes_unavailable").is_some()
        {
            if let Some(text) = step["message"].as_str() {
                events.push(make(StepKind::Note, text));
            }
            return events;
        }
        if let Some(text) = status(&extra) {
            events.push(CoderEvent::Status(Status {
                turn: self.turn,
                step_id: id,
                text,
            }));
            return events;
        }
        if let Some(record) = extra.get("microcoder") {
            let event = &record["event"];
            let at_seconds = record["seconds"].as_f64();
            match event["event"].as_str() {
                Some("judged") => {
                    let answer = |list: &str, id: &str| {
                        event["judgment"][list]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|pair| pair[0] == id)
                            .and_then(|pair| pair[1].as_f64())
                    };
                    events.push(CoderEvent::Progress(Progress {
                        turn: self.turn,
                        step: event["step"]
                            .as_u64()
                            .and_then(|n| usize::try_from(n).ok())
                            .unwrap_or(0),
                        seconds: seconds(at_seconds),
                        done: answer("answers", "done"),
                        complete: answer("scores", "complete"),
                    }));
                }
                Some("replying") => {
                    if let Some(text) = event["text"].as_str() {
                        self.streamed.push_str(text);
                        events.push(make(StepKind::Reply, text));
                    }
                }
                Some("generated") => {
                    let action = &event["generated"]["action"];
                    let action = action.get("Ok").unwrap_or(action);
                    if action.get("Err").is_some() || !action.is_object() {
                        self.streamed.clear();
                        return events;
                    }
                    if let Some(rationale) = action["rationale"].as_str()
                        && !rationale.trim().is_empty()
                    {
                        events.push(make(StepKind::Thinking, rationale));
                    }
                    let reply = action["reply"].as_str().unwrap_or("");
                    if !reply.trim().is_empty() {
                        // What streamed already showed; show only the rest.
                        let shown = event["reply_streamed"]
                            .as_u64()
                            .and_then(|n| usize::try_from(n).ok())
                            .filter(|n| reply.is_char_boundary(*n))
                            .unwrap_or(0);
                        let rest = &reply[shown..];
                        if !rest.trim().is_empty() {
                            events.push(make(StepKind::Reply, rest));
                        }
                        self.reply = reply.to_owned();
                    }
                    self.streamed.clear();
                    for command in action["commands"].as_array().into_iter().flatten() {
                        if let Some(command) = command.as_str() {
                            Self::note_installs(&mut self.installs, command);
                            events.push(with_call(
                                make(StepKind::Command, command),
                                Call {
                                    verb: Verb::Run,
                                    target: command.to_owned(),
                                    about: None,
                                    failed: false,
                                },
                            ));
                        }
                    }
                }
                Some("ran") => {
                    let result = &event["result"];
                    let command = result["command"].as_str().unwrap_or("").to_owned();
                    let exit = result["exit"].as_i64().and_then(|n| i32::try_from(n).ok());
                    let timed_out = result["timed_out"].as_bool().unwrap_or(false);
                    let took = result["seconds"].as_f64().unwrap_or(0.0);
                    if timed_out {
                        self.timed_out = Some((command.clone(), took));
                    }
                    // The command itself is the step before; `output` names it.
                    let summary = match (exit, timed_out) {
                        (_, true) => format!("timed out after {took:.1}s"),
                        (Some(code), _) => format!("exit {code} in {took:.1}s"),
                        (None, _) => format!("stopped after {took:.1}s"),
                    };
                    events.push(make(StepKind::Observation, &summary));
                    let (text, truncated) =
                        bounded(result["output"].as_str().unwrap_or(""), MAX_OUTPUT);
                    events.push(CoderEvent::Output(Output {
                        turn: self.turn,
                        step_id: id,
                        command,
                        exit,
                        timed_out,
                        seconds: took,
                        text,
                        truncated,
                    }));
                }
                Some("tested") => {
                    let results = event["results"].as_array();
                    let passed = results.is_some_and(|results| {
                        !results.is_empty() && results.iter().all(|result| result["exit"] == 0)
                    });
                    self.checked = passed.then(|| {
                        results
                            .into_iter()
                            .flatten()
                            .filter_map(|result| result["output"].as_str())
                            .flat_map(str::lines)
                            .map(str::trim)
                            .rfind(|line| !line.is_empty())
                            .unwrap_or_default()
                            .to_owned()
                    });
                }
                Some("ended") => {
                    let ending = &event["outcome"]["ending"];
                    let reason = ending["reason"]
                        .as_str()
                        .or_else(|| ending.as_str())
                        .unwrap_or("ended")
                        .to_owned();
                    let detail = match &ending["detail"] {
                        Value::String(text) => Some(text.clone()),
                        Value::Null => None,
                        other => Some(other.to_string()),
                    };
                    self.ending = Some((reason, detail));
                }
                _ => {}
            }
            return events;
        }
        // A whole coding agent's turn (Devin, OpenCode): its tool calls,
        // their results, and its words.
        if source == "agent" {
            // A decision-model call (a Jev judgment such as
            // `openagents.microcoder.judge.v1`) is evidence for the
            // trajectory, never a row of the person's transcript (#10073);
            // a plan call's acknowledgement is not one either.
            let decisions: Vec<&str> = step["tool_calls"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|call| is_decision_call(call) || crate::plan::called(call).is_some())
                .filter_map(|call| call["tool_call_id"].as_str())
                .collect();
            let results: Vec<&Value> = step
                .pointer("/observation/results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .collect();
            for call in step["tool_calls"].as_array().into_iter().flatten() {
                if is_decision_call(call) {
                    continue;
                }
                // `TodoWrite` or `update_plan`: the plan, not a tool row.
                if let Some(items) = crate::plan::called(call) {
                    events.push(planned(items));
                    continue;
                }
                // An engine's own shell calls (Claude, Codex, Grok Build)
                // install software too (#10336).
                if let Some(command) = ["command", "cmd"]
                    .iter()
                    .find_map(|name| call["arguments"][*name].as_str())
                {
                    Self::note_installs(&mut self.installs, command);
                }
                let (line, mut typed) = tool_call(call, &step["extra"]);
                typed.failed = results.iter().any(|result| {
                    result["source_call_id"] == call["tool_call_id"]
                        && result.pointer("/extra/status").and_then(Value::as_str) == Some("failed")
                });
                events.push(with_call(make(StepKind::ToolCall, &line), typed));
            }
            for result in step
                .pointer("/observation/results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|result| {
                    !result["source_call_id"]
                        .as_str()
                        .is_some_and(|id| decisions.contains(&id))
                })
            {
                let content = match &result["content"] {
                    Value::String(text) => text.clone(),
                    Value::Null => continue,
                    other => other.to_string(),
                };
                events.push(make(StepKind::Observation, &content));
            }
            if let Some(reasoning) = step["reasoning_content"].as_str() {
                events.push(make(StepKind::Thinking, reasoning));
            }
            if let Some(text) = step["message"].as_str()
                && !text.trim().is_empty()
                && step["tool_calls"].is_null()
            {
                self.reply = text.to_owned();
                events.push(make(StepKind::Reply, text));
            }
        }
        events
    }

    /// The event that ends the turn, from the task's `ending` (the owner's
    /// result record). `changes` is what changed in `worktree`, and
    /// `trajectory` the turn's ATIF file; `resets_at` names when a provider
    /// has capacity again.
    #[must_use]
    pub fn end(
        &self,
        ending: &str,
        changes: Vec<FileChange>,
        worktree: &str,
        trajectory: &str,
        resets_at: Option<u64>,
    ) -> CoderEvent {
        let turn = self.turn;
        match ending {
            "model_finished" | "checks_passed" => {
                let insertions = changes.iter().filter_map(|c| c.added).sum();
                let deletions = changes.iter().filter_map(|c| c.removed).sum();
                CoderEvent::Result(Finished {
                    turn,
                    summary: self.answer_or_checks(),
                    files_changed: changes,
                    insertions,
                    deletions,
                    worktree: worktree.to_owned(),
                    trajectory: trajectory.to_owned(),
                    issue: None,
                    pushed_to: None,
                    cost_microusd: None,
                })
            }
            "asked_question" => CoderEvent::Question(Asked {
                turn,
                text: self.reply.clone(),
                answer: self.answer.clone(),
            }),
            "asked_approval" => CoderEvent::Approval(Asked {
                turn,
                text: self.reply.clone(),
                answer: self.answer.clone(),
            }),
            "cancelled_or_host_refusal" | "cancelled" => CoderEvent::Stopped(Stopped {
                turn,
                message: "Coder stopped: the task was stopped, or its host refused to go on.".into(),
            }),
            // The agent ended the turn itself: after the host refused a
            // tool it asked for, or reporting `cancelled` with no stop from
            // the host. Neither is a stop by the person (#10092).
            "engine_stopped_after_refusal" | "engine_cancelled" => CoderEvent::Stopped(Stopped {
                turn,
                message: format!(
                    "Coder stopped: {}",
                    self.stopped.clone().unwrap_or_else(|| if ending
                        == "engine_cancelled"
                    {
                        "the coding agent ended the turn as cancelled on its own; nobody stopped the task.".to_owned()
                    } else {
                        "the coding agent stopped after the host refused a tool it asked to run.".to_owned()
                    })
                ),
            }),
            // The host ended the turn for a fault of its own; nobody
            // stopped the task (#10993).
            "host_fault" => CoderEvent::Failure(Failure {
                turn,
                message: match &self.fault {
                    Some(fault) => format!(
                        "Coder stopped before finishing: {}. Nobody stopped the task.",
                        bounded(fault.trim_end_matches('.'), 300).0
                    ),
                    None => "Coder stopped before finishing: something on its host failed. Nobody stopped the task.".to_owned(),
                },
                ending: Some(ending.into()),
                resets_at: None,
                issue: None,
            }),
            // A command's processes weren't confirmed gone, so the host
            // ended the turn; nobody stopped the task (#10281).
            "process_cleanup_unknown" => CoderEvent::Failure(Failure {
                turn,
                message: match &self.timed_out {
                    Some((command, seconds)) => format!(
                        "Coder stopped before finishing: `{}` ran past its deadline ({seconds:.0}s) and its processes could not be confirmed gone, so Coder ended the turn rather than run beside them. Nobody stopped the task.",
                        bounded(command.lines().next().unwrap_or(""), 120).0
                    ),
                    None => "Coder stopped before finishing: a command's processes could not be confirmed gone, so Coder ended the turn rather than run beside them. Nobody stopped the task.".to_owned(),
                },
                ending: Some(ending.into()),
                resets_at: None,
                issue: None,
            }),
            "no_capacity" => CoderEvent::Failure(Failure {
                turn,
                message: unavailable(resets_at),
                ending: Some(ending.into()),
                resets_at,
                issue: None,
            }),
            // The task store ended a run whose owner process died
            // (`coder::task::owner::OWNER_ENDED`, #10248).
            "owner_process_ended" => CoderEvent::Failure(Failure {
                turn,
                message: OWNER_ENDED_MESSAGE.into(),
                ending: Some(ending.into()),
                resets_at: None,
                issue: None,
            }),
            other => {
                let why = match &self.ending {
                    Some((reason, Some(detail))) => {
                        format!("{} ({})", reason.replace('_', " "), bounded(detail, 300).0)
                    }
                    Some((reason, None)) => reason.replace('_', " "),
                    None => match &self.stopped {
                        Some(stopped) => stopped.trim_end_matches('.').to_owned(),
                        None => other.replace('_', " "),
                    },
                };
                CoderEvent::Failure(Failure {
                    turn,
                    message: format!("Coder stopped before finishing: {why}."),
                    ending: Some(other.into()),
                    resets_at: None,
                    issue: None,
                })
            }
        }
    }
}

/// The result of a task's last turn, from its events, once that turn has
/// ended with a result, a failure, or a stop (#10094): the context a
/// follow-up carries to the chat ([`crate::router::CoderRun`]). `None`
/// while the turn runs, when it ended by asking the person, and for a task
/// with no ended turn. The engine is the one the turn started on, or the
/// one it switched to last.
#[must_use]
pub fn run_result(lines: &[Line]) -> Option<crate::router::CoderRun> {
    use crate::router::{CoderRun, RunEnding, RunFile};
    let ended = lines.iter().rposition(|line| line.event.ends_turn())?;
    // A later start is a turn that has not ended.
    if lines[ended + 1..]
        .iter()
        .any(|line| matches!(line.event, CoderEvent::CoderStarted(_)))
    {
        return None;
    }
    let (turn, ending, summary, files) = match &lines[ended].event {
        CoderEvent::Result(result) => (
            result.turn,
            RunEnding::Finished,
            result.summary.clone(),
            result
                .files_changed
                .iter()
                .map(|file| RunFile {
                    path: file.path.clone(),
                    status: file.status.clone(),
                })
                .collect(),
        ),
        CoderEvent::Failure(failure) => {
            (failure.turn, RunEnding::Failed, failure.shown(), Vec::new())
        }
        CoderEvent::Stopped(stopped) => (
            stopped.turn,
            RunEnding::Stopped,
            stopped.message.clone(),
            Vec::new(),
        ),
        _ => return None,
    };
    let this_turn = &lines[..ended];
    let mut engine = None;
    let mut model = None;
    let mut commands = Vec::new();
    for line in this_turn {
        match &line.event {
            CoderEvent::CoderStarted(started) if started.turn == turn => {
                engine = Some(started.provider.clone());
                model = Some(started.model.clone());
            }
            CoderEvent::ProviderSwitched(switch) if switch.turn == turn => {
                if let Some((provider, to)) = switch.to.as_deref().and_then(|to| to.split_once(':'))
                {
                    engine = Some(provider.to_owned());
                    model = Some(to.to_owned());
                }
            }
            CoderEvent::Step(step) if step.turn == turn && step.kind == StepKind::Command => {
                commands.push(step.text.clone());
            }
            _ => {}
        }
    }
    Some(CoderRun {
        ending,
        turn,
        engine,
        model,
        summary,
        files,
        commands,
    })
}

/// A compact line for a terminal, or `None` for an event a terminal shows
/// no line for.
#[must_use]
pub fn text(event: &CoderEvent) -> Option<String> {
    let first = |text: &str| -> String {
        let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        bounded(line.trim(), 200).0
    };
    Some(match event {
        CoderEvent::CoderStarted(s) => match s.news() {
            Some(news) => format!("{}\n{news}", s.line()),
            None => s.line(),
        },
        CoderEvent::Step(step) => match step.kind {
            StepKind::Message => return None,
            StepKind::Thinking => format!("  · {}", first(&step.text)),
            StepKind::Command => format!("  $ {}", first(&step.text)),
            StepKind::ToolCall => format!("  > {}", first(&step.text)),
            StepKind::Observation => format!("    {}", first(&step.text)),
            StepKind::Reply => return None,
            StepKind::Note => format!("  ! {}", first(&step.text)),
        },
        CoderEvent::Output(output) => {
            let lines: Vec<&str> = output.text.lines().collect();
            let tail = lines.len().saturating_sub(3);
            lines[tail..]
                .iter()
                .map(|line| format!("    | {}", bounded(line, 160).0))
                .collect::<Vec<_>>()
                .join("\n")
        }
        CoderEvent::ProviderSwitched(s) => format!("  ~ {}", s.line()),
        CoderEvent::Progress(p) => format!(
            "  [step {}{}, {:.0}s]",
            p.step,
            p.complete
                .map(|c| format!(", ≈{:.0}% done", c.clamp(0.0, 1.0) * 100.0))
                .unwrap_or_default(),
            p.seconds
        ),
        CoderEvent::Status(status) => format!("  {}", status.text),
        CoderEvent::Question(a) | CoderEvent::Approval(a) => format!(
            "Coder asks: {}{}",
            a.text.trim(),
            a.answer
                .as_ref()
                .map(|how| format!("\nanswer: {how}"))
                .unwrap_or_default()
        ),
        CoderEvent::Result(r) => {
            let mut out = format!(
                "Coder finished turn {}: {} file{} changed, +{} -{} in {}",
                r.turn,
                r.files_changed.len(),
                if r.files_changed.len() == 1 { "" } else { "s" },
                r.insertions,
                r.deletions,
                r.worktree
            );
            for file in &r.files_changed {
                out.push_str(&format!(
                    "\n  {} {} (+{} -{})",
                    file.status,
                    file.path,
                    file.added.map_or("?".into(), |n| n.to_string()),
                    file.removed.map_or("?".into(), |n| n.to_string())
                ));
            }
            if let Some(issue) = &r.issue {
                out.push_str(&format!("\n{}", issue.line()));
            }
            out
        }
        CoderEvent::Failure(f) => match &f.issue {
            Some(issue) => format!("{}\n{}", f.shown(), issue.line()),
            None => f.shown(),
        },
        CoderEvent::Stopped(s) => s.message.clone(),
    })
}

/// `provider:model` of a route as a grant or transcript names it.
fn route(value: &Value) -> String {
    match (value["provider"].as_str(), value["model"].as_str()) {
        (Some(provider), Some(model)) => format!("{provider}:{model}"),
        (Some(provider), None) => provider.into(),
        _ => value.as_str().unwrap_or("unknown").into(),
    }
}

/// How a person signs in to `provider`'s coding agent, in words they can
/// act on.
#[must_use]
pub fn sign_in_step(provider: &str) -> &'static str {
    match provider {
        "codex" => "run `codex login`",
        "claude" => "run `claude` and log in",
        "devin" => "run `devin auth login`",
        "opencode" => "install `opencode` and run `opencode auth login`",
        "grok" => "run `grok` and log in, or set XAI_API_KEY",
        _ => "sign in to it",
    }
}

/// Why Coder cannot run here: none of `providers` (the agents Coder may
/// use, in order) is signed in. One sentence every client shows, with how
/// to sign in to each (#10314).
#[must_use]
pub fn not_signed_in(providers: &[String]) -> String {
    let named = |provider: &String| {
        format!(
            "{} ({})",
            provider_name(&Value::String(provider.clone())),
            sign_in_step(provider)
        )
    };
    match providers {
        [] => format!(
            "No coding agent is signed in on this computer. Sign in to one, then ask again: {}; {}.",
            named(&"codex".to_owned()),
            named(&"claude".to_owned())
        ),
        [one] => format!(
            "{} is not signed in on this computer. Sign in ({}), then ask again.",
            provider_name(&Value::String(one.clone())),
            sign_in_step(one)
        ),
        many => format!(
            "No coding agent Coder can use is signed in on this computer. Sign in to one, \
             then ask again: {}.",
            many.iter().map(named).collect::<Vec<_>>().join("; ")
        ),
    }
}

/// The provider's product name.
#[must_use]
pub fn provider_name(provider: &Value) -> String {
    match provider.as_str() {
        Some("codex") => "Codex".into(),
        Some("claude") => "Claude Code".into(),
        Some("devin") => "Devin".into(),
        Some("opencode") => "OpenCode".into(),
        Some("grok") => "Grok Build".into(),
        Some(other) => other.into(),
        None => "The provider".into(),
    }
}

/// The status an adapter's effect step says, if it says one: an engine's
/// session opening ("Starting Grok Build…") and opened ("Grok Build
/// connected · grok-4.7"), and a prompt or model request going out
/// ("Thinking…"). Commands and decisions say nothing here: their own rows
/// show them.
fn status(extra: &Value) -> Option<String> {
    let engine = |kind: &str, suffix: &str| {
        kind.strip_suffix(suffix)
            .map(|provider| provider_name(&Value::String(provider.to_owned())))
    };
    if let Some(kind) = extra.pointer("/effect/kind").and_then(Value::as_str) {
        if let Some(engine) = engine(kind, "_session") {
            return Some(format!("Starting {engine}…"));
        }
        if kind.ends_with("_prompt") || kind.ends_with("_request") || kind == "generation" {
            return Some("Thinking…".to_owned());
        }
        return None;
    }
    let kind = extra
        .pointer("/effect_result/kind")
        .and_then(Value::as_str)?;
    let engine = engine(kind, "_session")?;
    let result = &extra["effect_result"]["result"];
    if result.get("error").is_some_and(|error| !error.is_null()) {
        return None;
    }
    Some(
        match result["model"].as_str().filter(|model| !model.is_empty()) {
            Some(model) => format!("{engine} connected · {model}"),
            None => format!("{engine} connected"),
        },
    )
}

/// At most `max` bytes of `text`, cut at a character boundary, and whether
/// it was cut.
fn bounded(text: &str, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text.to_owned(), false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…", &text[..end]), true)
}

/// Unix seconds as `2026-10-04 13:27 UTC`.
#[must_use]
pub fn utc(seconds: u64) -> String {
    let iso = atif::iso(seconds.saturating_mul(1000));
    // `2026-10-04T13:27:03.000Z`
    match (iso.get(..10), iso.get(11..16)) {
        (Some(day), Some(time)) => format!("{day} {time} UTC"),
        _ => iso,
    }
}

/// Milliseconds of an ISO time `crates/atif` wrote (`…T…Z`).
fn parse_iso(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let millis = text
        .get(20..23)
        .and_then(|part| part.parse::<i64>().ok())
        .unwrap_or(0);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let ms = ((days * 86_400 + hour * 3600 + minute * 60 + second) * 1000) + millis;
    u64::try_from(ms).ok()
}

/// The longest step line a tool call becomes, in characters.
const TOOL_LINE_CHARS: usize = 120;

/// A coding agent's tool call (Devin, OpenCode, Grok Build) as a readable
/// step line, never its raw JSON (#10113): `Read README.md`, `Listed the
/// project`, `Ran cargo test`. It reads the call's typed arguments first, by
/// the field names the agents use, with the ACP tool kind the recorder kept
/// (`extra.<engine>_tool.kind`), then the title the agent showed beside the
/// call (`extra.purpose`), and else names the tool.
#[cfg(test)]
fn tool_line(call: &Value, extra: &Value) -> String {
    tool_call(call, extra).0
}

/// [`tool_line`]'s line with the call typed (#10117): the same fields
/// read once, so the line and the [`Call`] always agree.
fn tool_call(call: &Value, extra: &Value) -> (String, Call) {
    let arguments = &call["arguments"];
    let field = |names: &[&str]| {
        names
            .iter()
            .find_map(|name| arguments[*name].as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    let kind = extra
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(key, _)| key.ends_with("_tool"))
        .find_map(|(_, value)| value["kind"].as_str())
        .unwrap_or("");
    let file = field(&["target_file", "file_path", "filePath", "path"]).map(shown);
    let first = |text: &str| text.lines().next().unwrap_or(text).to_owned();
    let typed = if let Some(dir) = field(&["target_directory", "directory", "dir_path"]) {
        Some((
            match dir {
                "." | "./" => "Listed the project".to_owned(),
                dir => format!("Listed {}", shown(dir)),
            },
            Verb::List,
            match dir {
                "./" => ".".to_owned(),
                dir => shown(dir),
            },
        ))
    } else if let Some(command) = field(&["command", "cmd"]) {
        Some((
            format!("Ran {}", first(command)),
            Verb::Run,
            command.to_owned(),
        ))
    } else if let Some(pattern) = field(&["pattern", "query", "regex"]) {
        Some((
            format!("Searched for {pattern}"),
            Verb::Search,
            pattern.to_owned(),
        ))
    } else if let Some(url) = field(&["url"]) {
        Some((format!("Fetched {url}"), Verb::Fetch, url.to_owned()))
    } else {
        file.map(|file| {
            let (word, verb) = match kind {
                "edit" => ("Edited", Verb::Edit),
                "delete" => ("Deleted", Verb::Delete),
                "move" => ("Moved", Verb::Move),
                _ => ("Read", Verb::Read),
            };
            (format!("{word} {file}"), verb, file)
        })
    };
    let title = extra["purpose"]
        .as_str()
        .and_then(|text| text.lines().next())
        .map(|text| text.replace('`', "").trim().to_owned())
        .filter(|text| !text.is_empty() && !text.starts_with(['{', '[']));
    let name = call["function_name"]
        .as_str()
        .filter(|name| *name != "other" && !name.is_empty() && name.len() <= 40);
    // A call whose arguments are already words (`read a.rs`) reads as is.
    let plain = arguments
        .as_str()
        .and_then(|text| text.lines().next())
        .map(str::trim)
        .filter(|text| !text.is_empty() && !text.starts_with(['{', '[']))
        .map(|text| format!("{} {text}", name.unwrap_or("tool")));
    let clean = |line: String| -> String {
        let line: String = line.chars().filter(|ch| !ch.is_control()).collect();
        match line.char_indices().nth(TOOL_LINE_CHARS) {
            Some((at, _)) => format!("{}…", &line[..at]),
            None => line,
        }
    };
    let about = field(&["description"]).map(|text| clean(first(text)));
    match typed {
        Some((line, verb, target)) => {
            let target = match verb {
                Verb::Run => target.chars().filter(|ch| *ch != '\r').collect(),
                _ => clean(target),
            };
            (
                clean(line),
                Call {
                    verb,
                    target,
                    about: about.filter(|_| verb == Verb::Run),
                    failed: false,
                },
            )
        }
        None => {
            let line = clean(
                title
                    .or(plain)
                    .or_else(|| name.map(|name| format!("Used {name}")))
                    .unwrap_or_else(|| "Used a tool".to_owned()),
            );
            (
                line.clone(),
                Call {
                    verb: Verb::Other,
                    target: line,
                    about: None,
                    failed: false,
                },
            )
        }
    }
}

/// A path as a step line shows it: an absolute one by its last part.
fn shown(path: &str) -> String {
    let at = std::path::Path::new(path);
    if at.is_absolute()
        && let Some(name) = at.file_name()
    {
        return name.to_string_lossy().into_owned();
    }
    path.to_owned()
}

/// Whether an ATIF document's tool call is a decision-model call: its
/// `extra` names [`atif::DECISION_CALL_SCHEMA`], as `atif::Call::is_decision`
/// reads the log's record.
fn is_decision_call(call: &Value) -> bool {
    call.pointer("/extra/schema").and_then(Value::as_str) == Some(atif::DECISION_CALL_SCHEMA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn change(path: &str) -> FileChange {
        FileChange {
            path: path.into(),
            status: "modified".into(),
            added: Some(1),
            removed: Some(1),
            ..FileChange::default()
        }
    }

    #[test]
    fn each_file_gets_its_own_patch_from_its_first_hunk() {
        let diff = "diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs\n\
                    @@ -1 +1 @@\n-old\n+new\n\
                    diff --git a/b c.md b/b c.md\nnew file mode 100644\n--- /dev/null\n\
                    +++ b/b c.md\n@@ -0,0 +1 @@\n+hi\n\
                    diff --git a/pic.png b/pic.png\nBinary files differ\n";
        let mut changes = vec![
            change("a.rs"),
            change("b c.md"),
            change("pic.png"),
            change("gone"),
        ];
        attach_patches(&mut changes, diff);
        assert_eq!(changes[0].patch.as_deref(), Some("@@ -1 +1 @@\n-old\n+new"));
        assert_eq!(changes[1].patch.as_deref(), Some("@@ -0,0 +1 @@\n+hi"));
        assert_eq!(changes[2].patch, None);
        assert_eq!(changes[3].patch, None);
        assert!(changes.iter().all(|c| c.patch_cut == 0));
    }

    #[test]
    fn a_long_patch_is_cut_and_says_how_much() {
        let body: String = (0..PATCH_LINES + 50).map(|n| format!("+{n}\n")).collect();
        let one = format!("diff --git a/a b/a\n@@ -0,0 +1 @@\n{body}");
        let diff = (0..6)
            .map(|n| one.replace("a/a b/a", &format!("a/{n} b/{n}")))
            .collect::<String>();
        let mut changes: Vec<FileChange> = (0..6).map(|n| change(&n.to_string())).collect();
        attach_patches(&mut changes, &diff);
        assert_eq!(
            changes[0].patch.as_deref().unwrap().lines().count(),
            PATCH_LINES
        );
        assert_eq!(changes[0].patch_cut, 51);
        let kept: usize = changes
            .iter()
            .map(|c| c.patch.as_deref().map_or(0, |p| p.lines().count()))
            .sum();
        assert_eq!(kept, PATCH_LINES_TOTAL);
        assert_eq!(changes[5].patch_cut, (PATCH_LINES + 51) as u64);
    }

    #[test]
    fn an_old_result_without_patches_still_reads() {
        let old = json!({"path": "a.rs", "status": "modified", "added": 1, "removed": 0});
        let read: FileChange = serde_json::from_value(old.clone()).unwrap();
        assert_eq!(read.patch, None);
        assert_eq!(serde_json::to_value(&read).unwrap(), old);
    }

    /// Grok Build's tool calls as the owner saw them on 2026-10-01
    /// (#10113): kind `other`, its typed arguments, and its own title. Each
    /// is a readable step line, and an unknown tool a short one, never JSON.
    #[test]
    fn an_agents_tool_calls_are_readable_lines() {
        let line = |name: &str, arguments: Value, extra: Value| {
            tool_line(
                &json!({"tool_call_id": "c", "function_name": name, "arguments": arguments}),
                &extra,
            )
        };
        let grok = |kind: &str, title: &str| {
            let tool = json!({"kind": kind, "status": "completed"});
            json!({"purpose": title, "grok_tool": tool})
        };
        assert_eq!(
            line(
                "other",
                json!({"target_directory": "."}),
                grok("other", "List `.`")
            ),
            "Listed the project"
        );
        assert_eq!(
            line(
                "other",
                json!({"target_file": "README.md"}),
                grok("other", "Read `README.md`")
            ),
            "Read README.md"
        );
        assert_eq!(
            line(
                "other",
                json!({"target_directory": "/private/var/x/scratch-1/src"}),
                json!({})
            ),
            "Listed src"
        );
        assert_eq!(
            line(
                "edit",
                json!({"file_path": "/w/src/lib.rs", "new_string": "x"}),
                grok("edit", "")
            ),
            "Edited lib.rs"
        );
        assert_eq!(
            line("other", json!({"command": "cargo test\n--more"}), json!({})),
            "Ran cargo test"
        );
        assert_eq!(
            line("other", json!({"pattern": "fn main"}), json!({})),
            "Searched for fn main"
        );
        // No field this reads: the agent's own title, then the tool's name.
        assert_eq!(
            line("other", json!({"x": 1}), grok("other", "Think about `it`")),
            "Think about it"
        );
        assert_eq!(
            line("web_search", json!({"x": 1}), json!({})),
            "Used web_search"
        );
        assert_eq!(
            line("other", json!({"x": 1}), json!({"purpose": "{\"x\":1}"})),
            "Used a tool"
        );
        assert_eq!(line("read", json!("a.rs"), json!({})), "read a.rs");
        let long = line("other", json!({"command": "y".repeat(500)}), json!({}));
        assert!(long.chars().count() <= TOOL_LINE_CHARS + 1 && long.ends_with('…'));
    }

    /// The same calls, typed (#10117): the verb and target every surface
    /// groups by, read from the same fields as the line. Devin's and
    /// A plan an engine records, as an ACP note or a `TodoWrite` call,
    /// is one note carrying the whole plan; the call's acknowledgement
    /// draws nothing (#10471).
    #[test]
    fn a_recorded_plan_is_a_note_carrying_the_plan() {
        use crate::plan::{Item, Status};
        let mut mapper = Mapper::new(1, None);
        let noted = mapper.step(&json!({
            "step_id": 2, "source": "system", "message": "Devin's plan.",
            "extra": {"devin_plan": [
                {"content": "Read", "status": "completed"},
                {"content": "Fix", "status": "in_progress"}]},
        }));
        let [CoderEvent::Step(step)] = noted.as_slice() else {
            panic!("one step: {noted:?}")
        };
        assert_eq!(step.kind, StepKind::Note);
        assert_eq!(step.text, "Updated the plan: 1 of 2 done.");
        assert_eq!(
            step.plan.as_deref(),
            Some(
                &[
                    Item::new("Read", Status::Completed),
                    Item::new("Fix", Status::InProgress)
                ][..]
            )
        );
        let called = mapper.step(&json!({
            "step_id": 3, "source": "agent", "message": "",
            "tool_calls": [{"tool_call_id": "t", "function_name": "TodoWrite",
                "arguments": {"todos": [{"content": "Read", "status": "completed"},
                    {"content": "Fix", "status": "completed"}]}}],
            "observation": {"results": [{"source_call_id": "t",
                "content": "Todos have been modified successfully."}]},
        }));
        let [CoderEvent::Step(step)] = called.as_slice() else {
            panic!("one step: {called:?}")
        };
        assert_eq!(step.text, "Updated the plan: 2 of 2 done.");
        assert!(step.call.is_none());
        let line = serde_json::to_value(&called[0]).unwrap();
        assert_eq!(
            line["plan"][1],
            json!({"text": "Fix", "status": "completed"})
        );
        assert_eq!(
            serde_json::from_value::<CoderEvent>(line).unwrap(),
            called[0]
        );
        let latest = crate::plan::latest(noted.iter().chain(&called)).unwrap();
        assert_eq!(latest[1].status, Status::Completed);
    }

    /// OpenCode's argument names (`file_path`, `filePath`, `path`) type the
    /// same way Grok Build's do.
    #[test]
    fn an_agents_tool_calls_are_typed() {
        let typed = |name: &str, arguments: Value, extra: Value| {
            tool_call(
                &json!({"tool_call_id": "c", "function_name": name, "arguments": arguments}),
                &extra,
            )
            .1
        };
        let is = |call: Call, verb: Verb, target: &str| {
            assert_eq!((call.verb, call.target.as_str()), (verb, target));
        };
        is(
            typed("other", json!({"target_directory": "./"}), json!({})),
            Verb::List,
            ".",
        );
        is(
            typed("read", json!({"filePath": "/w/src/main.rs"}), json!({})),
            Verb::Read,
            "main.rs",
        );
        is(
            typed(
                "edit",
                json!({"file_path": "src/lib.rs"}),
                json!({"devin_tool": {"kind": "edit"}}),
            ),
            Verb::Edit,
            "src/lib.rs",
        );
        is(
            typed(
                "rm",
                json!({"path": "old.rs"}),
                json!({"opencode_tool": {"kind": "delete"}}),
            ),
            Verb::Delete,
            "old.rs",
        );
        is(
            typed("grep", json!({"pattern": "fn main"}), json!({})),
            Verb::Search,
            "fn main",
        );
        is(
            typed("webfetch", json!({"url": "https://example.com"}), json!({})),
            Verb::Fetch,
            "https://example.com",
        );
        let run = typed(
            "other",
            json!({"command": "git log -1\ngit status", "description": "Show the latest commit"}),
            json!({}),
        );
        assert_eq!(run.verb, Verb::Run);
        assert_eq!(run.target, "git log -1\ngit status");
        assert_eq!(run.about.as_deref(), Some("Show the latest commit"));
        is(
            typed("web_search", json!({"x": 1}), json!({})),
            Verb::Other,
            "Used web_search",
        );
        // A failed result marks its call; the mapper reads it typed.
        let mut mapper = Mapper::new(1, None);
        let events = mapper.step(&json!({
            "step_id": 3, "source": "agent", "message": "",
            "tool_calls": [{"tool_call_id": "c1", "function_name": "other",
                "arguments": {"target_file": "gone.rs"}}],
            "observation": {"results": [{"source_call_id": "c1", "content": "no such file",
                "extra": {"status": "failed"}}]},
        }));
        let CoderEvent::Step(step) = &events[0] else {
            panic!("a step: {events:?}")
        };
        let call = step.call.as_ref().unwrap();
        assert_eq!((call.verb, call.failed), (Verb::Read, true));
    }

    fn step(id: u64, source: &str, message: &str, extra: Value) -> Value {
        json!({"step_id": id, "timestamp": format!("2026-09-30T12:00:{:02}.000Z", id), "source": source, "message": message, "extra": extra})
    }

    fn mc(event: Value) -> Value {
        json!({"microcoder": {"seconds": 1.5, "event": event}})
    }

    /// A turn as a scripted provider writes it: a failover, a command, its
    /// output, a streamed reply, and the end.
    fn scripted() -> Vec<Value> {
        vec![
            step(1, "user", "add a unit test for slugify", json!({})),
            step(
                2,
                "system",
                "Repository adapter admitted by the local operator.",
                json!({"admission": {"grant": {"adapter_configuration": {"max_steps": 24}}}}),
            ),
            step(
                3,
                "system",
                "judged",
                mc(json!({"event": "judged", "step": 1,
                "judgment": {"answers": [["done", 0.02], ["progress", 0.5]],
                    "scores": [["complete", 0.4]]}})),
            ),
            step(
                4,
                "system",
                "The provider refused for a usage or rate limit; the run switches to the next admitted route.",
                json!({"route_switch": {"from": {"provider": "codex", "model": "gpt-6-luna"},
                    "to": {"provider": "claude", "model": "claude-opus-5-5"},
                    "refusal": {"provider": "codex", "kind": "usage_limit", "resets_at": 1_791_050_823u64, "until": 1_791_050_823u64}}}),
            ),
            step(
                5,
                "system",
                "generated",
                mc(json!({"event": "generated", "step": 1,
                "generated": {"action": {"Ok": {"rationale": "Write the test file.", "commands": ["cat > test_slug.py <<'EOF'\nEOF", "python3 -m unittest"], "finished": false, "reply": ""}}}})),
            ),
            step(
                6,
                "system",
                "ran",
                mc(json!({"event": "ran", "step": 1,
                "result": {"command": "python3 -m unittest", "exit": 0, "timed_out": false, "seconds": 0.2, "output": "OK\n"}})),
            ),
            step(
                7,
                "system",
                "replying",
                mc(json!({"event": "replying", "text": "I added "})),
            ),
            step(
                8,
                "system",
                "generated",
                mc(json!({"event": "generated", "step": 2, "reply_streamed": 8,
                "generated": {"action": {"Ok": {"rationale": "Done.", "commands": [], "finished": true, "reply": "I added test_slug.py; it passes."}}}})),
            ),
            step(
                9,
                "system",
                "ended",
                mc(json!({"event": "ended", "outcome": {"ending": "finished", "steps": 2}})),
            ),
        ]
    }

    #[test]
    fn a_scripted_turn_maps_to_every_step_event_in_order() {
        let mut mapper = Mapper::new(1, Some("openagents chat answer --thread T TEXT".into()));
        let events: Vec<CoderEvent> = scripted().iter().flat_map(|s| mapper.step(s)).collect();
        let names: Vec<&str> = events.iter().map(CoderEvent::name).collect();
        assert_eq!(
            names,
            [
                "step",
                "progress",
                "provider_switched",
                "step",
                "step",
                "step",
                "step",
                "output",
                "step",
                "step",
                "step"
            ]
        );
        let CoderEvent::ProviderSwitched(switch) = &events[2] else {
            panic!()
        };
        assert_eq!(switch.line(), "Claude Code is working.");
        let json = serde_json::to_value(switch).unwrap();
        assert_eq!(json["reason"], switch.reason);
        assert_eq!(json["resets_at"], switch.resets_at.unwrap());
        assert_eq!(switch.from, "codex:gpt-6-luna");
        assert_eq!(switch.to.as_deref(), Some("claude:claude-opus-5-5"));
        assert!(
            switch
                .reason
                .starts_with("Codex refused for a usage limit until 2026-")
        );
        let CoderEvent::Progress(progress) = &events[1] else {
            panic!()
        };
        // An older grant's step limit is never shown (#10103); Jev's
        // estimate of how much is complete is.
        assert_eq!(
            (progress.step, progress.done, progress.complete),
            (1, Some(0.02), Some(0.4))
        );
        assert_eq!(progress.line(), "Working · step 1 · ≈40% done · 2s");
        let line = serde_json::to_string(&CoderEvent::Progress(progress.clone())).unwrap();
        assert!(
            !line.contains("max_steps") && line.contains("\"complete\":0.4"),
            "{line}"
        );
        // The streamed start of the reply is not repeated.
        let replies: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                CoderEvent::Step(s) if s.kind == StepKind::Reply => Some(s.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(replies, ["I added ", "test_slug.py; it passes."]);
        let end = mapper.end(
            "model_finished",
            vec![FileChange {
                path: "test_slug.py".into(),
                status: "added".into(),
                added: Some(9),
                removed: Some(0),
                ..FileChange::default()
            }],
            "/w",
            "/s/t.1.atif.jsonl",
            None,
        );
        let CoderEvent::Result(result) = &end else {
            panic!()
        };
        assert_eq!(result.summary, "I added test_slug.py; it passes.");
        assert_eq!((result.insertions, result.deletions), (9, 0));
        assert!(end.ends_turn());
    }

    /// #10103: the running line never names a budget. An older line
    /// with `max_steps` still reads; the share is Jev's estimate of how
    /// much of the task is complete, shown once Jev gave one.
    #[test]
    fn the_running_line_names_no_budget() {
        let old: CoderEvent = serde_json::from_str(
            r#"{"event":"progress","turn":1,"step":5,"max_steps":24,"seconds":9.2,"done":0.04}"#,
        )
        .unwrap();
        let CoderEvent::Progress(mut progress) = old else {
            panic!()
        };
        assert_eq!(progress.complete, None);
        assert_eq!(progress.line(), "Working · step 5 · 9s");
        progress.complete = Some(0.4);
        assert_eq!(progress.line(), "Working · step 5 · ≈40% done · 9s");
        assert!(!progress.line().contains(" of "));
        let text = text(&CoderEvent::Progress(progress)).unwrap();
        assert_eq!(text, "  [step 5, ≈40% done, 9s]");
    }

    #[test]
    fn endings_map_to_their_events() {
        let mapper = Mapper::new(2, Some("answer here".into()));
        let end = |ending: &str| mapper.end(ending, vec![], "/w", "/t", Some(1_791_050_823));
        assert_eq!(end("asked_question").name(), "question");
        assert_eq!(end("asked_approval").name(), "approval");
        assert_eq!(end("cancelled_or_host_refusal").name(), "stopped");
        // The delegate recipe's frozen checks passed and ended the turn:
        // a result, like a finish (#10208).
        assert_eq!(end("checks_passed").name(), end("model_finished").name());
        let CoderEvent::Failure(failure) = end("no_capacity") else {
            panic!()
        };
        assert_eq!(failure.resets_at, Some(1_791_050_823));
        assert_eq!(end("loop_incomplete").name(), "failure");
        let CoderEvent::Question(asked) = end("asked_question") else {
            panic!()
        };
        assert_eq!(asked.answer.as_deref(), Some("answer here"));
        let CoderEvent::Stopped(stopped) = end("engine_cancelled") else {
            panic!()
        };
        assert!(
            !stopped.message.contains("cancelled or reached"),
            "{}",
            stopped.message
        );
        assert!(stopped.message.contains("on its own"));
    }

    /// A whole coding agent's turn that the agent ended after the host
    /// refused a tool says what happened, never "cancelled or reached its
    /// time limit" (#10092).
    #[test]
    fn an_agent_that_stopped_after_a_refusal_says_so() {
        let mut mapper = Mapper::new(1, None);
        let summary = json!({"step_id": 9, "source": "system", "message": "Repository adapter ended; independent checks are separate.",
            "extensions": {"adapter_summary": {"configuration": {"provider": "grok"},
                "grok": {"engine": "grok-acp", "stop_reason": "cancelled",
                    "stopped": "Grok Build stopped after the host refused a tool it asked to run (Write /etc/hosts: it would write /etc/hosts, outside the workspace)."}}}});
        assert!(mapper.step(&summary).is_empty());
        let CoderEvent::Stopped(stopped) =
            mapper.end("engine_stopped_after_refusal", vec![], "/w", "/t", None)
        else {
            panic!()
        };
        assert_eq!(
            stopped.message,
            "Coder stopped: Grok Build stopped after the host refused a tool it asked to run (Write /etc/hosts: it would write /etc/hosts, outside the workspace)."
        );
        let CoderEvent::Failure(failure) =
            mapper.end("engine_incomplete", vec![], "/w", "/t", None)
        else {
            panic!()
        };
        assert!(
            failure.message.contains("Grok Build stopped after"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn a_line_round_trips_with_its_envelope() {
        let line = Line {
            seq: 3,
            task: "t".into(),
            thread: Some("a".repeat(32)),
            event: CoderEvent::Stopped(Stopped {
                turn: 1,
                message: "stopped".into(),
            }),
        };
        let text = serde_json::to_string(&line).unwrap();
        assert!(text.starts_with("{\"seq\":3,\"task\":\"t\",\"thread\":"));
        assert!(text.contains("\"event\":\"stopped\""));
        let back: Line = serde_json::from_str(&text).unwrap();
        assert_eq!(back, line);
    }

    /// A decision call in the exported ATIF document (a Jev judgment the
    /// loop asked) is not a transcript row; a real tool call still is
    /// (#10073).
    #[test]
    fn a_decision_call_is_not_a_transcript_row() {
        let mut mapper = Mapper::new(1, None);
        let judged = json!({
            "step_id": 2,
            "source": "agent",
            "message": "",
            "tool_calls": [{
                "tool_call_id": "decision-openagents.microcoder.judge.v1",
                "function_name": "openagents.microcoder.judge.v1",
                "arguments": {"model": "jev-1.13.0", "state": {"task": "t"}},
                "extra": {"schema": atif::DECISION_CALL_SCHEMA, "model": "jev-1.13.0"}
            }],
            "observation": {"results": [{
                "source_call_id": "decision-openagents.microcoder.judge.v1",
                "content": "{\"done\":{\"noul\":0.21}}"
            }]}
        });
        assert!(mapper.step(&judged).is_empty());
        let tool = json!({
            "step_id": 3,
            "source": "agent",
            "message": "",
            "tool_calls": [{"tool_call_id": "c1", "function_name": "read", "arguments": "a.rs"}],
            "observation": {"results": [{"source_call_id": "c1", "content": "fn main() {}"}]}
        });
        let events = mapper.step(&tool);
        assert!(
            matches!(&events[0], CoderEvent::Step(s) if s.kind == StepKind::ToolCall && s.text == "read a.rs")
        );
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn output_is_bounded_and_times_parse() {
        let mut mapper = Mapper::new(1, None);
        let long = "x".repeat(MAX_OUTPUT * 2);
        let events = mapper.step(&step(
            1,
            "system",
            "ran",
            mc(json!({"event": "ran", "step": 1, "result": {"command": "yes", "exit": null, "timed_out": true, "seconds": 3.0, "output": long}})),
        ));
        let CoderEvent::Output(output) = &events[1] else {
            panic!()
        };
        assert!(output.truncated && output.text.len() <= MAX_OUTPUT + 3);
        assert_eq!(parse_iso("1970-01-01T00:00:01.500Z"), Some(1500));
        assert_eq!(utc(1_791_050_823), "2026-10-03 18:07 UTC");
    }

    /// #10281: a command that ran past its deadline with its processes not
    /// confirmed gone ends the turn as that fault, naming the command, and
    /// never as a stop the person asked for.
    #[test]
    fn a_cleanup_fault_names_the_timed_out_command() {
        let mut mapper = Mapper::new(1, None);
        mapper.step(&step(
            1,
            "system",
            "ran",
            mc(json!({"event": "ran", "step": 1, "result": {"command": "cargo test -p coder follower_\necho done", "exit": null, "timed_out": true, "seconds": 300.4, "output": ""}})),
        ));
        let end = mapper.end("process_cleanup_unknown", vec![], "", "", None);
        assert_eq!(end.name(), "failure");
        let CoderEvent::Failure(failure) = &end else {
            panic!("{end:?}")
        };
        assert!(
            failure
                .message
                .contains("`cargo test -p coder follower_` ran past its deadline (300s)"),
            "{}",
            failure.message
        );
        assert!(failure.message.contains("Nobody stopped the task."));
        assert_eq!(failure.ending.as_deref(), Some("process_cleanup_unknown"));
        let unnamed = Mapper::new(1, None).end("process_cleanup_unknown", vec![], "", "", None);
        assert_eq!(unnamed.name(), "failure");
    }

    /// A turn the loop ended because the task's checks passed, before the
    /// agent wrote an answer, answers with what the checks said (#10331).
    #[test]
    fn a_turn_ended_by_passing_checks_answers_with_them() {
        let mut mapper = Mapper::new(1, None);
        mapper.step(&step(
            1,
            "system",
            "tested",
            mc(json!({"event": "tested", "step": 1, "results": [{"command": "check-1", "exit": 1, "output": "No module named pytest\n"}]})),
        ));
        mapper.step(&step(
            2,
            "system",
            "tested",
            mc(json!({"event": "tested", "step": 2, "results": [{"command": "check-1", "exit": 0, "output": "...   [100%]\n3 passed in 0.00s\n"}]})),
        ));
        let end = mapper.end("checks_passed", vec![], "/w", "/t", None);
        let CoderEvent::Result(finished) = &end else {
            panic!("{end:?}")
        };
        assert_eq!(
            finished.summary,
            "The task's checks pass: 3 passed in 0.00s"
        );
        let failing = Mapper::new(1, None).end("model_finished", vec![], "/w", "/t", None);
        let CoderEvent::Result(finished) = &failing else {
            panic!("{failing:?}")
        };
        assert_eq!(finished.summary, "");
    }

    #[test]
    fn installs_are_named_with_where_they_landed() {
        assert_eq!(
            install_reach("python3 -m pip install --user pytest"),
            Some(InstallReach::Run)
        );
        assert_eq!(install_reach("pip install requests"), None);
        assert_eq!(
            install_reach("sudo pip3 install requests"),
            Some(InstallReach::Computer)
        );
        assert_eq!(
            install_reach("pip install --break-system-packages x"),
            Some(InstallReach::Computer)
        );
        assert_eq!(
            install_reach("npm install -g typescript"),
            Some(InstallReach::Run)
        );
        assert_eq!(
            install_reach("cd x && cargo install ripgrep"),
            Some(InstallReach::Run)
        );
        assert_eq!(
            install_reach("brew install jq"),
            Some(InstallReach::Computer)
        );
        assert_eq!(
            install_reach("apt-get install -y jq"),
            Some(InstallReach::Computer)
        );
        assert_eq!(install_reach(".venv/bin/pip install pytest"), None);
        assert_eq!(install_reach("npm install"), None);
        assert_eq!(install_reach("python3 -m pytest -q"), None);
        let mut mapper = Mapper::new(1, None);
        mapper.step(&step(
            1,
            "system",
            "generated",
            mc(json!({"event": "generated", "generated": {"action": {"Ok": {"rationale": "", "reply": "Fixed calc.py.", "commands": ["python3 -m pip install --user pytest\nbrew install jq"]}}}})),
        ));
        let end = mapper.end("model_finished", vec![], "/w", "/t", None);
        let CoderEvent::Result(finished) = &end else {
            panic!("{end:?}")
        };
        assert_eq!(
            finished.summary,
            "Fixed calc.py.\n\nOutside its worktree, this turn installed software on this computer: `brew install jq`. This turn installed software just for this run, gone when it ends: `python3 -m pip install --user pytest`."
        );
    }

    /// An engine session's own shell calls are read too, not only the
    /// Microcoder loop's commands (#10336).
    #[test]
    fn an_engines_shell_call_that_installs_is_named() {
        let mut mapper = Mapper::new(1, None);
        mapper.step(&json!({
            "step_id": 2, "source": "agent", "message": "",
            "tool_calls": [{"tool_call_id": "c1", "function_name": "Bash",
                "arguments": {"command": "npm install -g typescript"}}],
        }));
        mapper.step(&json!({"step_id": 3, "source": "agent", "message": "Done."}));
        let end = mapper.end("model_finished", vec![], "/w", "/t", None);
        let CoderEvent::Result(finished) = &end else {
            panic!("{end:?}")
        };
        assert!(
            finished
                .summary
                .contains("just for this run, gone when it ends: `npm install -g typescript`"),
            "{}",
            finished.summary
        );
    }

    /// When the person asked for an engine, the runner says so first, and
    /// why another runs when it does (#10076).
    #[test]
    fn a_runner_states_the_engine_the_person_asked_for() {
        let honored = Runner::Runs {
            provider: "claude".into(),
            model: "claude-opus-5-5".into(),
            passed: vec![],
            requested: Some("claude".into()),
        };
        assert_eq!(
            honored.text(),
            "You asked for Claude Code; it will do this."
        );
        let wire = serde_json::to_value(&honored).unwrap();
        assert_eq!(wire["requested"], "claude");
        assert_eq!(serde_json::from_value::<Runner>(wire).unwrap(), honored);
        let limited = Runner::Runs {
            provider: "codex".into(),
            model: "gpt-6-luna".into(),
            passed: vec![Passed {
                provider: "claude".into(),
                why: PassedOver::Refused {
                    kind: "usage_limit".into(),
                    until: 1_791_050_823,
                },
            }],
            requested: Some("claude".into()),
        };
        // The provider's window is never named (#10120).
        assert_eq!(
            limited.text(),
            "You asked for Claude Code; it isn't available right now, so Codex will do this."
        );
        for (why, clause) in [
            (PassedOver::NotSignedIn, "it is not signed in here"),
            (
                PassedOver::NotAllowed,
                "it is turned off in your Coder settings",
            ),
            (
                PassedOver::NearLimit { used_percent: 95 },
                "it isn't available right now",
            ),
        ] {
            let runner = Runner::Runs {
                provider: "codex".into(),
                model: "gpt-6-luna".into(),
                passed: vec![Passed {
                    provider: "grok".into(),
                    why,
                }],
                requested: Some("grok".into()),
            };
            assert_eq!(
                runner.text(),
                format!("You asked for Grok Build; {clause}, so Codex will do this.")
            );
        }
        // An old runner, without the field, reads as no request.
        let old: Runner = serde_json::from_value(
            json!({"state": "runs", "provider": "codex", "model": "gpt-6-luna"}),
        )
        .unwrap();
        assert_eq!(old.text(), "Codex will do this.");
    }

    /// A start is one line, "Grok Build is working.", and why only when
    /// that is news: another engine than asked, or one passed over
    /// (#10115). Never the task or the worktree.
    #[test]
    fn a_start_line_says_who_works_and_only_news() {
        let started = |runner: Option<Runner>, reason: &str| Started {
            turn: 1,
            project: "openagents".into(),
            checkout: "/c".into(),
            worktree: "/w/openagents-b9c1ffbaee82".into(),
            base: "abc".into(),
            provider: "grok".into(),
            model: "default".into(),
            reason: reason.into(),
            fallbacks: vec!["codex:gpt".into()],
            via: "local".into(),
            runner,
        };
        let asked = started(
            Some(Runner::Runs {
                provider: "grok".into(),
                model: "default".into(),
                passed: vec![],
                requested: Some("grok".into()),
            }),
            "You asked for Grok Build; it is signed in and has capacity.",
        );
        assert_eq!(asked.line(), "Grok Build is working.");
        assert_eq!(asked.news(), None);
        let event = CoderEvent::CoderStarted(asked);
        assert_eq!(text(&event).unwrap(), "Grok Build is working.");
        // An old record, without the prediction, says only who works.
        assert_eq!(started(None, "ready").news(), None);
        // Another engine than asked: the line, then why.
        let other = started(
            Some(Runner::Runs {
                provider: "grok".into(),
                model: "default".into(),
                passed: vec![Passed {
                    provider: "codex".into(),
                    why: PassedOver::NotSignedIn,
                }],
                requested: Some("codex".into()),
            }),
            "You asked for Codex; it is not signed in here, so Grok Build is running.",
        );
        let shown = text(&CoderEvent::CoderStarted(other)).unwrap();
        assert_eq!(
            shown,
            "Grok Build is working.\nYou asked for Codex; it is not signed in here, so Grok \
             Build is running."
        );
        assert!(!shown.contains("b9c1ff") && !shown.contains("/w/"));
        assert_eq!(starting("claude"), "Starting Claude Code…");
    }

    /// The three things an offer can say, from their typed fields, and
    /// the wire form a phone reads back.
    #[test]
    fn a_runner_says_who_will_run_or_why_none_can() {
        let codex = Runner::Runs {
            provider: "codex".into(),
            model: "gpt-6-luna".into(),
            passed: vec![],
            requested: None,
        };
        assert_eq!(codex.text(), "Codex will do this.");
        assert_eq!(codex.provider(), Some("codex"));
        assert_eq!(
            serde_json::to_value(&codex).unwrap(),
            json!({"state": "runs", "provider": "codex", "model": "gpt-6-luna"})
        );

        let claude = Runner::Runs {
            provider: "claude".into(),
            model: "claude-opus-5-5".into(),
            passed: vec![Passed {
                provider: "codex".into(),
                why: PassedOver::NearLimit { used_percent: 92 },
            }],
            requested: None,
        };
        // A run fails over past a provider's window silently: the offer
        // says only who runs, and the record keeps why (#10120).
        assert_eq!(claude.text(), "Claude Code will do this.");
        assert!(claude.plain());
        assert_eq!(claude.started("is running."), "Claude Code is running.");
        let wire = serde_json::to_value(&claude).unwrap();
        assert_eq!(
            wire["passed"],
            json!([{"provider": "codex", "why": "near_limit", "used_percent": 92}])
        );
        assert_eq!(serde_json::from_value::<Runner>(wire).unwrap(), claude);
        let refused = Runner::Runs {
            provider: "claude".into(),
            model: "claude-opus-5-5".into(),
            passed: vec![Passed {
                provider: "codex".into(),
                why: PassedOver::Refused {
                    kind: "usage_limit".into(),
                    until: 1_791_050_823,
                },
            }],
            requested: None,
        };
        assert_eq!(refused.text(), "Claude Code will do this.");
        let signed_out = Runner::Runs {
            provider: "claude".into(),
            model: "claude-opus-5-5".into(),
            passed: vec![Passed {
                provider: "codex".into(),
                why: PassedOver::NotSignedIn,
            }],
            requested: None,
        };
        assert_eq!(
            signed_out.text(),
            "Codex is not signed in here; Claude Code will do this."
        );
        assert_eq!(
            signed_out.started("is running."),
            "Codex is not signed in here; using Claude Code."
        );

        let nobody = Runner::NotSignedIn {
            providers: vec!["codex".into(), "claude".into()],
        };
        assert_eq!(
            nobody.text(),
            "No coding agent Coder can use is signed in on this computer. Sign in to one, \
             then ask again: Codex (run `codex login`); Claude Code (run `claude` and log in)."
        );
        assert_eq!(nobody.provider(), None);
        assert_eq!(
            serde_json::to_value(&nobody).unwrap(),
            json!({"state": "not_signed_in", "providers": ["codex", "claude"]})
        );
        assert_eq!(
            Runner::NotSignedIn {
                providers: vec!["claude".into()]
            }
            .text(),
            "Claude Code is not signed in on this computer. Sign in (run `claude` and log in), \
             then ask again."
        );
        assert_eq!(
            Runner::NoCapacity {
                until: Some(1_791_050_823)
            }
            .text(),
            "No coding agent signed in on this computer is available right now; \
             try again after 2026-10-03 18:07 UTC."
        );
    }

    fn at(seq: u64, event: CoderEvent) -> Line {
        Line {
            seq,
            task: "t".into(),
            thread: None,
            event,
        }
    }

    fn started(turn: usize, provider: &str, model: &str) -> CoderEvent {
        CoderEvent::CoderStarted(Started {
            turn,
            project: "p".into(),
            checkout: "/c".into(),
            worktree: "/w".into(),
            base: "abc".into(),
            provider: provider.into(),
            model: model.into(),
            reason: "ready".into(),
            fallbacks: vec![],
            via: "local".into(),
            runner: None,
        })
    }

    fn command(turn: usize, text: &str) -> CoderEvent {
        CoderEvent::Step(Step {
            turn,
            step_id: 1,
            kind: StepKind::Command,
            source: "agent".into(),
            text: text.into(),
            call: None,
            plan: None,
        })
    }

    /// A follow-up after a run carries the last turn's result: how it
    /// ended, the engine it switched to, its summary, files, and commands;
    /// a turn that runs or asks carries none (#10094).
    #[test]
    fn a_run_result_is_the_last_ended_turn() {
        use crate::router::{RunEnding, RunFile};
        let mut lines = vec![
            at(1, started(1, "claude", "claude-opus-5-5")),
            at(
                2,
                CoderEvent::ProviderSwitched(Switched {
                    turn: 1,
                    step_id: 2,
                    from: "claude:claude-opus-5-5".into(),
                    to: Some("codex:gpt-6-luna".into()),
                    reason: "Claude Code is at 99%".into(),
                    resets_at: None,
                }),
            ),
            at(3, command(1, "ls\nsecond line")),
            at(
                4,
                CoderEvent::Result(Finished {
                    turn: 1,
                    summary: "I listed the files.".into(),
                    files_changed: vec![FileChange {
                        path: "NOTE.md".into(),
                        status: "added".into(),
                        added: Some(3),
                        removed: Some(0),
                        ..FileChange::default()
                    }],
                    insertions: 3,
                    deletions: 0,
                    worktree: "/w".into(),
                    trajectory: "/t".into(),
                    issue: None,
                    pushed_to: None,
                    cost_microusd: None,
                }),
            ),
        ];
        let run = run_result(&lines).unwrap();
        assert_eq!(run.ending, RunEnding::Finished);
        assert_eq!(run.turn, 1);
        assert_eq!(run.engine.as_deref(), Some("codex"));
        assert_eq!(run.model.as_deref(), Some("gpt-6-luna"));
        assert_eq!(run.summary, "I listed the files.");
        assert_eq!(
            run.files,
            vec![RunFile {
                path: "NOTE.md".into(),
                status: "added".into()
            }]
        );
        let json = run.json();
        assert_eq!(json["commands"], json!(["ls"]));
        assert_eq!(json["engine"], "codex");
        // The next turn runs: no result until it ends.
        lines.push(at(5, started(2, "codex", "gpt-6-luna")));
        assert!(run_result(&lines).is_none());
        lines.push(at(
            6,
            CoderEvent::Question(Asked {
                turn: 2,
                text: "Which crate?".into(),
                answer: None,
            }),
        ));
        assert!(
            run_result(&lines).is_none(),
            "a question waits for an answer"
        );
        lines.push(at(
            7,
            CoderEvent::Stopped(Stopped {
                turn: 2,
                message: "Stopped from the desktop.".into(),
            }),
        ));
        let stopped = run_result(&lines).unwrap();
        assert_eq!(stopped.ending, RunEnding::Stopped);
        assert_eq!(stopped.turn, 2);
        assert!(stopped.commands.is_empty());
        assert!(run_result(&[]).is_none());
    }

    /// The steps Grok Build's adapter records before its first word, as a
    /// real run recorded them, become the status the terminal shows beside
    /// its spinner: starting, connected with the model it reported, then
    /// thinking. A command's intent says nothing: its own row shows it.
    #[test]
    fn an_engines_start_becomes_status() {
        let statuses = |steps: &[Value]| -> Vec<String> {
            let mut mapper = Mapper::new(1, None);
            steps
                .iter()
                .flat_map(|step| mapper.step(step))
                .filter_map(|event| match event {
                    CoderEvent::Status(status) => Some(status.text),
                    _ => None,
                })
                .collect()
        };
        let steps = [
            json!({"step_id": 6, "source": "system", "message": "Adapter effect intent retained before dispatch.",
                "extensions": {"effect": {"sequence": 1, "kind": "grok_session", "arguments": {}}}}),
            json!({"step_id": 7, "source": "system", "message": "Adapter effect observation retained.",
                "extensions": {"effect_result": {"sequence": 1, "kind": "grok_session",
                    "result": {"session": "01a0", "pid": 37416, "model": "grok-4.7"}}}}),
            json!({"step_id": 9, "source": "system", "message": "Adapter effect intent retained before dispatch.",
                "extensions": {"effect": {"sequence": 2, "kind": "grok_prompt", "arguments": {}}}}),
            json!({"step_id": 10, "source": "system", "message": "Adapter effect intent retained before dispatch.",
                "extensions": {"effect": {"sequence": 3, "kind": "command", "arguments": {}}}}),
        ];
        assert_eq!(
            statuses(&steps),
            [
                "Starting Grok Build…",
                "Grok Build connected · grok-4.7",
                "Thinking…"
            ]
        );
        let failed = json!({"step_id": 7, "source": "system",
            "extensions": {"effect_result": {"kind": "grok_session", "result": {"error": "no login"}}}});
        assert!(statuses(&[failed]).is_empty());
    }
}
