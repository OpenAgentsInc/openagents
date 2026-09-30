//! Coder on this computer, for the person sitting at it.
//!
//! When a person asks for coding from a terminal (`openagents chat`), or
//! from an app, on the computer they are using, nothing has to be paired or
//! registered first: the project is the Git checkout they are in, and the
//! providers are the coding agents signed in here. This module starts that
//! run and follows it, through exactly what a host's auto-start uses:
//!
//! - **The same start.** A task is submitted to a task store (by default
//!   `~/.openagents/tasks`, the store a host on this computer serves; see
//!   [`default_store`]) and started with an execution grant through
//!   [`Policy::launch`], so the same engine (`microcoder repository`), the
//!   same provider failover, and the same ATIF recording run it.
//! - **Its own worktree.** Coder never writes in the person's checkout.
//!   Each task gets a detached worktree of the checkout's `HEAD` under
//!   `worktrees/` beside the store; the checkout changes only by Git's
//!   record of the worktree. Uncommitted changes in the checkout are not
//!   carried over.
//! - **Providers detected here.** Codex, then Claude Code, each only when
//!   [`capacity::probe`] finds its login on this computer (no network, no
//!   credential read), skipping one with a refusal that still holds in the
//!   store's capacity book and, when the store holds a fresh usage
//!   reading, one near its limit ([`Policy::choose`]). The start says which
//!   provider it chose and why.
//! - **One event stream.** [`Follow`] reads the task's trajectories and its
//!   store record and yields [`openagents_chat::coder_events`] lines, the
//!   stream the CLI prints and the apps render. A finished turn replays
//!   identically: what the turn changed is computed once and kept in the
//!   run's record.
//!
//! The person running this is the computer's owner acting for themselves,
//! so no device grant is involved; see the INVARIANTS row on local runs.
//! Nothing here reads or prints a credential.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openagents_chat::coder_events::{self, CoderEvent, FileChange, Line, Mapper, Started};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::autostart::{self, Choice, Engine, Launch, Policy, Route, UsageProbe};
use super::capacity::{self, Connection, Provider};
use super::{
    Action, COMMAND_SCHEMA, Command, RequestedConfiguration, Status, Store, TaskIntent, Workspace,
    adapter, owner, usage,
};

/// The routes a local run admits, in preference order: Codex, then Claude
/// Code, with the models the desktop's auto-start switch admits.
pub const ROUTES: [(Provider, &str); 2] = [
    (Provider::Codex, "gpt-6-luna"),
    (Provider::Claude, "claude-opus-5-5"),
];
/// Names another task store than [`default_store`].
pub const STORE_VAR: &str = "OPENAGENTS_TASKS";
/// Names the engine (`microcoder`) instead of the one beside the running
/// program or in `~/.openagents/bin`.
pub const CONTROLLER_VAR: &str = "OPENAGENTS_CODER_CONTROLLER";
/// The record a local run keeps beside its task.
pub const RECORD_SCHEMA: &str = "openagents.coder.local-run.v1";
/// The host name a thread's binding gives a local run.
pub const LOCAL_HOST: &str = "local";
/// How many steps a turn may take, and for how long.
const MAX_STEPS: usize = 24;
const WALL_SECONDS: u64 = 1800;
const MEMORY_BYTES: u64 = 4096 * 1024 * 1024;
/// How long a started turn may wait for its owner before a missing
/// admission counts as a failure, when the owner left no diagnostic.
const ADMISSION_WAIT: u64 = 120;

/// The task store local runs use: `$OPENAGENTS_TASKS`, else
/// `~/.openagents/tasks`, the store `coder task` and a host on this
/// computer use by default.
#[must_use]
pub fn default_store() -> PathBuf {
    if let Some(dir) = std::env::var_os(STORE_VAR).filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents/tasks")
}

/// The engine that runs a turn: `$OPENAGENTS_CODER_CONTROLLER`, else the
/// `microcoder` beside the running program or in `~/.openagents/bin`.
///
/// # Errors
/// Names where it looked.
pub fn controller() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(CONTROLLER_VAR).filter(|v| !v.is_empty()) {
        return PathBuf::from(path)
            .canonicalize()
            .map_err(|_| format!("{CONTROLLER_VAR} names no file"));
    }
    autostart::default_controller().and_then(|path| {
        path.canonicalize()
            .map_err(|_| "the microcoder engine is missing".into())
    })
}

/// A person's Git checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkout {
    /// Its top level.
    pub top: PathBuf,
    /// Its folder's name, which names the project.
    pub name: String,
    /// The commit `HEAD` names.
    pub head: String,
}

fn git() -> std::process::Command {
    let program = owner::GIT_PATHS
        .iter()
        .find(|path| Path::new(path).exists())
        .copied()
        .unwrap_or("git");
    let mut command = std::process::Command::new(program);
    command.stdin(std::process::Stdio::null());
    command
}

fn git_out(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = git()
        .arg("-C")
        .arg(coder_boundary::plain_path(dir))
        .args(args)
        .output()
        .map_err(|_| "cannot run git".to_owned())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The Git checkout `dir` is in.
///
/// # Errors
/// A plain sentence when `dir` is not in a checkout or its checkout has no
/// commit yet.
pub fn checkout(dir: &Path) -> Result<Checkout, String> {
    let top = git_out(dir, &["rev-parse", "--show-toplevel"]).map_err(|_| {
        format!(
            "{} is not in a Git checkout, so Coder has no project here. Run this from the \
             project's folder.",
            dir.display()
        )
    })?;
    let top = PathBuf::from(top.trim());
    let top = top.canonicalize().unwrap_or(top);
    let head = git_out(&top, &["rev-parse", "--verify", "HEAD^{commit}"]).map_err(|_| {
        format!(
            "{} has no commit yet; Coder works in a worktree of a commit. Commit once and try again.",
            top.display()
        )
    })?;
    let name: String = top
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into())
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(64)
        .collect();
    let name = if name.is_empty() || name.starts_with('.') {
        "project".into()
    } else {
        name
    };
    Ok(Checkout {
        top,
        name,
        head: head.trim().to_owned(),
    })
}

/// How one turn started.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnStart {
    pub turn: usize,
    pub revision: u64,
    pub provider: String,
    pub model: String,
    pub reason: String,
    pub fallbacks: Vec<String>,
    /// Unix seconds.
    pub at: u64,
}

/// What a local run keeps beside its task, in `<store>/local/<task>.json`:
/// where it works, how each turn started, and how each finished turn ended,
/// so following it again replays the same events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    pub project: String,
    pub checkout: String,
    pub worktree: String,
    pub base: String,
    pub turns: Vec<TurnStart>,
    /// Each finished turn's ending event, by turn.
    #[serde(default)]
    pub ends: BTreeMap<usize, CoderEvent>,
}

impl Record {
    /// The `coder_started` event of `turn`.
    #[must_use]
    pub fn started(&self, turn: usize) -> Option<CoderEvent> {
        let start = self.turns.iter().find(|start| start.turn == turn)?;
        Some(CoderEvent::CoderStarted(Started {
            turn,
            project: self.project.clone(),
            checkout: self.checkout.clone(),
            worktree: self.worktree.clone(),
            base: self.base.clone(),
            provider: start.provider.clone(),
            model: start.model.clone(),
            reason: start.reason.clone(),
            fallbacks: start.fallbacks.clone(),
            via: LOCAL_HOST.into(),
        }))
    }
}

fn record_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.json"))
}

/// The record of `task` in `store`, if a local run started it.
#[must_use]
pub fn record(store: &Path, task: &str) -> Option<Record> {
    let bytes = std::fs::read(record_path(store, task)).ok()?;
    serde_json::from_slice::<Record>(&bytes)
        .ok()
        .filter(|record| record.schema == RECORD_SCHEMA && record.task == task)
}

fn save(store: &Path, record: &Record) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    autostart::write_private(&record_path(store, &record.task), &bytes)
}

/// Starts, answers, and stops Coder runs on this computer.
pub struct Local {
    store: PathBuf,
    worktrees: PathBuf,
    launcher: Box<dyn Launch>,
    probe: fn(Provider) -> Connection,
    now: fn() -> u64,
    controller: Option<PathBuf>,
}

impl std::fmt::Debug for Local {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Local")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl Local {
    /// Runs over the task store `store`, with worktrees beside it and the
    /// engine started as a detached process.
    #[must_use]
    pub fn new(store: PathBuf) -> Self {
        let worktrees = store.parent().map_or_else(
            || store.join("worktrees"),
            |parent| parent.join("worktrees"),
        );
        Local {
            store,
            worktrees,
            launcher: Box::new(autostart::Process),
            probe: capacity::probe,
            now: autostart::unix_now,
            controller: None,
        }
    }

    /// Start turns with `launcher` instead of a detached engine process.
    #[must_use]
    pub fn with_launcher(mut self, launcher: Box<dyn Launch>) -> Self {
        self.launcher = launcher;
        self
    }

    /// Decide which providers are signed in with `probe`.
    #[must_use]
    pub fn with_probe(mut self, probe: fn(Provider) -> Connection) -> Self {
        self.probe = probe;
        self
    }

    /// Name the engine instead of [`controller`].
    #[must_use]
    pub fn with_controller(mut self, controller: PathBuf) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Keep worktrees in `dir`.
    #[must_use]
    pub fn with_worktrees(mut self, dir: PathBuf) -> Self {
        self.worktrees = dir;
        self
    }

    #[must_use]
    pub fn store(&self) -> &Path {
        &self.store
    }

    fn policy(&self, label: &str) -> Result<Policy, String> {
        let controller = match &self.controller {
            Some(path) => path.clone(),
            None => controller()?,
        };
        let routes: Vec<Route> = ROUTES
            .iter()
            .map(|(provider, model)| Route {
                provider: *provider,
                model: (*model).into(),
                effort: None,
            })
            .collect();
        let policy = Policy {
            schema: autostart::POLICY_SCHEMA.into(),
            enabled: true,
            workspaces: vec![label.into()],
            max_running: 1,
            engine: Engine {
                adapter: adapter::NAME.into(),
                controller,
                model: routes[0].model.clone(),
                effort: Some("medium".into()),
                max_steps: MAX_STEPS,
                wall_seconds: WALL_SECONDS,
                memory_bytes: MEMORY_BYTES,
                write_workspace: true,
                decision_endpoint: "https://api.typesafe.ai".into(),
                decision_model: autostart::DEFAULT_DECISION_MODEL.into(),
                routes,
                // Honors a fresh reading the store already holds, from a
                // host's usage probe; this asks no provider.
                usage_probe: Some(UsageProbe {
                    threshold_percent: usage::DEFAULT_THRESHOLD_PERCENT,
                }),
                access: adapter::Access::Toolchains,
            },
            changed_at: (self.now)(),
        };
        policy.validate()?;
        Ok(policy)
    }

    /// The routes to start on, first one first, and why that one.
    ///
    /// # Errors
    /// Why no provider can start: none is signed in here, or every one
    /// signed in has a refusal that holds.
    pub fn choose(&self, policy: &Policy) -> Result<(Vec<Route>, String), String> {
        let now = (self.now)();
        let book = capacity::Book::load(&self.store);
        let readings = usage::Book::load(&self.store);
        let probe = self.probe;
        match policy.choose(&book, &readings, &probe, now) {
            Choice::Start { order } => {
                let reason = reason(policy, &order, &book, &readings, probe, now);
                Ok((order, reason))
            }
            Choice::NoCapacity { until } => Err(format!(
                "No coding agent signed in here has capacity{}.",
                until
                    .map(|at| format!("; the earliest resets {}", coder_events::utc(at)))
                    .unwrap_or_default()
            )),
            Choice::Unconnected { .. } => Err(
                "Neither Codex nor Claude Code is signed in on this computer. Sign in to one \
                 (`codex login`, or run `claude` and log in) and try again."
                    .into(),
            ),
        }
    }

    /// Whether a run could start here now: a provider is signed in with
    /// capacity. Reads only.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.policy("project")
            .ok()
            .is_some_and(|policy| self.choose(&policy).is_ok())
    }

    /// Start a task for `prompt` in the checkout `dir` is in, titled
    /// `title`, for the chat `thread`.
    ///
    /// # Errors
    /// A plain sentence: no checkout here, no provider, or a start that
    /// failed.
    pub fn start(
        &self,
        dir: &Path,
        title: &str,
        prompt: &str,
        thread: Option<&str>,
    ) -> Result<Record, String> {
        let checkout = checkout(dir)?;
        let policy = self.policy(&checkout.name)?;
        let (order, reason) = self.choose(&policy)?;
        let now = (self.now)();
        let task = identity(&format!(
            "task:{}:{}:{}:{}",
            checkout.top.display(),
            thread.unwrap_or(""),
            now,
            nonce()
        ));
        let worktree = self.worktree(&checkout, &task)?;
        let model = order[0].model.clone();
        let intent = TaskIntent {
            title: one_line(title, 200),
            prompt: prompt.to_owned(),
            workspace: Workspace {
                path: worktree.display().to_string(),
                source_revision: Some(checkout.head.clone()),
            },
            configuration: RequestedConfiguration {
                adapter: adapter::NAME.into(),
                model: Some(model),
            },
        };
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: identity(&format!("submit:{task}")),
            task_id: task.clone(),
            expected_revision: None,
            action: Action::Submit { intent },
        };
        let submitted = self.apply(&command).and_then(|()| {
            let store = Store::open(&self.store).map_err(|e| e.to_string())?;
            store.show(&task).map_err(|e| e.to_string())
        });
        let submitted = match submitted {
            Ok(task) => task,
            Err(why) => {
                self.remove_worktree(&checkout.top, &worktree);
                return Err(format!("Coder could not save the task: {why}"));
            }
        };
        let mut record = Record {
            schema: RECORD_SCHEMA.into(),
            task: task.clone(),
            thread: thread.map(str::to_owned),
            project: checkout.name.clone(),
            checkout: checkout.top.display().to_string(),
            worktree: worktree.display().to_string(),
            base: checkout.head.clone(),
            turns: Vec::new(),
            ends: BTreeMap::new(),
        };
        self.launch(&policy, &order, reason, &submitted, &mut record)?;
        Ok(record)
    }

    fn launch(
        &self,
        policy: &Policy,
        order: &[Route],
        reason: String,
        task: &super::Task,
        record: &mut Record,
    ) -> Result<(), String> {
        record.turns.push(TurnStart {
            turn: task.turn(),
            revision: task.revision,
            provider: order[0].provider.as_str().into(),
            model: order[0].model.clone(),
            reason,
            fallbacks: order[1..].iter().map(ToString::to_string).collect(),
            at: (self.now)(),
        });
        // The record first, so a follower always knows how the turn began.
        save(&self.store, record)?;
        policy
            .launch(
                &self.store.join("local").join("grants"),
                &self.store,
                order,
                &task.task_id,
                &task.intent_digest,
                task.revision,
                self.launcher.as_ref(),
            )
            .map(|_| ())
            .map_err(|why| format!("Coder could not start: {why}"))
    }

    /// Answer `task`'s question (or approval) with `text`: its next turn,
    /// in the same worktree, on the provider chosen now.
    ///
    /// # Errors
    /// The task is not waiting, is not a local run, or cannot start.
    pub fn answer(&self, task: &str, text: &str) -> Result<Record, String> {
        let mut record = record(&self.store, task)
            .ok_or("This task was not started on this computer from a chat.")?;
        let current = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|e| e.to_string())?;
        // A turn that ended, however it ended (finished, asked, failed,
        // or stopped), continues; a running one does not.
        if !matches!(current.status, Status::Finished | Status::Cancelled) {
            return Err("Coder is still working on this task; wait for it to ask.".into());
        }
        let policy = self.policy(&record.project)?;
        let (order, reason) = self.choose(&policy)?;
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: identity(&format!("continue:{task}:{}:{}", current.revision, nonce())),
            task_id: task.into(),
            expected_revision: Some(current.revision),
            action: Action::Continue {
                prompt: text.trim().to_owned(),
            },
        };
        self.apply(&command)?;
        let next = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|e| e.to_string())?;
        self.launch(&policy, &order, reason, &next, &mut record)?;
        Ok(record)
    }

    /// Ask `task`'s running turn to stop. The engine stops its command and
    /// ends the turn as stopped.
    ///
    /// # Errors
    /// The task is unknown or already ended.
    pub fn stop(&self, task: &str) -> Result<(), String> {
        let current = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|e| e.to_string())?;
        if !matches!(current.status, Status::Queued | Status::Running) {
            return Err("This task is not running.".into());
        }
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: identity(&format!("cancel:{task}:{}", current.revision)),
            task_id: task.into(),
            expected_revision: Some(current.revision),
            action: Action::Cancel {
                reason: "Stopped by the person who started it.".into(),
            },
        };
        self.apply(&command)
    }

    fn apply(&self, command: &Command) -> Result<(), String> {
        let bytes = serde_json::to_vec(command).map_err(|e| e.to_string())?;
        let mut store = Store::open(&self.store).map_err(|e| e.to_string())?;
        store.apply(&bytes).map(|_| ()).map_err(|e| e.to_string())
    }

    /// A detached worktree of the checkout's `HEAD` for `task`.
    fn worktree(&self, checkout: &Checkout, task: &str) -> Result<PathBuf, String> {
        crate::private::create_dir_all(&self.worktrees)
            .map_err(|_| format!("cannot create {}", self.worktrees.display()))?;
        let target = self
            .worktrees
            .join(format!("{}-{}", checkout.name, &task[..12]));
        git_out(
            &checkout.top,
            &[
                "worktree",
                "add",
                "--detach",
                "--quiet",
                &coder_boundary::plain_path(&target).display().to_string(),
                &checkout.head,
            ],
        )
        .map_err(|why| format!("Git could not make Coder's worktree: {why}"))?;
        target
            .canonicalize()
            .map_err(|_| "Coder's worktree is missing".into())
    }

    fn remove_worktree(&self, top: &Path, worktree: &Path) {
        let _ = git_out(
            top,
            &[
                "worktree",
                "remove",
                "--force",
                &worktree.display().to_string(),
            ],
        );
    }

    /// Follow `task`'s events from the first, for the chat `thread`;
    /// `answer` is how the person answers a question.
    #[must_use]
    pub fn follow(&self, task: &str, thread: Option<&str>, answer: Option<String>) -> Follow {
        Follow {
            store: self.store.clone(),
            task: task.into(),
            thread: thread.map(str::to_owned),
            answer,
            seq: 0,
            turn: 1,
            mapper: None,
            seen: 0,
            now: self.now,
            ended: None,
        }
    }
}

/// Whether Coder could start on this computer now over
/// [`default_store`]: [`Local::ready`], read at most every
/// [`READY_EVERY`] seconds. A host's chat asks it on every command, so it
/// tells the chat router a coding request can run here without a
/// registered project.
#[must_use]
pub fn ready_here() -> bool {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(u64, bool)>> = Mutex::new(None);
    let now = autostart::unix_now();
    let mut cache = CACHE.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some((at, ready)) = *cache
        && now.saturating_sub(at) < READY_EVERY
    {
        return ready;
    }
    let ready = Local::new(default_store()).ready();
    *cache = Some((now, ready));
    ready
}

/// How long [`ready_here`] keeps its answer, in seconds.
pub const READY_EVERY: u64 = 15;

/// Why the first route of `order` was chosen, in a sentence.
fn reason(
    policy: &Policy,
    order: &[Route],
    book: &capacity::Book,
    readings: &usage::Book,
    probe: fn(Provider) -> Connection,
    now: u64,
) -> String {
    let chosen = order[0].provider;
    let name = |provider: Provider| coder_events::provider_name(&json!(provider.as_str()));
    let mut passed = Vec::new();
    for route in policy.routes() {
        if route.provider == chosen {
            break;
        }
        let who = name(route.provider);
        if let Connection::Missing(_) = probe(route.provider) {
            passed.push(format!("{who} is not signed in here"));
        } else if let Some(refusal) = book.blocking(route.provider, now) {
            passed.push(format!(
                "{who} reached its {} until {}",
                serde_json::to_value(refusal.kind)
                    .ok()
                    .and_then(|kind| kind.as_str().map(|k| k.replace('_', " ")))
                    .unwrap_or_else(|| "limit".into()),
                coder_events::utc(refusal.until)
            ));
        } else if readings.near_limit(route.provider, usage::DEFAULT_THRESHOLD_PERCENT, now) {
            passed.push(format!(
                "{who} is near its usage limit ({})",
                readings.describe(route.provider, now)
            ));
        }
    }
    if passed.is_empty() {
        format!("{} is signed in and has capacity.", name(chosen))
    } else {
        format!("{}; using {}.", passed.join("; "), name(chosen))
    }
}

/// A 64-character hex identity for `seed`.
fn identity(seed: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(seed.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn nonce() -> String {
    let bytes: [u8; 16] = secp256k1::rand::random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn one_line(text: &str, max: usize) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("Coder task");
    let mut out: String = line.chars().filter(|c| !c.is_control()).take(max).collect();
    if out.trim().is_empty() {
        out = "Coder task".into();
    }
    out.trim().to_owned()
}

/// What changed in `worktree` since `base`: tracked changes and new files.
#[must_use]
pub fn changes(worktree: &Path, base: &str) -> Vec<FileChange> {
    let mut out: Vec<FileChange> = Vec::new();
    let statuses: BTreeMap<String, String> = git_out(worktree, &["diff", "--name-status", base])
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let code = parts.next()?;
            let path = parts.next_back()?;
            let status = match code.chars().next()? {
                'A' => "added",
                'D' => "deleted",
                'R' => "renamed",
                _ => "modified",
            };
            Some((path.to_owned(), status.to_owned()))
        })
        .collect();
    for line in git_out(worktree, &["diff", "--numstat", base])
        .unwrap_or_default()
        .lines()
    {
        let mut parts = line.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let path = path
            .rsplit(" => ")
            .next()
            .unwrap_or(path)
            .trim_end_matches('}');
        out.push(FileChange {
            path: path.to_owned(),
            status: statuses
                .get(path)
                .cloned()
                .unwrap_or_else(|| "modified".into()),
            added: added.parse().ok(),
            removed: removed.parse().ok(),
        });
    }
    for path in git_out(worktree, &["ls-files", "--others", "--exclude-standard"])
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.is_empty())
    {
        let lines = std::fs::read(worktree.join(path))
            .ok()
            .filter(|bytes| !bytes.contains(&0))
            .map(|bytes| {
                let count = bytes.iter().filter(|b| **b == b'\n').count() as u64;
                count + u64::from(bytes.last().is_some_and(|b| *b != b'\n'))
            });
        out.push(FileChange {
            path: path.to_owned(),
            status: "added".into(),
            added: lines,
            removed: lines.map(|_| 0),
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// What changed in `worktree` since `base` as one unified diff: tracked
/// changes, then each new file, at most `max` bytes (cut at a line). The
/// "What changed" pane reads it; nothing here writes.
#[must_use]
pub fn unified_diff(worktree: &Path, base: &str, max: usize) -> String {
    let run = |args: &[&str]| {
        git()
            .arg("-C")
            .arg(coder_boundary::plain_path(worktree))
            .args(["-c", "core.quotepath=off"])
            .args(args)
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default()
    };
    let mut out = run(&["diff", "--no-color", "--no-ext-diff", base]);
    for path in run(&["ls-files", "--others", "--exclude-standard"])
        .lines()
        .filter(|line| !line.is_empty())
    {
        if out.len() >= max {
            break;
        }
        // `--no-index` exits 1 when the files differ, which they do.
        out.push_str(&run(&[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-index",
            "--",
            "/dev/null",
            path,
        ]));
    }
    if out.len() > max {
        let mut end = max;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        let end = out[..end].rfind('\n').map_or(end, |at| at + 1);
        out.truncate(end);
    }
    out
}

/// Where a followed task is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// A turn is running or waiting to be admitted.
    Running,
    /// The last turn asked the person and waits for an answer.
    Waiting,
    /// The last turn ended.
    Ended,
}

/// Follows one task's events: every event from the first, then new ones as
/// they are recorded. Each [`Follow::poll`] returns what is new.
#[derive(Debug)]
pub struct Follow {
    store: PathBuf,
    task: String,
    thread: Option<String>,
    answer: Option<String>,
    seq: u64,
    turn: usize,
    /// The current turn's mapper, once its start was emitted.
    mapper: Option<Mapper>,
    /// Steps of the current turn already mapped.
    seen: usize,
    now: fn() -> u64,
    /// The current turn's ending was already emitted, and where it left
    /// the task: a later poll emits nothing more until another turn
    /// starts.
    ended: Option<State>,
}

impl Follow {
    #[must_use]
    pub fn task(&self) -> &str {
        &self.task
    }

    fn line(&mut self, event: CoderEvent, out: &mut Vec<Line>) {
        self.seq += 1;
        out.push(Line {
            seq: self.seq,
            task: self.task.clone(),
            thread: self.thread.clone(),
            event,
        });
    }

    /// The events recorded since the last poll, and where the task is.
    ///
    /// # Errors
    /// The task store cannot be read or holds no such task.
    pub fn poll(&mut self) -> Result<(Vec<Line>, State), String> {
        let mut out = Vec::new();
        loop {
            // The task first: a turn whose result is recorded has its whole
            // trajectory written before it.
            let task = Store::open(&self.store)
                .and_then(|store| store.show(&self.task))
                .map_err(|e| e.to_string())?;
            let mut record = record(&self.store, &self.task);
            if let Some(state) = self.ended {
                let runs = task.earlier.len() + usize::from(task.run.is_some());
                if task.turn() > self.turn && runs >= self.turn {
                    self.turn += 1;
                    self.mapper = None;
                    self.seen = 0;
                    self.ended = None;
                } else {
                    return Ok((out, state));
                }
            }
            if self.mapper.is_none() {
                if let Some(started) = record.as_ref().and_then(|r| r.started(self.turn)) {
                    self.line(started, &mut out);
                }
                self.mapper = Some(Mapper::new(self.turn, self.answer.clone()));
                self.seen = 0;
            }
            let runs: Vec<&owner::Run> = task.earlier.iter().chain(task.run.iter()).collect();
            let Some(run) = runs.get(self.turn - 1).copied() else {
                // Not admitted yet.
                if task.status == Status::Cancelled {
                    let event = CoderEvent::Stopped(coder_events::Stopped {
                        turn: self.turn,
                        message: "Coder stopped before the turn started.".into(),
                    });
                    self.line(event, &mut out);
                    self.ended = Some(State::Ended);
                    return Ok((out, State::Ended));
                }
                let since = record
                    .as_ref()
                    .and_then(|r| r.turns.iter().find(|t| t.turn == self.turn))
                    .map_or(0, |t| t.at);
                if let Some(why) = launch_error(&self.store, &self.task, since) {
                    let event = CoderEvent::Failure(coder_events::Failure {
                        turn: self.turn,
                        message: format!("Coder did not start: {why}"),
                        ending: Some("not_started".into()),
                        resets_at: None,
                    });
                    self.line(event, &mut out);
                    self.ended = Some(State::Ended);
                    return Ok((out, State::Ended));
                }
                if since > 0 && (self.now)().saturating_sub(since) > ADMISSION_WAIT {
                    let event = CoderEvent::Failure(coder_events::Failure {
                        turn: self.turn,
                        message: "Coder did not start: the engine never admitted the task.".into(),
                        ending: Some("not_started".into()),
                        resets_at: None,
                    });
                    self.line(event, &mut out);
                    self.ended = Some(State::Ended);
                    return Ok((out, State::Ended));
                }
                return Ok((out, State::Running));
            };
            let trace = self.store.join(&run.admission.trace_file);
            let steps: Vec<Value> = atif::log::read(&trace)
                .map(|recording| {
                    recording.document()["steps"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let mapper = self.mapper.as_mut().unwrap_or_else(|| unreachable!());
            let mut events = Vec::new();
            for step in steps.iter().skip(self.seen) {
                events.extend(mapper.step(step));
            }
            self.seen = self.seen.max(steps.len());
            for event in events {
                self.line(event, &mut out);
            }
            let Some(result) = &run.result else {
                return Ok((out, State::Running));
            };
            // The turn ended: its ending, as recorded the first time.
            let kept = record
                .as_ref()
                .and_then(|r| r.ends.get(&self.turn).cloned());
            let end = match kept {
                Some(end) => end,
                None => {
                    let mapper = self.mapper.as_ref().unwrap_or_else(|| unreachable!());
                    let worktree = record.as_ref().map_or_else(
                        || task.intent.workspace.path.clone(),
                        |r| r.worktree.clone(),
                    );
                    let base = record
                        .as_ref()
                        .map(|r| r.base.clone())
                        .or_else(|| task.intent.workspace.source_revision.clone());
                    let files = base
                        .map(|base| changes(Path::new(&worktree), &base))
                        .unwrap_or_default();
                    let resets_at = (result.ending == capacity::NO_CAPACITY_ENDING)
                        .then(|| {
                            let book = capacity::Book::load(&self.store);
                            book.earliest_reset(&[Provider::Codex, Provider::Claude], (self.now)())
                        })
                        .flatten();
                    let end = mapper.end(
                        &result.ending,
                        files,
                        &worktree,
                        &trace.display().to_string(),
                        resets_at,
                    );
                    if let Some(record) = record.as_mut() {
                        record.ends.insert(self.turn, end.clone());
                        let _ = save(&self.store, record);
                    }
                    end
                }
            };
            let waiting = matches!(end, CoderEvent::Question(_) | CoderEvent::Approval(_));
            self.line(end, &mut out);
            if task.turn() > self.turn {
                self.turn += 1;
                self.mapper = None;
                self.seen = 0;
                continue;
            }
            let state = if waiting {
                State::Waiting
            } else {
                State::Ended
            };
            self.ended = Some(state);
            return Ok((out, state));
        }
    }
}

/// The error the engine's launcher wrote for `task` since `since`, if its
/// owner stopped before admitting it.
fn launch_error(store: &Path, task: &str, since: u64) -> Option<String> {
    let prefix = format!("repository-launch-{task}-");
    let entries = std::fs::read_dir(store).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || !name.ends_with(".jsonl") {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        if modified + 1 < since {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).ok()?;
        if let Some(error) = text
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find_map(|value| value["error"].as_str().map(str::to_owned))
        {
            return Some(error);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(dir: &Path) -> PathBuf {
        let top = dir.join("proj");
        std::fs::create_dir_all(&top).unwrap();
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "one",
            ],
        ] {
            assert!(
                git()
                    .arg("-C")
                    .arg(&top)
                    .args(&args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        top
    }

    #[test]
    fn outside_a_checkout_says_so_plainly() {
        let dir = tempfile::tempdir().unwrap();
        let error = checkout(dir.path()).unwrap_err();
        assert!(error.contains("is not in a Git checkout"), "{error}");
        let top = dir.path().join("empty");
        std::fs::create_dir_all(&top).unwrap();
        assert!(
            git()
                .arg("-C")
                .arg(&top)
                .args(["init", "-q"])
                .status()
                .unwrap()
                .success()
        );
        assert!(checkout(&top).unwrap_err().contains("has no commit yet"));
    }

    #[test]
    fn a_checkout_is_its_top_level_and_head() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let sub = top.join("src");
        std::fs::create_dir_all(&sub).unwrap();
        let found = checkout(&sub).unwrap();
        assert_eq!(found.top, top.canonicalize().unwrap());
        assert_eq!(found.name, "proj");
        assert_eq!(found.head.len(), 40);
    }

    fn both(_: Provider) -> Connection {
        Connection::Connected
    }

    fn only_claude(provider: Provider) -> Connection {
        match provider {
            Provider::Claude => Connection::Connected,
            _ => Connection::Missing("no login".into()),
        }
    }

    fn nobody(_: Provider) -> Connection {
        Connection::Missing("no login".into())
    }

    fn local(dir: &Path, probe: fn(Provider) -> Connection) -> Local {
        Local::new(dir.join("tasks"))
            .with_probe(probe)
            .with_controller(std::env::current_exe().unwrap())
    }

    #[test]
    fn codex_first_then_claude_code_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let run = local(dir.path(), both);
        let policy = run.policy("proj").unwrap();
        // A person's own run uses this computer's tools (#10045).
        assert_eq!(policy.engine.access, adapter::Access::Toolchains);
        let (order, reason) = run.choose(&policy).unwrap();
        assert_eq!(order[0].provider, Provider::Codex);
        assert_eq!(order[1].provider, Provider::Claude);
        assert_eq!(reason, "Codex is signed in and has capacity.");

        // A refusal that holds in this store's book passes Codex over.
        let now = autostart::unix_now();
        capacity::record(
            run.store(),
            capacity::Refusal::new(
                Provider::Codex,
                capacity::Kind::UsageLimit,
                now,
                Some(now + 3600),
            ),
        )
        .unwrap();
        let (order, reason) = run.choose(&policy).unwrap();
        assert_eq!(order[0].provider, Provider::Claude);
        assert!(
            reason.starts_with("Codex reached its usage limit until "),
            "{reason}"
        );
        assert!(reason.ends_with("; using Claude Code."), "{reason}");

        let run = local(dir.path(), only_claude);
        let fresh = tempfile::tempdir().unwrap();
        let run2 = local(fresh.path(), only_claude);
        let (_, reason) = run2.choose(&run2.policy("proj").unwrap()).unwrap();
        assert_eq!(reason, "Codex is not signed in here; using Claude Code.");
        // Claude Code alone, with Codex refused: still Claude Code.
        assert!(run.choose(&policy).is_ok());

        let none = local(fresh.path(), nobody);
        assert!(
            none.choose(&policy)
                .unwrap_err()
                .contains("Neither Codex nor Claude Code is signed in")
        );
    }

    #[test]
    fn changes_name_modified_and_new_files_with_their_lines() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        std::fs::write(top.join("a.txt"), "one\n").unwrap();
        for args in [
            vec!["add", "a.txt"],
            vec![
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                "a",
            ],
        ] {
            assert!(
                git()
                    .arg("-C")
                    .arg(&top)
                    .args(&args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let base = git_out(&top, &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_owned();
        std::fs::write(top.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(top.join("new.txt"), "x\ny").unwrap();
        let found = changes(&top, &base);
        let diff = unified_diff(&top, &base, 64 * 1024);
        assert!(diff.contains("diff --git a/a.txt b/a.txt\n"), "{diff}");
        assert!(diff.contains("+two\n"), "{diff}");
        assert!(diff.contains("diff --git a/new.txt b/new.txt\n"), "{diff}");
        assert!(diff.contains("+x\n+y"), "{diff}");
        assert!(unified_diff(&top, &base, 40).len() <= 40);
        assert_eq!(
            found,
            vec![
                FileChange {
                    path: "a.txt".into(),
                    status: "modified".into(),
                    added: Some(1),
                    removed: Some(0)
                },
                FileChange {
                    path: "new.txt".into(),
                    status: "added".into(),
                    added: Some(2),
                    removed: Some(0)
                },
            ]
        );
    }
}
