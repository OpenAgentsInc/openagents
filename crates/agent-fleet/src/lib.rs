//! Background agents: one registry and lifecycle for every surface (#11163).
//!
//! An agent is a child chat with its own transcript, its own worktree and
//! branch, an engine, and a place to run. A parent starts one and keeps
//! talking; when the child finishes, fails or is stopped, exactly one
//! [`Notice`] with its report is produced for the parent to read as its
//! next input.
//!
//! This crate holds the parts every surface shares, with no terminal,
//! website or engine code in it:
//!
//! - [`Registry`]: the agent list and its state machine. Each agent's
//!   [`Control`] is what the code running it holds: the stop flag, its
//!   queued messages, its usage, and the one call that finishes it.
//!   `Registry<E>` carries the host's own event type `E` (the terminal
//!   passes its runtime events), so a surface can show a child's work live.
//! - [`AgentRow`]: the serialisable row the agent list shows (name, engine,
//!   where it runs, status, elapsed, tokens, dollars), the same JSON on the
//!   terminal, the website and the apps.
//! - [`guard`]: the free-disk floor for new worktree agents and the shared
//!   pool of build folders.
//! - [`lease`]: the `worktree/<id>` lease an agent holds while it runs, so
//!   nothing removes its checkout under it.
//!
//! There are no step or time limits anywhere here; an agent runs until it
//! finishes or someone stops it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

/// The schema name a row's JSON carries for the website and apps.
pub const SCHEMA: &str = "openagents.agent.v1";

/// The longest report a notice carries, in bytes. The full report stays
/// in the agent's transcript.
pub const NOTICE_REPORT_BYTES: usize = 16 * 1024;

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Where an agent is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Working.
    Running,
    /// Finished with a report.
    Done,
    /// Ended with an error.
    Failed,
    /// Stopped by a person or the parent.
    Stopped,
}

impl Status {
    /// The word the agent list shows.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    /// Whether the agent has ended (it can be resumed).
    #[must_use]
    pub const fn ended(self) -> bool {
        !matches!(self, Self::Running)
    }
}

/// One row of the agent list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentRow {
    /// Stable within its registry, such as `agent-3`.
    pub id: String,
    /// A short unique name, such as `fix-login`.
    pub name: String,
    /// The engine, such as `codex`, `claude` or `microcoder`.
    pub engine: String,
    /// Where it runs, in plain words: `this computer`, a computer's name,
    /// or a Cloud environment.
    pub place: String,
    pub status: Status,
    /// What it was asked to do.
    pub task: String,
    pub started_ms: u64,
    /// When the latest run ended; `None` while running.
    pub ended_ms: Option<u64>,
    /// Seconds spent in earlier runs, before the current one (a resumed
    /// agent's elapsed time counts every run).
    pub earlier_seconds: u64,
    pub tokens: u64,
    /// Dollars, when the engine reports a cost or one can be estimated.
    pub cost_usd: Option<f64>,
    /// Its own checkout, when it has one.
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    /// The chat that started it.
    pub parent_session: Option<String>,
    /// Its own transcript (ATIF), linked to the parent session.
    pub transcript: Option<PathBuf>,
    /// The final report of its latest run.
    pub report: Option<String>,
    pub error: Option<String>,
    /// Messages waiting for it at the end of its current step.
    pub pending_messages: usize,
    /// How many times it has run: 1, then one more per resume.
    pub runs: u32,
    /// When this run started.
    pub run_started_ms: u64,
}

impl AgentRow {
    /// Seconds it has worked, across every run, as of `now_ms`.
    #[must_use]
    pub fn elapsed_seconds(&self, now_ms: u64) -> u64 {
        let end = self.ended_ms.unwrap_or(now_ms);
        self.earlier_seconds
            .saturating_add(end.saturating_sub(self.run_started_ms) / 1000)
    }

    /// The row as the JSON the website and apps read, with `elapsed_seconds`
    /// filled in.
    #[must_use]
    pub fn to_json(&self, now_ms: u64) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or_default();
        value["schema"] = serde_json::json!(SCHEMA);
        value["elapsed_seconds"] = serde_json::json!(self.elapsed_seconds(now_ms));
        value
    }
}

/// What a host asks for when it starts an agent.
#[derive(Clone, Debug, Default)]
pub struct Spec {
    /// A short name; one is made from the task when `None`.
    pub name: Option<String>,
    pub engine: String,
    pub task: String,
    /// Where it runs; `this computer` when empty.
    pub place: String,
    pub parent_session: Option<String>,
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Finished, with its final report.
    Done(String),
    /// Ended with an error.
    Failed(String),
    /// Stopped before it finished.
    Stopped,
}

/// The message a parent receives when an agent's run ends: one per run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub status: Status,
    pub elapsed_seconds: u64,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    pub report: Option<String>,
    pub error: Option<String>,
    pub parent_session: Option<String>,
}

impl Notice {
    /// The text the parent chat receives as its next input.
    #[must_use]
    pub fn text(&self) -> String {
        let mut facts = vec![elapsed_words(self.elapsed_seconds)];
        if self.tokens > 0 {
            facts.push(format!("{} tokens", token_words(self.tokens)));
        }
        if let Some(cost) = self.cost_usd {
            facts.push(dollars(cost));
        }
        let what = match self.status {
            Status::Done => "finished",
            Status::Failed => "ran into a problem",
            Status::Stopped => "was stopped",
            Status::Running => "is still running",
        };
        let mut text = format!(
            "Background agent {} ({}) {what} after {}.",
            self.name,
            self.engine,
            facts.join(" · ")
        );
        if let (Some(branch), Some(worktree)) = (&self.branch, &self.worktree) {
            text.push_str(&format!(
                "\nIts work is on branch {branch} in {}.",
                worktree.display()
            ));
        }
        if let Some(error) = &self.error {
            text.push_str(&format!("\nWhat went wrong: {error}"));
        }
        if let Some(report) = self.report.as_deref().filter(|r| !r.trim().is_empty()) {
            text.push_str("\n\nIts report:\n");
            text.push_str(bounded(report.trim(), NOTICE_REPORT_BYTES));
            if report.trim().len() > NOTICE_REPORT_BYTES {
                text.push_str("\n[The rest is in its transcript.]");
            }
        }
        text
    }
}

/// `4m 12s`, `38s`, or `1h 5m`.
#[must_use]
pub fn elapsed_words(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    }
}

/// `950`, `31.2k`, or `1.4M`.
#[must_use]
pub fn token_words(tokens: u64) -> String {
    match tokens {
        0..1000 => tokens.to_string(),
        1000..1_000_000 => format!("{:.1}k", tokens as f64 / 1000.0),
        _ => format!("{:.1}M", tokens as f64 / 1_000_000.0),
    }
}

/// `$0.42`, or `$0.0031` for amounts under a cent.
#[must_use]
pub fn dollars(cost: f64) -> String {
    if cost > 0.0 && cost < 0.01 {
        format!("${cost:.4}")
    } else {
        format!("${cost:.2}")
    }
}

fn bounded(text: &str, bytes: usize) -> &str {
    if text.len() <= bytes {
        return text;
    }
    let mut end = bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// What sending a message to an agent did.
#[derive(Clone, Debug, PartialEq)]
pub enum Delivery {
    /// The agent is running; it reads the message when its current step
    /// ends.
    Queued(AgentRow),
    /// The agent had ended; the host resumes it with the message.
    Resume(AgentRow),
}

struct Slot<E> {
    row: AgentRow,
    cancel: Arc<AtomicBool>,
    inbox: Vec<String>,
    events: Vec<E>,
}

struct Inner<E> {
    agents: Vec<Slot<E>>,
    notices: Vec<Notice>,
    next: u64,
    revision: u64,
}

impl<E> Default for Inner<E> {
    fn default() -> Self {
        Self {
            agents: Vec::new(),
            notices: Vec::new(),
            next: 1,
            revision: 0,
        }
    }
}

impl<E> Inner<E> {
    fn find(&self, key: &str) -> Option<usize> {
        let key = key.trim();
        self.agents
            .iter()
            .position(|slot| slot.row.id == key)
            .or_else(|| self.agents.iter().position(|slot| slot.row.name == key))
    }
}

/// The agent list and its state machine. Cloning shares it.
pub struct Registry<E> {
    inner: Arc<Mutex<Inner<E>>>,
}

impl<E> Clone for Registry<E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E> Default for Registry<E> {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }
}

impl<E> std::fmt::Debug for Registry<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("agents", &self.list().len())
            .finish()
    }
}

/// A short name from a task: its first three plain words, joined by `-`.
#[must_use]
pub fn name_from(task: &str) -> String {
    let words: Vec<String> = task
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(3)
        .map(str::to_ascii_lowercase)
        .collect();
    if words.is_empty() {
        "agent".into()
    } else {
        bounded(&words.join("-"), 32)
            .trim_end_matches('-')
            .to_owned()
    }
}

/// Whether `name` is a usable agent name: 1 to 48 letters, digits, `-`
/// or `_`, not starting with `-`.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

impl<E> Registry<E> {
    fn lock(&self) -> MutexGuard<'_, Inner<E>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The id and unique name the next [`Registry::start`] of `spec` would
    /// take, so a host can name its worktree and branch after them first.
    ///
    /// # Errors
    /// The requested name is unusable or taken.
    pub fn reserve(&self, spec: &Spec) -> Result<(String, String), String> {
        let inner = self.lock();
        let id = format!("agent-{}", inner.next);
        let name = match &spec.name {
            Some(name) if !valid_name(name) => {
                return Err(format!(
                    "`{name}` can't be an agent name; use up to 48 letters, digits, `-` or `_`."
                ));
            }
            Some(name) if inner.agents.iter().any(|slot| &slot.row.name == name) => {
                return Err(format!("An agent named {name} already exists."));
            }
            Some(name) => name.clone(),
            None => {
                let base = name_from(&spec.task);
                let mut name = base.clone();
                let mut n = 2;
                while inner.agents.iter().any(|slot| slot.row.name == name) {
                    name = format!("{base}-{n}");
                    n += 1;
                }
                name
            }
        };
        Ok((id, name))
    }

    /// Registers a running agent and returns the control its runner holds.
    ///
    /// # Errors
    /// The task is empty, or the name is unusable or taken.
    pub fn start(&self, spec: Spec) -> Result<Control<E>, String> {
        if spec.task.trim().is_empty() {
            return Err("An agent needs a task.".into());
        }
        if spec.engine.trim().is_empty() {
            return Err("An agent needs an engine.".into());
        }
        let (id, name) = self.reserve(&spec)?;
        let mut inner = self.lock();
        inner.next += 1;
        inner.revision += 1;
        let now = now_ms();
        let cancel = Arc::new(AtomicBool::new(false));
        inner.agents.push(Slot {
            row: AgentRow {
                id: id.clone(),
                name,
                engine: spec.engine,
                place: if spec.place.is_empty() {
                    "this computer".into()
                } else {
                    spec.place
                },
                status: Status::Running,
                task: spec.task,
                started_ms: now,
                ended_ms: None,
                earlier_seconds: 0,
                tokens: 0,
                cost_usd: None,
                worktree: spec.worktree,
                branch: spec.branch,
                parent_session: spec.parent_session,
                transcript: None,
                report: None,
                error: None,
                pending_messages: 0,
                runs: 1,
                run_started_ms: now,
            },
            cancel: Arc::clone(&cancel),
            inbox: Vec::new(),
            events: Vec::new(),
        });
        Ok(Control {
            id,
            cancel,
            registry: self.clone(),
            finished: false,
        })
    }

    /// Every agent, oldest first.
    #[must_use]
    pub fn list(&self) -> Vec<AgentRow> {
        self.lock()
            .agents
            .iter()
            .map(|slot| slot.row.clone())
            .collect()
    }

    /// The list as JSON for the website and apps.
    #[must_use]
    pub fn snapshot_json(&self) -> serde_json::Value {
        let now = now_ms();
        serde_json::Value::Array(self.list().iter().map(|row| row.to_json(now)).collect())
    }

    /// One agent, by id or name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<AgentRow> {
        let inner = self.lock();
        inner.find(key).map(|index| inner.agents[index].row.clone())
    }

    /// Changes whenever a row or notice changes, so a surface redraws or
    /// syncs only when something happened.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.lock().revision
    }

    /// How many are running.
    #[must_use]
    pub fn running(&self) -> usize {
        self.lock()
            .agents
            .iter()
            .filter(|slot| slot.row.status == Status::Running)
            .count()
    }

    /// Asks a running agent to stop. It ends, and its notice is sent, when
    /// its engine has stopped.
    ///
    /// # Errors
    /// No such agent, or it is not running.
    pub fn stop(&self, key: &str) -> Result<AgentRow, String> {
        let inner = self.lock();
        let index = inner
            .find(key)
            .ok_or_else(|| format!("There is no agent called {key}."))?;
        let slot = &inner.agents[index];
        if slot.row.status != Status::Running {
            return Err(format!(
                "{} is not running; it is {}.",
                slot.row.name,
                slot.row.status.word()
            ));
        }
        slot.cancel.store(true, Ordering::Relaxed);
        Ok(slot.row.clone())
    }

    /// Asks every running agent to stop.
    pub fn stop_all(&self) {
        for slot in &self.lock().agents {
            if slot.row.status == Status::Running {
                slot.cancel.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Sends a message: a running agent reads it when its current step
    /// ends; an ended one is to be resumed with it ([`Registry::resume`]).
    ///
    /// # Errors
    /// No such agent, or the message is empty.
    pub fn message(&self, key: &str, text: &str) -> Result<Delivery, String> {
        if text.trim().is_empty() {
            return Err("The message is empty.".into());
        }
        let mut inner = self.lock();
        let index = inner
            .find(key)
            .ok_or_else(|| format!("There is no agent called {key}."))?;
        inner.revision += 1;
        let slot = &mut inner.agents[index];
        if slot.row.status == Status::Running && !slot.cancel.load(Ordering::Relaxed) {
            slot.inbox.push(text.to_owned());
            slot.row.pending_messages = slot.inbox.len();
            Ok(Delivery::Queued(slot.row.clone()))
        } else if slot.row.status == Status::Running {
            Err(format!(
                "{} is stopping; send the message once it has stopped.",
                slot.row.name
            ))
        } else {
            Ok(Delivery::Resume(slot.row.clone()))
        }
    }

    /// Starts an ended agent's next run and returns its control.
    ///
    /// # Errors
    /// No such agent, or it is still running.
    pub fn resume(&self, key: &str) -> Result<Control<E>, String> {
        let mut inner = self.lock();
        let index = inner
            .find(key)
            .ok_or_else(|| format!("There is no agent called {key}."))?;
        inner.revision += 1;
        let slot = &mut inner.agents[index];
        if slot.row.status == Status::Running {
            return Err(format!("{} is still running.", slot.row.name));
        }
        let now = now_ms();
        slot.row.earlier_seconds = slot.row.elapsed_seconds(now);
        slot.row.status = Status::Running;
        slot.row.ended_ms = None;
        slot.row.error = None;
        slot.row.runs += 1;
        slot.row.run_started_ms = now;
        slot.cancel = Arc::new(AtomicBool::new(false));
        Ok(Control {
            id: slot.row.id.clone(),
            cancel: Arc::clone(&slot.cancel),
            registry: self.clone(),
            finished: false,
        })
    }

    /// The notices produced since the last call, oldest first.
    #[must_use]
    pub fn drain_notices(&self) -> Vec<Notice> {
        std::mem::take(&mut self.lock().notices)
    }

    /// The events runners reported since the last call, as `(id, event)`,
    /// oldest first.
    #[must_use]
    pub fn drain_events(&self) -> Vec<(String, E)> {
        let mut inner = self.lock();
        let mut drained = Vec::new();
        for slot in &mut inner.agents {
            let id = slot.row.id.clone();
            drained.extend(slot.events.drain(..).map(|event| (id.clone(), event)));
        }
        drained
    }

    fn with_slot<T>(&self, id: &str, change: impl FnOnce(&mut Slot<E>) -> T) -> Option<T> {
        let mut inner = self.lock();
        inner.revision += 1;
        let index = inner.agents.iter().position(|slot| slot.row.id == id)?;
        Some(change(&mut inner.agents[index]))
    }
}

/// What the code running one agent holds. Dropping it without
/// [`Control::finish`] (a panic, say) ends the agent as failed, so every
/// run sends exactly one notice.
pub struct Control<E> {
    id: String,
    cancel: Arc<AtomicBool>,
    registry: Registry<E>,
    finished: bool,
}

impl<E> Control<E> {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The agent's row as it is now.
    #[must_use]
    pub fn row(&self) -> Option<AgentRow> {
        self.registry.get(&self.id)
    }

    /// The flag a stop sets; engines watch it.
    #[must_use]
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    /// Whether someone asked it to stop.
    #[must_use]
    pub fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Reports one event for surfaces showing the agent's work live.
    pub fn event(&self, event: E) {
        self.registry
            .with_slot(&self.id, |slot| slot.events.push(event));
    }

    /// Adds usage from a finished step.
    pub fn add_usage(&self, tokens: u64, cost_usd: Option<f64>) {
        self.registry.with_slot(&self.id, |slot| {
            slot.row.tokens = slot.row.tokens.saturating_add(tokens);
            if let Some(cost) = cost_usd.filter(|cost| cost.is_finite() && *cost >= 0.0) {
                slot.row.cost_usd = Some(slot.row.cost_usd.unwrap_or(0.0) + cost);
            }
        });
    }

    /// Records its worktree and branch, or that it no longer has them.
    pub fn set_checkout(&self, worktree: Option<PathBuf>, branch: Option<String>) {
        self.registry.with_slot(&self.id, |slot| {
            slot.row.worktree = worktree;
            slot.row.branch = branch;
        });
    }

    /// Records where its transcript is kept.
    pub fn set_transcript(&self, path: PathBuf) {
        self.registry
            .with_slot(&self.id, |slot| slot.row.transcript = Some(path));
    }

    /// Takes the messages sent to it since its last step.
    #[must_use]
    pub fn take_messages(&self) -> Vec<String> {
        self.registry
            .with_slot(&self.id, |slot| {
                slot.row.pending_messages = 0;
                std::mem::take(&mut slot.inbox)
            })
            .unwrap_or_default()
    }

    /// Ends this run and queues its one notice, which it also returns.
    pub fn finish(mut self, outcome: Outcome) -> Option<Notice> {
        self.end(outcome)
    }

    fn end(&mut self, outcome: Outcome) -> Option<Notice> {
        if self.finished {
            return None;
        }
        self.finished = true;
        let stopped = self.stopped();
        let mut inner = self.registry.lock();
        inner.revision += 1;
        let index = inner
            .agents
            .iter()
            .position(|slot| slot.row.id == self.id)?;
        let slot = &mut inner.agents[index];
        let now = now_ms();
        slot.row.ended_ms = Some(now);
        // Messages that arrived too late for this run are not lost: they
        // stay queued and are named in the notice.
        let unread = std::mem::take(&mut slot.inbox);
        slot.row.pending_messages = 0;
        match outcome {
            Outcome::Done(report) => {
                slot.row.status = Status::Done;
                slot.row.report = Some(report);
                slot.row.error = None;
            }
            Outcome::Failed(_) | Outcome::Stopped if stopped => {
                slot.row.status = Status::Stopped;
                slot.row.error = None;
            }
            Outcome::Stopped => {
                slot.row.status = Status::Stopped;
                slot.row.error = None;
            }
            Outcome::Failed(error) => {
                slot.row.status = Status::Failed;
                slot.row.error = Some(error);
            }
        }
        let row = &slot.row;
        let mut notice = Notice {
            id: row.id.clone(),
            name: row.name.clone(),
            engine: row.engine.clone(),
            status: row.status,
            elapsed_seconds: row.elapsed_seconds(now),
            tokens: row.tokens,
            cost_usd: row.cost_usd,
            worktree: row.worktree.clone(),
            branch: row.branch.clone(),
            report: if row.status == Status::Done {
                row.report.clone()
            } else {
                None
            },
            error: row.error.clone(),
            parent_session: row.parent_session.clone(),
        };
        if !unread.is_empty() {
            let note = format!(
                "It ended before reading {} message(s) sent to it: {}",
                unread.len(),
                unread.join(" | ")
            );
            notice.error = Some(match notice.error.take() {
                Some(error) => format!("{error}\n{note}"),
                None => note,
            });
        }
        inner.notices.push(notice.clone());
        Some(notice)
    }
}

impl<E> Drop for Control<E> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.end(Outcome::Failed("The agent stopped unexpectedly.".into()));
        }
    }
}

/// Runs `run` for `control` on its own thread and finishes the agent with
/// what it returns.
pub fn spawn<E, F>(control: Control<E>, run: F) -> std::io::Result<std::thread::JoinHandle<()>>
where
    E: Send + 'static,
    F: FnOnce(&Control<E>) -> Outcome + Send + 'static,
{
    std::thread::Builder::new()
        .name(format!("agent-{}", control.id()))
        .spawn(move || {
            let outcome = run(&control);
            control.finish(outcome);
        })
}

/// The free-disk floor for new worktree agents and the shared build pool.
pub mod guard {
    use std::path::{Path, PathBuf};

    /// A person's own floor, in GB, for starting worktree agents.
    pub const FLOOR_VAR: &str = "OPENAGENTS_AGENT_DISK_FLOOR_GB";
    /// The floor when nobody set one.
    pub const DEFAULT_FLOOR_GB: u64 = 20;
    /// How many build folders agents share.
    pub const POOL_VAR: &str = "OPENAGENTS_AGENT_TARGET_POOL";
    /// The pool size when nobody set one.
    pub const DEFAULT_POOL: u64 = 4;

    /// The floor `env` chooses.
    #[must_use]
    pub fn floor_gb(env: &dyn Fn(&str) -> Option<String>) -> u64 {
        env(FLOOR_VAR)
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(DEFAULT_FLOOR_GB)
    }

    /// The pool size `env` chooses, at least one.
    #[must_use]
    pub fn pool_size(env: &dyn Fn(&str) -> Option<String>) -> u64 {
        env(POOL_VAR)
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(DEFAULT_POOL)
            .max(1)
    }

    /// Refuses a new worktree agent when the volume holding `path` has
    /// less than `floor_gb` free.
    ///
    /// # Errors
    /// A sentence with the free space and the floor.
    pub fn check_disk(
        path: &Path,
        floor_gb: u64,
        free: &dyn Fn(&Path) -> std::io::Result<u64>,
    ) -> Result<(), String> {
        let Ok(bytes) = free(path) else {
            // An unreadable volume is not a reason to refuse.
            return Ok(());
        };
        let gb = bytes / 1_000_000_000;
        if gb < floor_gb {
            return Err(format!(
                "Only {gb} GB is free, under the {floor_gb} GB floor for new background agents. Free some space, or set {FLOOR_VAR} to a lower number."
            ));
        }
        Ok(())
    }

    /// [`check_disk`] on this machine, with the floor from the environment.
    ///
    /// # Errors
    /// As [`check_disk`].
    pub fn check_disk_here(path: &Path) -> Result<(), String> {
        check_disk(
            path,
            floor_gb(&|name| std::env::var(name).ok()),
            &coder_lease::free_disk,
        )
    }

    /// The build folder agent number `index` uses: one of `size` shared
    /// folders under `root`, so worktrees reuse each other's builds instead
    /// of each filling a fresh one.
    #[must_use]
    pub fn target_slot(root: &Path, index: u64, size: u64) -> PathBuf {
        root.join(format!("slot-{}", index % size.max(1)))
    }
}

/// The `worktree/<id>` lease a running agent holds, so nothing removes its
/// checkout under it.
pub mod lease {
    use std::path::Path;

    use coder_lease::{Broker, Error, Holder, Lease, Limits, Request, Resource, Wait};

    fn broker(root: &Path) -> Broker {
        // Exclusive leases never read the counted limits.
        Broker::new(
            root.to_path_buf(),
            Limits {
                build: 1,
                memory_gib: 1,
                disk_floor_gb: 0,
                build_disk_gb: 0,
            },
        )
    }

    fn request(id: &str) -> Request {
        Request::new(
            Resource::Worktree(id.to_owned()),
            Holder::detect("coder-agent"),
        )
        .wait(Wait::No)
    }

    /// Takes the lease for agent `id`'s worktree. Hold it for the whole
    /// run; dropping it releases it.
    ///
    /// # Errors
    /// Someone already holds it, or the lease table can't be written.
    pub fn hold(root: &Path, id: &str) -> Result<Lease, String> {
        broker(root)
            .acquire(request(id))
            .map_err(|error| match error {
                Error::Busy(_) => format!("Agent {id}'s worktree is already in use."),
                other => format!("The worktree lease could not be taken: {other}"),
            })
    }

    /// Whether agent `id`'s worktree is held by a running agent.
    #[must_use]
    pub fn held(root: &Path, id: &str) -> bool {
        match broker(root).acquire(request(id)) {
            Ok(lease) => {
                let _ = lease.release(None);
                false
            }
            Err(Error::Busy(_)) => true,
            Err(_) => false,
        }
    }

    /// Refuses to remove a worktree whose agent is running.
    ///
    /// # Errors
    /// The agent still holds its lease.
    pub fn removable(root: &Path, id: &str) -> Result<(), String> {
        if held(root, id) {
            Err(format!(
                "Agent {id} is still running in that worktree; stop it first."
            ))
        } else {
            Ok(())
        }
    }
}

/// The folder agent `id`'s transcript is kept in, under `root`, grouped by
/// the parent chat.
#[must_use]
pub fn transcript_path(root: &Path, parent_session: Option<&str>, id: &str) -> PathBuf {
    let parent: String = parent_session
        .filter(|session| !session.is_empty())
        .unwrap_or("unsaved")
        .chars()
        .take(96)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    root.join(parent).join(format!("{id}.json"))
}

#[cfg(test)]
mod tests;
