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

use openagents_connect::control::{
    EngineAccount, EngineReport, EngineRoute, RouteUsage, UsageWindow,
};

use super::account;
use super::capacity::{self, Connection, Provider};
use super::usage;
use super::{Action, COMMAND_SCHEMA, Command, Status, Store, adapter, owner};

pub use coder_host::StartCause;

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
/// How long a started task may stay queued while its owner process still
/// runs before the host ends it as never started. An owner that has exited
/// without admitting the task ends it at the next sweep instead. It matches
/// [`StartCause::Timeout`]'s sentence.
const ADMISSION_DEADLINE: u64 = 600;
/// How many times the host launches an owner for one turn when the owner
/// stops without admitting it for a cause that may be transient.
const MAX_ATTEMPTS: usize = 2;
/// The most of a launch diagnostic the host reads, from its end.
const DIAGNOSTIC_TAIL: u64 = 64 * 1024;
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
    /// A step limit older policies carried. Coder runs have no step or
    /// time limit: a run ends when Coder finishes or asks, when the person
    /// stops it, or when the loop's stuck guard finds it repeating a failed
    /// approach without progress. An older policy's `max_steps` and
    /// `wall_seconds` are read without error and ignored, and a policy
    /// written now leaves them out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
    /// A time limit older policies carried; read and ignored, as
    /// [`Engine::max_steps`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_seconds: Option<u64>,
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
    /// How a Claude route runs (#10246): one lean Claude Code session
    /// briefed by Jev, or Microcoder's step loop. Absent means the default,
    /// [`ClaudeRuns::default`].
    #[serde(default, skip_serializing_if = "ClaudeRuns::is_default")]
    pub claude: ClaudeRuns,
}

/// How a Claude Code route takes a turn (#10246).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeRuns {
    /// One Claude Code session, briefed by Jev, with the cost audit's lean
    /// settings: six tools, the trimmed system prompt, the five-minute
    /// prompt cache, and medium effort. Under full access only: under the
    /// boundary or toolchains a Claude route runs the loop, whose commands
    /// the host bounds. The default since #10246 measured it at 0.61x raw
    /// Claude Code's cost (95% CI 0.57-0.65) at 21 of 21 passes, against
    /// the loop's 1.68x.
    #[default]
    Session,
    /// Microcoder's step loop: each step is one `claude -p` call that
    /// returns one action, and the host runs the commands.
    Loop,
}

impl ClaudeRuns {
    /// The setting's word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ClaudeRuns::Session => "session",
            ClaudeRuns::Loop => "loop",
        }
    }

    /// The setting from its word.
    ///
    /// # Errors
    /// A sentence naming the two words when `text` is neither.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "session" | "lean" | "lean_session" | "lean-session" => Ok(ClaudeRuns::Session),
            "loop" | "microcoder" => Ok(ClaudeRuns::Loop),
            other => Err(format!("`{other}` is not session or loop")),
        }
    }

    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == ClaudeRuns::default()
    }
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

    /// This policy with `provider`'s routes first, in their order, then
    /// the rest in theirs, and the engine's model the new first route's
    /// (#10076): how a start honors the engine the person asked for.
    /// [`Policy::choose`] then passes it over only as it would any first
    /// route: not connected, refused, or near its limit. `None` when the
    /// policy admits no route for `provider`, which the owner's settings
    /// then do not allow.
    #[must_use]
    pub fn preferring(&self, provider: Provider) -> Option<Policy> {
        let (mut routes, rest): (Vec<Route>, Vec<Route>) = self
            .routes()
            .into_iter()
            .partition(|route| route.provider == provider);
        let first = routes.first()?.model.clone();
        routes.extend(rest);
        let mut policy = self.clone();
        policy.engine.model = first;
        policy.engine.routes = routes;
        Some(policy)
    }

    /// This policy with only `provider`'s routes (#10183): a run of a
    /// dispatch plan is pinned to its engine, so it never falls back to
    /// an engine another run of the plan uses. `None` when the policy
    /// admits no route for `provider`.
    #[must_use]
    pub fn only(&self, provider: Provider) -> Option<Policy> {
        let routes: Vec<Route> = self
            .routes()
            .into_iter()
            .filter(|route| route.provider == provider)
            .collect();
        let first = routes.first()?.model.clone();
        let mut policy = self.clone();
        policy.engine.model = first;
        policy.engine.routes = routes;
        Some(policy)
    }

    /// This policy for read-only runs (#10183): the grant writes nothing
    /// in the worktree, so the command boundary is read-only and seals
    /// Git, and a full-access setting runs under this computer's
    /// toolchains instead, since full access has no boundary at all.
    #[must_use]
    pub fn read_only(&self) -> Policy {
        let mut policy = self.clone();
        policy.engine.write_workspace = false;
        if policy.engine.access == adapter::Access::Full {
            policy.engine.access = adapter::Access::Toolchains;
        }
        policy
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

    /// Start `task` at `revision` on `order[0]`, with the rest of `order`
    /// as fallbacks: write the execution grant under `grants` and launch
    /// the engine's detached owner over the task store `store`. The host's
    /// auto-start and a person's local run (`super::local`) both start a
    /// task only through here, so both run the same engine, failover, and
    /// ATIF recording.
    ///
    /// # Errors
    /// Why the grant could not be written or the owner could not start.
    #[allow(clippy::too_many_arguments)]
    pub fn launch(
        &self,
        grants: &Path,
        store: &Path,
        order: &[Route],
        task: &str,
        intent_digest: &str,
        revision: u64,
        launcher: &dyn Launch,
    ) -> std::result::Result<Launched, String> {
        if order.is_empty() {
            return Err("no route to start on".into());
        }
        let program = shell()?;
        // A run that writes gets its independent check (#10232): the
        // recipe's frozen checks and the touched packages' tests.
        let requirements = if self.engine.write_workspace {
            super::local_checks::requirements(grants, task, revision)?
        } else {
            None
        };
        let grant = owner::Grant {
            schema: owner::GRANT_SCHEMA.into(),
            task_id: task.into(),
            intent_digest: intent_digest.into(),
            expected_revision: revision,
            expected_source_snapshot: None,
            program,
            arguments: Vec::new(),
            write_workspace: self.engine.write_workspace,
            // A Coder run has no time limit.
            wall_seconds: 0,
            stream_bytes: 64 * 1024,
            memory_bytes: self.engine.memory_bytes,
            requirements,
            adapter_configuration: Some(self.configuration(order)),
        };
        let bytes = serde_json::to_vec_pretty(&grant).map_err(|e| e.to_string())?;
        owner::Grant::parse(&bytes).map_err(|e| format!("the grant is invalid: {e}"))?;
        let path = grants.join(format!("{task}-{revision}.grant.json"));
        write_private(&path, &bytes)?;
        launcher.launch(&self.engine, &path, store)
    }

    /// The grant configuration that starts on `order[0]` and falls back to
    /// the rest. `order` must not be empty.
    fn configuration(&self, order: &[Route]) -> adapter::Configuration {
        let engine = &self.engine;
        let route = |route: &Route| adapter::Route {
            provider: route.provider.as_str().into(),
            model: route.model.clone(),
            // Devin, OpenCode, and Grok Build take no effort: their model
            // names carry their own.
            effort: (!matches!(
                route.provider,
                Provider::Devin | Provider::OpenCode | Provider::Grok
            ))
            .then(|| route.effort.clone().or_else(|| engine.effort.clone()))
            .flatten(),
            // A Claude route runs as one lean session when the owner
            // chose it and the run has full access (#10246).
            generation_endpoint: if route.provider == Provider::Claude
                && engine.claude == ClaudeRuns::Session
                && engine.access == adapter::Access::Full
            {
                super::capacity::CLAUDE_SESSION_ENDPOINT.into()
            } else {
                route.provider.endpoint().into()
            },
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
            max_steps: None,
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
    /// `unadmitted` (an owner process never admitted a started turn),
    /// `retry` (the host starts that turn again), `not_started` (the host
    /// ended it, with the cause's name as the detail), `policy_on`, or
    /// `policy_off`.
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
    /// For `eligible`: the provider the person asked for (#10076), as its
    /// word, which the start puts first among the policy's own routes.
    /// Never a model, a bound, or a route the policy does not admit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<String>,
    /// For `started`: why the provider in `requested` did not start the
    /// turn, when it did not (#10081). The task's summary says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passed: Option<coder_host::Passed>,
    /// For `started` with `passed`: the provider that starts the turn
    /// instead, as its word.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs: Option<String>,
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
            requested: None,
            passed: None,
            runs: None,
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
            .envs(owner::base_environment())
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
        // The engine finds the Codex login where the readiness check did:
        // `$CODEX_HOME` when the person set it, else `~/.codex` (#10083).
        if let Some(codex) = codex_transport::codex::Login::home_override() {
            command.env(codex_transport::codex::HOME_VAR, codex);
        }
        // A run with this computer's tools derives them from the person's
        // own PATH, which the engine process otherwise never sees.
        if engine.access == adapter::Access::Toolchains {
            command.envs(coder_boundary::toolchains::carried());
        }
        // A host a service manager starts has a short PATH, and the engine
        // gets a shorter one, so the host names the `claude` it finds
        // (npm, Homebrew, or the login shell's PATH) for the engine.
        if let Some(claude) = claude_binary() {
            command.env(microcoder_loop::claude::BIN_VAR, claude);
        }
        // Grok Build the same way (#10091): the `grok` the readiness check
        // found, and its home when the person relocated it.
        let variable = |name: &str| std::env::var_os(name);
        if let Some(grok) = acp_client::grok::binary(&variable) {
            command.env(acp_client::grok::BIN_VAR, grok);
        }
        if let Some(home) = variable(acp_client::grok::HOME_VAR).filter(|v| !v.is_empty()) {
            command.env(acp_client::grok::HOME_VAR, home);
        }
        // How the engine reaches Jev (`jev_hosted::resolve`): this
        // computer's TypeSafe key when one is set here, as the detached
        // owner forwards it (`microcoder::repository::launch`), else the
        // hosted decision service, whose overrides pass through too. The
        // engine's shell children clear their environment again.
        //
        // The delegate recipe's switch (#10209) passes through too, so
        // `OPENAGENTS_DELEGATE_RECIPE=off` reaches the engine it names.
        for name in [
            "TYPESAFE_API_KEY",
            "TYPESAFE_BASE_URL",
            "OPENAGENTS_JEV_RELAY",
            "OPENAGENTS_JEV_WORKER",
            "OPENAGENTS_JEV_HOSTED",
            "OPENAGENTS_DELEGATE_RECIPE",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
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
            return Err(refused(
                &engine.controller,
                &String::from_utf8_lossy(&output.stderr),
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

/// Why the engine at `controller` refused a launch, in plain words, from
/// the first line it wrote to standard error (#10113). The engine writes
/// `{"error": ...}`; the person sees its sentence, never the JSON. An
/// engine that cannot read the grant's shape is older than this program,
/// and the sentence says so and how to update it.
fn refused(controller: &Path, stderr: &str) -> String {
    let line = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let said = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => value["error"].as_str().map(str::to_owned),
        Err(_) => Some(line.to_owned()).filter(|line| !line.is_empty()),
    };
    match said {
        Some(error) if error == owner::GRANT_SHAPE => format!(
            "the Coder engine at {} is older than this program; reinstall the OpenAgents app \
             or rebuild microcoder",
            controller.display()
        ),
        Some(error) => format!(
            "the controller refused the launch: {}",
            error
                .chars()
                .filter(|ch| !ch.is_control())
                .take(200)
                .collect::<String>()
        ),
        None => "the controller refused the launch without saying why".to_owned(),
    }
}

/// The `claude` binary the engine runs: `CLAUDE_BIN` when it names a file,
/// else the one [`microcoder_loop::claude::locate`] finds (which asks the
/// login shell at most once a process).
pub(crate) fn claude_binary() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os(microcoder_loop::claude::BIN_VAR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Some(named);
    }
    microcoder_loop::claude::locate()
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
    identify: account::Identify,
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
            identify: account::identify,
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

    /// Name each provider's signed-in login with `identify` instead of its
    /// local metadata ([`account::identify`]), for tests.
    #[must_use]
    pub fn with_identify(mut self, identify: account::Identify) -> Self {
        self.identify = identify;
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

    /// When the person asked for a provider and it did not start `task`'s
    /// latest started turn (#10081): the provider asked for, the one that
    /// started instead, and why.
    #[must_use]
    pub fn passed_over(&self, task: &str) -> Option<(Provider, Provider, coder_host::Passed)> {
        let entry = journal(&self.root)
            .into_iter()
            .rev()
            .find(|entry| entry.event == "started" && entry.task.as_deref() == Some(task))?;
        let asked = entry.requested.as_deref().and_then(Provider::from_config)?;
        let runs = entry.runs.as_deref().and_then(Provider::from_config)?;
        Some((asked, runs, entry.passed?))
    }

    /// Why the policy ended `task` at `turn` because its owner process never
    /// admitted it, if it did.
    #[must_use]
    pub fn not_started(&self, task: &str, turn: u64) -> Option<StartCause> {
        let subject = Some((task.to_owned(), turn));
        journal(&self.root)
            .into_iter()
            .rev()
            .find(|entry| entry.event == "not_started" && entry.subject() == subject)
            .map(|entry| {
                entry
                    .detail
                    .and_then(|detail| {
                        serde_json::from_value(serde_json::Value::String(detail)).ok()
                    })
                    .unwrap_or(StartCause::Stopped)
            })
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
        self.model_requesting(workspace, None)
    }

    /// [`Autostart::model_for`] for a task whose person asked for
    /// `requested` (#10076): that provider's model when the policy admits
    /// it ([`Policy::preferring`]), else the engine's.
    pub fn model_requesting(&self, workspace: &str, requested: Option<Provider>) -> Option<String> {
        self.policy()
            .filter(|policy| policy.admits(workspace) && self.workspaces.contains_key(workspace))
            .map(|policy| {
                requested
                    .and_then(|provider| policy.preferring(provider))
                    .unwrap_or(policy)
                    .engine
                    .model
            })
    }

    /// Record that `device` created `task` in `workspace` under the policy.
    pub fn eligible(&self, task: &str, device: &str, workspace: &str) {
        self.eligible_requesting(task, device, workspace, None);
    }

    /// [`Autostart::eligible`] for a task whose person asked for
    /// `requested` (#10076): every turn's start puts it first.
    pub fn eligible_requesting(
        &self,
        task: &str,
        device: &str,
        workspace: &str,
        requested: Option<Provider>,
    ) {
        self.record_eligible(task, device, workspace, 1, requested);
    }

    /// Record that `device` continued `task` in `workspace` into the turn
    /// that started at revision `turn` under the policy. The turn starts
    /// exactly as a new task would, under the same bounds.
    pub fn eligible_turn(&self, task: &str, device: &str, workspace: &str, turn: u64) {
        self.record_eligible(task, device, workspace, turn, None);
    }

    fn record_eligible(
        &self,
        task: &str,
        device: &str,
        workspace: &str,
        turn: u64,
        requested: Option<Provider>,
    ) {
        let mut entry = Entry::new((self.now)(), "eligible")
            .task(task)
            .at_turn(turn);
        entry.device = Some(device.into());
        entry.workspace = Some(workspace.into());
        entry.requested = requested.map(|provider| provider.as_str().to_owned());
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
        // Each turn's latest start, how many starts it had, and whether that
        // start's failure to be admitted is recorded or settled.
        let mut started: BTreeMap<(String, u64), (u64, Option<u32>)> = BTreeMap::new();
        let mut attempts: BTreeMap<(String, u64), usize> = BTreeMap::new();
        let mut unadmitted: BTreeSet<(String, u64)> = BTreeSet::new();
        let mut settled: BTreeSet<(String, u64)> = BTreeSet::new();
        // The provider each task's person asked for, from its first
        // eligible entry; every later turn asks for it again (#10076).
        let mut requested: BTreeMap<String, Provider> = BTreeMap::new();
        for entry in history {
            let Some(task) = entry.subject() else {
                continue;
            };
            if entry.event == "eligible"
                && let Some(provider) = entry.requested.as_deref().and_then(Provider::from_config)
            {
                requested.entry(task.0.clone()).or_insert(provider);
            }
            match entry.event.as_str() {
                "eligible" if !waiting.iter().any(|w| w.subject() == entry.subject()) => {
                    waiting.push(entry);
                }
                "started" => {
                    started.insert(task.clone(), (entry.at, entry.owner_process));
                    *attempts.entry(task.clone()).or_default() += 1;
                    unadmitted.remove(&task);
                    settled.remove(&task);
                    decided.insert(task);
                }
                "unadmitted" => {
                    unadmitted.insert(task);
                }
                "retry" => {
                    settled.insert(task.clone());
                    decided.remove(&task);
                }
                "not_started" => {
                    settled.insert(task);
                }
                "skipped" | "refused" | "no_capacity" => {
                    decided.insert(task);
                }
                _ => {}
            }
        }
        waiting.retain(|entry| entry.subject().is_some_and(|t| !decided.contains(&t)));
        if waiting.is_empty() && started.keys().all(|task| settled.contains(task)) {
            return Vec::new();
        }
        // Probe usage before opening the task store, so no request runs
        // under its lock. Cached, so a sweep every few seconds asks each
        // provider at most once per `usage::MIN_INTERVAL`, except that a
        // provider a waiting task asked for, or one this start would pass
        // over for capacity on an older reading, is read now (#10105).
        let usage_book = if policy.engine.usage_probe.is_some() && !waiting.is_empty() {
            let mut providers: Vec<Provider> =
                policy.routes().iter().map(|route| route.provider).collect();
            providers.dedup();
            let asked: Vec<Provider> = waiting
                .iter()
                .filter_map(|entry| entry.task.as_ref().and_then(|t| requested.get(t)))
                .copied()
                .collect();
            let fresh = recheck(&self.store, &policy, &asked, now, self.identify);
            usage::refresh_with(
                &self.store,
                &providers,
                &fresh,
                now,
                self.fetch,
                self.identify,
            )
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
            // A run whose owner process is gone holds neither a slot nor
            // its project (#10124): end it, keeping its record, before
            // counting what runs.
            for ended in store.settle_all(None, None) {
                write(
                    Entry::new(now, "ended")
                        .task(&ended.task_id)
                        .at_turn(ended.turn_started())
                        .detail(super::owner::OWNER_ENDED_TEXT),
                );
            }
            // A started turn still queued was never admitted. It waits while
            // its owner process runs, up to the deadline; once the owner has
            // exited, or the deadline passes, the host says why and either
            // starts it again or ends it, so it never waits forever.
            let mut active = 0;
            for (subject, &(at, owner_process)) in &started {
                if settled.contains(subject) {
                    continue;
                }
                let (id, turn) = subject;
                let Ok(task) = store.show(id) else {
                    continue;
                };
                if task.turn_started() != *turn {
                    continue;
                }
                match task.status {
                    Status::Running | Status::CancelRequested => {
                        active += 1;
                        continue;
                    }
                    Status::Queued if task.run.is_none() => {}
                    _ => continue,
                }
                let waited = now.saturating_sub(at);
                let running = owner_process.is_none_or(alive);
                if running && waited < ADMISSION_DEADLINE {
                    if waited < PENDING_GRACE {
                        active += 1;
                    }
                    continue;
                }
                let cause = if running {
                    StartCause::Timeout
                } else {
                    launch_cause(&self.store, id, at).unwrap_or(StartCause::Stopped)
                };
                if !unadmitted.contains(subject) {
                    write(
                        Entry::new(now, "unadmitted")
                            .task(id)
                            .at_turn(*turn)
                            .detail(format!(
                                "the owner process did not admit the task: {}; read its launch diagnostic in the task store",
                                cause.headline()
                            )),
                    );
                }
                if cause.retryable() && attempts.get(subject).copied().unwrap_or(0) < MAX_ATTEMPTS {
                    write(
                        Entry::new(now, "retry")
                            .task(id)
                            .at_turn(*turn)
                            .detail(cause_name(cause)),
                    );
                    continue;
                }
                // Record first, then end the task, so the summary a device
                // receives for the ending can say why.
                write(
                    Entry::new(now, "not_started")
                        .task(id)
                        .at_turn(*turn)
                        .detail(cause_name(cause)),
                );
                end_unstarted(&mut store, id, *turn, task.revision, cause);
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
                // The person's requested provider first, when the policy
                // admits it; the model the task recorded is that route's.
                let asked = requested.get(&id).copied();
                let preferred = asked.and_then(|provider| policy.preferring(provider));
                let not_allowed = asked.is_some() && preferred.is_none();
                let policy = preferred.unwrap_or_else(|| policy.clone());
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
                let book = capacity::Book::load_with(&self.store, self.identify);
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
                // Why the requested provider does not start this turn, when
                // it does not (#10081): from the policy and the same books
                // the choice read, never from text.
                let passed = asked.and_then(|asked| {
                    if not_allowed {
                        Some(coder_host::Passed::NotAllowed)
                    } else if order.first().is_some_and(|route| route.provider == asked) {
                        None
                    } else if !order.iter().any(|route| route.provider == asked) {
                        Some(coder_host::Passed::NotSignedIn)
                    } else if !book.has_capacity(asked, now) {
                        Some(coder_host::Passed::Refused {
                            until: book.earliest_reset(&[asked], now),
                        })
                    } else {
                        Some(coder_host::Passed::NearLimit)
                    }
                });
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
                    policy,
                    asked,
                    passed,
                ));
            }
            plans
        };
        for (id, turn, workspace, intent_digest, revision, order, policy, asked, passed) in plans {
            let entry = match self.start(&policy, &order, &id, &intent_digest, revision) {
                Ok(launched) => {
                    let mut entry = Entry::new(now, "started").task(&id).at_turn(turn);
                    entry.workspace = Some(workspace);
                    entry.requested = asked.map(|provider| provider.as_str().to_owned());
                    if passed.is_some() {
                        entry.passed = passed;
                        entry.runs = order
                            .first()
                            .map(|route| route.provider.as_str().to_owned());
                    }
                    entry.grant_digest = Some(launched.grant_digest);
                    entry.owner_process = Some(launched.owner_process);
                    entry.detail = Some(format!(
                        "{} {} fallbacks [{}] write_workspace {} access {}",
                        policy.engine.adapter,
                        order[0],
                        order[1..]
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
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
        policy.launch(
            &self.root.join("autostart"),
            &self.store,
            order,
            task,
            intent_digest,
            revision,
            self.launcher.as_ref(),
        )
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

/// End a queued turn whose owner process never admitted it: the host
/// cancels it with the cause's sentence. The command identity is fixed per
/// turn, so a repeat after a crash is an exact retry.
fn end_unstarted(store: &mut Store, task: &str, turn: u64, revision: u64, cause: StartCause) {
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: if turn > 1 {
            format!("autostart-not-started-{task}-{turn}")
        } else {
            format!("autostart-not-started-{task}")
        },
        task_id: task.into(),
        expected_revision: Some(revision),
        action: Action::Cancel {
            reason: format!("{}.", cause.headline()),
        },
    };
    let applied = serde_json::to_vec(&command)
        .map_err(|e| e.to_string())
        .and_then(|bytes| store.apply(&bytes).map_err(|e| e.to_string()));
    if let Err(error) = applied {
        eprintln!("coder host: auto-start cannot end a task that never started: {error}");
    }
}

/// A cause's journal name, such as `claude`.
fn cause_name(cause: StartCause) -> String {
    serde_json::to_value(cause)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Whether process `pid` still exists. Unknown (`0`) counts as running, so
/// only the admission deadline ends its task.
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return true;
    };
    if pid <= 0 {
        return true;
    }
    // SAFETY: signal 0 checks for the process and delivers nothing.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn alive(_pid: u32) -> bool {
    true
}

/// The cause the owner process wrote to the newest launch diagnostic for
/// `task` made at or after `since` (Unix seconds): the `cause` of its last
/// JSON line that has one. The launcher names each diagnostic
/// `repository-launch-TASK-PID-MILLISECONDS.jsonl` in the task store.
fn launch_cause(store: &Path, task: &str, since: u64) -> Option<StartCause> {
    use std::io::{Read, Seek, SeekFrom};
    let prefix = format!("repository-launch-{task}-");
    let newest = std::fs::read_dir(store)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let rest = name.strip_prefix(&prefix)?.strip_suffix(".jsonl")?;
            let (pid, millis) = rest.split_once('-')?;
            pid.parse::<u32>().ok()?;
            let millis = millis.parse::<u64>().ok()?;
            (millis / 1000 + 1 >= since).then_some((millis, entry.path()))
        })
        .max_by_key(|(millis, _)| *millis)?
        .1;
    let mut file = std::fs::File::open(newest).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(DIAGNOSTIC_TAIL)))
        .ok()?;
    let mut bytes = Vec::new();
    file.take(DIAGNOSTIC_TAIL).read_to_end(&mut bytes).ok()?;
    String::from_utf8_lossy(&bytes)
        .lines()
        .rev()
        .find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            serde_json::from_value(value.get("cause")?.clone()).ok()
        })
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

/// The canonical system shell the repository adapter admits: the first of
/// [`owner::SYSTEM_SHELLS`] that exists, canonicalized.
fn shell() -> std::result::Result<PathBuf, String> {
    owner::SYSTEM_SHELLS
        .iter()
        .find_map(|path| Path::new(path).canonicalize().ok())
        .ok_or_else(|| {
            if cfg!(windows) {
                "no system shell: install Git for Windows for all users (C:\\Program Files\\Git)"
                    .into()
            } else {
                "no system shell at /bin/bash or /bin/sh".into()
            }
        })
}

pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> std::result::Result<(), String> {
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

/// The providers a start must read again before it passes any over for
/// capacity (#10105): each of `asked` (the engines a person asked for),
/// and each admitted route's provider that a refusal in the capacity book
/// holds or a reading puts at the policy's threshold, when its reading is
/// [`usage::RECHECK_AFTER`] old or older. Only providers with a usage
/// endpoint; it reads the books and asks no provider.
#[must_use]
pub fn recheck(
    store: &Path,
    policy: &Policy,
    asked: &[Provider],
    now: u64,
    identify: account::Identify,
) -> Vec<Provider> {
    let book = capacity::Book::load_with(store, identify);
    let readings = usage::Book::load_with(store, identify);
    let threshold = policy
        .engine
        .usage_probe
        .as_ref()
        .map(|probe| probe.threshold_percent);
    let mut providers: Vec<Provider> = asked.to_vec();
    for route in policy.routes() {
        let passed_over = !book.has_capacity(route.provider, now)
            || threshold.is_some_and(|t| readings.near_limit(route.provider, t, now));
        if passed_over && readings.needs_recheck(route.provider, now) {
            providers.push(route.provider);
        }
    }
    providers.retain(|provider| Provider::PROBED.contains(provider));
    providers.sort();
    providers.dedup();
    providers
}

/// Read the usage book an engine report shows.
///
/// When the policy's usage probe is off, or there is no policy, this
/// returns an empty book and does not call `fetch`. A cached read loads
/// `usage.json`. A refresh probes only the admitted providers that have a
/// usage endpoint, and only when the probe is on; each of `fresh` is read
/// now rather than when its cache allows ([`usage::refresh_with`]).
#[must_use]
pub fn usage_book(
    policy: Option<&Policy>,
    store: &Path,
    now: u64,
    refresh: bool,
    fresh: &[Provider],
    fetch: usage::Fetch,
    identify: account::Identify,
) -> usage::Book {
    let Some(policy) = policy else {
        return usage::Book::default();
    };
    if policy.engine.usage_probe.is_none() {
        return usage::Book::default();
    }
    if !refresh {
        return usage::Book::load_with(store, identify);
    }
    let mut providers: Vec<Provider> = policy
        .routes()
        .into_iter()
        .map(|route| route.provider)
        .filter(|provider| Provider::PROBED.contains(provider))
        .collect();
    providers.sort();
    providers.dedup();
    usage::refresh_with(store, &providers, fresh, now, fetch, identify)
}

/// The read-only engine report for one computer.
///
/// `signed_in` says whether each provider's local login is present. The
/// report carries provider names, model ids, percents, and reset times. It
/// carries no credential, account identifier, or controller path. Its
/// accounts are Codex and Claude Code, then Grok Build when it is signed
/// in here ([`engine_report_with`] also shows it when only installed).
#[must_use]
pub fn engine_report(
    policy: Option<&Policy>,
    now: u64,
    signed_in: &dyn Fn(Provider) -> bool,
    usage: &usage::Book,
) -> EngineReport {
    engine_report_with(policy, now, signed_in, &|_| false, usage)
}

/// [`engine_report`], with `installed` saying which of the engines a
/// computer may not have ([`OPTIONAL_ACCOUNTS`], Grok Build) are installed
/// here: each gets an account line when installed or signed in (#10091).
#[must_use]
pub fn engine_report_with(
    policy: Option<&Policy>,
    now: u64,
    signed_in: &dyn Fn(Provider) -> bool,
    installed: &dyn Fn(Provider) -> bool,
    usage: &usage::Book,
) -> EngineReport {
    let Some(policy) = policy else {
        return EngineReport {
            enabled: false,
            adapter: String::new(),
            model: String::new(),
            routes: Vec::new(),
            accounts: account_lines(signed_in, installed),
            usage_probe: None,
            refresh_due: false,
        };
    };
    let probes = policy.engine.usage_probe.is_some();
    let routes = policy.routes();
    let refresh_due = probes
        && routes.iter().any(|route| {
            Provider::PROBED.contains(&route.provider) && usage.due(route.provider, now)
        });
    EngineReport {
        enabled: policy.enabled,
        adapter: bound(&policy.engine.adapter, 64),
        model: bound(&policy.engine.model, 128),
        routes: routes
            .iter()
            .map(|route| EngineRoute {
                provider: route.provider.as_str().into(),
                name: provider_name(route.provider).into(),
                model: bound(&route.model, 128),
                signed_in: signed_in(route.provider),
                usage: route_usage(probes, route.provider, usage, now),
            })
            .collect(),
        accounts: account_lines(signed_in, installed),
        usage_probe: policy
            .engine
            .usage_probe
            .as_ref()
            .map(|probe| probe.threshold_percent),
        refresh_due,
    }
}

/// The engines an engine report names whether or not they are set up here.
pub const ACCOUNTS: [Provider; 2] = [Provider::Codex, Provider::Claude];
/// The engines a report names only when installed or signed in here: Grok
/// Build, allowed by default for local runs (#10091).
pub const OPTIONAL_ACCOUNTS: [Provider; 1] = [Provider::Grok];

/// Whether `provider`'s coding agent is installed on this computer, from
/// where it lives and never from a credential (#10113): Codex's home
/// folder (`$CODEX_HOME`, else `~/.codex`), the `claude` binary, or the
/// `grok`, `opencode`, or `devin` binary. A signed-in agent is found by
/// [`capacity::probe`] instead; this says only that it is here at all.
#[must_use]
pub fn installed(provider: Provider) -> bool {
    let variable = |name: &str| std::env::var_os(name);
    match provider {
        Provider::Codex => codex_transport::codex::Login::home().is_some_and(|home| home.is_dir()),
        Provider::Claude => claude_binary().is_some(),
        Provider::Grok => acp_client::grok::binary(&variable).is_some(),
        Provider::OpenCode => acp_client::opencode::binary(&variable).is_some(),
        Provider::Devin => acp_client::devin::binary(&variable).is_some(),
        Provider::Vertex => false,
    }
}

fn account_lines(
    signed_in: &dyn Fn(Provider) -> bool,
    installed: &dyn Fn(Provider) -> bool,
) -> Vec<EngineAccount> {
    ACCOUNTS
        .into_iter()
        .chain(
            OPTIONAL_ACCOUNTS
                .into_iter()
                .filter(|provider| installed(*provider) || signed_in(*provider)),
        )
        .map(|provider| EngineAccount {
            provider: provider.as_str().into(),
            name: provider_name(provider).into(),
            signed_in: signed_in(provider),
        })
        .collect()
}

fn route_usage(probes: bool, provider: Provider, usage: &usage::Book, now: u64) -> RouteUsage {
    if !probes {
        return RouteUsage::Off;
    }
    if !Provider::PROBED.contains(&provider) {
        return RouteUsage::Unsupported;
    }
    if let Some(reading) = usage.reading(provider, now) {
        let windows = reading
            .windows
            .iter()
            .map(|window| UsageWindow {
                name: window.window.as_str().into(),
                label: window_label(window.window).into(),
                used_percent: percent(window.used_fraction),
                resets_at: window.resets_at,
                resets: window.resets_at.and_then(reset_text),
            })
            .collect();
        let used_percent = reading
            .fullest()
            .map(|window| percent(window.used_fraction))
            .unwrap_or(0);
        return RouteUsage::Windows {
            windows,
            limit_reached: reading.limit_reached,
            used_percent,
        };
    }
    let reason = usage
        .entry(provider)
        .and_then(|entry| entry.failure)
        .map(|failure| failure.as_str())
        .unwrap_or("not_probed");
    RouteUsage::Unknown {
        reason: reason.into(),
    }
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Codex => "Codex",
        Provider::Claude => "Claude Code",
        Provider::Devin => "Devin",
        Provider::OpenCode => "OpenCode",
        Provider::Grok => "Grok Build",
        Provider::Vertex => "Vertex",
    }
}

fn window_label(name: usage::WindowName) -> &'static str {
    match name {
        usage::WindowName::FiveHour => "5 hours",
        usage::WindowName::SevenDay => "7 days",
        usage::WindowName::Primary => "Primary",
        usage::WindowName::Secondary => "Secondary",
    }
}

fn percent(fraction: f64) -> u8 {
    if !fraction.is_finite() || fraction <= 0.0 {
        return 0;
    }
    let rounded = (fraction * 100.0).round();
    if rounded >= 100.0 {
        100
    } else {
        // `rounded` is in 1..=99.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            rounded as u8
        }
    }
}

fn reset_text(at: u64) -> Option<String> {
    let text = capacity::utc(at);
    text.chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | ' ' | 'U' | 'T' | 'C'))
        .then_some(text)
}

fn bound(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn providers_of(policy: Option<&Policy>) -> Vec<Provider> {
    let mut providers: Vec<Provider> = ACCOUNTS.into_iter().chain(OPTIONAL_ACCOUNTS).collect();
    if let Some(policy) = policy {
        for route in policy.routes() {
            if !providers.contains(&route.provider) {
                providers.push(route.provider);
            }
        }
    }
    providers
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
  status [--store DIR] [--refresh] [--fresh PROVIDER]...
                       Print one JSON report: the engine routes in order,
                       whether Codex and Claude Code (and Grok Build, when
                       installed) are signed in, and
                       each probed usage window as percents and reset
                       times. The report has no credential. --refresh asks
                       a provider only when this policy's usage probe is
                       on; --fresh asks PROVIDER now rather than when its
                       cached reading allows, and implies --refresh. DIR
                       is the task store (default ~/.openagents/tasks).
  on --workspace LABEL [--workspace LABEL]... [--max-running N]
     [--model ID | --route PROVIDER:MODEL [--route PROVIDER:MODEL]...]
     [--effort low|medium|high|xhigh]
     [--memory-mib N] [--read-only] [--controller PATH]
     [--decision-endpoint URL] [--decision-model ID]
     [--probe-usage] [--usage-threshold PERCENT] [--full-access]
     [--keep-engine]
                       Start tasks that enrolled devices with `operate` create
                       in these workspaces, at most N at once (default 1).
                       Each --route admits a provider (codex, claude,
                       devin, opencode, or grok) and model, in preference
                       order; a task starts on the first one that is
                       connected and has capacity. A devin route
                       (devin:default, or devin:MODEL) hands the whole turn
                       to the local Devin CLI over ACP; an opencode route
                       (opencode:PROVIDER/MODEL) hands it to OpenCode; a
                       grok route (grok:default, or grok:MODEL) hands it to
                       Grok Build.
                       --probe-usage reads each provider's usage windows
                       with its local login and prefers a route below
                       PERCENT (default 90) used.
                       --full-access runs each task's commands as you,
                       with network access and your login-shell
                       environment; on macOS a sandbox only keeps them
                       out of the folders macOS asks about (Music,
                       Photos, Documents, ...). Use it only on your own
                       computer.
                       --keep-engine keeps an existing policy's engine
                       (controller, routes, access, usage probes, and
                       limits) and changes only the workspaces and N; the
                       other options then set up a first policy only.
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
        return Err("usage: give show, status, on, or off".into());
    };
    let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut read_only = false;
    let mut full_access = false;
    let mut probe_usage = false;
    let mut keep_engine = false;
    let mut refresh = false;
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--keep-engine" {
            keep_engine = true;
        } else if arg == "--read-only" {
            read_only = true;
        } else if arg == "--full-access" {
            full_access = true;
        } else if arg == "--probe-usage" {
            probe_usage = true;
        } else if arg == "--refresh" {
            refresh = true;
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
        "status" => {
            // Reject engine-changing flags before any sign-in or usage probe.
            if read_only || full_access || keep_engine || probe_usage {
                return Err("usage: status does not take that option".into());
            }
            if let Some(name) = values
                .keys()
                .find(|name| !matches!(name.as_str(), "--store" | "--fresh"))
            {
                return Err(format!("usage: {name} does not apply to status"));
            }
            // `--fresh PROVIDER` (#10105): read that provider now, not when
            // its cache allows; it implies `--refresh`.
            let mut fresh = Vec::new();
            for name in values.remove("--fresh").unwrap_or_default() {
                fresh.push(
                    Provider::from_config(&name)
                        .ok_or_else(|| format!("usage: --fresh names no provider `{name}`"))?,
                );
            }
            let refresh = refresh || !fresh.is_empty();
            let store = match take_one(&mut values, "--store")? {
                Some(store) => PathBuf::from(store),
                None => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
                    .join(".openagents/tasks"),
            };
            let policy = Policy::load(&root)?;
            let mut signed_in = BTreeMap::new();
            for provider in providers_of(policy.as_ref()) {
                let connected = matches!(capacity::probe(provider), Connection::Connected);
                signed_in.insert(provider, connected);
            }
            let book = usage_book(
                policy.as_ref(),
                &store,
                now,
                refresh,
                &fresh,
                usage::fetch,
                account::identify,
            );
            let variable = |name: &str| std::env::var_os(name);
            let report = engine_report_with(
                policy.as_ref(),
                now,
                &|provider| signed_in.get(&provider).copied().unwrap_or(false),
                &|provider| {
                    provider == Provider::Grok && acp_client::grok::binary(&variable).is_some()
                },
                &book,
            );
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|error| error.to_string())?
            );
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
            // The desktop's switch changes only which projects start and how
            // many at once; the engine the owner set up (its controller,
            // routes, access, usage probes) stays as it is.
            let kept = if keep_engine {
                Policy::load(&root)?.map(|policy| policy.engine)
            } else {
                None
            };
            let engine = if let Some(engine) = kept {
                for name in ENGINE_FLAGS {
                    values.remove(name);
                }
                engine
            } else {
                // Runs have no step or time limit. The old options are
                // still accepted, so an older script keeps working, and do
                // nothing.
                take_one(&mut values, "--max-steps")?;
                take_one(&mut values, "--wall-seconds")?;
                let memory_mib =
                    number(take_one(&mut values, "--memory-mib")?, "--memory-mib", 4096)?;
                let controller = match take_one(&mut values, "--controller")? {
                    Some(path) => PathBuf::from(path),
                    None => default_controller()?,
                };
                let controller = controller.canonicalize().map_err(|_| {
                    format!("the controller {} does not exist", controller.display())
                })?;
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
                    (None, model) => model.unwrap_or_else(|| "gpt-6.1-sol".into()),
                };
                let engine = Engine {
                    adapter: adapter::NAME.into(),
                    controller,
                    model,
                    effort: Some(
                        take_one(&mut values, "--effort")?.unwrap_or_else(|| "medium".into()),
                    ),
                    max_steps: None,
                    wall_seconds: None,
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
                    claude: ClaudeRuns::default(),
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
                engine
            };
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
                    format!("the host admits no workspace labelled {label}; add it with coder host init --workspace")
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

/// The options of `on` that set up the engine, which `--keep-engine` leaves
/// to an existing policy.
const ENGINE_FLAGS: [&str; 10] = [
    "--model",
    "--route",
    "--effort",
    "--max-steps",
    "--wall-seconds",
    "--memory-mib",
    "--controller",
    "--decision-endpoint",
    "--decision-model",
    "--usage-threshold",
];

/// `PROVIDER:MODEL`, where the provider is one of the closed set.
fn parse_route(text: &str) -> std::result::Result<Route, String> {
    let (provider, model) = text
        .split_once(':')
        .ok_or_else(|| format!("usage: --route takes PROVIDER:MODEL, not `{text}`"))?;
    let provider = match Provider::from_config(provider) {
        Some(
            provider @ (Provider::Codex
            | Provider::Claude
            | Provider::Devin
            | Provider::OpenCode
            | Provider::Grok),
        ) => provider,
        Some(Provider::Vertex) => {
            return Err(format!(
                "usage: `{text}`: repository runs don't generate through vertex; use codex, claude, devin, opencode, or grok"
            ));
        }
        None => {
            return Err(format!(
                "usage: the provider in `{text}` is not codex, claude, devin, opencode, or grok"
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
    if provider == Provider::Grok
        && let Err(why) = acp_client::grok::parse_model(model)
    {
        return Err(format!("usage: `{text}`: {why}"));
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
        .arg(coder_boundary::plain_path(path))
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

/// The engine's file name on this platform.
const MICROCODER: &str = if cfg!(windows) {
    "microcoder.exe"
} else {
    "microcoder"
};

/// The engine beside the running program, else the one its macOS app
/// bundle ships, else the installed one.
///
/// # Errors
/// Names where it looked.
pub fn default_controller() -> std::result::Result<PathBuf, String> {
    let exe = std::env::current_exe().ok().map(controller_executable_path);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    controller_candidates(exe.as_deref(), home.as_deref())
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            "no microcoder beside coder or in ~/.openagents/bin; pass --controller".into()
        })
}

/// This running program: the path it was started from, without the
/// ` (deleted)` Linux appends once that file is replaced or removed, and a
/// path that opens or starts this very program even then (#10237): Linux's
/// `/proc/self/exe`, elsewhere that same path. A long-running owner reads
/// or re-executes itself through the second, so a rebuild that replaced
/// its file neither fails its next launch nor swaps its engine mid-flow.
///
/// # Errors
/// The program's path cannot be found.
pub fn running_program() -> std::io::Result<(PathBuf, PathBuf)> {
    let named = controller_executable_path(std::env::current_exe()?);
    let image = Path::new("/proc/self/exe");
    let image = if cfg!(target_os = "linux") && image.exists() {
        image.to_path_buf()
    } else {
        named.clone()
    };
    Ok((named, image))
}

/// Linux appends ` (deleted)` when a running executable has been replaced.
/// Keep its directory even if the original executable no longer exists.
fn controller_executable_path(exe: PathBuf) -> PathBuf {
    let exe = if let Some(name) = exe.file_name().and_then(|name| name.to_str())
        && let Some(name) = name.strip_suffix(" (deleted)")
    {
        exe.with_file_name(name)
    } else {
        exe
    };
    exe.canonicalize().unwrap_or(exe)
}

/// Where [`default_controller`] looks, in order: beside `exe`; when `exe`
/// is the `openagents` CLI in an app bundle's `Contents/Helpers`, the
/// bundle's `Contents/MacOS` (where `scripts/desktop/package-macos.sh`
/// puts `coder` and `microcoder`, so the CLI runs the engine it shipped
/// with rather than an older `~/.openagents/bin` copy that refuses a
/// newer grant's shape); then `~/.openagents/bin`.
fn controller_candidates(exe: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = exe.and_then(Path::parent) {
        candidates.push(dir.join(MICROCODER));
        if dir.file_name().is_some_and(|name| name == "Helpers")
            && let Some(contents) = dir
                .parent()
                .filter(|contents| contents.file_name().is_some_and(|name| name == "Contents"))
        {
            candidates.push(contents.join("MacOS").join(MICROCODER));
        }
    }
    if let Some(home) = home {
        candidates.push(home.join(".openagents/bin").join(MICROCODER));
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No login identity: tests never read the person's own files.
    fn no_login(_: Provider) -> Option<String> {
        None
    }
    use crate::task::remote::Inbox;
    use coder_host::{Code, TaskCreate, Tasks};
    use std::cell::Cell;

    thread_local! {
        // Each test runs on its own thread and sweeps in the foreground, so
        // a per-thread clock keeps tests from moving each other's time.
        static CLOCK: Cell<u64> = const { Cell::new(1_000) };
        // The stand-in account every engine is signed in as, by number; 0
        // is none. Tests never read the person's own login files.
        static LOGIN: Cell<u8> = const { Cell::new(0) };
    }

    fn login(_: Provider) -> Option<String> {
        match LOGIN.with(Cell::get) {
            0 => None,
            n => Some(format!("stand-in-account-{n}")),
        }
    }

    fn clock() -> u64 {
        CLOCK.with(Cell::get)
    }

    fn advance(seconds: u64) {
        CLOCK.with(|clock| clock.set(clock.get() + seconds));
    }

    /// Records each launch instead of starting a process. Its owner is
    /// this test process: still running, and never admitting the task.
    #[derive(Default)]
    struct Fake(Arc<Mutex<Vec<PathBuf>>>);

    impl Launch for Fake {
        fn launch(&self, _: &Engine, grant: &Path, _: &Path) -> Result<Launched, String> {
            owner::Grant::parse(&std::fs::read(grant).unwrap()).unwrap();
            self.0.lock().unwrap().push(grant.to_path_buf());
            Ok(Launched {
                owner_process: std::process::id(),
                grant_digest: "sha256:fake".into(),
            })
        }
    }

    /// A launcher whose owner process exits at once without admitting the
    /// task, as `microcoder repository` does without a Jev key. It writes
    /// `diagnostic`, when given, as the owner's last diagnostic line, named
    /// as the real launcher names it.
    struct Exiting {
        launched: Arc<Mutex<Vec<PathBuf>>>,
        diagnostic: Option<&'static str>,
    }

    /// The ID of a process that has exited and been reaped.
    fn exited_process() -> u32 {
        let mut child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    impl Launch for Exiting {
        fn launch(&self, _: &Engine, grant: &Path, store: &Path) -> Result<Launched, String> {
            let parsed = owner::Grant::parse(&std::fs::read(grant).unwrap()).unwrap();
            self.launched.lock().unwrap().push(grant.to_path_buf());
            let pid = exited_process();
            let attempt = self.launched.lock().unwrap().len() as u64;
            let name = format!(
                "repository-launch-{}-{pid}-{}.jsonl",
                parsed.task_id,
                clock() * 1000 + attempt
            );
            let mut text = String::from("starting\n");
            if let Some(line) = self.diagnostic {
                text.push_str(line);
                text.push('\n');
            }
            std::fs::write(store.join(name), text).unwrap();
            Ok(Launched {
                owner_process: pid,
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
        setup_launching(fetch, |launched| Box::new(Fake(launched)))
    }

    fn setup_launching(
        fetch: usage::Fetch,
        launcher: impl FnOnce(Arc<Mutex<Vec<PathBuf>>>) -> Box<dyn Launch>,
    ) -> Setup {
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
                launcher(launched.clone()),
                clock,
            )
            .with_probe(|_| Connection::Connected)
            .with_usage_fetch(fetch)
            .with_identify(login)
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

    /// An absolute controller path on this platform.
    const CONTROLLER: &str = if cfg!(windows) {
        r"C:\opt\coder\microcoder.exe"
    } else {
        "/opt/coder/microcoder"
    };

    fn policy(max_running: u32) -> Policy {
        Policy {
            schema: POLICY_SCHEMA.into(),
            enabled: true,
            workspaces: vec!["allowed".into()],
            max_running,
            engine: Engine {
                adapter: adapter::NAME.into(),
                controller: PathBuf::from(CONTROLLER),
                model: "gpt-6-luna".into(),
                effort: Some("medium".into()),
                max_steps: None,
                wall_seconds: None,
                memory_bytes: 4 * 1024 * 1024 * 1024,
                write_workspace: true,
                decision_endpoint: "https://api.typesafe.ai".into(),
                decision_model: "jev-latest".into(),
                routes: Vec::new(),
                usage_probe: None,
                access: adapter::Access::Boundary,
                claude: ClaudeRuns::default(),
            },
            changed_at: 1,
        }
    }

    /// A policy preferring a provider puts its routes first and takes its
    /// model as the engine's; one it does not admit is `None` (#10076).
    #[test]
    fn a_preferred_provider_goes_first_only_when_admitted() {
        let policy = routed(1);
        let claude = policy.preferring(Provider::Claude).unwrap();
        let order: Vec<Provider> = claude.routes().iter().map(|r| r.provider).collect();
        assert_eq!(order, [Provider::Claude, Provider::Codex]);
        assert_eq!(claude.engine.model, "claude-opus-5-5");
        claude.validate().unwrap();
        assert_eq!(policy.preferring(Provider::Codex).unwrap(), policy);
        assert_eq!(policy.preferring(Provider::Grok), None);
        // The implicit Codex route of a policy with none listed.
        let plain = super::tests::policy(1);
        assert!(plain.preferring(Provider::Codex).is_some());
        assert_eq!(plain.preferring(Provider::Claude), None);
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
        capacity::record_with(
            store,
            capacity::Refusal::codex(429, body, clock()).unwrap(),
            login,
        )
        .unwrap();
    }

    fn create(workspace: &str) -> TaskCreate {
        TaskCreate {
            title: "Fix the flaky test".into(),
            prompt: "Find why it fails.".into(),
            workspace: workspace.into(),
            images: Vec::new(),
            engine: None,
        }
    }

    fn events(root: &Path) -> Vec<(String, Option<String>)> {
        journal(root)
            .into_iter()
            .map(|e| (e.event, e.task))
            .collect()
    }

    /// The host tells devices a coding reply starts Coder here at once
    /// (#10101) only while created tasks run (the policy is on) and the
    /// owner's `coder.start` is `at_once`; `ask_first`, a policy that is
    /// off or missing, or a settings file Coder's loader refuses, asks.
    #[test]
    fn devices_hear_coder_starts_at_once_only_under_at_once_and_a_policy() {
        use coder_host::Tasks;
        use coder_host::access::protocol::CODER_START_AT_ONCE;
        let s = setup();
        let file = s.root.join("settings.json");
        let inbox = s.inbox.clone().with_settings(&file);
        // No policy: a created task waits inert, so the phone asks.
        assert!(inbox.capabilities().is_empty());
        let mut on = policy(1);
        on.save(&s.root).unwrap();
        // No settings file: the default, `at_once`.
        assert_eq!(inbox.capabilities(), [CODER_START_AT_ONCE]);
        let mut settings = super::super::settings::Settings::default();
        settings.coder.start = super::super::settings::Start::AskFirst;
        settings.save(&file).unwrap();
        assert!(inbox.capabilities().is_empty());
        settings.coder.start = super::super::settings::Start::AtOnce;
        settings.save(&file).unwrap();
        assert_eq!(inbox.capabilities(), [CODER_START_AT_ONCE]);
        std::fs::write(&file, b"{not json").unwrap();
        assert!(inbox.capabilities().is_empty());
        settings.save(&file).unwrap();
        on.enabled = false;
        on.save(&s.root).unwrap();
        assert!(inbox.capabilities().is_empty());
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
                // The fake owner still runs, so the first waits for it
                // without holding a slot.
                ("started".into(), Some(second.clone())),
            ]
        );
        assert_eq!(journal(&s.root)[0].device.as_deref(), Some("phone"));
        assert_eq!(journal(&s.root)[1].owner_process, Some(std::process::id()));
        assert!(
            s.autostart.sweep().iter().all(|e| e.event != "started"),
            "nothing starts twice"
        );
    }

    /// Hold `task`'s write lock on another thread for `hold`, as another
    /// process's slow write would.
    fn hold_task(store: &Path, task: &str, hold: Duration) -> std::thread::JoinHandle<()> {
        let store = store.to_path_buf();
        let task = task.to_owned();
        let (held, taken) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _held = Store::open(&store).unwrap().lock_task(&task).unwrap();
            held.send(()).unwrap();
            std::thread::sleep(hold);
        });
        taken.recv().unwrap();
        holder
    }

    /// A sweep only reads the store, and reads take no lock (#10231): a
    /// task another process is writing still starts, at once.
    #[test]
    fn a_sweep_is_not_held_up_by_another_writer_and_starts_the_task() {
        let s = setup();
        policy(1).save(&s.root).unwrap();
        let (first, second) = ("b".repeat(64), "c".repeat(64));
        s.inbox.create(&first, "phone", &create("allowed")).unwrap();
        s.inbox
            .create(&second, "phone", &create("allowed"))
            .unwrap();
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        advance(PENDING_GRACE + 1);
        let sweeper = Autostart::new(
            s.root.clone(),
            s.store.clone(),
            s.autostart.workspaces.clone(),
            Box::new(Fake(s.launched.clone())),
            clock,
        )
        .with_probe(|_| Connection::Connected)
        .with_usage_fetch(offline)
        .with_store_wait(Duration::from_millis(200))
        .foreground();
        let task = Store::open(&s.store)
            .unwrap()
            .list()
            .unwrap()
            .into_iter()
            .find(|task| task.status == Status::Queued && task.run.is_none())
            .map(|task| task.task_id)
            .unwrap();
        let holder = hold_task(&s.store, &task, Duration::from_millis(1500));
        let begun = std::time::Instant::now();
        let started = sweeper.sweep();
        assert!(begun.elapsed() < Duration::from_millis(1500));
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
                // The first's owner still runs, so it keeps waiting, past the
                // grace, without holding the slot.
                ("skipped".into(), Some(second.clone())),
                ("started".into(), Some(third.clone())),
            ]
        );
        assert_eq!(
            Store::open(&s.store).unwrap().show(&first).unwrap().status,
            Status::Queued
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
        // An older policy's step and time limits are no longer bounds:
        // any value reads, validates, and is ignored.
        let mut legacy = policy(1);
        legacy.engine.max_steps = Some(0);
        legacy.engine.wall_seconds = Some(0);
        assert!(legacy.validate().is_ok());
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

    /// The desktop's switch (`on --keep-engine`, through the host's control
    /// socket) changes only the workspaces and the number running; the
    /// owner's engine settings stay, full access and usage probes included.
    #[test]
    fn the_desktop_switch_keeps_the_owners_engine_settings() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let (one, two) = (dir.path().join("one"), dir.path().join("two"));
        std::fs::create_dir_all(&one).unwrap();
        std::fs::create_dir_all(&two).unwrap();
        coder_host::settings::ServeSettings::new(
            vec!["wss://relay.example/".into()],
            BTreeMap::from([
                ("one".into(), one.canonicalize().unwrap()),
                ("two".into(), two.canonicalize().unwrap()),
            ]),
        )
        .save(&root)
        .unwrap();
        let controller = std::env::current_exe().unwrap();
        let controller = controller.to_string_lossy().into_owned();
        let root_arg = root.to_string_lossy().into_owned();
        let run = |list: &[&str]| {
            let mut args: Vec<String> = list.iter().map(|a| (*a).to_owned()).collect();
            args.extend(["--root".into(), root_arg.clone()]);
            cli(&args)
        };
        // The owner's own policy, set up on the host.
        assert_eq!(
            run(&[
                "on",
                "--workspace",
                "one",
                "--controller",
                &controller,
                "--read-only",
                "--full-access",
                "--probe-usage",
                "--usage-threshold",
                "80",
                "--route",
                "claude:claude-opus-5-5",
                "--max-steps",
                "40",
            ]),
            0
        );
        let owners = Policy::load(&root).unwrap().unwrap();
        assert_eq!(owners.engine.access, adapter::Access::Full);
        // The switch, as the control socket runs it: other routes, and no
        // controller or access of its own.
        let switch = |extra: &[&str]| {
            let mut list = vec![
                "on",
                "--keep-engine",
                "--workspace",
                "two",
                "--max-running",
                "3",
                "--route",
                "codex:gpt-6-luna",
                "--route",
                "claude:claude-opus-5-5",
            ];
            list.extend_from_slice(extra);
            run(&list)
        };
        assert_eq!(switch(&[]), 0);
        let after = Policy::load(&root).unwrap().unwrap();
        assert!(after.enabled);
        assert_eq!(after.workspaces, ["two"]);
        assert_eq!(after.max_running, 3);
        assert_eq!(after.engine, owners.engine);
        // Off and on again keeps it too.
        assert_eq!(run(&["off"]), 0);
        assert_eq!(switch(&[]), 0);
        assert_eq!(Policy::load(&root).unwrap().unwrap().engine, owners.engine);
        // With no policy yet, the switch's options set up the first one.
        std::fs::remove_file(root.join("autostart.json")).unwrap();
        assert_eq!(switch(&["--controller", &controller, "--read-only"]), 0);
        let first = Policy::load(&root).unwrap().unwrap();
        assert_eq!(first.engine.access, adapter::Access::Boundary);
        assert_eq!(first.routes().len(), 2);
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
        let program = owner::GIT_PATHS
            .iter()
            .find(|path| Path::new(path).exists())
            .copied()
            .unwrap_or("git");
        let git = |args: &[&str], cwd: &Path| {
            assert!(
                std::process::Command::new(program)
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
        let old = old.replace(
            "\"/opt/coder/microcoder\"",
            &serde_json::to_string(CONTROLLER).unwrap(),
        );
        std::fs::write(dir.path().join(POLICY_FILE), &old).unwrap();
        let policy = Policy::load(dir.path()).unwrap().unwrap();
        assert_eq!(
            policy.routes(),
            [Route {
                provider: Provider::Codex,
                model: "gpt-6-luna".into(),
                effort: None,
            }]
        );
        // It saves without a routes field, keeping its old step and time
        // limits as they were, and its grant configuration is the one
        // earlier grants carried without the step limit: runs have none.
        policy.save(dir.path()).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(POLICY_FILE)).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::from_str::<serde_json::Value>(&old).unwrap()
        );
        let configuration = serde_json::to_value(policy.configuration(&policy.routes())).unwrap();
        assert_eq!(
            configuration,
            serde_json::json!({"schema":adapter::CONFIG_SCHEMA,"provider":"codex","model":"gpt-6-luna",
                "effort":"medium","generation_endpoint":"https://chatgpt.com/backend-api/codex",
                "decision_endpoint":"https://api.typesafe.ai","decision_model":"jev-1.13.0",
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
        let grant = launched_grant(&s, 0);
        // The run has no step or time limit (#10103): the grant carries
        // neither.
        assert_eq!(grant.wall_seconds, 0);
        let bytes = std::fs::read_to_string(&s.launched.lock().unwrap()[0]).unwrap();
        assert!(
            !bytes.contains("wall_seconds") && !bytes.contains("max_steps"),
            "{bytes}"
        );
        let configuration = grant.adapter_configuration.unwrap();
        assert_eq!(configuration.max_steps, None);
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

    /// A task the host's chat created for a person who asked for an
    /// engine starts on it first when the policy admits it, keeps the
    /// others as fallbacks, and asks for it again on the next turn; an
    /// engine the policy does not admit changes nothing (#10076).
    #[test]
    fn a_requested_engine_starts_first_when_the_policy_admits_it() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        routed(1).save(&s.root).unwrap();
        let task = "6".repeat(64);
        s.inbox.prefer(&task, Engine::ClaudeCode);
        s.inbox.create(&task, "host", &create("allowed")).unwrap();
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(
            stored.intent.configuration.model.as_deref(),
            Some("claude-opus-5-5")
        );
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
        assert_eq!(configuration.fallbacks.len(), 1);
        assert_eq!(configuration.fallbacks[0].model, "gpt-6-luna");
        let eligible = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "eligible")
            .unwrap();
        assert_eq!(eligible.requested.as_deref(), Some("claude"));
        // A preference is for one create: the next task asks for none.
        advance(PENDING_GRACE + 1);
        let next = "7".repeat(64);
        s.inbox.create(&next, "host", &create("allowed")).unwrap();
        advance(PENDING_GRACE + 1);
        s.autostart.sweep();
        assert_eq!(
            launched_grant(&s, 1)
                .adapter_configuration
                .unwrap()
                .provider,
            "codex"
        );
        // An engine the policy does not admit: the policy's own order.
        let other = "8".repeat(64);
        s.inbox.prefer(&other, Engine::GrokBuild);
        s.inbox.create(&other, "host", &create("allowed")).unwrap();
        advance(PENDING_GRACE + 1);
        s.autostart.sweep();
        let stored = Store::open(&s.store).unwrap().show(&other).unwrap();
        assert_eq!(
            stored.intent.configuration.model.as_deref(),
            Some("gpt-6-luna")
        );
        assert_eq!(
            launched_grant(&s, 2)
                .adapter_configuration
                .unwrap()
                .provider,
            "codex"
        );
    }

    /// A requested engine that is out of capacity falls back to the next
    /// admitted route (#10076).
    #[test]
    fn a_requested_engine_without_capacity_falls_back() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        routed(1).save(&s.root).unwrap();
        exhaust_codex(&s.store);
        let task = "9".repeat(64);
        s.inbox.prefer(&task, Engine::Codex);
        s.inbox.create(&task, "host", &create("allowed")).unwrap();
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
    }

    /// A device's own `task.create` names the engine the person asked for
    /// (#10081): the host puts it first among its policy's routes, exactly
    /// as its own chat's preference, and the summary says nothing more.
    #[test]
    fn a_devices_requested_engine_starts_first_when_the_policy_admits_it() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        routed(1).save(&s.root).unwrap();
        let task = "6".repeat(64);
        let asked = TaskCreate {
            engine: Some(Engine::ClaudeCode),
            ..create("allowed")
        };
        let receipt = s.inbox.create(&task, "phone", &asked).unwrap();
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(
            stored.intent.configuration.model.as_deref(),
            Some("claude-opus-5-5")
        );
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
        assert_eq!(configuration.fallbacks.len(), 1);
        assert_eq!(configuration.fallbacks[0].provider, "codex");
        assert_eq!(s.inbox.note(&task), None);
        // A retry of the same request is the same task.
        assert_eq!(s.inbox.create(&task, "phone", &asked).unwrap(), receipt);
    }

    /// A device cannot widen the owner's policy (#10081): an engine the
    /// policy admits no route for adds none, the task runs on the policy's
    /// own first route, and its summary says why in plain words.
    #[test]
    fn a_devices_requested_engine_outside_the_policy_falls_back_and_says_why() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        routed(1).save(&s.root).unwrap();
        let task = "8".repeat(64);
        let asked = TaskCreate {
            engine: Some(Engine::Devin),
            ..create("allowed")
        };
        s.inbox.create(&task, "phone", &asked).unwrap();
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(
            stored.intent.configuration.model.as_deref(),
            Some("gpt-6-luna")
        );
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "codex");
        assert_eq!(configuration.model, "gpt-6-luna");
        assert_eq!(configuration.fallbacks.len(), 1);
        assert_eq!(configuration.fallbacks[0].provider, "claude");
        let started = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "started")
            .unwrap();
        assert_eq!(started.requested.as_deref(), Some("devin"));
        assert_eq!(started.passed, Some(coder_host::Passed::NotAllowed));
        let note = s.inbox.note(&task).unwrap();
        assert_eq!(
            note,
            coder_host::Note::Requested {
                asked: Engine::Devin,
                runs: "Codex",
                why: coder_host::Passed::NotAllowed,
            }
        );
        assert_eq!(
            note.headline(),
            "You asked for Devin; it is not one of the engines this computer's Coder policy allows, so Codex is running."
        );
    }

    /// A requested engine at its limit falls back, and the summary names
    /// when the limit resets (#10081).
    #[test]
    fn a_devices_requested_engine_at_its_limit_says_until_when() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        routed(1).save(&s.root).unwrap();
        exhaust_codex(&s.store);
        let task = "9".repeat(64);
        let asked = TaskCreate {
            engine: Some(Engine::Codex),
            ..create("allowed")
        };
        s.inbox.create(&task, "phone", &asked).unwrap();
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
        assert_eq!(
            s.inbox.note(&task),
            Some(coder_host::Note::Requested {
                asked: Engine::Codex,
                runs: "Claude Code",
                why: coder_host::Passed::Refused {
                    until: Some(500_000)
                },
            })
        );
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
            Provider::Claude
            | Provider::Vertex
            | Provider::Devin
            | Provider::OpenCode
            | Provider::Grok => Connection::Missing("not signed in".into()),
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

    fn exiting(diagnostic: Option<&'static str>) -> Setup {
        setup_launching(offline, move |launched| {
            Box::new(Exiting {
                launched,
                diagnostic,
            })
        })
    }

    #[test]
    fn an_owner_that_exits_without_admitting_ends_the_task_with_its_reason() {
        let s = exiting(Some(
            r#"{"error":"no claude binary: install Claude Code or set CLAUDE_BIN","cause":"claude"}"#,
        ));
        policy(1).save(&s.root).unwrap();
        let task = "7".repeat(64);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        assert_eq!(s.launched.lock().unwrap().len(), 1);
        // Before the fix the task stayed queued forever. The next sweep sees
        // the owner gone, with the task never admitted, and ends it without
        // waiting out any grace: a missing key does not fix itself, so
        // there is no second launch.
        advance(SWEEP_EVERY.as_secs());
        s.autostart.sweep();
        assert_eq!(s.launched.lock().unwrap().len(), 1, "no retry");
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(stored.status, Status::Cancelled);
        assert_eq!(
            stored.cancellation_reason.as_deref(),
            Some("Couldn't start: Claude Code isn't set up on this computer.")
        );
        assert_eq!(
            events(&s.root)[2..],
            [
                ("unadmitted".into(), Some(task.clone())),
                ("not_started".into(), Some(task.clone())),
            ]
        );
        assert_eq!(journal(&s.root)[3].detail.as_deref(), Some("claude"));
        // The device's summary says why, in words.
        assert_eq!(
            s.inbox.current()[0].phase,
            nostr::activity_summary::Phase::Cancelled
        );
        let note = s.inbox.note(&task).unwrap();
        assert_eq!(
            note,
            coder_host::Note::NotStarted {
                cause: StartCause::Claude
            }
        );
        assert_eq!(
            note.headline(),
            "Couldn't start: Claude Code isn't set up on this computer"
        );
        // Nothing is decided twice, and the policy's slot is free again.
        assert!(s.autostart.sweep().is_empty());
        let next = "8".repeat(64);
        s.inbox.create(&next, "phone", &create("allowed")).unwrap();
        assert_eq!(s.launched.lock().unwrap().len(), 2);
    }

    #[test]
    fn an_owner_that_stops_without_a_reason_is_started_once_more_then_ends() {
        let s = exiting(None);
        policy(1).save(&s.root).unwrap();
        let task = "9".repeat(64);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        advance(SWEEP_EVERY.as_secs());
        // The unexplained stop may be transient: the host starts it again.
        let first = s.autostart.sweep();
        assert_eq!(
            first.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
            ["unadmitted", "retry"]
        );
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(stored.status, Status::Queued);
        advance(SWEEP_EVERY.as_secs());
        let second = s.autostart.sweep();
        assert_eq!(
            second.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
            ["started"]
        );
        assert_eq!(s.launched.lock().unwrap().len(), 2);
        // The second owner stops too; the bound is reached, so it ends.
        advance(SWEEP_EVERY.as_secs());
        let third = s.autostart.sweep();
        assert_eq!(
            third.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
            ["unadmitted", "not_started"]
        );
        assert_eq!(s.launched.lock().unwrap().len(), 2);
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(stored.status, Status::Cancelled);
        assert_eq!(
            s.inbox.note(&task),
            Some(coder_host::Note::NotStarted {
                cause: StartCause::Stopped
            })
        );
        advance(SWEEP_EVERY.as_secs());
        assert!(s.autostart.sweep().is_empty());
    }

    #[test]
    fn an_owner_that_runs_but_never_admits_ends_the_task_at_the_deadline() {
        let s = setup();
        policy(1).save(&s.root).unwrap();
        let task = "a1".repeat(32);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        advance(ADMISSION_DEADLINE - 1);
        assert!(
            s.autostart.sweep().is_empty(),
            "the owner may still admit it"
        );
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(stored.status, Status::Queued);
        advance(1);
        let ended = s.autostart.sweep();
        assert_eq!(
            ended.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
            ["unadmitted", "not_started"]
        );
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(stored.status, Status::Cancelled);
        assert_eq!(
            s.inbox.note(&task),
            Some(coder_host::Note::NotStarted {
                cause: StartCause::Timeout
            })
        );
        assert_eq!(
            s.launched.lock().unwrap().len(),
            1,
            "a running owner is not doubled"
        );
    }

    #[test]
    fn a_task_an_earlier_host_left_unadmitted_ends_at_the_next_sweep() {
        let s = exiting(Some(r#"{"error":"no claude","cause":"claude"}"#));
        policy(1).save(&s.root).unwrap();
        let task = "b2".repeat(32);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        // What an earlier host wrote: the task reported once, then left
        // queued.
        record(
            &s.root,
            &Entry::new(clock(), "unadmitted")
                .task(&task)
                .detail("left queued"),
        )
        .unwrap();
        advance(PENDING_GRACE + 1);
        let ended = s.autostart.sweep();
        assert_eq!(
            ended.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
            ["not_started"],
            "the earlier report is not repeated"
        );
        assert_eq!(
            s.inbox.note(&task),
            Some(coder_host::Note::NotStarted {
                cause: StartCause::Claude
            })
        );
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
            Provider::Vertex | Provider::Devin | Provider::OpenCode | Provider::Grok => {
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
        let usage = usage::refresh_with(dir.path(), &Provider::ALL, &[], now, recorded, no_login);
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

    /// The owner on 2026-10-01 (#10105): Claude refused on an exhausted
    /// account, the owner signed in to another, and a start that asked for
    /// Claude still passed it over. Probes off: the hold keeps the refused
    /// login's fingerprint, and once another login is signed in the next
    /// start that asks for Claude runs on Claude.
    #[test]
    fn an_account_change_clears_the_hold_and_the_next_start_uses_the_engine() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        let mut policy = routed(2);
        policy.engine.usage_probe = None;
        policy.save(&s.root).unwrap();
        LOGIN.with(|login| login.set(1));
        let now = clock();
        capacity::record_with(
            &s.store,
            capacity::Refusal::new(
                Provider::Claude,
                capacity::Kind::UsageLimit,
                now,
                Some(now + 5 * 86_400),
            ),
            login,
        )
        .unwrap();
        let asked = |task: &str| {
            let created = TaskCreate {
                engine: Some(Engine::ClaudeCode),
                ..create("allowed")
            };
            s.inbox.create(task, "phone", &created).unwrap();
        };
        asked(&"1".repeat(64));
        let first = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(first.provider, "codex", "the refused login is held");
        advance(5);
        LOGIN.with(|login| login.set(2));
        asked(&"2".repeat(64));
        let second = launched_grant(&s, 1).adapter_configuration.unwrap();
        assert_eq!(second.provider, "claude", "another login is not held");
        // The book keeps no identity, only the salted fingerprint.
        let book = std::fs::read_to_string(s.store.join(capacity::FILE)).unwrap();
        assert!(!book.contains("stand-in-account"));
        LOGIN.with(|login| login.set(0));
    }

    /// A start does not pass an engine over on an old reading: with probes
    /// on, the engine a person asked for, at its limit on a reading two
    /// minutes old and not due by the cache, is read again first, and its
    /// fresh reading under the limit starts it (#10105).
    #[test]
    fn a_stale_reading_is_probed_again_before_an_engine_is_passed_over() {
        use nostr::cj_conversation::Engine;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CLAUDE_PROBES: AtomicUsize = AtomicUsize::new(0);
        fn calm(provider: Provider) -> Result<usage::Response, usage::Failure> {
            if provider == Provider::Claude {
                CLAUDE_PROBES.fetch_add(1, Ordering::SeqCst);
                return Ok(usage::Response {
                    status: 200,
                    retry_after: None,
                    body: br#"{"five_hour":{"utilization":6.0,"resets_at":null},"seven_day":{"utilization":2.0,"resets_at":null}}"#.to_vec(),
                });
            }
            // Codex well under its limit: an old Claude reading alone would
            // send the start to Codex.
            Ok(usage::Response {
                status: 200,
                retry_after: None,
                body: br#"{"plan_type":"pro","rate_limit":{"primary_window":{"used_percent":5}}}"#
                    .to_vec(),
            })
        }
        let s = setup_with(calm);
        let now = 1_790_572_210;
        CLOCK.with(|clock| clock.set(now));
        probed(90).save(&s.root).unwrap();
        // Claude read at its limit two minutes ago; the cache says not to
        // ask again for five more minutes.
        let full = usage::Book {
            schema: usage::SCHEMA.into(),
            entries: vec![usage::Entry {
                provider: Provider::Claude,
                attempted_at: now - 120,
                next_probe_at: now + 300,
                reading: Some(usage::Reading {
                    provider: Provider::Claude,
                    observed_at: now - 120,
                    windows: vec![usage::Window {
                        window: usage::WindowName::SevenDay,
                        used_fraction: 1.0,
                        resets_at: Some(now + 3 * 86_400),
                        length_seconds: Some(604_800),
                    }],
                    limit_reached: false,
                    plan: None,
                    account: None,
                }),
                failure: None,
            }],
        };
        drop(Store::open(&s.store).unwrap());
        std::fs::write(
            s.store.join(usage::FILE),
            serde_json::to_vec(&full).unwrap(),
        )
        .unwrap();
        assert_eq!(
            recheck(&s.store, &probed(90), &[], now, login),
            [Provider::Claude],
            "a provider at its limit on a reading older than a minute"
        );
        let task = "9".repeat(64);
        let asked = TaskCreate {
            engine: Some(Engine::ClaudeCode),
            ..create("allowed")
        };
        s.inbox.create(&task, "phone", &asked).unwrap();
        assert_eq!(CLAUDE_PROBES.load(Ordering::SeqCst), 1);
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
        assert!(recheck(&s.store, &probed(90), &[], now, login).is_empty());
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
        let book = usage::Book::load_with(&s.store, login);
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

    fn explode(_: Provider) -> Result<usage::Response, usage::Failure> {
        panic!("a usage probe ran");
    }

    fn unauthorized(_: Provider) -> Result<usage::Response, usage::Failure> {
        Err(usage::Failure::Unauthorized)
    }

    #[test]
    fn the_engine_report_matches_the_policy_and_the_usage_book() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_790_572_210;
        let policy = probed(90);
        let book = usage_book(
            Some(&policy),
            dir.path(),
            now,
            true,
            &[],
            recorded,
            no_login,
        );
        let report = engine_report(
            Some(&policy),
            now,
            &|provider| provider == Provider::Codex,
            &book,
        );
        let json = serde_json::to_string(&report).unwrap();
        for secret in [
            "owner@example.invalid",
            "account-redacted",
            "user-redacted",
            "access_token",
            CONTROLLER,
            "\"plan\"",
        ] {
            assert!(!json.contains(secret), "{secret} leaked into {json}");
        }
        assert_eq!(report.usage_probe, Some(90));
        assert!(!report.refresh_due);
        assert_eq!(report.accounts[0].name, "Codex");
        assert!(report.accounts[0].signed_in);
        assert_eq!(report.accounts[1].name, "Claude Code");
        assert!(!report.accounts[1].signed_in);
        let codex = &report.routes[0];
        assert_eq!(codex.name, "Codex");
        assert_eq!(codex.model, "gpt-6-luna");
        assert!(codex.signed_in);
        let RouteUsage::Windows {
            windows,
            limit_reached,
            used_percent,
        } = &codex.usage
        else {
            panic!("{:?}", codex.usage);
        };
        assert!(*limit_reached);
        assert_eq!(*used_percent, 100);
        assert_eq!(windows[0].label, "Primary");
        let resets = windows[0].resets.as_deref().unwrap();
        assert!(
            resets
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | ' ' | 'U' | 'T' | 'C')),
            "{resets}"
        );
        let claude = &report.routes[1];
        assert!(!claude.signed_in);
        let RouteUsage::Windows {
            windows,
            limit_reached,
            used_percent,
        } = &claude.usage
        else {
            panic!("{:?}", claude.usage);
        };
        assert!(!*limit_reached);
        assert_eq!(*used_percent, 66);
        assert_eq!(windows[0].label, "5 hours");
        assert_eq!(windows[0].used_percent, 4);
        assert_eq!(windows[1].label, "7 days");
        assert_eq!(windows[1].used_percent, 66);
    }

    #[test]
    fn usage_limits_stay_off_until_the_owner_turns_probes_on() {
        let dir = tempfile::tempdir().unwrap();
        let policy = routed(1);
        let book = usage_book(Some(&policy), dir.path(), 1, true, &[], explode, no_login);
        let report = engine_report(Some(&policy), 1, &|_| false, &book);
        assert!(
            report
                .routes
                .iter()
                .all(|route| route.usage == RouteUsage::Off)
        );
        assert!(!report.refresh_due);
        assert_eq!(report.usage_probe, None);
    }

    #[test]
    fn a_cached_report_does_not_probe_and_says_when_a_reading_is_due() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_790_572_210;
        let policy = probed(90);
        let book = usage_book(
            Some(&policy),
            dir.path(),
            now,
            false,
            &[],
            explode,
            no_login,
        );
        let report = engine_report(Some(&policy), now, &|_| false, &book);
        assert!(report.refresh_due);
        assert!(report.routes.iter().all(|route| {
            matches!(&route.usage, RouteUsage::Unknown { reason } if reason == "not_probed")
        }));
    }

    #[test]
    fn without_a_policy_the_report_still_names_the_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let book = usage_book(None, dir.path(), 1, true, &[], explode, no_login);
        let report = engine_report(None, 1, &|provider| provider == Provider::Claude, &book);
        assert!(!report.enabled);
        assert!(report.routes.is_empty());
        assert!(!report.refresh_due);
        assert_eq!(report.accounts[0].name, "Codex");
        assert!(!report.accounts[0].signed_in);
        assert!(report.accounts[1].signed_in);
        assert_eq!(report.accounts.len(), 2, "Grok Build is not set up here");
    }

    /// Grok Build is allowed by default, so a report names it when it is
    /// installed here, signed in or not (#10091), and only then.
    #[test]
    fn grok_build_gets_an_account_line_when_installed_or_signed_in() {
        let dir = tempfile::tempdir().unwrap();
        let book = usage_book(None, dir.path(), 1, true, &[], explode, no_login);
        let grok = |provider: Provider| provider == Provider::Grok;
        let names = |report: &EngineReport| {
            report
                .accounts
                .iter()
                .map(|a| (a.name.clone(), a.signed_in))
                .collect::<Vec<_>>()
        };
        let installed = engine_report_with(None, 1, &|_| false, &grok, &book);
        assert_eq!(
            names(&installed),
            [
                ("Codex".into(), false),
                ("Claude Code".into(), false),
                ("Grok Build".into(), false)
            ]
        );
        let signed_in = engine_report(None, 1, &grok, &book);
        assert_eq!(names(&signed_in)[2], ("Grok Build".into(), true));
        assert_eq!(engine_report(None, 1, &|_| false, &book).accounts.len(), 2);
        // A Grok Build route reports no usage: no meter, never hidden.
        let mut policy = policy(1);
        policy.engine.routes.push(Route {
            provider: Provider::Grok,
            model: "default".into(),
            effort: None,
        });
        policy.engine.usage_probe = Some(UsageProbe {
            threshold_percent: 90,
        });
        let report = engine_report(Some(&policy), 1, &|_| true, &book);
        let route = report.routes.iter().find(|r| r.provider == "grok").unwrap();
        assert_eq!(route.name, "Grok Build");
        assert!(route.signed_in);
        assert_eq!(route.usage, RouteUsage::Unsupported);
    }

    #[test]
    fn a_provider_without_a_usage_endpoint_is_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = policy(1);
        policy.engine.routes = vec![Route {
            provider: Provider::Devin,
            model: "default".into(),
            effort: None,
        }];
        policy.engine.usage_probe = Some(UsageProbe {
            threshold_percent: 90,
        });
        let book = usage_book(Some(&policy), dir.path(), 1, true, &[], explode, no_login);
        let report = engine_report(Some(&policy), 1, &|_| true, &book);
        assert_eq!(report.routes[0].usage, RouteUsage::Unsupported);
        assert!(!report.refresh_due);
    }

    #[test]
    fn a_refused_probe_reports_its_closed_reason() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_790_572_210;
        let policy = probed(90);
        let book = usage_book(
            Some(&policy),
            dir.path(),
            now,
            true,
            &[],
            unauthorized,
            no_login,
        );
        let report = engine_report(Some(&policy), now, &|_| false, &book);
        assert!(report.routes.iter().all(|route| {
            matches!(&route.usage, RouteUsage::Unknown { reason } if reason == "unauthorized")
        }));
    }

    #[test]
    fn status_help_does_not_probe() {
        assert_eq!(cli(&["status".into(), "--help".into()]), 0);
    }

    #[test]
    fn status_rejects_a_probe_flag_before_it_reads_a_login() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            cli(&[
                "status".into(),
                "--probe-usage".into(),
                "--root".into(),
                dir.path().to_string_lossy().into_owned(),
            ]),
            2
        );
    }

    /// #10113: a dev build ran an older `~/.openagents/bin/microcoder`
    /// that could not read the newer grant, and the person saw the
    /// engine's raw JSON. A refusal is a plain sentence: an older engine
    /// is named with how to update it, and any other refusal shows the
    /// engine's own words, never its JSON.
    #[test]
    #[cfg(unix)]
    fn an_older_engine_is_named_in_plain_words() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let engine = |name: &str, stderr: &str| {
            let path = temp.path().join(name);
            std::fs::write(
                &path,
                format!("#!/bin/sh\nprintf '%s\\n' '{stderr}' >&2\nexit 2\n"),
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Engine {
                adapter: adapter::NAME.into(),
                controller: path,
                model: "gpt-6-luna".into(),
                effort: None,
                max_steps: None,
                wall_seconds: None,
                memory_bytes: 1 << 30,
                write_workspace: false,
                decision_endpoint: "https://api.typesafe.ai".into(),
                decision_model: "jev-latest".into(),
                routes: Vec::new(),
                usage_probe: None,
                access: adapter::Access::Full,
                claude: ClaudeRuns::default(),
            }
        };
        let grant = temp.path().join("grant.json");
        let store = temp.path().join("tasks");
        let older = engine(
            "microcoder",
            &format!(r#"{{"error":"{}"}}"#, owner::GRANT_SHAPE),
        );
        let said = Process.launch(&older, &grant, &store).unwrap_err();
        assert_eq!(
            said,
            format!(
                "the Coder engine at {} is older than this program; reinstall the OpenAgents \
                 app or rebuild microcoder",
                older.controller.display()
            )
        );
        assert!(!said.contains('{'), "{said}");
        let other = engine(
            "other",
            r#"{"error":"no task store path","cause":"configuration"}"#,
        );
        assert_eq!(
            Process.launch(&other, &grant, &store).unwrap_err(),
            "the controller refused the launch: no task store path"
        );
        let bare = engine("bare", "{}");
        assert_eq!(
            Process.launch(&bare, &grant, &store).unwrap_err(),
            "the controller refused the launch without saying why"
        );
        let words = engine("words", "cannot open the grant");
        assert_eq!(
            Process.launch(&words, &grant, &store).unwrap_err(),
            "the controller refused the launch: cannot open the grant"
        );
    }

    #[test]
    fn a_replaced_executable_keeps_the_engine_beside_it() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(home.join(".openagents/bin")).unwrap();
        let beside = bin.join(MICROCODER);
        let installed = home.join(".openagents/bin").join(MICROCODER);
        std::fs::write(&beside, b"new").unwrap();
        std::fs::write(&installed, b"old").unwrap();
        let exe = controller_executable_path(bin.join("openagents (deleted)"));
        assert_eq!(exe, bin.join("openagents"));
        let find = || {
            controller_candidates(Some(&exe), Some(&home))
                .into_iter()
                .find(|path| path.is_file())
        };
        assert_eq!(find(), Some(beside.clone()));
        std::fs::remove_file(beside).unwrap();
        assert_eq!(find(), Some(installed));
    }

    /// #10237: a program whose file a rebuild replaced still finds and
    /// starts itself. A copy of this test binary removes its own file, then
    /// reports what [`running_program`] gives it.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_replaced_program_still_starts_itself() {
        const CHILD: &str = "OPENAGENTS_TEST_RUNNING_PROGRAM";
        if std::env::var_os(CHILD).is_some() {
            let exe = std::env::current_exe().unwrap();
            std::fs::remove_file(&exe).unwrap();
            let (named, image) = running_program().unwrap();
            let bytes = std::fs::read(&image).unwrap();
            let started = std::process::Command::new(&image)
                .arg("--list")
                .output()
                .unwrap();
            println!(
                "NAMED={} IMAGE={} READ={} STARTED={}",
                named.display(),
                image.display(),
                !bytes.is_empty(),
                started.status.success()
            );
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let copy = temp.path().join("openagents");
        std::fs::copy(std::env::current_exe().unwrap(), &copy).unwrap();
        let output = std::process::Command::new(&copy)
            .args([
                "--exact",
                "task::autostart::tests::a_replaced_program_still_starts_itself",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "{printed}");
        assert!(!copy.exists());
        let expected = format!(
            "NAMED={} IMAGE=/proc/self/exe READ=true STARTED=true",
            copy.display()
        );
        assert!(printed.contains(&expected), "{printed}");
    }

    /// #10074: the Mac app's `openagents` CLI lives in `Contents/Helpers`
    /// and its `microcoder` in `Contents/MacOS`. The CLI must run that
    /// engine, not an older `~/.openagents/bin` copy that refuses a newer
    /// grant ("the execution grant has an invalid shape").
    #[test]
    fn the_app_cli_runs_the_engine_its_bundle_ships() {
        let temp = tempfile::tempdir().unwrap();
        let contents = temp.path().join("OpenAgents.app/Contents");
        let home = temp.path().join("home");
        for dir in [
            contents.join("MacOS"),
            contents.join("Helpers"),
            home.join(".openagents/bin"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let bundled = contents.join("MacOS").join(MICROCODER);
        let installed = home.join(".openagents/bin").join(MICROCODER);
        std::fs::write(&bundled, b"").unwrap();
        std::fs::write(&installed, b"").unwrap();
        let first = |exe: &Path| {
            controller_candidates(Some(exe), Some(&home))
                .into_iter()
                .find(|path| path.is_file())
        };
        let cli = contents.join("Helpers/openagents");
        assert_eq!(first(&cli), Some(bundled.clone()));
        assert_eq!(first(&contents.join("MacOS/coder")), Some(bundled.clone()));
        // Beside the program still wins, and elsewhere the install is used.
        let beside = contents.join("Helpers").join(MICROCODER);
        std::fs::write(&beside, b"").unwrap();
        assert_eq!(first(&cli), Some(beside));
        assert_eq!(first(&temp.path().join("bin/openagents")), Some(installed));
        // A Helpers folder outside a bundle's Contents is not a bundle.
        assert!(
            !controller_candidates(Some(&temp.path().join("Helpers/openagents")), None)
                .contains(&temp.path().join("MacOS").join(MICROCODER))
        );
    }
}
