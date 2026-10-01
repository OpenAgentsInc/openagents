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
//! - **Providers detected here.** Codex, then Claude Code, then Grok
//!   Build (#10091), each only when
//!   [`capacity::probe`] finds its login on this computer (no network, no
//!   credential read), skipping one with a refusal that still holds in the
//!   store's capacity book and, when the store holds a fresh usage
//!   reading, one near its limit ([`Policy::choose`]). The start says which
//!   provider it chose and why. [`Local::predict`] makes the same choice
//!   without starting, so a chat's offer says who will run before it runs
//!   ([`coder_events::Runner`]), and `coder_started` carries it.
//! - **The person's settings.** [`Local::here`] reads the local capability
//!   settings ([`settings`]): which providers may run and in what order,
//!   the usage threshold, which folders are projects, and what commands
//!   may reach. With no settings file it runs exactly as above.
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

use openagents_chat::coder_events::{
    self, CoderEvent, FileChange, Line, Mapper, Passed, PassedOver, Runner, Started,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::autostart::{self, Choice, Engine, Launch, Policy, Route, UsageProbe};
use super::capacity::{self, Connection, Provider};
use super::{
    Action, COMMAND_SCHEMA, Command, RequestedConfiguration, Status, Store, TaskIntent, Workspace,
    account, adapter, owner, settings, usage,
};

/// The routes a local run admits by default, in preference order: Codex,
/// then Claude Code, then Grok Build (#10091), with the models the
/// desktop's auto-start switch admits. The settings' `coder.providers`
/// replaces them.
pub const ROUTES: [(Provider, &str); 3] = [
    (Provider::Codex, "gpt-6.1-sol"),
    (Provider::Claude, "claude-opus-5-5"),
    (Provider::Grok, acp_client::grok::DEFAULT_MODEL),
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
// A turn has no step or time limit: it ends when Coder finishes or asks,
// when the person stops it, or when the loop's stuck guard finds it
// repeating a failed approach without progress (#10103).
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
/// `microcoder` beside the running program, in its app bundle, or in
/// `~/.openagents/bin` ([`autostart::default_controller`]).
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

pub(crate) fn git() -> std::process::Command {
    let program = owner::GIT_PATHS
        .iter()
        .find(|path| Path::new(path).exists())
        .copied()
        .unwrap_or("git");
    let mut command = std::process::Command::new(program);
    command.stdin(std::process::Stdio::null());
    command
}

/// The branch `origin/HEAD` names in the checkout at `top`, or `main`.
#[must_use]
pub fn default_branch(top: &Path) -> String {
    git_out(
        top,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .ok()
    .and_then(|name| name.trim().strip_prefix("origin/").map(str::to_owned))
    .filter(|name| !name.is_empty())
    .unwrap_or_else(|| "main".to_owned())
}

pub(crate) fn git_out(dir: &Path, args: &[&str]) -> Result<String, String> {
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
    // A linked worktree (`git worktree add`, such as a host's task
    // worktree) is named by the repository it belongs to, never by its own
    // folder (#10073): the main checkout's folder, or a bare repository's
    // name without `.git`.
    let folder = repository_folder(&top).unwrap_or_else(|| top.clone());
    let name: String = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .map(|name| name.strip_suffix(".git").map(str::to_owned).unwrap_or(name))
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

/// The folder of the repository a linked worktree at `top` belongs to:
/// the parent of its common Git directory when that is a checkout's `.git`,
/// or the common directory itself for a bare repository. `None` for a main
/// checkout, or when Git does not say.
fn repository_folder(top: &Path) -> Option<PathBuf> {
    let common = git_out(
        top,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .ok()?;
    let common = PathBuf::from(common.trim());
    let common = common.canonicalize().unwrap_or(common);
    let own = top.join(".git");
    if own.is_dir() && own.canonicalize().ok().as_deref() == Some(common.as_path()) {
        return None;
    }
    if common.file_name().is_some_and(|name| name == ".git") {
        common.parent().map(Path::to_path_buf)
    } else {
        Some(common)
    }
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
    /// The prediction the turn started on, which names the same provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<Runner>,
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
    /// The provider the person asked for when the task started (#10076),
    /// as its word (`claude`); every turn puts it first again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<String>,
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
            runner: start.runner.clone(),
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
    /// Whether a provider's coding agent is installed here
    /// ([`autostart::installed`]).
    installed: fn(Provider) -> bool,
    now: fn() -> u64,
    controller: Option<PathBuf>,
    /// The person's settings, or why they could not be read: a run then
    /// refuses rather than falling back to the defaults.
    settings: Result<settings::Coder, String>,
    /// Names each provider's signed-in login ([`account::identify`]).
    identify: account::Identify,
    /// Asks this computer's host to read these providers' usage now
    /// (#10105). Only the host reads a probe's token; this process never
    /// does.
    fresh: Option<Fresh>,
}

/// Asks the host on this computer to read some providers' usage now, and
/// returns once it has or has given up ([`Local::with_fresh`]).
pub type Fresh = Box<dyn Fn(&[Provider]) + Send + Sync>;

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
            installed: autostart::installed,
            now: autostart::unix_now,
            controller: None,
            settings: Ok(settings::Coder::default()),
            identify: account::identify,
            fresh: None,
        }
    }

    /// [`Local::new`] with the person's settings from [`settings::path`]:
    /// what `openagents chat`, the desktop, and a host on this computer
    /// run.
    #[must_use]
    pub fn here(store: PathBuf) -> Self {
        Local::new(store).with_settings_result(settings::load().map(|s| s.coder))
    }

    /// Read the person's settings from [`settings::path`] again, so a
    /// change made since (the desktop's Settings page) applies to the next
    /// start.
    pub fn reload_settings(&mut self) {
        self.settings = settings::load()
            .map(|s| s.coder)
            .and_then(|coder| coder.validate().map(|()| coder));
    }

    /// Run with `settings` instead of the defaults.
    #[must_use]
    pub fn with_settings(self, settings: settings::Coder) -> Self {
        self.with_settings_result(Ok(settings))
    }

    fn with_settings_result(mut self, settings: Result<settings::Coder, String>) -> Self {
        self.settings = settings.and_then(|coder| coder.validate().map(|()| coder));
        self
    }

    /// The settings this runner uses.
    ///
    /// # Errors
    /// Why the settings file could not be read.
    pub fn settings(&self) -> Result<&settings::Coder, String> {
        self.settings.as_ref().map_err(Clone::clone)
    }

    /// Whether a coding request from a chat waits for the person to
    /// accept the offer (`coder.start: ask_first`). A settings file that
    /// cannot be read asks first, so its refusal shows when the person
    /// accepts.
    #[must_use]
    pub fn asks_first(&self) -> bool {
        self.settings
            .as_ref()
            .map_or(true, |s| s.start == settings::Start::AskFirst)
    }

    /// The Git checkout `dir` is in, when it counts as a project here:
    /// [`checkout`], within one of the settings' project folders when they
    /// name any.
    ///
    /// # Errors
    /// A plain sentence: not a checkout, no commit, outside the project
    /// folders, or unreadable settings.
    pub fn project(&self, dir: &Path) -> Result<Checkout, String> {
        let settings = self.settings()?;
        let found = checkout(dir)?;
        if settings.admits_project(&found.top) {
            return Ok(found);
        }
        Err(format!(
            "{} is not in one of your project folders ({}), so Coder does not work there. Add              it with `openagents settings set coder.projects …`.",
            found.top.display(),
            settings
                .projects
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
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

    /// Decide which coding agents are installed with `installed`.
    #[must_use]
    pub fn with_installed(mut self, installed: fn(Provider) -> bool) -> Self {
        self.installed = installed;
        self
    }

    /// Name each provider's signed-in login with `identify` instead of its
    /// local metadata, for tests.
    #[must_use]
    pub fn with_identify(mut self, identify: account::Identify) -> Self {
        self.identify = identify;
        self
    }

    /// Before a start passes a provider over for capacity, or starts the
    /// engine the person asked for, ask `fresh` to have the host read those
    /// providers' usage now ([`Local::recheck`]), so the choice reads a
    /// reading of the login signed in now (#10105). Without it, a start
    /// reads only what the books hold.
    #[must_use]
    pub fn with_fresh(mut self, fresh: Fresh) -> Self {
        self.fresh = Some(fresh);
        self
    }

    /// The providers a start for `requested` should have read now before it
    /// chooses (#10105): the requested one, and each the books would pass
    /// over for capacity on a reading [`usage::RECHECK_AFTER`] old or
    /// older ([`autostart::recheck`]). Reads only.
    #[must_use]
    pub fn recheck(&self, requested: Option<Provider>) -> Vec<Provider> {
        let controller = self
            .controller
            .clone()
            .unwrap_or_else(|| PathBuf::from("/"));
        let Ok(policy) = self.policy_with("project", controller) else {
            return Vec::new();
        };
        let asked: Vec<Provider> = requested.into_iter().collect();
        autostart::recheck(&self.store, &policy, &asked, (self.now)(), self.identify)
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
        self.policy_with(label, controller)
    }

    /// The policy for a run that asked for `requested` (#10076): its routes
    /// first when the settings allow it, and whether they do.
    fn asking(&self, policy: Policy, requested: Option<Provider>) -> (Policy, Option<Asked>) {
        let Some(provider) = requested else {
            return (policy, None);
        };
        match policy.preferring(provider) {
            Some(preferring) => (
                preferring,
                Some(Asked {
                    provider,
                    allowed: true,
                }),
            ),
            None => (
                policy,
                Some(Asked {
                    provider,
                    allowed: false,
                }),
            ),
        }
    }

    fn policy_with(&self, label: &str, controller: PathBuf) -> Result<Policy, String> {
        let settings = self.settings()?;
        let routes: Vec<Route> = settings.routes()?;
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
                max_steps: None,
                wall_seconds: None,
                memory_bytes: MEMORY_BYTES,
                write_workspace: true,
                decision_endpoint: "https://api.typesafe.ai".into(),
                decision_model: autostart::DEFAULT_DECISION_MODEL.into(),
                routes,
                // Honors a fresh reading the store already holds, from a
                // host's usage probe; this asks no provider.
                usage_probe: settings
                    .usage_threshold_percent
                    .map(|threshold_percent| UsageProbe { threshold_percent }),
                access: settings.access,
            },
            changed_at: (self.now)(),
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Who a run started now would use, and why ([`Runner`]): the same
    /// choice [`Local::start`] makes, from the same probe, capacity book,
    /// and usage readings, so the prediction names what then runs. `None`
    /// only when the local policy cannot be built. Reads only; no
    /// credential is read.
    ///
    /// `requested` is the provider the person asked for (#10076): it is
    /// put first when the settings allow it, and the prediction says so,
    /// and why another runs when it can't.
    #[must_use]
    pub fn predict(&self, requested: Option<Provider>) -> Option<Runner> {
        // Which engine runs a turn does not change who runs it.
        let controller = self
            .controller
            .clone()
            .or_else(|| controller().ok())
            .unwrap_or_else(|| PathBuf::from("/"));
        let policy = self.policy_with("project", controller).ok()?;
        let (policy, asked) = self.asking(policy, requested);
        Some(self.forecast(&policy, asked).0)
    }

    /// Every coding agent on this computer and its state, for the chat's
    /// context and the welcome card (#10113): first the engines the
    /// settings allow, in their order, then each other one installed or
    /// signed in here, as not enabled. An allowed engine is ready, not
    /// signed in, or at its usage limit, from the same login probe,
    /// capacity book, and usage readings a start reads; an allowed one
    /// that is neither installed nor signed in is left out, except Codex
    /// and Claude Code, which the engine report always names. Reads only;
    /// no credential is read.
    #[must_use]
    pub fn engines(&self) -> Vec<openagents_chat::router::Engine> {
        use openagents_chat::router::{Engine, EngineState, MAX_ENGINES};
        let Ok(settings) = self.settings() else {
            return Vec::new();
        };
        let allowed = settings.provider_list();
        let now = (self.now)();
        let book = capacity::Book::load_with(&self.store, self.identify);
        let readings = usage::Book::load_with(&self.store, self.identify);
        let threshold = settings.usage_threshold_percent;
        let others = settings::PROVIDERS
            .into_iter()
            .filter(|provider| !allowed.contains(provider));
        let mut engines = Vec::new();
        for provider in allowed.iter().copied().chain(others) {
            let signed_in = (self.probe)(provider).is_connected();
            let named =
                signed_in || autostart::ACCOUNTS.contains(&provider) || (self.installed)(provider);
            let state = if !allowed.contains(&provider) {
                if !(signed_in || (self.installed)(provider)) {
                    continue;
                }
                EngineState::NotEnabled
            } else if !named {
                continue;
            } else if !signed_in {
                EngineState::NotSignedIn
            } else if book.blocking(provider, now).is_some()
                || threshold.is_some_and(|t| readings.near_limit(provider, t, now))
            {
                EngineState::Limited
            } else {
                EngineState::Ready
            };
            engines.push(Engine {
                engine: provider.as_str().to_owned(),
                state,
            });
        }
        engines.truncate(MAX_ENGINES);
        engines
    }

    /// The prediction for `policy` now, and the routes to start on when one
    /// can start.
    fn forecast(&self, policy: &Policy, asked: Option<Asked>) -> (Runner, Option<Vec<Route>>) {
        let now = (self.now)();
        let book = capacity::Book::load_with(&self.store, self.identify);
        let readings = usage::Book::load_with(&self.store, self.identify);
        let probe = self.probe;
        let routes = policy.routes();
        let unconnected = || Runner::NotSignedIn {
            providers: names(policy)
                .iter()
                .map(|provider| provider.as_str().to_owned())
                .collect(),
        };
        // A one-route policy starts without probing (`Policy::choose`); a
        // person's own run says plainly that its one provider is missing.
        if routes.len() == 1
            && let Connection::Missing(_) = probe(routes[0].provider)
        {
            return (unconnected(), None);
        }
        match policy.choose(&book, &readings, &probe, now) {
            Choice::Start { order } => (
                runner(policy, &order, &book, &readings, probe, now, asked),
                Some(order),
            ),
            Choice::NoCapacity { until } => (Runner::NoCapacity { until }, None),
            Choice::Unconnected { .. } => (unconnected(), None),
        }
    }

    /// The routes to start on, first one first, and the prediction that
    /// names the first one and why.
    ///
    /// # Errors
    /// Why no provider can start: none is signed in here, or every one
    /// signed in has a refusal that holds.
    pub fn choose(&self, policy: &Policy) -> Result<(Vec<Route>, Runner), String> {
        self.choose_asking(policy, None)
    }

    /// [`Local::choose`] for a policy [`Local::asking`] made.
    fn choose_asking(
        &self,
        policy: &Policy,
        asked: Option<Asked>,
    ) -> Result<(Vec<Route>, Runner), String> {
        match self.forecast(policy, asked) {
            (runner, Some(order)) => Ok((order, runner)),
            (Runner::NoCapacity { until }, None) => Err(format!(
                "No coding agent signed in here has capacity{}.",
                until
                    .map(|at| format!("; the earliest resets {}", coder_events::utc(at)))
                    .unwrap_or_default()
            )),
            (_, None) => Err(unconnected(&names(policy))),
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
        self.start_from(dir, None, title, prompt, thread)
    }

    /// [`Local::start`], with the worktree made from the commit `base`
    /// (such as the fetched `origin/main`) instead of the checkout's
    /// `HEAD`. The issue flow starts here.
    ///
    /// # Errors
    /// As [`Local::start`], or `base` names no commit.
    pub fn start_from(
        &self,
        dir: &Path,
        base: Option<&str>,
        title: &str,
        prompt: &str,
        thread: Option<&str>,
    ) -> Result<Record, String> {
        self.start_full(dir, base, title, prompt, thread, &[], None)
    }

    /// [`Local::start`] with the images a chat on this computer attached:
    /// their exact bytes are kept with the task ([`super::media`]) and the
    /// task's intent names them, so the run's grant admits exactly those
    /// bytes and the engine reads them back checked. A run whose first
    /// route cannot take images is refused before anything is saved.
    ///
    /// # Errors
    /// As [`Local::start`], or an image is not a PNG or JPEG within its
    /// bounds, or the first route cannot take images.
    pub fn start_with_images(
        &self,
        dir: &Path,
        title: &str,
        prompt: &str,
        thread: Option<&str>,
        images: &[super::media::wire::Upload],
    ) -> Result<Record, String> {
        self.start_full(dir, None, title, prompt, thread, images, None)
    }

    /// [`Local::start_with_images`] for a person who asked for `requested`
    /// (#10076): that provider's routes go first when the settings allow
    /// it, and a start falls back only when it is not signed in, refused
    /// for a limit, or near its limit. The start card says what was asked
    /// and, when another runs, why; every later turn asks again.
    ///
    /// # Errors
    /// As [`Local::start_with_images`].
    pub fn start_requested(
        &self,
        dir: &Path,
        title: &str,
        prompt: &str,
        thread: Option<&str>,
        images: &[super::media::wire::Upload],
        requested: Option<Provider>,
    ) -> Result<Record, String> {
        self.start_full(dir, None, title, prompt, thread, images, requested)
    }

    #[allow(clippy::too_many_arguments)]
    fn start_full(
        &self,
        dir: &Path,
        base: Option<&str>,
        title: &str,
        prompt: &str,
        thread: Option<&str>,
        images: &[super::media::wire::Upload],
        requested: Option<Provider>,
    ) -> Result<Record, String> {
        let references: Vec<_> = images.iter().map(|image| image.reference.clone()).collect();
        super::media::wire::validate_all(&references).map_err(|error| error.message)?;
        let mut checkout = self.project(dir)?;
        if let Some(base) = base {
            let commit = git_out(
                &checkout.top,
                &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
            )
            .map_err(|_| format!("{base} names no commit in {}", checkout.top.display()))?;
            checkout.head = commit.trim().to_owned();
        }
        let (policy, asked) = self.asking(self.policy(&checkout.name)?, requested);
        // Read again before passing an engine over, and always the one the
        // person asked for (#10105): a reading or hold may be another
        // login's, or old.
        if let Some(fresh) = &self.fresh {
            let providers = self.recheck(requested);
            if !providers.is_empty() {
                fresh(&providers);
            }
        }
        let (order, runner) = self.choose_asking(&policy, asked)?;
        let now = (self.now)();
        let task = identity(&format!(
            "task:{}:{}:{}:{}",
            checkout.top.display(),
            thread.unwrap_or(""),
            now,
            nonce()
        ));
        // Codex and Claude Code take images natively; the whole-agent
        // routes do not, and a run never silently drops an attached image.
        if !images.is_empty() && !matches!(order[0].provider, Provider::Codex | Provider::Claude) {
            return Err(format!(
                "{} can't take images. Use Codex or Claude Code for a task with images, or remove the images.",
                settings::provider_name(order[0].provider)
            ));
        }
        for image in images {
            super::media::save(&self.store, &task, &image.reference, &image.bytes)
                .map_err(|error| format!("Coder could not keep the attached image: {error}"))?;
        }
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
            images: references,
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
            requested: requested.map(|provider| provider.as_str().to_owned()),
        };
        self.launch(&policy, &order, runner, &submitted, &mut record)?;
        Ok(record)
    }

    fn launch(
        &self,
        policy: &Policy,
        order: &[Route],
        runner: Runner,
        task: &super::Task,
        record: &mut Record,
    ) -> Result<(), String> {
        record.turns.push(TurnStart {
            turn: task.turn(),
            revision: task.revision,
            provider: order[0].provider.as_str().into(),
            model: order[0].model.clone(),
            reason: started_reason(&runner),
            fallbacks: shown_fallbacks(&order[1..], &runner),
            at: (self.now)(),
            runner: Some(runner),
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
        let requested = record.requested.as_deref().and_then(Provider::from_config);
        let (policy, asked) = self.asking(self.policy(&record.project)?, requested);
        let (order, runner) = self.choose_asking(&policy, asked)?;
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
        self.launch(&policy, &order, runner, &next, &mut record)?;
        Ok(record)
    }

    /// Ask `task`'s running turn to stop. The engine stops its command and
    /// ends the turn as stopped.
    ///
    /// # Errors
    /// The task is unknown or already ended.
    pub fn stop(&self, task: &str) -> Result<(), String> {
        // An issue flow between turns (checking, landing) stops at its
        // next step and says so on the issue.
        let flow = super::issue_run::load(&self.store, task).filter(|flow| !flow.finished);
        if flow.is_some() {
            super::issue_run::request_stop(&self.store, task)?;
        }
        let current = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|e| e.to_string())?;
        if !matches!(current.status, Status::Queued | Status::Running) {
            if flow.is_some() {
                return Ok(());
            }
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
            providers: self
                .settings
                .as_ref()
                .map(settings::Coder::provider_list)
                .unwrap_or_else(|_| ROUTES.iter().map(|(p, _)| *p).collect()),
            noted: 0,
            reading: super::Reading::default(),
        }
    }
}

/// The policy's providers, once each, in preference order.
fn names(policy: &Policy) -> Vec<Provider> {
    policy.routes().iter().fold(Vec::new(), |mut out, route| {
        if !out.contains(&route.provider) {
            out.push(route.provider);
        }
        out
    })
}

/// Why no admitted provider can start: none of `providers` is signed in.
fn unconnected(providers: &[Provider]) -> String {
    let how = |provider: Provider| match provider {
        Provider::Codex => "`codex login`",
        Provider::Claude => "run `claude` and log in",
        Provider::Devin => "`devin auth login`",
        Provider::OpenCode => "install `opencode` and run `opencode auth login`",
        Provider::Grok => "run `grok` and log in, or set XAI_API_KEY",
        Provider::Vertex => "turn on the OpenAgents cloud",
    };
    match providers {
        [Provider::Codex, Provider::Claude] | [Provider::Claude, Provider::Codex] => {
            "Neither Codex nor Claude Code is signed in on this computer. Sign in to one \
             (`codex login`, or run `claude` and log in) and try again."
                .into()
        }
        [one] => format!(
            "{} is not signed in on this computer, and your settings allow only it. Sign in \
             ({}) or allow another provider (`openagents settings set coder.providers …`).",
            settings::provider_name(*one),
            how(*one)
        ),
        many => format!(
            "None of the coding agents your settings allow ({}) is signed in on this computer. \
             Sign in to one ({}) and try again.",
            many.iter()
                .map(|p| settings::provider_name(*p))
                .collect::<Vec<_>>()
                .join(", "),
            many.iter().map(|p| how(*p)).collect::<Vec<_>>().join("; ")
        ),
    }
}

/// Whether Coder could start on this computer now over
/// [`default_store`]: [`Local::ready`], read at most every
/// [`READY_EVERY`] seconds. A host's chat asks it on every command, so it
/// tells the chat router a coding request can run here without a
/// registered project.
#[must_use]
pub fn ready_here() -> bool {
    here(None).0
}

/// Who a Coder run on this computer would use now over [`default_store`]
/// ([`Local::predict`]), for a person who asked for `requested` or for
/// none (#10076), read at most every [`READY_EVERY`] seconds. A host's
/// chat puts it on a reply that offers Coder.
#[must_use]
pub fn runner_here(requested: Option<nostr::cj_conversation::Engine>) -> Option<Runner> {
    here(requested.map(settings::provider_of)).1
}

fn here(requested: Option<Provider>) -> (bool, Option<Runner>) {
    use std::sync::Mutex;
    type Cached = Vec<(Option<Provider>, u64, bool, Option<Runner>)>;
    static CACHE: Mutex<Cached> = Mutex::new(Vec::new());
    let now = autostart::unix_now();
    let mut cache = CACHE.lock().unwrap_or_else(|poison| poison.into_inner());
    cache.retain(|(_, at, _, _)| now.saturating_sub(*at) < READY_EVERY);
    if let Some((_, _, ready, runner)) = cache.iter().find(|(asked, ..)| *asked == requested) {
        return (*ready, runner.clone());
    }
    let run = Local::here(default_store());
    let runner = run.predict(requested);
    // `ready` is `choose` succeeding under a policy with a real engine.
    let ready =
        run.policy("project").is_ok() && runner.as_ref().is_some_and(|r| r.provider().is_some());
    cache.push((requested, now, ready, runner.clone()));
    (ready, runner)
}

/// Every coding agent on this computer over [`default_store`]
/// ([`Local::engines`]), read at most every [`READY_EVERY`] seconds. A
/// host's chat puts it on each turn's context (#10113).
#[must_use]
pub fn engines_here() -> Vec<openagents_chat::router::Engine> {
    use std::sync::Mutex;
    type Cached = Option<(u64, Vec<openagents_chat::router::Engine>)>;
    static CACHE: Mutex<Cached> = Mutex::new(None);
    let now = autostart::unix_now();
    let mut cache = CACHE.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some((at, engines)) = cache.as_ref()
        && now.saturating_sub(*at) < READY_EVERY
    {
        return engines.clone();
    }
    let engines = Local::here(default_store()).engines();
    *cache = Some((now, engines.clone()));
    engines
}

/// What the last turn of `task` did, once it has ended, for a follow-up's
/// context (#10094): its events read from the first over `store`, else
/// [`default_store`], and summarized by
/// [`openagents_chat::coder_events::run_result`]. `None` while the turn
/// runs or waits for an answer, and for a task the store does not hold. A
/// host's chat asks it on a send in a thread bound to a Coder task.
#[must_use]
pub fn result_in(store: Option<&Path>, task: &str) -> Option<openagents_chat::router::CoderRun> {
    let store = store.map_or_else(default_store, Path::to_path_buf);
    let mut follow = Local::new(store).follow(task, None, None);
    let mut lines = Vec::new();
    // Each poll reads what is recorded; a few reach a long task's end.
    for _ in 0..64 {
        let (more, state) = follow.poll().ok()?;
        let caught_up = more.is_empty();
        lines.extend(more);
        match state {
            State::Ended => return openagents_chat::coder_events::run_result(&lines),
            State::Waiting => return None,
            State::Running if caught_up => return None,
            State::Running => {}
        }
    }
    None
}

/// How long [`ready_here`] keeps its answer, in seconds.
pub const READY_EVERY: u64 = 15;

/// The provider a person asked for, and whether the settings allow it
/// (#10076).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Asked {
    provider: Provider,
    allowed: bool,
}

/// Who runs first in `order`, and why each route before it in the
/// policy's preference order was passed over: the checks
/// [`Policy::choose`] made, in its order. A provider the person asked for
/// that the settings do not allow is passed over first, as not allowed.
fn runner(
    policy: &Policy,
    order: &[Route],
    book: &capacity::Book,
    readings: &usage::Book,
    probe: fn(Provider) -> Connection,
    now: u64,
    asked: Option<Asked>,
) -> Runner {
    let chosen = &order[0];
    let threshold = policy
        .engine
        .usage_probe
        .as_ref()
        .map(|p| p.threshold_percent);
    let mut passed = Vec::new();
    if let Some(Asked {
        provider,
        allowed: false,
    }) = asked
    {
        passed.push(Passed {
            provider: provider.as_str().into(),
            why: PassedOver::NotAllowed,
        });
    }
    for route in policy.routes() {
        if route.provider == chosen.provider {
            break;
        }
        let why = if let Connection::Missing(_) = probe(route.provider) {
            PassedOver::NotSignedIn
        } else if let Some(refusal) = book.blocking(route.provider, now) {
            PassedOver::Refused {
                kind: serde_json::to_value(refusal.kind)
                    .ok()
                    .and_then(|kind| kind.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "limit".into()),
                until: refusal.until,
            }
        } else if let Some(threshold) = threshold
            && let Some(reading) = readings.reading(route.provider, now)
            && readings.near_limit(route.provider, threshold, now)
        {
            PassedOver::NearLimit {
                used_percent: reading.fullest().map_or(100, |window| {
                    // Clamped to 0..=100 before the cast.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        (window.used_fraction * 100.0).round().clamp(0.0, 100.0) as u8
                    }
                }),
            }
        } else {
            continue;
        };
        if !passed
            .iter()
            .any(|p: &Passed| p.provider == route.provider.as_str())
        {
            passed.push(Passed {
                provider: route.provider.as_str().into(),
                why,
            });
        }
    }
    Runner::Runs {
        provider: chosen.provider.as_str().into(),
        model: chosen.model.clone(),
        passed,
        requested: asked.map(|asked| asked.provider.as_str().to_owned()),
    }
}

/// Why the run started where it did, in the sentence `coder_started`
/// has always carried: "Codex is signed in and has capacity." or
/// "Codex reached its usage limit until …; using Claude Code."
fn started_reason(runner: &Runner) -> String {
    let Runner::Runs {
        provider,
        passed,
        requested,
        ..
    } = runner
    else {
        return runner.text();
    };
    if let Some(reason) = coder_events::requested_reason(
        provider,
        passed,
        requested.as_deref(),
        "is signed in and has capacity.",
        "is running.",
    ) {
        return reason;
    }
    let name = coder_events::provider_name(&json!(provider));
    if passed.is_empty() {
        format!("{name} is signed in and has capacity.")
    } else {
        let why: Vec<String> = passed.iter().map(Passed::text).collect();
        format!("{}; using {name}.", why.join("; "))
    }
}

/// The fallbacks a start card names: the order after the first route,
/// less each provider the start passed over because it is not signed in
/// or refused for a limit, which the reason already says (#10073). A card
/// never says "Falls back to Codex" beside "Codex reached its usage limit".
fn shown_fallbacks(rest: &[Route], runner: &Runner) -> Vec<String> {
    let held: Vec<&str> = match runner {
        Runner::Runs { passed, .. } => passed
            .iter()
            .filter(|p| !matches!(p.why, PassedOver::NearLimit { .. }))
            .map(|p| p.provider.as_str())
            .collect(),
        _ => Vec::new(),
    };
    rest.iter()
        .filter(|route| !held.contains(&route.provider.as_str()))
        .map(ToString::to_string)
        .collect()
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
    /// The providers the run admits, whose earliest reset a `no_capacity`
    /// ending names.
    providers: Vec<Provider>,
    /// The issue flow's notes already emitted.
    noted: usize,
    /// Reads of the task, which wait out a store another process holds.
    reading: super::Reading,
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

    /// The issue flow's notes not yet emitted that follow turns up to
    /// `through` (0: before the first turn).
    fn notes(
        &mut self,
        flow: Option<&super::issue_run::Flow>,
        through: usize,
        out: &mut Vec<Line>,
    ) {
        let Some(flow) = flow else {
            return;
        };
        while let Some(note) = flow.notes.get(self.noted) {
            if note.after_turn > through {
                break;
            }
            self.noted += 1;
            let event = CoderEvent::Step(coder_events::Step {
                turn: note.after_turn.max(1),
                step_id: 0,
                kind: coder_events::StepKind::Note,
                source: "system".into(),
                text: note.text.clone(),
                call: None,
            });
            self.line(event, out);
        }
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
            // A store another process holds (a slow disk sync, the task's
            // owner recording a step) is not the task ending: nothing new
            // yet, and the next poll reads again, until it has stayed busy
            // for `READER_BUSY_WAIT`.
            let Some(task) = self
                .reading
                .show(&self.store, &self.task)
                .map_err(|e| e.to_string())?
            else {
                return Ok((out, State::Running));
            };
            let mut record = record(&self.store, &self.task);
            let flow = super::issue_run::load(&self.store, &self.task);
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
                self.notes(flow.as_ref(), self.turn - 1, &mut out);
                if let Some(started) = record.as_ref().and_then(|r| r.started(self.turn)) {
                    self.line(started, &mut out);
                }
                self.mapper = Some(Mapper::new(self.turn, self.answer.clone()));
                self.seen = 0;
            }
            let runs: Vec<&owner::Run> = task.earlier.iter().chain(task.run.iter()).collect();
            let Some(run) = runs.get(self.turn - 1).copied() else {
                // Not admitted yet.
                let since = record
                    .as_ref()
                    .and_then(|r| r.turns.iter().find(|t| t.turn == self.turn))
                    .map_or(0, |t| t.at);
                let unstarted = if task.status == Status::Cancelled {
                    Some(CoderEvent::Stopped(coder_events::Stopped {
                        turn: self.turn,
                        message: "Coder stopped before the turn started.".into(),
                    }))
                } else {
                    unadmitted(&self.store, &self.task, since, (self.now)()).map(|why| {
                        CoderEvent::Failure(coder_events::Failure {
                            turn: self.turn,
                            message: why,
                            ending: Some("not_started".into()),
                            resets_at: None,
                            issue: None,
                        })
                    })
                };
                let Some(event) = unstarted else {
                    return Ok((out, State::Running));
                };
                self.notes(flow.as_ref(), self.turn, &mut out);
                if flow.as_ref().is_some_and(|flow| !flow.finished) {
                    return Ok((out, State::Running));
                }
                let event = match &flow {
                    Some(flow) => flow.ending(event),
                    None => event,
                };
                self.line(event, &mut out);
                self.ended = Some(State::Ended);
                return Ok((out, State::Ended));
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
            // An issue flow checks and lands after its latest turn: that
            // turn's ending waits for the flow's, which it carries.
            self.notes(flow.as_ref(), self.turn, &mut out);
            let last = task.turn() == self.turn;
            if last && flow.as_ref().is_some_and(|flow| !flow.finished) {
                return Ok((out, State::Running));
            }
            let flow = flow.filter(|_| last);
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
                    let files = match flow.as_ref().and_then(|flow| flow.files.clone()) {
                        Some(files) => files,
                        None => base
                            .map(|base| changes(Path::new(&worktree), &base))
                            .unwrap_or_default(),
                    };
                    let resets_at = (result.ending == capacity::NO_CAPACITY_ENDING)
                        .then(|| {
                            let book = capacity::Book::load(&self.store);
                            book.earliest_reset(&self.providers, (self.now)())
                        })
                        .flatten();
                    let end = mapper.end(
                        &result.ending,
                        files,
                        &worktree,
                        &trace.display().to_string(),
                        resets_at,
                    );
                    let end = match &flow {
                        Some(flow) => flow.ending(end),
                        None => end,
                    };
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

/// Why a turn started at `since` (Unix seconds) will never be admitted:
/// the engine's launcher wrote an error, or it waited longer than the
/// admission bound. `None` while it may still start.
#[must_use]
pub fn unadmitted(store: &Path, task: &str, since: u64, now: u64) -> Option<String> {
    if let Some(why) = launch_error(store, task, since) {
        return Some(format!("Coder did not start: {why}"));
    }
    (since > 0 && now.saturating_sub(since) > ADMISSION_WAIT)
        .then(|| "Coder did not start: the engine never admitted the task.".to_owned())
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

    /// A project folder that is a linked worktree of a repository, such as
    /// a host's `openagents-host-tasks` worktree, is named by the
    /// repository, so no chat is grouped under the worktree's folder
    /// (#10073).
    #[test]
    fn a_linked_worktree_is_named_by_its_repository() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let linked = dir.path().join("proj-host-tasks");
        assert!(
            git()
                .arg("-C")
                .arg(&top)
                .args(["worktree", "add", "--detach", "-q"])
                .arg(&linked)
                .status()
                .unwrap()
                .success()
        );
        let found = checkout(&linked).unwrap();
        assert_eq!(found.top, linked.canonicalize().unwrap());
        assert_eq!(found.name, "proj");
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
            .with_identify(|_| None)
    }

    /// The owner's Mac on 2026-10-01 (#10113): Codex, Claude Code, and
    /// Grok Build signed in, Devin signed in but not in the settings, and
    /// OpenCode installed. The context and the welcome card name every one
    /// of them with its own state, in the settings' order, not only the
    /// one a run would start on.
    #[test]
    fn every_coding_agent_here_is_listed_with_its_state() {
        use openagents_chat::router::{Engine as Agent, EngineState as S};
        fn all_but_opencode(provider: Provider) -> Connection {
            match provider {
                Provider::OpenCode => Connection::Missing("no opencode".into()),
                _ => Connection::Connected,
            }
        }
        fn opencode_and_devin(provider: Provider) -> Connection {
            match provider {
                Provider::OpenCode | Provider::Devin => Connection::Connected,
                _ => Connection::Missing("not here".into()),
            }
        }
        let listed = |run: &Local| -> Vec<(String, S)> {
            run.engines()
                .into_iter()
                .map(|Agent { engine, state }| (engine, state))
                .collect()
        };
        let word = |provider: Provider, state| (provider.as_str().to_owned(), state);
        let dir = tempfile::tempdir().unwrap();
        let run = local(dir.path(), all_but_opencode)
            .with_installed(|provider| matches!(provider, Provider::OpenCode | Provider::Devin));
        let now = autostart::unix_now();
        capacity::record_with(
            run.store(),
            capacity::Refusal::new(
                Provider::Claude,
                capacity::Kind::UsageLimit,
                now,
                Some(now + 3600),
            ),
            |_| None,
        )
        .unwrap();
        assert_eq!(
            listed(&run),
            vec![
                word(Provider::Codex, S::Ready),
                word(Provider::Claude, S::Limited),
                word(Provider::Grok, S::Ready),
                word(Provider::OpenCode, S::NotEnabled),
                word(Provider::Devin, S::NotEnabled),
            ]
        );
        // The settings' order, and an allowed engine that is installed but
        // not signed in says so.
        let mut settings = settings::Coder::default();
        settings.providers = vec![
            settings::Choice::new(Provider::Devin),
            settings::Choice::new(Provider::Claude),
        ];
        let run = run.with_settings(settings);
        assert_eq!(
            listed(&run),
            vec![
                word(Provider::Devin, S::Ready),
                word(Provider::Claude, S::Limited),
                word(Provider::Codex, S::NotEnabled),
                word(Provider::Grok, S::NotEnabled),
                word(Provider::OpenCode, S::NotEnabled),
            ]
        );
        let run = local(dir.path(), opencode_and_devin).with_installed(|_| false);
        assert_eq!(
            listed(&run),
            vec![
                word(Provider::Codex, S::NotSignedIn),
                word(Provider::Claude, S::NotSignedIn),
                word(Provider::OpenCode, S::NotEnabled),
                word(Provider::Devin, S::NotEnabled),
            ]
        );
        // Nothing installed or signed in: only the two the engine report
        // always names, and no optional engine.
        let bare = tempfile::tempdir().unwrap();
        let run = local(bare.path(), nobody).with_installed(|_| false);
        assert_eq!(
            listed(&run),
            vec![
                word(Provider::Codex, S::NotSignedIn),
                word(Provider::Claude, S::NotSignedIn),
            ]
        );
        // Unreadable settings name nothing rather than guess.
        let broken = Local::new(bare.path().join("tasks"))
            .with_settings_result(Err("bad settings".into()))
            .with_probe(both)
            .with_installed(|_| true);
        assert!(broken.engines().is_empty());
    }

    /// Starts nothing: the task stays queued, as a run whose engine has
    /// not admitted it yet.
    struct Held;

    impl Launch for Held {
        fn launch(&self, _: &Engine, _: &Path, _: &Path) -> Result<autostart::Launched, String> {
            Ok(autostart::Launched {
                owner_process: std::process::id(),
                grant_digest: "sha256:held".into(),
            })
        }
    }

    /// A follower whose store another process holds past the open's wait
    /// waits it out and then continues, instead of ending with "another
    /// process holds the task store lock" (#10049's chat issue flow, which
    /// released its claim on a transient busy store). Only a store busy
    /// past the reader's limit is a read failure, and it never touches the
    /// task.
    #[test]
    fn a_follower_waits_out_a_store_another_process_holds_then_continues() {
        use std::time::{Duration, Instant};
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let run = local(dir.path(), both).with_launcher(Box::new(Held));
        let record = run
            .start(
                &top,
                "Fix the parser",
                "Fix the parser.",
                Some(&"4c".repeat(16)),
            )
            .unwrap();
        let mut follow = run.follow(&record.task, None, None);
        follow.reading =
            super::super::Reading::within(Duration::from_millis(50), Duration::from_secs(30));
        let (lines, state) = follow.poll().unwrap();
        assert_eq!(state, State::Running);
        assert!(!lines.is_empty(), "the turn's start is followed");
        let before = Store::open(run.store())
            .unwrap()
            .show(&record.task)
            .unwrap();

        // Another holder keeps the store well past the open's wait.
        let store = run.store().to_path_buf();
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let held = Store::open(&store).unwrap();
            held_tx.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(600));
            drop(held);
        });
        held_rx.recv().unwrap();
        let started = Instant::now();
        let mut waited = 0;
        while started.elapsed() < Duration::from_millis(400) {
            let (lines, state) = follow.poll().expect("a busy store is waited out");
            assert_eq!(state, State::Running);
            assert!(lines.is_empty());
            waited += 1;
        }
        assert!(waited > 0);
        holder.join().unwrap();

        // Released: the follower reads again, and the task is untouched.
        let (_, state) = follow.poll().expect("the follower continues");
        assert_eq!(state, State::Running);
        let after = Store::open(run.store())
            .unwrap()
            .show(&record.task)
            .unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.status, before.status);

        // A store busy past the reader's limit is a read failure, and says so.
        let mut impatient = run.follow(&record.task, None, None);
        impatient.reading =
            super::super::Reading::within(Duration::from_millis(20), Duration::from_millis(150));
        let held = Store::open(run.store()).unwrap();
        let started = Instant::now();
        let why = loop {
            match impatient.poll() {
                Ok((_, State::Running)) => std::thread::sleep(Duration::from_millis(20)),
                Ok(other) => panic!("a held store ended the follow as {other:?}"),
                Err(why) => break why,
            }
        };
        assert!(started.elapsed() >= Duration::from_millis(150));
        assert!(why.contains("holds the task store lock"), "{why}");
        drop(held);
        let (_, state) = impatient.poll().unwrap();
        assert_eq!(state, State::Running);
    }

    /// A host serving this store names a chat's local run as its own task
    /// (#10043), and a paired phone's interrupt stops it; a host serving
    /// another store, or asked for another thread, leaves it outside.
    #[test]
    fn a_host_on_the_same_store_holds_the_local_run_and_a_phone_stops_it() {
        use coder_host::Tasks as _;
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let thread = "4c".repeat(16);
        let run = local(dir.path(), both).with_launcher(Box::new(Held));
        let record = run
            .start(&top, "Fix the parser", "Fix the parser.", Some(&thread))
            .unwrap();
        let host = super::super::remote::Inbox::new(run.store(), BTreeMap::new());
        assert!(host.local_run(&record.task, &thread));
        assert!(!host.local_run(&record.task, &"5d".repeat(16)));
        assert!(!host.local_run(&"e".repeat(64), &thread));
        let elsewhere = tempfile::tempdir().unwrap();
        let other =
            super::super::remote::Inbox::new(elsewhere.path().join("tasks"), BTreeMap::new());
        assert!(!other.local_run(&record.task, &thread));

        // The host lists it, so a device follows its summary.
        assert!(host.current().iter().any(|task| task.task == record.task));
        let phone = coder_host::Principal {
            device: "ab".repeat(32),
            grant: Some("1".repeat(64)),
            epoch: Some(0),
        };
        let current = Store::open(run.store())
            .unwrap()
            .show(&record.task)
            .unwrap();
        let stopped = host
            .command(
                &phone,
                &coder_host::TaskCommand {
                    command: "7".repeat(64),
                    task: record.task.clone(),
                    action: coder_host::CommandAction::Interrupt,
                    based_on: current.revision,
                    text: "Stopped from a phone.".into(),
                    emulate: false,
                    issued_at: autostart::unix_now(),
                },
                &|_: &coder_host::Principal| true,
            )
            .unwrap();
        assert_eq!(stopped.task, record.task);
        let after = Store::open(run.store())
            .unwrap()
            .show(&record.task)
            .unwrap();
        assert!(
            matches!(after.status, Status::Cancelled | Status::CancelRequested),
            "{:?}",
            after.status
        );
    }

    #[test]
    fn codex_first_then_claude_code_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let run = local(dir.path(), both);
        let policy = run.policy("proj").unwrap();
        // A person's own run has every step approved: full access (#10104).
        assert_eq!(policy.engine.access, adapter::Access::Full);
        let (order, runner) = run.choose(&policy).unwrap();
        assert_eq!(order[0].provider, Provider::Codex);
        assert_eq!(order[1].provider, Provider::Claude);
        assert_eq!(
            started_reason(&runner),
            "Codex is signed in and has capacity."
        );

        // A refusal that holds in this store's book passes Codex over.
        let now = autostart::unix_now();
        capacity::record_with(
            run.store(),
            capacity::Refusal::new(
                Provider::Codex,
                capacity::Kind::UsageLimit,
                now,
                Some(now + 3600),
            ),
            |_| None,
        )
        .unwrap();
        let (order, runner) = run.choose(&policy).unwrap();
        let why = started_reason(&runner);
        assert_eq!(order[0].provider, Provider::Claude);
        assert!(
            why.starts_with("Codex reached its usage limit until "),
            "{why}"
        );
        assert!(why.ends_with("; using Claude Code."), "{why}");
        // The card does not also say it falls back to Codex (#10073); Grok
        // Build, signed in here, is the fallback after Claude Code (#10091).
        assert_eq!(
            shown_fallbacks(&order[1..], &runner),
            vec!["grok:default".to_owned()]
        );
        // A fresher probe reading under the limit lifts the refusal: Codex
        // runs again, as the engine reading says.
        let reading = serde_json::json!({
            "schema": "openagents.coder.provider-usage.v1",
            "entries": [{
                "provider": "codex", "attempted_at": now + 1, "next_probe_at": now + 61,
                "reading": {"provider": "codex", "observed_at": now + 1,
                    "windows": [{"window": "primary", "used_fraction": 0.08}]}
            }]
        });
        let usage_file = run.store().join("usage.json");
        std::fs::write(&usage_file, reading.to_string()).unwrap();
        let (order, runner) = run.choose(&policy).unwrap();
        assert_eq!(order[0].provider, Provider::Codex);
        assert_eq!(
            started_reason(&runner),
            "Codex is signed in and has capacity."
        );
        assert_eq!(
            shown_fallbacks(&order[1..], &runner),
            vec![
                "claude:claude-opus-5-5".to_owned(),
                "grok:default".to_owned()
            ]
        );
        std::fs::remove_file(&usage_file).unwrap();

        let run = local(dir.path(), only_claude);
        let fresh = tempfile::tempdir().unwrap();
        let run2 = local(fresh.path(), only_claude);
        let (_, runner) = run2.choose(&run2.policy("proj").unwrap()).unwrap();
        assert_eq!(
            started_reason(&runner),
            "Codex is not signed in here; using Claude Code."
        );
        // Claude Code alone, with Codex refused: still Claude Code.
        assert!(run.choose(&policy).is_ok());

        let none = local(fresh.path(), nobody);
        let why = none.choose(&policy).unwrap_err();
        assert!(
            why.contains(
                "None of the coding agents your settings allow (Codex, Claude Code, Grok Build) \
                 is signed in"
            ),
            "{why}"
        );
        assert!(
            why.contains("`codex login`") && why.contains("run `grok`"),
            "{why}"
        );
    }

    fn claude_only() -> settings::Coder {
        settings::Coder {
            providers: vec![settings::Choice::new(Provider::Claude)],
            ..settings::Coder::default()
        }
    }

    /// No settings file and the default settings are the same run (#10036):
    /// the same routes, threshold, access, and start as before settings
    /// existed.
    #[test]
    fn the_default_settings_change_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let plain = local(dir.path(), both);
        let set = local(dir.path(), both).with_settings(settings::Coder::default());
        let missing = settings::Settings::load(&dir.path().join("none.json")).unwrap();
        let loaded = local(dir.path(), both).with_settings(missing.coder);
        for run in [&plain, &set, &loaded] {
            let mut policy = run.policy("proj").unwrap();
            policy.changed_at = 0;
            let routes: Vec<(Provider, String)> = policy
                .engine
                .routes
                .iter()
                .map(|r| (r.provider, r.model.clone()))
                .collect();
            assert_eq!(
                routes,
                ROUTES
                    .iter()
                    .map(|(p, m)| (*p, (*m).to_owned()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(policy.engine.model, ROUTES[0].1);
            assert_eq!(
                policy
                    .engine
                    .usage_probe
                    .as_ref()
                    .map(|p| p.threshold_percent),
                Some(usage::DEFAULT_THRESHOLD_PERCENT)
            );
            assert_eq!(policy.engine.access, adapter::Access::Full);
            assert!(!run.asks_first());
            let mut first = plain.policy("proj").unwrap();
            first.changed_at = 0;
            assert_eq!(policy, first);
        }
    }

    /// A chat's images start a run here as exact bytes bound to the task's
    /// intent; a first route that cannot take images refuses before
    /// anything is saved, and the caller keeps its draft.
    #[test]
    fn attached_images_are_kept_with_the_task_and_named_by_its_intent() {
        use super::super::media::wire::Upload;
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend((0..40_000u32).map(|i| (i % 253) as u8));
        let upload = Upload::new("Layout.png", std::sync::Arc::new(png.clone())).unwrap();
        let run = local(dir.path(), both).with_launcher(Box::new(Held));
        let record = run
            .start_with_images(
                &top,
                "Fix the layout",
                "Fix the layout in this screenshot.",
                None,
                std::slice::from_ref(&upload),
            )
            .unwrap();
        let task = Store::open(run.store())
            .unwrap()
            .show(&record.task)
            .unwrap();
        assert_eq!(task.intent.images, vec![upload.reference.clone()]);
        let kept = super::super::media::load(run.store(), &record.task, &upload.reference).unwrap();
        assert_eq!(kept, png);

        let agents = settings::Coder {
            providers: vec!["opencode:anthropic/claude-sonnet-5".parse().unwrap()],
            ..settings::Coder::default()
        };
        let refused = local(dir.path(), both)
            .with_settings(agents)
            .with_launcher(Box::new(Held));
        let before = Store::open(refused.store()).unwrap().list().unwrap().len();
        let why = refused
            .start_with_images(&top, "t", "p", None, std::slice::from_ref(&upload))
            .unwrap_err();
        assert!(why.starts_with("OpenCode can't take images."), "{why}");
        assert_eq!(
            Store::open(refused.store()).unwrap().list().unwrap().len(),
            before
        );
    }

    /// `coder.providers` decides which providers may run and in what
    /// order, each still only when signed in here.
    #[test]
    fn the_providers_setting_admits_and_orders_the_routes() {
        let dir = tempfile::tempdir().unwrap();
        let run = local(dir.path(), both).with_settings(claude_only());
        let policy = run.policy("proj").unwrap();
        let (order, runner) = run.choose(&policy).unwrap();
        let reason = started_reason(&runner);
        assert_eq!(order.len(), 1);
        assert_eq!(order[0].provider, Provider::Claude);
        assert_eq!(order[0].model, "claude-opus-5-5");
        assert_eq!(reason, "Claude Code is signed in and has capacity.");

        // Codex signed in, but only Claude Code allowed: nothing runs.
        fn only_codex(provider: Provider) -> Connection {
            match provider {
                Provider::Codex => Connection::Connected,
                _ => Connection::Missing("no login".into()),
            }
        }
        let refused = local(dir.path(), only_codex).with_settings(claude_only());
        let why = refused
            .choose(&refused.policy("proj").unwrap())
            .unwrap_err();
        assert!(
            why.starts_with(
                "Claude Code is not signed in on this computer, and your settings allow only it."
            ),
            "{why}"
        );
        assert!(!refused.ready());

        // Claude Code first, then Codex.
        let order_set = settings::Coder {
            providers: vec![
                settings::Choice::new(Provider::Claude),
                "codex:gpt-6-sol".parse().unwrap(),
            ],
            ..settings::Coder::default()
        };
        let run = local(dir.path(), both).with_settings(order_set);
        let policy = run.policy("proj").unwrap();
        assert_eq!(policy.engine.model, "claude-opus-5-5");
        let (order, _) = run.choose(&policy).unwrap();
        assert_eq!(
            order.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["claude:claude-opus-5-5", "codex:gpt-6-sol"]
        );

        // OpenCode and Devin are routes like the others.
        let agents = settings::Coder {
            providers: vec![
                "opencode:anthropic/claude-sonnet-5".parse().unwrap(),
                settings::Choice::new(Provider::Devin),
            ],
            ..settings::Coder::default()
        };
        let run = local(dir.path(), both).with_settings(agents);
        let policy = run.policy("proj").unwrap();
        let (order, runner) = run.choose(&policy).unwrap();
        let reason = started_reason(&runner);
        assert_eq!(order[0].provider, Provider::OpenCode);
        assert_eq!(order[1].provider, Provider::Devin);
        assert_eq!(order[1].model, acp_client::devin::DEFAULT_MODEL);
        assert_eq!(reason, "OpenCode is signed in and has capacity.");
        let none = local(dir.path(), nobody).with_settings(run.settings().unwrap().clone());
        assert!(
            none.choose(&policy)
                .unwrap_err()
                .contains("None of the coding agents your settings allow (OpenCode, Devin)")
        );
    }

    /// `coder.usage_threshold_percent` is the reading at which a provider
    /// is passed over; `null` ignores readings.
    #[test]
    fn the_usage_threshold_setting_decides_when_a_reading_passes_a_provider_over() {
        let dir = tempfile::tempdir().unwrap();
        let now = autostart::unix_now();
        let book = usage::Book {
            schema: usage::SCHEMA.into(),
            entries: vec![usage::Entry {
                provider: Provider::Codex,
                attempted_at: now,
                next_probe_at: now + 600,
                reading: Some(usage::Reading {
                    provider: Provider::Codex,
                    observed_at: now,
                    windows: vec![usage::Window {
                        window: usage::WindowName::Primary,
                        used_fraction: 0.8,
                        resets_at: Some(now + 3600),
                        length_seconds: None,
                    }],
                    limit_reached: false,
                    plan: None,
                    account: None,
                }),
                failure: None,
            }],
        };
        std::fs::create_dir_all(dir.path().join("tasks")).unwrap();
        std::fs::write(
            dir.path().join("tasks").join(usage::FILE),
            serde_json::to_vec(&book).unwrap(),
        )
        .unwrap();
        let first = |threshold: Option<u8>| {
            let run = local(dir.path(), both).with_settings(settings::Coder {
                usage_threshold_percent: threshold,
                ..settings::Coder::default()
            });
            let policy = run.policy("proj").unwrap();
            assert_eq!(
                policy
                    .engine
                    .usage_probe
                    .as_ref()
                    .map(|p| p.threshold_percent),
                threshold
            );
            let (order, runner) = run.choose(&policy).unwrap();
            let reason = started_reason(&runner);
            (order[0].provider, reason)
        };
        // 80% used: under the default 90%, Codex still runs.
        assert_eq!(first(Some(90)).0, Provider::Codex);
        let (provider, reason) = first(Some(75));
        assert_eq!(provider, Provider::Claude);
        assert_eq!(reason, "Codex is at 80% of its window; using Claude Code.");
        assert_eq!(first(None).0, Provider::Codex);
    }

    /// `coder.projects` names which checkouts count as projects; a run
    /// outside them does not start.
    #[test]
    fn the_projects_setting_decides_which_checkouts_are_projects() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let elsewhere = tempfile::tempdir().unwrap();
        let only_elsewhere = settings::Coder {
            projects: vec![elsewhere.path().canonicalize().unwrap()],
            ..settings::Coder::default()
        };
        let run = local(dir.path(), both)
            .with_settings(only_elsewhere)
            .with_launcher(Box::new(Held));
        let why = run.start(&top, "t", "add a test", None).unwrap_err();
        assert!(
            why.contains("is not in one of your project folders"),
            "{why}"
        );
        assert!(!dir.path().join("worktrees").exists());
        let inside = settings::Coder {
            projects: vec![dir.path().canonicalize().unwrap()],
            ..settings::Coder::default()
        };
        let run = local(dir.path(), both)
            .with_settings(inside)
            .with_launcher(Box::new(Held));
        assert_eq!(run.project(&top.join(".")).unwrap().name, "proj");
        run.start(&top, "t", "add a test", None).unwrap();
    }

    /// `coder.access` is what a run's commands may reach, and
    /// `coder.start` whether a chat asks first.
    #[test]
    fn the_access_and_start_settings_reach_the_policy_and_the_chat() {
        let dir = tempfile::tempdir().unwrap();
        for access in [
            adapter::Access::Toolchains,
            adapter::Access::Full,
            adapter::Access::Boundary,
        ] {
            let run = local(dir.path(), both).with_settings(settings::Coder {
                access,
                ..settings::Coder::default()
            });
            assert_eq!(run.policy("proj").unwrap().engine.access, access);
        }
        let run = local(dir.path(), both).with_settings(settings::Coder {
            start: settings::Start::AskFirst,
            ..settings::Coder::default()
        });
        assert!(run.asks_first());
        // Settings that cannot be read ask first and refuse to run.
        let broken = local(dir.path(), both).with_settings_result(Err("bad file".into()));
        assert!(broken.asks_first());
        assert!(!broken.ready());
        assert_eq!(broken.policy("proj").unwrap_err(), "bad file");
    }

    /// Write a fresh usage reading that puts `provider` at `fraction`.
    fn reading(store: &Path, provider: Provider, fraction: f64) {
        let now = autostart::unix_now();
        let book = usage::Book {
            schema: usage::SCHEMA.into(),
            entries: vec![usage::Entry {
                provider,
                attempted_at: now,
                next_probe_at: now + 60,
                reading: Some(usage::Reading {
                    provider,
                    observed_at: now,
                    windows: vec![usage::Window {
                        window: usage::WindowName::FiveHour,
                        used_fraction: fraction,
                        resets_at: Some(now + 3600),
                        length_seconds: None,
                    }],
                    limit_reached: false,
                    plan: None,
                    account: None,
                }),
                failure: None,
            }],
        };
        autostart::write_private(
            &store.join(usage::FILE),
            &serde_json::to_vec(&book).unwrap(),
        )
        .unwrap();
    }

    /// A start that asks for an engine, or would pass one over on an old
    /// reading, has the host read it now first (#10105): the hook runs with
    /// those providers before the choice, and the choice reads what the
    /// host then wrote. Here Claude, at its limit on a reading two minutes
    /// old, reads 5% fresh and runs; without the hook the old reading
    /// passes it over.
    #[test]
    fn a_start_has_the_host_read_an_engine_again_before_passing_it_over() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        let old_full = |store: &Path| {
            let now = autostart::unix_now();
            let book = usage::Book {
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
                            resets_at: Some(now + 86_400),
                            length_seconds: None,
                        }],
                        limit_reached: false,
                        plan: None,
                        account: None,
                    }),
                    failure: None,
                }],
            };
            autostart::write_private(
                &store.join(usage::FILE),
                &serde_json::to_vec(&book).unwrap(),
            )
            .unwrap();
        };
        // Without a host to ask: the old reading passes Claude over.
        let alone = local(&dir.path().join("alone"), both).with_launcher(Box::new(Held));
        old_full(alone.store());
        assert_eq!(alone.recheck(Some(Provider::Claude)), [Provider::Claude]);
        let record = alone
            .start_requested(&top, "Fix it", "Fix it.", None, &[], Some(Provider::Claude))
            .unwrap();
        assert_eq!(record.turns[0].provider, "codex");
        // With the host asked first: it reads Claude at 5% and Claude runs.
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let store = dir.path().join("hosted").join("tasks");
        let hook = {
            let asked = asked.clone();
            let store = store.clone();
            Box::new(move |providers: &[Provider]| {
                asked.lock().unwrap().push(providers.to_vec());
                reading(&store, Provider::Claude, 0.05);
            })
        };
        let hosted = local(&dir.path().join("hosted"), both)
            .with_launcher(Box::new(Held))
            .with_fresh(hook);
        old_full(hosted.store());
        let record = hosted
            .start_requested(&top, "Fix it", "Fix it.", None, &[], Some(Provider::Claude))
            .unwrap();
        assert_eq!(record.turns[0].provider, "claude");
        assert_eq!(asked.lock().unwrap().as_slice(), [vec![Provider::Claude]]);
    }

    /// The three things an offer says, each from the store's own books
    /// and this computer's logins.
    #[test]
    fn the_prediction_names_codex_claude_code_or_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let run = local(dir.path(), both);
        assert_eq!(run.predict(None).unwrap().text(), "Codex will do this.");

        reading(run.store(), Provider::Codex, 0.92);
        let runner = run.predict(None).unwrap();
        assert_eq!(
            runner.text(),
            "Codex is at 92% of its window; Claude Code will do this."
        );
        assert_eq!(runner.provider(), Some("claude"));

        let none = local(dir.path(), nobody);
        assert_eq!(
            none.predict(None).unwrap(),
            Runner::NotSignedIn {
                providers: vec!["codex".into(), "claude".into(), "grok".into()]
            },
            "no login, whatever the books say"
        );
        assert_eq!(
            none.predict(None).unwrap().text(),
            "None of Codex, Claude Code, Grok Build is signed in on this computer. \
             Sign in to one to run Coder here."
        );
    }

    /// What the offer predicted is what starts: the same provider, model,
    /// and prediction, recorded on the turn and its `coder_started`.
    #[test]
    fn the_prediction_is_what_the_run_starts_on() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        type Probe = fn(Provider) -> Connection;
        let cases: [(Probe, Option<f64>, bool); 4] = [
            (both, None, false),
            (both, Some(0.95), false),
            (both, None, true),
            (only_claude, None, false),
        ];
        for (index, (probe, near, refused)) in cases.into_iter().enumerate() {
            let home = dir.path().join(format!("case{index}"));
            let run = local(&home, probe).with_launcher(Box::new(Held));
            if let Some(fraction) = near {
                reading(run.store(), Provider::Codex, fraction);
            }
            if refused {
                let now = autostart::unix_now();
                capacity::record_with(
                    run.store(),
                    capacity::Refusal::new(
                        Provider::Codex,
                        capacity::Kind::UsageLimit,
                        now,
                        Some(now + 3600),
                    ),
                    |_| None,
                )
                .unwrap();
            }
            let predicted = run.predict(None).unwrap();
            let record = run
                .start(&top, "Fix the parser", "Fix the parser.", None)
                .unwrap();
            let start = &record.turns[0];
            assert_eq!(
                Some(start.provider.as_str()),
                predicted.provider(),
                "case {index}"
            );
            assert_eq!(start.runner.as_ref(), Some(&predicted), "case {index}");
            let Some(CoderEvent::CoderStarted(started)) = record.started(1) else {
                panic!("no start")
            };
            assert_eq!(started.runner, Some(predicted.clone()));
            let Runner::Runs { model, .. } = &predicted else {
                panic!("{predicted:?}")
            };
            assert_eq!(&started.model, model);
        }
    }

    fn only_codex(provider: Provider) -> Connection {
        match provider {
            Provider::Codex => Connection::Connected,
            _ => Connection::Missing("no login".into()),
        }
    }

    /// The engine the person asked for goes first, and the run falls back
    /// only when it is not signed in, refused for a limit, or not allowed
    /// by the settings; the start card says what was asked and why, and
    /// the prediction is what starts (#10076). Fake engines: the probe
    /// says who is signed in, the capacity book who is refused.
    #[test]
    fn the_engine_the_person_asked_for_runs_first_or_says_why_not() {
        let dir = tempfile::tempdir().unwrap();
        let top = repo(dir.path());
        type Probe = fn(Provider) -> Connection;
        // (probe, Claude refused, asked for, runs on, reason)
        let cases: [(Probe, bool, Provider, &str, &str); 7] = [
            (
                both,
                false,
                Provider::Claude,
                "claude",
                "You asked for Claude Code; it is signed in and has capacity.",
            ),
            (
                both,
                false,
                Provider::Codex,
                "codex",
                "You asked for Codex; it is signed in and has capacity.",
            ),
            (
                both,
                true,
                Provider::Claude,
                "codex",
                "You asked for Claude Code; it reached its usage limit until",
            ),
            (
                only_codex,
                false,
                Provider::Claude,
                "codex",
                "You asked for Claude Code; it is not signed in here, so Codex is running.",
            ),
            // Grok Build is allowed by default (#10091): asked for, it runs
            // when signed in, and is passed over with a plain reason when
            // not.
            (
                both,
                false,
                Provider::Grok,
                "grok",
                "You asked for Grok Build; it is signed in and has capacity.",
            ),
            (
                only_codex,
                false,
                Provider::Grok,
                "codex",
                "You asked for Grok Build; it is not signed in here, so Codex is running.",
            ),
            // Devin, a paid API, runs only when the settings name it.
            (
                both,
                false,
                Provider::Devin,
                "codex",
                "You asked for Devin; it is not one of the engines your Coder settings \
                 allow, so Codex is running.",
            ),
        ];
        for (index, (probe, refused, asked, runs, reason)) in cases.into_iter().enumerate() {
            let home = dir.path().join(format!("asked{index}"));
            let run = local(&home, probe).with_launcher(Box::new(Held));
            if refused {
                let now = autostart::unix_now();
                capacity::record_with(
                    run.store(),
                    capacity::Refusal::new(
                        Provider::Claude,
                        capacity::Kind::UsageLimit,
                        now,
                        Some(now + 3600),
                    ),
                    |_| None,
                )
                .unwrap();
            }
            let predicted = run.predict(Some(asked)).unwrap();
            let record = run
                .start_requested(&top, "Fix it", "Fix it.", None, &[], Some(asked))
                .unwrap();
            let start = &record.turns[0];
            assert_eq!(start.provider, runs, "case {index}");
            assert!(
                start.reason.starts_with(reason),
                "case {index}: {}",
                start.reason
            );
            assert_eq!(start.runner.as_ref(), Some(&predicted), "case {index}");
            assert_eq!(record.requested.as_deref(), Some(asked.as_str()));
            // A fallback that the reason says can't run is not named again.
            if runs != asked.as_str() {
                assert!(
                    start
                        .fallbacks
                        .iter()
                        .all(|f| !f.starts_with(asked.as_str())),
                    "case {index}: {:?}",
                    start.fallbacks
                );
                assert!(
                    start.reason.ends_with("so Codex is running."),
                    "case {index}"
                );
            }
        }
        // No request: the settings' order, as before.
        let run = local(&dir.path().join("plain"), both).with_launcher(Box::new(Held));
        let record = run.start(&top, "Fix it", "Fix it.", None).unwrap();
        assert_eq!(record.turns[0].provider, "codex");
        assert_eq!(
            record.turns[0].reason,
            "Codex is signed in and has capacity."
        );
        assert_eq!(record.requested, None);
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
        let review = super::super::review::read(&"a".repeat(64), &top, &base, 64 * 1024).unwrap();
        let diff = &review.diff;
        assert!(diff.contains("diff --git a/a.txt b/a.txt\n"), "{diff}");
        assert!(diff.contains("+two\n"), "{diff}");
        assert!(diff.contains("diff --git a/new.txt b/new.txt\n"), "{diff}");
        assert!(diff.contains("+x\n+y"), "{diff}");
        assert_eq!(
            review.completeness,
            coder_host::access::review::Completeness::Complete
        );
        let cut = super::super::review::read(&"a".repeat(64), &top, &base, 40).unwrap();
        assert!(cut.diff.len() <= 40);
        assert!(matches!(
            cut.completeness,
            coder_host::access::review::Completeness::Truncated { .. }
        ));
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
