//! The owner's auto-start policy: start tasks that enrolled devices create.
//!
//! `task.create` from an enrolled device is an inert submission. This policy
//! is the one exception, and only the host's owner can turn it on, locally,
//! with `coder host autostart on`. It is off by default and off whenever its
//! file is missing or unreadable. While it is on, a task a device creates in
//! an allowlisted workspace is recorded as *eligible*, and the host starts it
//! through the configured engine with an execution grant the policy bounds:
//!
//! - **Workspaces**: only the labels the policy lists, each also admitted by
//!   the host's own settings. A device still never sends a path.
//! - **Concurrency**: at most `max_running` auto-started tasks run at once;
//!   the rest wait queued and start as earlier ones finish.
//! - **Engine**: one adapter (`microcoder-repository`), one controller
//!   executable, the admitted models in the owner's preference order, step
//!   and wall-clock limits, and the same filesystem boundary and supervisor
//!   as a hand-written grant.
//! - **Routes**: each start names the first admitted route whose provider
//!   has a local login and no recorded usage-limit refusal (see
//!   [`super::capacity`]); the other connected routes follow as the grant's
//!   fallbacks. When none has capacity, the task ends as `no_capacity` with
//!   the earliest reset instead of starting a run that cannot succeed.
//! - **Usage probes** (off unless the owner passes `--probe-usage`): the
//!   host also reads each admitted provider's usage windows (see
//!   [`super::usage`]) and prefers a route whose provider is below the
//!   policy's threshold. A probe is advisory: it never adds a route and
//!   never overrides a recorded refusal, and any probe failure leaves the
//!   refusal-only choice above.
//!
//! Every decision is appended to `autostart.jsonl` beside the policy:
//! eligible, started (with the grant digest and owner process), skipped, and
//! refused, plus each policy change. Turning the policy off stops new starts
//! at once; tasks already started keep running under their grants and can
//! be cancelled as usual. Read `docs/coder/runtime/host-autostart.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::capacity::{self, Connection, Provider};
use super::usage;
use super::{Action, COMMAND_SCHEMA, Command, Status, Store, adapter, owner};

/// The policy file in the host root.
pub const POLICY_FILE: &str = "autostart.json";
/// The append-only record in the host root.
pub const JOURNAL_FILE: &str = "autostart.jsonl";
pub const POLICY_SCHEMA: &str = "openagents.coder.host-autostart.v1";
pub const ENTRY_SCHEMA: &str = "openagents.coder.host-autostart-entry.v1";
/// The most tasks a policy may run at once.
pub const MAX_RUNNING: u32 = 8;
/// The decision model a new policy names. The engine refuses a Jev reply
/// whose model differs from the admitted one, so this is an exact version,
/// never an alias such as `jev-latest`.
pub const DEFAULT_DECISION_MODEL: &str = "jev-1.13.0";
/// How long a started task may stay queued, waiting for its owner process
/// to admit it, before it stops counting against the concurrency bound.
const PENDING_GRACE: u64 = 120;
/// How often the host looks for eligible tasks it could not start earlier.
pub const SWEEP_EVERY: Duration = Duration::from_secs(10);
/// How long a sweep waits for a busy task store. A save's disk sync can
/// hold the store lock for seconds while a build writes to a nearly full
/// volume, as on a host being updated; no device waits on a sweep.
pub const STORE_WAIT: Duration = Duration::from_secs(120);

/// The owner's policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub enabled: bool,
    /// Workspace labels whose tasks may start.
    pub workspaces: Vec<String>,
    /// Auto-started tasks that may run at once, 1 to 8.
    pub max_running: u32,
    pub engine: Engine,
    /// When the owner last changed the policy, in Unix seconds.
    pub changed_at: u64,
}

/// What runs an auto-started task, and its bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// Only `microcoder-repository`.
    pub adapter: String,
    /// The absolute path of the `microcoder` executable that owns the run.
    pub controller: PathBuf,
    /// The model recorded in each eligible task and admitted by its grant.
    pub model: String,
    pub effort: Option<String>,
    pub max_steps: usize,
    pub wall_seconds: u64,
    pub memory_bytes: u64,
    /// Let the engine write the workspace. The workspace must then be an
    /// isolated Git worktree, as the task owner requires.
    pub write_workspace: bool,
    pub decision_endpoint: String,
    pub decision_model: String,
    /// The admitted routes, in preference order. Empty means the one route
    /// every policy had before routes existed: the Codex login with `model`
    /// and `effort`. When set, the first route's model is `model`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<Route>,
    /// Probe each admitted provider's usage windows before routing. Absent
    /// means off: no credential is read for a probe and routing uses
    /// recorded refusals only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_probe: Option<UsageProbe>,
    /// What each started task's commands may reach. Absent means the
    /// filesystem boundary, so a policy written before this field keeps
    /// its meaning; `full` is the owner's full access
    /// (`coder host autostart on --full-access`).
    #[serde(default, skip_serializing_if = "adapter::Access::is_boundary")]
    pub access: adapter::Access,
}

/// The owner's usage-probe setting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageProbe {
    /// Utilization, in percent (1 to 100), at or above which routing
    /// prefers another admitted route with capacity.
    pub threshold_percent: u8,
}

/// One admitted provider and model, in a policy's preference order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub provider: Provider,
    /// The exact model identity the provider reports.
    pub model: String,
    /// The route's effort; the engine's when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.provider, self.model)
    }
}

/// The route a start chose, and the admitted, connected routes that follow
/// it as the grant's fallbacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Start on `order[0]`; the rest are fallbacks, in preference order.
    Start { order: Vec<Route> },
    /// Every connected admitted provider has a refusal that holds; the
    /// earliest one ends at `until`.
    NoCapacity { until: Option<u64> },
    /// No admitted provider has a usable login here.
    Unconnected { why: String },
}

impl Policy {
    /// The admitted routes in preference order.
    #[must_use]
    pub fn routes(&self) -> Vec<Route> {
        if self.engine.routes.is_empty() {
            vec![Route {
                provider: Provider::Codex,
                model: self.engine.model.clone(),
                effort: None,
            }]
        } else {
            self.engine.routes.clone()
        }
    }

    /// Choose a route at `now`: the first admitted route whose provider is
    /// connected and has capacity in `book`. A one-route policy skips the
    /// connection probe, so an existing policy starts exactly as before
    /// and a login problem shows in the task's diagnostic file.
    ///
    /// With usage probes on, a route whose provider's fresh reading in
    /// `usage` is at or above the threshold is passed over for a later
    /// route with capacity that is below it. When every route with
    /// capacity is near its limit, the first one still starts: a reading
    /// is advisory, and only a recorded refusal ends a task as
    /// `no_capacity`.
    #[must_use]
    pub fn choose(
        &self,
        book: &capacity::Book,
        usage: &usage::Book,
        probe: &dyn Fn(Provider) -> Connection,
        now: u64,
    ) -> Choice {
        let routes = self.routes();
        let mut missing = Vec::new();
        let connected: Vec<Route> = if routes.len() == 1 {
            routes
        } else {
            let mut probed: BTreeMap<Provider, Connection> = BTreeMap::new();
            routes
                .into_iter()
                .filter(|route| {
                    let connection = probed
                        .entry(route.provider)
                        .or_insert_with(|| probe(route.provider));
                    match connection {
                        Connection::Connected => true,
                        Connection::Missing(why) => {
                            missing.push(format!("{}: {why}", route.provider));
                            false
                        }
                    }
                })
                .collect()
        };
        if connected.is_empty() {
            missing.dedup();
            return Choice::Unconnected {
                why: format!("no admitted provider is connected ({})", missing.join("; ")),
            };
        }
        let with_capacity: Vec<usize> = (0..connected.len())
            .filter(|index| book.has_capacity(connected[*index].provider, now))
            .collect();
        let below = |index: &&usize| {
            self.engine.usage_probe.as_ref().is_none_or(|setting| {
                !usage.near_limit(connected[**index].provider, setting.threshold_percent, now)
            })
        };
        match with_capacity
            .iter()
            .find(below)
            .or_else(|| with_capacity.first())
            .copied()
        {
            Some(index) => {
                let mut order = connected;
                let chosen = order.remove(index);
                order.insert(0, chosen);
                Choice::Start { order }
            }
            None => {
                let providers: Vec<Provider> = connected.iter().map(|r| r.provider).collect();
                Choice::NoCapacity {
                    until: book.earliest_reset(&providers, now),
                }
            }
        }
    }

    /// Check the policy's own bounds.
    ///
    /// # Errors
    /// Names the first bound it breaks.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != POLICY_SCHEMA {
            return Err("the auto-start policy has an unsupported schema".into());
        }
        if self.workspaces.is_empty() || self.workspaces.iter().any(|w| w.is_empty()) {
            return Err("the policy needs at least one workspace label".into());
        }
        if !(1..=MAX_RUNNING).contains(&self.max_running) {
            return Err(format!("max_running is 1 to {MAX_RUNNING}"));
        }
        let engine = &self.engine;
        if engine.routes.len() > 1 + adapter::MAX_FALLBACKS {
            return Err(format!(
                "a policy admits at most {} routes",
                1 + adapter::MAX_FALLBACKS
            ));
        }
        if engine
            .routes
            .first()
            .is_some_and(|first| first.model != engine.model)
        {
            return Err("the first route's model must be the engine's model".into());
        }
        if engine
            .usage_probe
            .as_ref()
            .is_some_and(|setting| !(1..=100).contains(&setting.threshold_percent))
        {
            return Err("the usage threshold is 1 to 100 percent".into());
        }
        if engine.adapter != adapter::NAME {
            return Err(format!("the only engine adapter is {}", adapter::NAME));
        }
        if !engine.controller.is_absolute() {
            return Err("the controller path must be absolute".into());
        }
        // The same checks the task owner makes at admission, so a policy
        // that could never start a task refuses now.
        self.configuration(&self.routes())
            .validate()
            .map_err(|e| e.to_string())?;
        if !(1..=3600).contains(&engine.wall_seconds) {
            return Err("wall_seconds is 1 to 3600".into());
        }
        if !(64 * 1024 * 1024..=8 * 1024 * 1024 * 1024).contains(&engine.memory_bytes) {
            return Err("memory_bytes is 64 MiB to 8 GiB".into());
        }
        Ok(())
    }

    /// Whether a task in `workspace` starts: the policy is on and lists it.
    #[must_use]
    pub fn admits(&self, workspace: &str) -> bool {
        self.enabled && self.workspaces.iter().any(|w| w == workspace)
    }

    /// The grant configuration that starts on `order[0]` and falls back to
    /// the rest. `order` must not be empty.
    fn configuration(&self, order: &[Route]) -> adapter::Configuration {
        let engine = &self.engine;
        let route = |route: &Route| adapter::Route {
            provider: route.provider.as_str().into(),
            model: route.model.clone(),
            // Devin and OpenCode take no effort: their model names carry
            // their own.
            effort: (!matches!(route.provider, Provider::Devin | Provider::OpenCode))
                .then(|| route.effort.clone().or_else(|| engine.effort.clone()))
                .flatten(),
            generation_endpoint: route.provider.endpoint().into(),
        };
        let primary = route(&order[0]);
        adapter::Configuration {
            schema: adapter::CONFIG_SCHEMA.into(),
            provider: primary.provider,
            model: primary.model,
            effort: primary.effort,
            generation_endpoint: primary.generation_endpoint,
            decision_endpoint: engine.decision_endpoint.clone(),
            decision_model: engine.decision_model.clone(),
            max_steps: engine.max_steps,
            acceptance: false,
            route: "never".into(),
            knowledge: "off".into(),
            dollar_limit_micros: None,
            expected_controller_digest: None,
            container: None,
            fallbacks: order[1..].iter().map(route).collect(),
            access: engine.access,
        }
    }

    /// Read `root/autostart.json`. `None` when the file is missing: off.
    ///
    /// # Errors
    /// Reports an unreadable or invalid policy. A caller that must decide
    /// treats it as off.
    pub fn load(root: &Path) -> Result<Option<Self>, String> {
        let path = root.join(POLICY_FILE);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(format!("cannot read {}", path.display())),
        };
        let policy: Self = serde_json::from_slice(&bytes)
            .map_err(|_| format!("{} is malformed", path.display()))?;
        policy.validate()?;
        Ok(Some(policy))
    }

    /// Write `root/autostart.json`, mode `0600`, atomically.
    ///
    /// # Errors
    /// Refuses an invalid policy and reports a failed write.
    pub fn save(&self, root: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        write_private(&root.join(POLICY_FILE), &bytes)
    }
}

/// One journal line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub schema: String,
    pub at: u64,
    /// `eligible`, `started`, `skipped`, `refused`, `no_capacity`,
    /// `usage` (the probed windows a start was routed with),
    /// `unadmitted`, `policy_on`, or `policy_off`.
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// For a follow-up's later turn: the task revision that turn started
    /// at. Absent means the first turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<u64>,
    /// The enrolled device that created the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_process: Option<u32>,
    /// Why, for a skip or refusal; the policy's bounds for a policy change.
    /// Never a prompt or title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// For `no_capacity`: when the earliest admitted provider's limit
    /// resets, in Unix seconds, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
}

impl Entry {
    fn new(at: u64, event: &str) -> Self {
        Self {
            schema: ENTRY_SCHEMA.into(),
            at,
            event: event.into(),
            task: None,
            turn: None,
            device: None,
            workspace: None,
            grant_digest: None,
            owner_process: None,
            detail: None,
            resets_at: None,
        }
    }

    fn task(mut self, task: &str) -> Self {
        self.task = Some(task.into());
        self
    }

    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Name a later turn by the revision it started at; the first is left
    /// implicit, as before turns.
    fn at_turn(mut self, turn: u64) -> Self {
        self.turn = (turn > 1).then_some(turn);
        self
    }

    /// The task and turn this entry decides.
    fn subject(&self) -> Option<(String, u64)> {
        self.task.clone().map(|task| (task, self.turn.unwrap_or(1)))
    }
}

/// Append one entry to `root/autostart.jsonl`, created `0600`.
///
/// # Errors
/// Reports a failed write.
pub fn record(root: &Path, entry: &Entry) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|_| "cannot create the host root".to_owned())?;
    let mut line = serde_json::to_vec(entry).map_err(|e| e.to_string())?;
    line.push(b'\n');
    let mut file = crate::private::file(std::fs::OpenOptions::new().append(true).create(true))
        .open(root.join(JOURNAL_FILE))
        .map_err(|_| "cannot open the auto-start journal".to_owned())?;
    file.write_all(&line)
        .and_then(|()| file.sync_all())
        .map_err(|_| "cannot write the auto-start journal".into())
}

/// Every readable entry, oldest first. Unreadable lines are skipped.
#[must_use]
pub fn journal(root: &Path) -> Vec<Entry> {
    std::fs::read_to_string(root.join(JOURNAL_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// What the task owner's detached launcher reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launched {
    pub owner_process: u32,
    pub grant_digest: String,
}

/// Starts the engine's detached owner for one grant.
pub trait Launch: Send + Sync {
    /// # Errors
    /// Returns why the owner could not start.
    fn launch(
        &self,
        engine: &Engine,
        grant: &Path,
        store: &Path,
    ) -> std::result::Result<Launched, String>;
}

/// Runs `CONTROLLER repository --grant FILE --store DIR --detach`.
pub struct Process;

impl Launch for Process {
    fn launch(
        &self,
        engine: &Engine,
        grant: &Path,
        store: &Path,
    ) -> std::result::Result<Launched, String> {
        let mut command = std::process::Command::new(&engine.controller);
        command
            .env_clear()
            .env("PATH", owner::SYSTEM_PATH)
            .arg("repository")
            .arg("--grant")
            .arg(grant)
            .arg("--store")
            .arg(store)
            .arg("--detach")
            .stdin(std::process::Stdio::null());
        if let Some(home) = std::env::var_os("HOME") {
            command.env("HOME", home);
        }
        // The judge's client must name the decision model the grant admits.
        if engine.decision_model != "jev-latest" {
            command.env("TYPESAFE_DEFAULT_MODEL", &engine.decision_model);
        }
        let output = command
            .output()
            .map_err(|_| "the controller could not start".to_owned())?;
        let text = String::from_utf8_lossy(&output.stdout);
        let value: serde_json::Value = text
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str(line).ok())
            .unwrap_or_default();
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "the controller refused the launch: {}",
                error
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(200)
                    .collect::<String>()
            ));
        }
        Ok(Launched {
            owner_process: value["owner_process"]
                .as_u64()
                .and_then(|pid| u32::try_from(pid).ok())
                .unwrap_or(0),
            grant_digest: value["grant_digest"].as_str().unwrap_or("").to_owned(),
        })
    }
}

/// The host's auto-starter: shared by the inbox and its periodic sweep.
pub struct Autostart {
    root: PathBuf,
    store: PathBuf,
    workspaces: BTreeMap<String, PathBuf>,
    launcher: Box<dyn Launch>,
    now: fn() -> u64,
    probe: fn(Provider) -> Connection,
    fetch: usage::Fetch,
    sweeping: Mutex<()>,
    background: bool,
    store_wait: Duration,
}

impl std::fmt::Debug for Autostart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Autostart")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Autostart {
    /// An auto-starter reading its policy from `root`, starting tasks from
    /// the task store at `store` in the host's admitted `workspaces`.
    #[must_use]
    pub fn new(
        root: PathBuf,
        store: PathBuf,
        workspaces: BTreeMap<String, PathBuf>,
        launcher: Box<dyn Launch>,
        now: fn() -> u64,
    ) -> Self {
        Self {
            root,
            store,
            workspaces,
            launcher,
            now,
            probe: capacity::probe,
            fetch: usage::fetch,
            sweeping: Mutex::new(()),
            background: true,
            store_wait: STORE_WAIT,
        }
    }

    /// Wait up to `wait` for a busy task store instead of [`STORE_WAIT`],
    /// for tests.
    #[must_use]
    pub fn with_store_wait(mut self, wait: Duration) -> Self {
        self.store_wait = wait;
        self
    }

    /// Decide which providers are connected with `probe` instead of the
    /// local login probe, for tests.
    #[must_use]
    pub fn with_probe(mut self, probe: fn(Provider) -> Connection) -> Self {
        self.probe = probe;
        self
    }

    /// Ask usage endpoints with `fetch` instead of the network, for tests.
    #[must_use]
    pub fn with_usage_fetch(mut self, fetch: usage::Fetch) -> Self {
        self.fetch = fetch;
        self
    }

    /// Whether the policy ended `task` for lack of capacity, and if so the
    /// reset it recorded.
    #[must_use]
    pub fn no_capacity(&self, task: &str) -> Option<Option<u64>> {
        journal(&self.root)
            .into_iter()
            .rev()
            .find(|entry| entry.event == "no_capacity" && entry.task.as_deref() == Some(task))
            .map(|entry| entry.resets_at)
    }

    /// Sweep on the creating thread instead of a new one, for tests.
    #[must_use]
    pub fn foreground(mut self) -> Self {
        self.background = false;
        self
    }

    /// Sweep after a creation: on a new thread, so the host answers the
    /// device first, unless [`Autostart::foreground`] asked otherwise.
    pub fn sweep_soon(self: &Arc<Self>) {
        if self.background {
            let autostart = self.clone();
            std::thread::spawn(move || {
                autostart.sweep();
            });
        } else {
            self.sweep();
        }
    }

    /// The policy, read fresh so turning it off applies at once. An
    /// unreadable policy is off.
    #[must_use]
    pub fn policy(&self) -> Option<Policy> {
        Policy::load(&self.root).ok().flatten()
    }

    /// The model to record for a new task in `workspace`: the engine's when
    /// the policy admits the workspace, else `None`, as without a policy.
    #[must_use]
    pub fn model_for(&self, workspace: &str) -> Option<String> {
        self.policy()
            .filter(|policy| policy.admits(workspace) && self.workspaces.contains_key(workspace))
            .map(|policy| policy.engine.model)
    }

    /// Record that `device` created `task` in `workspace` under the policy.
    pub fn eligible(&self, task: &str, device: &str, workspace: &str) {
        self.eligible_turn(task, device, workspace, 1);
    }

    /// Record that `device` continued `task` in `workspace` into the turn
    /// that started at revision `turn` under the policy. The turn starts
    /// exactly as a new task would, under the same bounds.
    pub fn eligible_turn(&self, task: &str, device: &str, workspace: &str, turn: u64) {
        let mut entry = Entry::new((self.now)(), "eligible")
            .task(task)
            .at_turn(turn);
        entry.device = Some(device.into());
        entry.workspace = Some(workspace.into());
        if let Err(error) = record(&self.root, &entry) {
            eprintln!("coder host: auto-start: {error}");
        }
    }

    /// Start every eligible task the policy and its bounds allow now.
    /// Returns the entries it wrote.
    pub fn sweep(&self) -> Vec<Entry> {
        let Ok(_guard) = self.sweeping.lock() else {
            return Vec::new();
        };
        let Some(policy) = self.policy().filter(|p| p.enabled) else {
            return Vec::new();
        };
        let now = (self.now)();
        let history = journal(&self.root);
        let mut waiting: Vec<Entry> = Vec::new();
        let mut decided: BTreeSet<(String, u64)> = BTreeSet::new();
        let mut started: Vec<((String, u64), u64)> = Vec::new();
        let mut unadmitted: BTreeSet<(String, u64)> = BTreeSet::new();
        for entry in history {
            let Some(task) = entry.subject() else {
                continue;
            };
            match entry.event.as_str() {
                "eligible" if !waiting.iter().any(|w| w.subject() == entry.subject()) => {
                    waiting.push(entry);
                }
                "started" => {
                    started.push((task.clone(), entry.at));
                    decided.insert(task);
                }
                "unadmitted" => {
                    unadmitted.insert(task);
                }
                "skipped" | "refused" | "no_capacity" => {
                    decided.insert(task);
                }
                _ => {}
            }
        }
        waiting.retain(|entry| entry.subject().is_some_and(|t| !decided.contains(&t)));
        if waiting.is_empty() && started.iter().all(|(task, _)| unadmitted.contains(task)) {
            return Vec::new();
        }
        // Probe usage before opening the task store, so no request runs
        // under its lock. Cached, so a sweep every few seconds asks each
        // provider at most once per `usage::MIN_INTERVAL`.
        let usage_book = if policy.engine.usage_probe.is_some() && !waiting.is_empty() {
            let mut providers: Vec<Provider> =
                policy.routes().iter().map(|route| route.provider).collect();
            providers.dedup();
            usage::refresh(&self.store, &providers, now, self.fetch)
        } else {
            usage::Book::default()
        };
        let mut written = Vec::new();
        let mut write = |entry: Entry| {
            if let Err(error) = record(&self.root, &entry) {
                eprintln!("coder host: auto-start: {error}");
            }
            written.push(entry);
        };
        // Read the store once, then release its lock before any launch: the
        // launched owner opens the same store.
        let plans = {
            // A busy store is waited out: its eligible tasks stay eligible,
            // and a store still busy after the wait leaves them to the next
            // sweep.
            let mut store = match Store::open_waiting(&self.store, self.store_wait) {
                Ok(store) => store,
                Err(super::Error::Busy) => {
                    eprintln!(
                        "coder host: auto-start: the task store stayed busy for {} seconds; the next sweep tries again",
                        self.store_wait.as_secs()
                    );
                    return written;
                }
                Err(error) => {
                    eprintln!("coder host: auto-start cannot open the task store: {error}");
                    return written;
                }
            };
            let mut active = started
                .iter()
                .filter(|((task, turn), at)| match store.show(task) {
                    Ok(task) if task.turn_started() != *turn => false,
                    Ok(task) => match task.status {
                        Status::Running | Status::CancelRequested => true,
                        Status::Queued => {
                            task.run.is_none() && now.saturating_sub(*at) < PENDING_GRACE
                        }
                        _ => false,
                    },
                    Err(_) => false,
                })
                .count();
            // A started task still queued after the grace was never admitted
            // by its owner process; say so once, so the journal shows it.
            for (subject, at) in &started {
                let (id, turn) = subject;
                let stalled = store.show(id).is_ok_and(|task| {
                    task.turn_started() == *turn
                        && task.status == Status::Queued
                        && task.run.is_none()
                        && now.saturating_sub(*at) >= PENDING_GRACE
                });
                if stalled && !unadmitted.contains(subject) {
                    write(Entry::new(now, "unadmitted").task(id).at_turn(*turn).detail(
                        "the owner process did not admit the task; read its launch diagnostic in the task store",
                    ));
                }
            }
            let mut plans = Vec::new();
            for entry in &waiting {
                let id = entry.task.clone().unwrap_or_default();
                let turn = entry.subject().map_or(1, |(_, turn)| turn);
                let workspace = entry.workspace.clone().unwrap_or_default();
                let task = match store.show(&id) {
                    Ok(task) => task,
                    Err(_) => {
                        write(
                            Entry::new(now, "skipped")
                                .task(&id)
                                .at_turn(turn)
                                .detail("the task is gone"),
                        );
                        continue;
                    }
                };
                if task.turn_started() != turn
                    || task.status != Status::Queued
                    || task.run.is_some()
                {
                    write(
                        Entry::new(now, "skipped")
                            .task(&id)
                            .at_turn(turn)
                            .detail("the task is no longer queued"),
                    );
                    continue;
                }
                if !policy.admits(&workspace) || !self.workspaces.contains_key(&workspace) {
                    write(
                        Entry::new(now, "skipped")
                            .task(&id)
                            .at_turn(turn)
                            .detail("the policy no longer admits the workspace"),
                    );
                    continue;
                }
                if task.intent.configuration.model.as_deref() != Some(&policy.engine.model) {
                    write(
                        Entry::new(now, "skipped")
                            .task(&id)
                            .at_turn(turn)
                            .detail("the policy's model changed after the task was created"),
                    );
                    continue;
                }
                if active >= policy.max_running as usize {
                    break;
                }
                // Route at start time: capacity changes while tasks wait.
                let book = capacity::Book::load(&self.store);
                let order = match policy.choose(&book, &usage_book, &self.probe, now) {
                    Choice::Start { order } => order,
                    Choice::NoCapacity { until } => {
                        // Record first, then end the task, so the summary a
                        // device receives for the ending can name the reset.
                        let mut entry = Entry::new(now, "no_capacity").task(&id).at_turn(turn).detail(
                            "no admitted provider has capacity; the task ends instead of starting",
                        );
                        entry.workspace = Some(workspace.clone());
                        entry.resets_at = until;
                        write(entry);
                        end_without_capacity(&mut store, &id, turn, task.revision, until);
                        continue;
                    }
                    Choice::Unconnected { why } => {
                        write(
                            Entry::new(now, "refused")
                                .task(&id)
                                .at_turn(turn)
                                .detail(why),
                        );
                        continue;
                    }
                };
                active += 1;
                if policy.engine.usage_probe.is_some() {
                    let mut entry = Entry::new(now, "usage").task(&id).at_turn(turn).detail(
                        policy
                            .routes()
                            .iter()
                            .map(|route| {
                                format!(
                                    "{}: {}",
                                    route.provider,
                                    usage_book.describe(route.provider, now)
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; "),
                    );
                    entry.workspace = Some(workspace.clone());
                    write(entry);
                }
                plans.push((
                    id,
                    turn,
                    workspace,
                    task.intent_digest.clone(),
                    task.revision,
                    order,
                ));
            }
            plans
        };
        for (id, turn, workspace, intent_digest, revision, order) in plans {
            let entry = match self.start(&policy, &order, &id, &intent_digest, revision) {
                Ok(launched) => {
                    let mut entry = Entry::new(now, "started").task(&id).at_turn(turn);
                    entry.workspace = Some(workspace);
                    entry.grant_digest = Some(launched.grant_digest);
                    entry.owner_process = Some(launched.owner_process);
                    entry.detail = Some(format!(
                        "{} {} fallbacks [{}] max_steps {} wall_seconds {} write_workspace {} access {}",
                        policy.engine.adapter,
                        order[0],
                        order[1..]
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                        policy.engine.max_steps,
                        policy.engine.wall_seconds,
                        policy.engine.write_workspace,
                        policy.engine.access.as_str()
                    ));
                    entry
                }
                Err(why) => Entry::new(now, "refused")
                    .task(&id)
                    .at_turn(turn)
                    .detail(why),
            };
            write(entry);
        }
        written
    }

    fn start(
        &self,
        policy: &Policy,
        order: &[Route],
        task: &str,
        intent_digest: &str,
        revision: u64,
    ) -> std::result::Result<Launched, String> {
        let program = shell()?;
        let grant = owner::Grant {
            schema: owner::GRANT_SCHEMA.into(),
            task_id: task.into(),
            intent_digest: intent_digest.into(),
            expected_revision: revision,
            expected_source_snapshot: None,
            program,
            arguments: Vec::new(),
            write_workspace: policy.engine.write_workspace,
            wall_seconds: policy.engine.wall_seconds,
            stream_bytes: 64 * 1024,
            memory_bytes: policy.engine.memory_bytes,
            requirements: None,
            adapter_configuration: Some(policy.configuration(order)),
        };
        let bytes = serde_json::to_vec_pretty(&grant).map_err(|e| e.to_string())?;
        owner::Grant::parse(&bytes).map_err(|e| format!("the grant is invalid: {e}"))?;
        let path = self
            .root
            .join("autostart")
            .join(format!("{task}-{revision}.grant.json"));
        write_private(&path, &bytes)?;
        self.launcher.launch(&policy.engine, &path, &self.store)
    }
}

/// End a queued task that no admitted provider can serve: the host cancels
/// it with a reason that names the earliest reset. The command identity is
/// fixed per task, so a repeat after a crash is an exact retry.
fn end_without_capacity(
    store: &mut Store,
    task: &str,
    turn: u64,
    revision: u64,
    until: Option<u64>,
) {
    let reason = match until {
        Some(until) => format!(
            "No admitted model provider has capacity until {}.",
            capacity::utc(until)
        ),
        None => "No admitted model provider has capacity.".to_owned(),
    };
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: if turn > 1 {
            format!("autostart-no-capacity-{task}-{turn}")
        } else {
            format!("autostart-no-capacity-{task}")
        },
        task_id: task.into(),
        expected_revision: Some(revision),
        action: Action::Cancel { reason },
    };
    let applied = serde_json::to_vec(&command)
        .map_err(|e| e.to_string())
        .and_then(|bytes| store.apply(&bytes).map_err(|e| e.to_string()));
    if let Err(error) = applied {
        eprintln!("coder host: auto-start cannot end a task without capacity: {error}");
    }
}

/// Sweep every [`SWEEP_EVERY`] for as long as the process runs, so a task
/// that waited for a free slot starts when one opens.
pub fn spawn_sweeper(autostart: Arc<Autostart>) {
    let _ = std::thread::Builder::new()
        .name("coder-autostart".into())
        .spawn(move || {
            loop {
                autostart.sweep();
                std::thread::sleep(SWEEP_EVERY);
            }
        });
}

/// The canonical system shell the repository adapter admits.
/// The task owner's program runs under the Unix write boundary, which
/// Windows does not have, so auto-start refuses there.
#[cfg(not(unix))]
fn shell() -> std::result::Result<PathBuf, String> {
    Err("repository tasks need the Unix write boundary, which this computer does not have".into())
}

#[cfg(unix)]
fn shell() -> std::result::Result<PathBuf, String> {
    ["/bin/bash", "/bin/sh"]
        .iter()
        .find_map(|path| Path::new(path).canonicalize().ok())
        .ok_or_else(|| "no system shell at /bin/bash or /bin/sh".into())
}

fn write_private(path: &Path, bytes: &[u8]) -> std::result::Result<(), String> {
    let parent = path.parent().ok_or("no parent directory")?;
    crate::private::create_dir_all(parent)
        .map_err(|_| format!("cannot create {}", parent.display()))?;
    let temporary = path.with_extension("tmp");
    let mut file = crate::private::file(
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true),
    )
    .open(&temporary)
    .map_err(|_| format!("cannot write {}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| format!("cannot write {}", path.display()))?;
    std::fs::rename(&temporary, path).map_err(|_| format!("cannot replace {}", path.display()))
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub const USAGE: &str = "usage: coder host autostart COMMAND
  show [--store DIR] [--probe-usage]
                       Print the policy, each provider's login, capacity
                       (from DIR, default ~/.openagents/tasks), and usage
                       windows, and the latest decisions. Usage is probed
                       when the policy probes it or --probe-usage is given.
  on --workspace LABEL [--workspace LABEL]... [--max-running N]
     [--model ID | --route PROVIDER:MODEL [--route PROVIDER:MODEL]...]
     [--effort low|medium|high|xhigh] [--max-steps N] [--wall-seconds N]
     [--memory-mib N] [--read-only] [--controller PATH]
     [--decision-endpoint URL] [--decision-model ID]
     [--probe-usage] [--usage-threshold PERCENT] [--full-access]
                       Start tasks that enrolled devices with `operate` create
                       in these workspaces, at most N at once (default 1).
                       Each --route admits a provider (codex, claude,
                       devin, or opencode) and model, in preference order; a
                       task starts on the first one that is connected and
                       has capacity. A devin route (devin:default, or
                       devin:MODEL) hands the whole turn to the local Devin
                       CLI over ACP; an opencode route
                       (opencode:PROVIDER/MODEL) hands it to OpenCode.
                       --probe-usage reads each provider's usage windows
                       with its local login and prefers a route below
                       PERCENT (default 90) used.
                       --full-access runs each task's commands as you,
                       with no sandbox, network access, and your
                       login-shell environment. Use it only on your own
                       computer.
  off                  Stop starting tasks; queued tasks stay queued.
Every command takes --root DIR (default ~/.openagents/host). The policy is
off until `on` runs.";

/// `coder host autostart ARGS`. Returns the exit code.
pub fn cli(args: &[String]) -> u8 {
    match cli_inner(args) {
        Ok(()) => 0,
        Err(message) if message.starts_with("usage: ") => {
            eprintln!("coder host autostart: {}\n\n{USAGE}", &message[7..]);
            2
        }
        Err(message) => {
            eprintln!("coder host autostart: {message}");
            1
        }
    }
}

fn cli_inner(args: &[String]) -> std::result::Result<(), String> {
    let Some((command, rest)) = args.split_first() else {
        return Err("usage: give show, on, or off".into());
    };
    let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut read_only = false;
    let mut full_access = false;
    let mut probe_usage = false;
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--read-only" {
            read_only = true;
        } else if arg == "--full-access" {
            full_access = true;
        } else if arg == "--probe-usage" {
            probe_usage = true;
        } else if arg == "--help" {
            println!("{USAGE}");
            return Ok(());
        } else if arg.starts_with("--") {
            let value = rest
                .next()
                .ok_or_else(|| format!("usage: {arg} needs a value"))?;
            values.entry(arg.clone()).or_default().push(value.clone());
        } else {
            return Err(format!("usage: unexpected argument `{arg}`"));
        }
    }
    let root = match take_one(&mut values, "--root")? {
        Some(root) => PathBuf::from(root),
        None => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
            .join(".openagents/host"),
    };
    let now = unix_now();
    match command.as_str() {
        "show" => {
            match Policy::load(&root)? {
                Some(policy) => println!(
                    "{}",
                    serde_json::to_string_pretty(&policy).map_err(|e| e.to_string())?
                ),
                None => println!("off (no policy)"),
            }
            let store = match take_one(&mut values, "--store")? {
                Some(store) => PathBuf::from(store),
                None => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
                    .join(".openagents/tasks"),
            };
            let book = capacity::Book::load(&store);
            let probing = probe_usage
                || Policy::load(&root)
                    .ok()
                    .flatten()
                    .is_some_and(|policy| policy.engine.usage_probe.is_some());
            let usage_book = if probing {
                usage::refresh(&store, &Provider::PROBED, now, usage::fetch)
            } else {
                usage::Book::load(&store)
            };
            for provider in Provider::ALL {
                let connection = match capacity::probe(provider) {
                    Connection::Connected => "connected".to_owned(),
                    Connection::Missing(why) => format!("not connected ({why})"),
                };
                let capacity = match book.blocking(provider, now) {
                    Some(refusal) => format!("no capacity until {}", capacity::utc(refusal.until)),
                    None => "capacity".to_owned(),
                };
                println!(
                    "{provider}: {connection}; {capacity}; usage {}",
                    usage_book.describe(provider, now)
                );
            }
            for entry in journal(&root).iter().rev().take(20).rev() {
                println!(
                    "{}",
                    serde_json::to_string(entry).map_err(|e| e.to_string())?
                );
            }
        }
        "off" => {
            let Some(mut policy) = Policy::load(&root)? else {
                println!("off (no policy)");
                return Ok(());
            };
            policy.enabled = false;
            policy.changed_at = now;
            policy.save(&root)?;
            record(&root, &Entry::new(now, "policy_off"))?;
            println!("off");
        }
        "on" => {
            let workspaces = values.remove("--workspace").unwrap_or_default();
            if workspaces.is_empty() {
                return Err("usage: on needs --workspace LABEL".into());
            }
            let number = |value: Option<String>, name: &str, default: u64| {
                value.map_or(Ok(default), |v| {
                    v.parse::<u64>()
                        .map_err(|_| format!("usage: {name} takes a whole number"))
                })
            };
            let max_running = number(take_one(&mut values, "--max-running")?, "--max-running", 1)?;
            let max_steps = number(take_one(&mut values, "--max-steps")?, "--max-steps", 24)?;
            let wall_seconds = number(
                take_one(&mut values, "--wall-seconds")?,
                "--wall-seconds",
                1800,
            )?;
            let memory_mib = number(take_one(&mut values, "--memory-mib")?, "--memory-mib", 4096)?;
            let controller = match take_one(&mut values, "--controller")? {
                Some(path) => PathBuf::from(path),
                None => default_controller()?,
            };
            let controller = controller
                .canonicalize()
                .map_err(|_| format!("the controller {} does not exist", controller.display()))?;
            let routes = values
                .remove("--route")
                .unwrap_or_default()
                .iter()
                .map(|route| parse_route(route))
                .collect::<std::result::Result<Vec<Route>, String>>()?;
            let model = take_one(&mut values, "--model")?;
            let model = match (routes.first(), model) {
                (Some(_), Some(_)) => {
                    return Err(
                        "usage: give --model or --route, not both; name the Codex model as --route codex:MODEL"
                            .into(),
                    );
                }
                (Some(first), None) => first.model.clone(),
                (None, model) => model.unwrap_or_else(|| "gpt-6-luna".into()),
            };
            let engine = Engine {
                adapter: adapter::NAME.into(),
                controller,
                model,
                effort: Some(take_one(&mut values, "--effort")?.unwrap_or_else(|| "medium".into())),
                max_steps: usize::try_from(max_steps).map_err(|_| "--max-steps is too large")?,
                wall_seconds,
                memory_bytes: memory_mib.saturating_mul(1024 * 1024),
                write_workspace: !read_only,
                decision_endpoint: take_one(&mut values, "--decision-endpoint")?
                    .unwrap_or_else(|| "https://api.typesafe.ai".into()),
                decision_model: take_one(&mut values, "--decision-model")?
                    .unwrap_or_else(|| DEFAULT_DECISION_MODEL.into()),
                routes,
                usage_probe: None,
                access: if full_access {
                    adapter::Access::Full
                } else {
                    adapter::Access::Boundary
                },
            };
            let threshold = take_one(&mut values, "--usage-threshold")?;
            let mut engine = engine;
            if probe_usage || threshold.is_some() {
                let threshold_percent = match threshold {
                    Some(value) => value
                        .parse::<u8>()
                        .map_err(|_| "usage: --usage-threshold takes a percent, 1 to 100")?,
                    None => usage::DEFAULT_THRESHOLD_PERCENT,
                };
                engine.usage_probe = Some(UsageProbe { threshold_percent });
            }
            if let Some(name) = values.keys().next() {
                return Err(format!("usage: {name} does not apply to on"));
            }
            let policy = Policy {
                schema: POLICY_SCHEMA.into(),
                enabled: true,
                workspaces: workspaces.clone(),
                max_running: u32::try_from(max_running)
                    .map_err(|_| "--max-running is too large")?,
                engine,
                changed_at: now,
            };
            policy.validate()?;
            let settings =
                coder_host::settings::ServeSettings::load(&root).map_err(|e| e.to_string())?;
            for label in &workspaces {
                let path = settings.workspaces.get(label).ok_or_else(|| {
                    format!("the host admits no workspace labelled {label}; add it with coder link setup --workspace")
                })?;
                if policy.engine.write_workspace {
                    isolated_worktree(path)?;
                }
            }
            policy.save(&root)?;
            record(
                &root,
                &Entry::new(now, "policy_on").detail(format!(
                    "workspaces {} max_running {} routes {} write_workspace {} access {} usage_probe {}",
                    workspaces.join(","),
                    policy.max_running,
                    policy
                        .routes()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    policy.engine.write_workspace,
                    policy.engine.access.as_str(),
                    policy
                        .engine
                        .usage_probe
                        .as_ref()
                        .map_or_else(|| "off".to_owned(), |p| format!("{}%", p.threshold_percent))
                )),
            )?;
            println!("on");
        }
        _ => return Err(format!("usage: unknown command `{command}`")),
    }
    Ok(())
}

/// `PROVIDER:MODEL`, where the provider is one of the closed set.
fn parse_route(text: &str) -> std::result::Result<Route, String> {
    let (provider, model) = text
        .split_once(':')
        .ok_or_else(|| format!("usage: --route takes PROVIDER:MODEL, not `{text}`"))?;
    let provider = match Provider::from_config(provider) {
        Some(
            provider @ (Provider::Codex | Provider::Claude | Provider::Devin | Provider::OpenCode),
        ) => provider,
        Some(Provider::Vertex) => {
            return Err(format!(
                "usage: `{text}`: repository runs don't generate through vertex; use codex, claude, devin, or opencode"
            ));
        }
        None => {
            return Err(format!(
                "usage: the provider in `{text}` is not codex, claude, devin, or opencode"
            ));
        }
    };
    if model.is_empty() {
        return Err(format!("usage: `{text}` names no model"));
    }
    if provider == Provider::OpenCode
        && let Err(why) = acp_client::opencode::Model::parse(model)
    {
        return Err(format!(
            "usage: `{text}`: an opencode route names OpenCode's PROVIDER/MODEL: {why}"
        ));
    }
    Ok(Route {
        provider,
        model: model.into(),
        effort: None,
    })
}

fn take_one(
    values: &mut BTreeMap<String, Vec<String>>,
    name: &str,
) -> std::result::Result<Option<String>, String> {
    match values.remove(name) {
        None => Ok(None),
        Some(mut list) if list.len() == 1 => Ok(list.pop()),
        Some(_) => Err(format!("usage: {name} is given twice")),
    }
}

/// A writing grant needs a worktree whose common Git directory is outside
/// it, so the engine cannot rewrite the repository's history or hooks.
fn isolated_worktree(path: &Path) -> std::result::Result<(), String> {
    let git = owner::GIT_PATHS
        .iter()
        .find(|p| Path::new(p).exists())
        .copied()
        .unwrap_or("git");
    let output = std::process::Command::new(git)
        .arg("-C")
        .arg(path)
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ])
        .output()
        .map_err(|_| "cannot run git".to_owned())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines().map(str::trim);
    let top = PathBuf::from(lines.next().unwrap_or_default());
    let common = PathBuf::from(lines.next().unwrap_or_default());
    let root = path
        .canonicalize()
        .map_err(|_| "the workspace is missing".to_owned())?;
    if !output.status.success() || common.as_os_str().is_empty() {
        return Err(format!("{} is not a Git checkout", root.display()));
    }
    // An empty or plain directory inside another repository answers with
    // that repository; the workspace must be a checkout's own top level.
    if top.canonicalize().ok().as_deref() != Some(root.as_path()) {
        return Err(format!(
            "{} is not the top level of a Git checkout; create the worktree there \
             (git worktree add --detach {} origin/main)",
            root.display(),
            root.display()
        ));
    }
    let common = common.canonicalize().unwrap_or(common);
    if common.starts_with(&root) {
        return Err(format!(
            "{} holds its own Git directory; give the host an isolated worktree \
             (git worktree add) or pass --read-only",
            root.display()
        ));
    }
    Ok(())
}

fn default_controller() -> std::result::Result<PathBuf, String> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok())
        .and_then(|exe| exe.parent().map(|dir| dir.join("microcoder")))
        .filter(|path| path.is_file());
    let installed = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".openagents/bin/microcoder"))
        .filter(|path| path.is_file());
    beside.or(installed).ok_or_else(|| {
        "no microcoder beside coder or in ~/.openagents/bin; pass --controller".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::remote::Inbox;
    use coder_host::{Code, TaskCreate, Tasks};
    use std::cell::Cell;

    thread_local! {
        // Each test runs on its own thread and sweeps in the foreground, so
        // a per-thread clock keeps tests from moving each other's time.
        static CLOCK: Cell<u64> = const { Cell::new(1_000) };
    }

    fn clock() -> u64 {
        CLOCK.with(Cell::get)
    }

    fn advance(seconds: u64) {
        CLOCK.with(|clock| clock.set(clock.get() + seconds));
    }

    /// Records each launch instead of starting a process.
    #[derive(Default)]
    struct Fake(Arc<Mutex<Vec<PathBuf>>>);

    impl Launch for Fake {
        fn launch(&self, _: &Engine, grant: &Path, _: &Path) -> Result<Launched, String> {
            owner::Grant::parse(&std::fs::read(grant).unwrap()).unwrap();
            self.0.lock().unwrap().push(grant.to_path_buf());
            Ok(Launched {
                owner_process: 4242,
                grant_digest: "sha256:fake".into(),
            })
        }
    }

    struct Setup {
        _temp: tempfile::TempDir,
        root: PathBuf,
        inbox: Inbox,
        autostart: Arc<Autostart>,
        launched: Arc<Mutex<Vec<PathBuf>>>,
        store: PathBuf,
    }

    /// Every usage probe fails as if offline, so no test reaches a network.
    fn offline(_: Provider) -> Result<usage::Response, usage::Failure> {
        Err(usage::Failure::Network)
    }

    fn setup() -> Setup {
        setup_with(offline)
    }

    fn setup_with(fetch: usage::Fetch) -> Setup {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("host");
        let store = temp.path().join("tasks");
        let mut workspaces = BTreeMap::new();
        for label in ["allowed", "other"] {
            let path = temp.path().join(label);
            std::fs::create_dir_all(&path).unwrap();
            workspaces.insert(label.to_owned(), path.canonicalize().unwrap());
        }
        let launched = Arc::new(Mutex::new(Vec::new()));
        let autostart = Arc::new(
            Autostart::new(
                root.clone(),
                store.clone(),
                workspaces.clone(),
                Box::new(Fake(launched.clone())),
                clock,
            )
            .with_probe(|_| Connection::Connected)
            .with_usage_fetch(fetch)
            .foreground(),
        );
        let inbox = Inbox::new(&store, workspaces).with_autostart(autostart.clone());
        Setup {
            _temp: temp,
            root,
            inbox,
            autostart,
            launched,
            store,
        }
    }

    fn policy(max_running: u32) -> Policy {
        Policy {
            schema: POLICY_SCHEMA.into(),
            enabled: true,
            workspaces: vec!["allowed".into()],
            max_running,
            engine: Engine {
                adapter: adapter::NAME.into(),
                controller: PathBuf::from("/opt/coder/microcoder"),
                model: "gpt-6-luna".into(),
                effort: Some("medium".into()),
                max_steps: 24,
                wall_seconds: 1800,
                memory_bytes: 4 * 1024 * 1024 * 1024,
                write_workspace: true,
                decision_endpoint: "https://api.typesafe.ai".into(),
                decision_model: "jev-latest".into(),
                routes: Vec::new(),
                usage_probe: None,
                access: adapter::Access::Boundary,
            },
            changed_at: 1,
        }
    }

    fn routed(max_running: u32) -> Policy {
        let mut policy = policy(max_running);
        policy.engine.routes = vec![
            Route {
                provider: Provider::Codex,
                model: "gpt-6-luna".into(),
                effort: None,
            },
            Route {
                provider: Provider::Claude,
                model: "claude-opus-5-5".into(),
                effort: Some("high".into()),
            },
        ];
        policy
    }

    fn launched_grant(s: &Setup, index: usize) -> owner::Grant {
        owner::Grant::parse(&std::fs::read(&s.launched.lock().unwrap()[index]).unwrap()).unwrap()
    }

    /// Codex's weekly limit, as the backend reported it, observed now.
    fn exhaust_codex(store: &Path) {
        let body = r#"{"error":{"type":"usage_limit_reached","resets_at":500000,"resets_in_seconds":499000}}"#;
        capacity::record(store, capacity::Refusal::codex(429, body, clock()).unwrap()).unwrap();
    }

    fn create(workspace: &str) -> TaskCreate {
        TaskCreate {
            title: "Fix the flaky test".into(),
            prompt: "Find why it fails.".into(),
            workspace: workspace.into(),
        }
    }

    fn events(root: &Path) -> Vec<(String, Option<String>)> {
        journal(root)
            .into_iter()
            .map(|e| (e.event, e.task))
            .collect()
    }

    #[test]
    fn without_a_policy_creation_is_inert_and_unchanged() {
        let s = setup();
        let key = "a".repeat(64);
        s.inbox.create(&key, "device", &create("allowed")).unwrap();
        let task = Store::open(&s.store).unwrap().show(&key).unwrap();
        assert_eq!(task.intent.configuration.model, None);
        assert_eq!(task.status, Status::Queued);
        assert!(s.autostart.sweep().is_empty());
        assert!(s.launched.lock().unwrap().is_empty());
        assert!(journal(&s.root).is_empty());
    }

    #[test]
    fn a_policy_starts_admitted_tasks_within_its_bounds_and_records_each() {
        let s = setup();
        policy(1).save(&s.root).unwrap();
        let (first, second, outside) = ("b".repeat(64), "c".repeat(64), "d".repeat(64));
        s.inbox.create(&first, "phone", &create("allowed")).unwrap();
        // The first task started; its grant names the engine's model.
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        let task = Store::open(&s.store).unwrap().show(&first).unwrap();
        assert_eq!(
            task.intent.configuration.model.as_deref(),
            Some("gpt-6-luna")
        );
        let grant =
            owner::Grant::parse(&std::fs::read(&s.launched.lock().unwrap()[0]).unwrap()).unwrap();
        assert_eq!(grant.task_id, first);
        assert_eq!(grant.intent_digest, task.intent_digest);
        assert!(grant.write_workspace);
        assert_eq!(grant.adapter_configuration.unwrap().model, "gpt-6-luna");
        // A workspace outside the allowlist stays inert.
        s.inbox.create(&outside, "phone", &create("other")).unwrap();
        let task = Store::open(&s.store).unwrap().show(&outside).unwrap();
        assert_eq!(task.intent.configuration.model, None);
        // The second waits: the first still counts while its owner admits it.
        s.inbox
            .create(&second, "phone", &create("allowed"))
            .unwrap();
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        advance(PENDING_GRACE + 1);
        s.autostart.sweep();
        assert_eq!(s.launched.lock().unwrap().len(), 2);
        // Each decision is recorded once, with the creating device.
        let recorded = events(&s.root);
        assert_eq!(
            recorded,
            [
                ("eligible".into(), Some(first.clone())),
                ("started".into(), Some(first.clone())),
                ("eligible".into(), Some(second.clone())),
                // The fake owner never admits, so the first is reported once.
                ("unadmitted".into(), Some(first.clone())),
                ("started".into(), Some(second.clone())),
            ]
        );
        assert_eq!(journal(&s.root)[0].device.as_deref(), Some("phone"));
        assert_eq!(journal(&s.root)[1].owner_process, Some(4242));
        assert!(
            s.autostart.sweep().iter().all(|e| e.event != "started"),
            "nothing starts twice"
        );
    }

    /// Hold the task store's lock on another thread for `hold`.
    fn hold_store(store: &Path, hold: Duration) -> std::thread::JoinHandle<()> {
        let store = store.to_path_buf();
        let (held, taken) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _held = Store::open(&store).unwrap();
            held.send(()).unwrap();
            std::thread::sleep(hold);
        });
        taken.recv().unwrap();
        holder
    }

    #[test]
    fn a_sweep_waits_out_a_busy_store_and_its_tasks_stay_eligible() {
        let s = setup();
        policy(1).save(&s.root).unwrap();
        let (first, second) = ("b".repeat(64), "c".repeat(64));
        s.inbox.create(&first, "phone", &create("allowed")).unwrap();
        s.inbox
            .create(&second, "phone", &create("allowed"))
            .unwrap();
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        advance(PENDING_GRACE + 1);
        let sweeper = |wait: Duration| {
            Autostart::new(
                s.root.clone(),
                s.store.clone(),
                s.autostart.workspaces.clone(),
                Box::new(Fake(s.launched.clone())),
                clock,
            )
            .with_probe(|_| Connection::Connected)
            .with_usage_fetch(offline)
            .with_store_wait(wait)
            .foreground()
        };
        // A store busy past the sweep's wait leaves the task eligible.
        let holder = hold_store(&s.store, Duration::from_millis(1500));
        let gave_up = sweeper(Duration::from_millis(200)).sweep();
        holder.join().unwrap();
        assert!(gave_up.iter().all(|e| e.event != "started"), "{gave_up:?}");
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        assert!(
            !events(&s.root).contains(&("skipped".into(), Some(second.clone()))),
            "a busy store decides nothing"
        );
        // A sweep that waits longer than the holder starts it.
        let holder = hold_store(&s.store, Duration::from_millis(1500));
        let started = sweeper(Duration::from_secs(30)).sweep();
        holder.join().unwrap();
        assert!(
            started
                .iter()
                .any(|e| e.event == "started" && e.task.as_deref() == Some(second.as_str())),
            "{started:?}"
        );
        assert_eq!(s.launched.lock().unwrap().len(), 2);
    }

    fn follow_up(task: &str, command: &str, based_on: u64) -> coder_host::TaskCommand {
        coder_host::TaskCommand {
            command: command.repeat(64),
            task: task.into(),
            action: coder_host::CommandAction::Send,
            based_on,
            text: "Now fix the second flaky test.".into(),
            emulate: false,
            // The inbox dates commands by the host's clock.
            issued_at: unix_now(),
        }
    }

    #[test]
    fn a_follow_up_starts_its_turn_only_within_the_policy_bounds() {
        let s = setup();
        policy(1).save(&s.root).unwrap();
        let phone = coder_host::Principal {
            device: "phone".into(),
            grant: Some("e".repeat(64)),
            epoch: Some(1),
        };
        let (allowed, outside) = ("b".repeat(64), "d".repeat(64));
        s.inbox
            .create(&allowed, "phone", &create("allowed"))
            .unwrap();
        s.inbox.create(&outside, "phone", &create("other")).unwrap();
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        s.inbox
            .cancel(&"1".repeat(64), "phone", &allowed, 1, "Ended")
            .unwrap();
        s.inbox
            .cancel(&"2".repeat(64), "phone", &outside, 1, "Ended")
            .unwrap();
        // A follow-up in an admitted workspace starts its turn with a fresh
        // grant at the revision the turn started at.
        let first = follow_up(&allowed, "a", 2);
        let continued = s.inbox.command(&phone, &first, &|_| true).unwrap();
        assert_eq!(
            (continued.revision, continued.phase),
            (3, nostr::activity_summary::Phase::Queued)
        );
        assert_eq!(s.launched.lock().unwrap().len(), 2);
        let grant = launched_grant(&s, 1);
        assert_eq!(
            (grant.task_id.as_str(), grant.expected_revision),
            (allowed.as_str(), 3)
        );
        let started = journal(&s.root)
            .into_iter()
            .filter(|entry| entry.event == "started")
            .map(|entry| entry.turn)
            .collect::<Vec<_>>();
        assert_eq!(started, [None, Some(3)]);
        // A follow-up outside the policy's workspaces stays inert.
        s.inbox
            .command(&phone, &follow_up(&outside, "c", 2), &|_| true)
            .unwrap();
        s.autostart.sweep();
        assert_eq!(s.launched.lock().unwrap().len(), 2);
        let task = Store::open(&s.store).unwrap().show(&outside).unwrap();
        assert_eq!((task.status, task.run.is_none()), (Status::Queued, true));
        // A replay of the first follow-up, byte for byte, starts nothing
        // more.
        s.inbox.command(&phone, &first, &|_| true).unwrap();
        s.autostart.sweep();
        assert_eq!(s.launched.lock().unwrap().len(), 2);
    }

    #[test]
    fn turning_the_policy_off_stops_new_starts_and_cancelled_tasks_are_skipped() {
        let s = setup();
        policy(1).save(&s.root).unwrap();
        let (first, second, third) = ("e".repeat(64), "f".repeat(64), "0".repeat(64));
        s.inbox.create(&first, "phone", &create("allowed")).unwrap();
        s.inbox
            .create(&second, "phone", &create("allowed"))
            .unwrap();
        s.inbox.create(&third, "phone", &create("allowed")).unwrap();
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        // The owner cancels the second while it waits.
        s.inbox
            .cancel(&"1".repeat(64), "phone", &second, 1, "Not needed")
            .unwrap();
        let mut off = policy(1);
        off.enabled = false;
        off.save(&s.root).unwrap();
        advance(PENDING_GRACE + 1);
        assert!(s.autostart.sweep().is_empty());
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        // With the policy off, a new task records no model and is not eligible.
        let fourth = "2".repeat(64);
        s.inbox
            .create(&fourth, "phone", &create("allowed"))
            .unwrap();
        let task = Store::open(&s.store).unwrap().show(&fourth).unwrap();
        assert_eq!(task.intent.configuration.model, None);
        // On again: the cancelled task is skipped and the third starts.
        policy(1).save(&s.root).unwrap();
        let written: Vec<(String, Option<String>)> = s
            .autostart
            .sweep()
            .into_iter()
            .map(|e| (e.event, e.task))
            .collect();
        assert_eq!(
            written,
            [
                ("unadmitted".into(), Some(first.clone())),
                ("skipped".into(), Some(second.clone())),
                ("started".into(), Some(third.clone())),
            ]
        );
    }

    #[test]
    fn a_retry_across_a_policy_change_returns_the_original_receipt() {
        let s = setup();
        policy(1).save(&s.root).unwrap();
        let key = "3".repeat(64);
        let first = s.inbox.create(&key, "phone", &create("allowed")).unwrap();
        let mut off = policy(1);
        off.enabled = false;
        off.save(&s.root).unwrap();
        assert_eq!(
            s.inbox.create(&key, "phone", &create("allowed")).unwrap(),
            first
        );
        // Another request that reuses the key with other content still refuses.
        let changed = TaskCreate {
            prompt: "Something else".into(),
            ..create("allowed")
        };
        assert_eq!(s.inbox.create(&key, "phone", &changed), Err(Code::Conflict));
    }

    #[test]
    fn policies_outside_their_bounds_refuse() {
        let dir = tempfile::tempdir().unwrap();
        let mut bad = policy(0);
        assert!(bad.save(dir.path()).is_err());
        bad.max_running = MAX_RUNNING + 1;
        assert!(bad.validate().is_err());
        let mut bad = policy(1);
        bad.workspaces.clear();
        assert!(bad.validate().is_err());
        let mut bad = policy(1);
        bad.engine.adapter = "bounded-command".into();
        assert!(bad.validate().is_err());
        let mut bad = policy(1);
        bad.engine.controller = PathBuf::from("microcoder");
        assert!(bad.validate().is_err());
        let mut bad = policy(1);
        bad.engine.decision_endpoint = "http://api.typesafe.ai".into();
        assert!(bad.validate().is_err());
        let mut bad = policy(1);
        bad.engine.wall_seconds = 0;
        assert!(bad.validate().is_err());
        // A malformed file is off.
        std::fs::write(dir.path().join(POLICY_FILE), b"{").unwrap();
        assert!(Policy::load(dir.path()).is_err());
        let autostart = Autostart::new(
            dir.path().to_path_buf(),
            dir.path().join("tasks"),
            BTreeMap::new(),
            Box::new(Fake::default()),
            clock,
        );
        assert_eq!(autostart.policy(), None);
        assert_eq!(autostart.model_for("allowed"), None);
    }

    #[test]
    fn the_command_line_turns_the_policy_on_and_off() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let workspace = dir.path().join("checkout");
        std::fs::create_dir_all(&workspace).unwrap();
        coder_host::settings::ServeSettings::new(
            vec!["wss://relay.example/".into()],
            BTreeMap::from([("checkout".into(), workspace.canonicalize().unwrap())]),
        )
        .save(&root)
        .unwrap();
        let controller = std::env::current_exe().unwrap();
        let args = |list: &[&str]| -> Vec<String> {
            let mut args: Vec<String> = list.iter().map(|a| (*a).to_owned()).collect();
            args.extend(["--root".into(), root.to_string_lossy().into_owned()]);
            args
        };
        let controller = controller.to_string_lossy().into_owned();
        // An unknown label refuses; so does a writing policy on a directory
        // that is not an isolated worktree.
        let on = |label: &str, extra: &[&str]| {
            let mut list = vec!["on", "--workspace", label, "--controller", &controller];
            list.extend_from_slice(extra);
            cli(&args(&list))
        };
        assert_eq!(on("nope", &["--read-only"]), 1);
        assert_eq!(on("checkout", &[]), 1);
        assert_eq!(on("checkout", &["--read-only", "--max-running", "2"]), 0);
        let policy = Policy::load(&root).unwrap().unwrap();
        assert!(policy.enabled && !policy.engine.write_workspace);
        assert_eq!(policy.max_running, 2);
        assert_eq!(cli(&args(&["off"])), 0);
        assert!(!Policy::load(&root).unwrap().unwrap().enabled);
        let recorded: Vec<String> = journal(&root).into_iter().map(|e| e.event).collect();
        assert_eq!(recorded, ["policy_on", "policy_off"]);
        assert_eq!(cli(&args(&["on"])), 2);
    }

    /// Full access is the owner's choice, made with a command on the host,
    /// and every grant the policy writes carries it. A workspace must be a
    /// checkout's own top level: an empty directory inside another
    /// repository, as the owner's host had, refuses.
    #[test]
    fn the_owner_turns_on_full_access_on_the_host() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let repo = dir.path().join("repo");
        let checkout = dir.path().join("checkout");
        let empty = repo.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let git = |args: &[&str], cwd: &Path| {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(cwd)
                    .status()
                    .unwrap()
                    .success()
            );
        };
        git(&["init", "-q"], &repo);
        git(
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "Fixture",
            ],
            &repo,
        );
        git(
            &[
                "worktree",
                "add",
                "--detach",
                "-q",
                checkout.to_str().unwrap(),
            ],
            &repo,
        );
        coder_host::settings::ServeSettings::new(
            vec!["wss://relay.example/".into()],
            BTreeMap::from([
                ("checkout".into(), checkout.canonicalize().unwrap()),
                ("empty".into(), empty.canonicalize().unwrap()),
            ]),
        )
        .save(&root)
        .unwrap();
        let controller = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let root_arg = root.to_string_lossy().into_owned();
        let on = |label: &str, extra: &[&str]| {
            let mut list = vec![
                "on",
                "--workspace",
                label,
                "--controller",
                &controller,
                "--root",
                &root_arg,
            ];
            list.extend_from_slice(extra);
            cli(&list.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
        };
        assert_eq!(on("empty", &["--full-access"]), 1);
        assert!(Policy::load(&root).unwrap().is_none());
        assert_eq!(on("checkout", &[]), 0);
        let policy = Policy::load(&root).unwrap().unwrap();
        assert_eq!(policy.engine.access, adapter::Access::Boundary);
        assert!(policy.configuration(&policy.routes()).access.is_boundary());
        assert_eq!(on("checkout", &["--full-access"]), 0);
        let policy = Policy::load(&root).unwrap().unwrap();
        assert_eq!(policy.engine.access, adapter::Access::Full);
        let configuration = policy.configuration(&policy.routes());
        assert_eq!(configuration.access, adapter::Access::Full);
        configuration.validate().unwrap();
        let saved = std::fs::read_to_string(root.join(POLICY_FILE)).unwrap();
        assert!(saved.contains("\"access\": \"full\""), "{saved}");
        let last = journal(&root).pop().unwrap();
        assert!(
            last.detail
                .as_deref()
                .is_some_and(|d| d.contains("access full")),
            "{last:?}"
        );
    }

    #[test]
    fn an_existing_policy_file_keeps_its_meaning() {
        // A policy written before routes existed, byte for byte.
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"schema":"openagents.coder.host-autostart.v1","enabled":true,"workspaces":["allowed"],"max_running":1,"engine":{"adapter":"microcoder-repository","controller":"/opt/coder/microcoder","model":"gpt-6-luna","effort":"medium","max_steps":24,"wall_seconds":1800,"memory_bytes":4294967296,"write_workspace":true,"decision_endpoint":"https://api.typesafe.ai","decision_model":"jev-1.13.0"},"changed_at":1}"#;
        std::fs::write(dir.path().join(POLICY_FILE), old).unwrap();
        let policy = Policy::load(dir.path()).unwrap().unwrap();
        assert_eq!(
            policy.routes(),
            [Route {
                provider: Provider::Codex,
                model: "gpt-6-luna".into(),
                effort: None,
            }]
        );
        // It saves without a routes field, and its grant configuration is
        // the one every earlier grant carried.
        policy.save(dir.path()).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(POLICY_FILE)).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::from_str::<serde_json::Value>(old).unwrap()
        );
        let configuration = serde_json::to_value(policy.configuration(&policy.routes())).unwrap();
        assert_eq!(
            configuration,
            serde_json::json!({"schema":adapter::CONFIG_SCHEMA,"provider":"codex","model":"gpt-6-luna",
                "effort":"medium","generation_endpoint":"https://chatgpt.com/backend-api/codex",
                "decision_endpoint":"https://api.typesafe.ai","decision_model":"jev-1.13.0","max_steps":24,
                "acceptance":false,"route":"never","knowledge":"off","dollar_limit_micros":null,
                "expected_controller_digest":null})
        );
        // A one-route policy starts without a login probe, as before.
        let book = capacity::Book::default();
        assert!(matches!(
            policy.choose(
                &book,
                &usage::Book::default(),
                &|_| Connection::Missing("no login".into()),
                1
            ),
            Choice::Start { .. }
        ));
    }

    #[test]
    fn a_task_starts_on_the_first_connected_route_with_capacity() {
        let s = setup();
        routed(1).save(&s.root).unwrap();
        exhaust_codex(&s.store);
        let task = "4".repeat(64);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        // The task records the policy's first model; its grant starts on
        // Claude, which has capacity, and keeps Codex as a fallback.
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(
            stored.intent.configuration.model.as_deref(),
            Some("gpt-6-luna")
        );
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(
            (
                configuration.provider.as_str(),
                configuration.model.as_str()
            ),
            ("claude", "claude-opus-5-5")
        );
        assert_eq!(configuration.effort.as_deref(), Some("high"));
        assert_eq!(
            configuration.generation_endpoint,
            "https://api.anthropic.com"
        );
        assert_eq!(configuration.fallbacks.len(), 1);
        assert_eq!(configuration.fallbacks[0].model, "gpt-6-luna");
        assert_eq!(configuration.fallbacks[0].effort.as_deref(), Some("medium"));
        assert!(configuration.admits_model("gpt-6-luna"));
        let started = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "started")
            .unwrap();
        assert!(started.detail.unwrap().contains("claude:claude-opus-5-5"));
        // Once Codex resets, the next task starts on it again.
        advance(500_000);
        let next = "5".repeat(64);
        s.inbox.create(&next, "phone", &create("allowed")).unwrap();
        advance(PENDING_GRACE + 1);
        s.autostart.sweep();
        let configuration = launched_grant(&s, 1).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "codex");
    }

    #[test]
    fn an_unconnected_provider_is_not_routed_to() {
        let mut policy = routed(1);
        policy.engine.routes.reverse();
        policy.engine.model = "claude-opus-5-5".into();
        policy.validate().unwrap();
        let book = capacity::Book::default();
        let only_codex = |provider: Provider| match provider {
            Provider::Codex => Connection::Connected,
            Provider::Claude | Provider::Vertex | Provider::Devin | Provider::OpenCode => {
                Connection::Missing("not signed in".into())
            }
        };
        let usage = usage::Book::default();
        match policy.choose(&book, &usage, &only_codex, 1) {
            Choice::Start { order } => {
                assert_eq!(order.len(), 1);
                assert_eq!(order[0].provider, Provider::Codex);
            }
            other => panic!("{other:?}"),
        }
        match policy.choose(
            &book,
            &usage,
            &|_| Connection::Missing("no login".into()),
            1,
        ) {
            Choice::Unconnected { why } => assert!(why.contains("claude: no login")),
            other => panic!("{other:?}"),
        }
        // The first route's model must be the model a task records.
        policy.engine.model = "gpt-6-luna".into();
        assert!(policy.validate().is_err());
    }

    #[test]
    fn without_capacity_a_task_ends_as_no_capacity_with_the_reset() {
        let s = setup();
        // The old one-route policy: Codex only, and Codex is exhausted.
        policy(1).save(&s.root).unwrap();
        exhaust_codex(&s.store);
        let task = "6".repeat(64);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        assert!(
            s.launched.lock().unwrap().is_empty(),
            "no doomed run starts"
        );
        let entry = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "no_capacity")
            .unwrap();
        assert_eq!(entry.resets_at, Some(500_000));
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(stored.status, Status::Cancelled);
        assert!(
            stored
                .cancellation_reason
                .unwrap()
                .contains(&capacity::utc(500_000))
        );
        // The device's summary says why, with the reset.
        let current = s.inbox.current();
        assert_eq!(current[0].phase, nostr::activity_summary::Phase::Cancelled);
        let note = s.inbox.note(&task).unwrap();
        assert_eq!(
            note,
            coder_host::Note::NoCapacity {
                until: Some(500_000)
            }
        );
        assert_eq!(
            note.headline(),
            format!("No model capacity until {}", capacity::utc(500_000))
        );
        // Nothing is decided twice.
        assert!(s.autostart.sweep().is_empty());
    }

    #[test]
    fn the_command_line_admits_routes_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let workspace = dir.path().join("checkout");
        std::fs::create_dir_all(&workspace).unwrap();
        coder_host::settings::ServeSettings::new(
            vec!["wss://relay.example/".into()],
            BTreeMap::from([("checkout".into(), workspace.canonicalize().unwrap())]),
        )
        .save(&root)
        .unwrap();
        let controller = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let root_text = root.to_string_lossy().into_owned();
        let on = |extra: &[&str]| {
            let mut list = vec![
                "on",
                "--workspace",
                "checkout",
                "--read-only",
                "--controller",
                &controller,
                "--root",
                &root_text,
            ];
            list.extend_from_slice(extra);
            cli(&list.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
        };
        assert_eq!(
            on(&[
                "--route",
                "claude:claude-opus-5-5",
                "--route",
                "codex:gpt-6-luna"
            ]),
            0
        );
        let policy = Policy::load(&root).unwrap().unwrap();
        assert_eq!(policy.engine.model, "claude-opus-5-5");
        assert_eq!(
            policy
                .routes()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["claude:claude-opus-5-5", "codex:gpt-6-luna"]
        );
        assert_eq!(policy.engine.usage_probe, None);
        assert_eq!(
            on(&[
                "--route",
                "codex:gpt-6-luna",
                "--route",
                "claude:claude-opus-5-5",
                "--probe-usage"
            ]),
            0
        );
        let policy = Policy::load(&root).unwrap().unwrap();
        assert_eq!(
            policy.engine.usage_probe,
            Some(UsageProbe {
                threshold_percent: usage::DEFAULT_THRESHOLD_PERCENT
            })
        );
        assert_eq!(on(&["--route", "codex:a", "--usage-threshold", "75"]), 0);
        let policy = Policy::load(&root).unwrap().unwrap();
        assert_eq!(
            policy.engine.usage_probe.map(|p| p.threshold_percent),
            Some(75)
        );
        assert_eq!(on(&["--usage-threshold", "0"]), 1);
        assert_eq!(on(&["--usage-threshold", "most"]), 2);
        assert_eq!(on(&["--route", "codex:a", "--model", "b"]), 2);
        assert_eq!(on(&["--route", "gemini:x"]), 2);
        assert_eq!(on(&["--route", "vertex:qwen/qwen3"]), 2);
        assert_eq!(on(&["--route", "codex:a", "--route", "codex:a"]), 1);
        // A Devin route takes the whole turn: no effort, and the local
        // `devin acp` process as its endpoint.
        assert_eq!(
            on(&["--route", "devin:default", "--route", "codex:gpt-6-luna"]),
            0
        );
        let policy = Policy::load(&root).unwrap().unwrap();
        assert_eq!(policy.engine.model, "default");
        let configuration = policy.configuration(&policy.routes());
        assert_eq!(configuration.provider, "devin");
        assert_eq!(configuration.effort, None);
        assert_eq!(configuration.generation_endpoint, capacity::DEVIN_ENDPOINT);
        assert_eq!(configuration.fallbacks[0].effort.as_deref(), Some("medium"));
        configuration.validate().unwrap();
        // An OpenCode route takes the whole turn too, and names OpenCode's
        // own PROVIDER/MODEL.
        assert_eq!(on(&["--route", "opencode:sonnet"]), 2);
        assert_eq!(
            on(&[
                "--route",
                "opencode:anthropic/claude-sonnet-5",
                "--route",
                "devin:default"
            ]),
            0
        );
        let policy = Policy::load(&root).unwrap().unwrap();
        let configuration = policy.configuration(&policy.routes());
        assert_eq!(configuration.provider, "opencode");
        assert_eq!(configuration.model, "anthropic/claude-sonnet-5");
        assert_eq!(configuration.effort, None);
        assert_eq!(
            configuration.generation_endpoint,
            capacity::OPENCODE_ENDPOINT
        );
        assert_eq!(configuration.fallbacks[0].provider, "devin");
        configuration.validate().unwrap();
        assert_eq!(
            configuration.capabilities()["steering"]["adapter"],
            "opencode-acp"
        );
    }
    /// The recorded usage answers: Codex at its limit, Claude at 66%.
    fn recorded(provider: Provider) -> Result<usage::Response, usage::Failure> {
        let body: &str = match provider {
            Provider::Codex => {
                include_str!("../../../microcoder-loop/fixtures/usage/codex-wham-usage.json")
            }
            Provider::Claude => {
                include_str!("../../../microcoder-loop/fixtures/usage/claude-oauth-usage.json")
            }
            Provider::Vertex | Provider::Devin | Provider::OpenCode => {
                return Err(usage::Failure::Unsupported);
            }
        };
        Ok(usage::Response {
            status: 200,
            retry_after: None,
            body: body.as_bytes().to_vec(),
        })
    }

    fn probed(threshold_percent: u8) -> Policy {
        let mut policy = routed(1);
        policy.engine.usage_probe = Some(UsageProbe { threshold_percent });
        policy.validate().unwrap();
        policy
    }

    #[test]
    fn routing_passes_over_a_provider_at_its_probed_limit() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_790_572_210;
        let usage = usage::refresh(dir.path(), &Provider::ALL, now, recorded);
        let refusals = capacity::Book::default();
        let connected = |_: Provider| Connection::Connected;
        let first = |policy: &Policy, usage: &usage::Book, at: u64| match policy
            .choose(&refusals, usage, &connected, at)
        {
            Choice::Start { order } => order.iter().map(|r| r.provider).collect::<Vec<_>>(),
            other => panic!("{other:?}"),
        };
        // Codex reports its limit reached: Claude starts, Codex follows.
        assert_eq!(
            first(&probed(90), &usage, now),
            [Provider::Claude, Provider::Codex]
        );
        // Without probes in the policy, the same readings change nothing.
        assert_eq!(
            first(&routed(1), &usage, now),
            [Provider::Codex, Provider::Claude]
        );
        // Every route at or above the threshold: advisory, so the first
        // route with capacity still starts; only a refusal ends a task.
        let mut all_near = usage.clone();
        for entry in &mut all_near.entries {
            if let Some(reading) = &mut entry.reading {
                reading.limit_reached = true;
            }
        }
        assert_eq!(
            first(&probed(90), &all_near, now),
            [Provider::Codex, Provider::Claude]
        );
        // A threshold of 60% counts Claude's 66% seven-day window.
        assert_eq!(
            first(&probed(60), &usage, now),
            [Provider::Codex, Provider::Claude]
        );
        // A stale reading is not used.
        assert_eq!(
            first(&probed(90), &usage, now + usage::STALE_AFTER),
            [Provider::Codex, Provider::Claude]
        );
        // A recorded refusal stays the authority over any reading.
        let mut refused = capacity::Book::default();
        for provider in Provider::ALL {
            refused.refusals.push(capacity::Refusal::new(
                provider,
                capacity::Kind::UsageLimit,
                now,
                Some(now + 600),
            ));
        }
        assert_eq!(
            probed(90).choose(&refused, &usage, &connected, now),
            Choice::NoCapacity {
                until: Some(now + 600)
            }
        );
        // Out-of-range thresholds refuse.
        let mut bad = probed(90);
        bad.engine.usage_probe = Some(UsageProbe {
            threshold_percent: 0,
        });
        assert!(bad.validate().is_err());
    }

    #[test]
    fn with_usage_probes_a_start_avoids_a_provider_above_the_threshold() {
        let s = setup_with(recorded);
        CLOCK.with(|clock| clock.set(1_790_572_210));
        probed(90).save(&s.root).unwrap();
        let task = "7".repeat(64);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
        assert_eq!(configuration.fallbacks[0].provider, "codex");
        // The journal names the windows the start was routed with, and the
        // probe kept no credential or account identifier.
        let entry = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "usage")
            .unwrap();
        let detail = entry.detail.unwrap();
        assert!(detail.contains("codex: primary 100%"), "{detail}");
        assert!(detail.contains("(limit reached)"), "{detail}");
        assert!(detail.contains("claude: five_hour 4%"), "{detail}");
        let book = std::fs::read_to_string(s.store.join(usage::FILE)).unwrap();
        assert!(!book.contains("redacted") && !book.contains("example.invalid"));
    }

    #[test]
    fn a_failing_usage_probe_falls_back_to_refusal_only_routing() {
        let s = setup();
        CLOCK.with(|clock| clock.set(1_790_572_210));
        probed(90).save(&s.root).unwrap();
        let task = "8".repeat(64);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        // Offline probes: the first admitted route starts, as without probes.
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "codex");
        let book = usage::Book::load(&s.store);
        assert_eq!(
            book.entry(Provider::Claude).unwrap().failure,
            Some(usage::Failure::Network)
        );
        let detail = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "usage")
            .unwrap()
            .detail
            .unwrap();
        assert!(detail.contains("unknown (probe: network)"), "{detail}");
    }
}
