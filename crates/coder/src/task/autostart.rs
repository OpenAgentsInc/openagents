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
//!   executable, one model, step and wall-clock limits, and the same
//!   filesystem boundary and supervisor as a hand-written grant.
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

use super::{Status, Store, adapter, owner};

/// The policy file in the host root.
pub const POLICY_FILE: &str = "autostart.json";
/// The append-only record in the host root.
pub const JOURNAL_FILE: &str = "autostart.jsonl";
pub const POLICY_SCHEMA: &str = "openagents.coder.host-autostart.v1";
pub const ENTRY_SCHEMA: &str = "openagents.coder.host-autostart-entry.v1";
/// The most tasks a policy may run at once.
pub const MAX_RUNNING: u32 = 8;
/// How long a started task may stay queued, waiting for its owner process
/// to admit it, before it stops counting against the concurrency bound.
const PENDING_GRACE: u64 = 120;
/// How often the host looks for eligible tasks it could not start earlier.
pub const SWEEP_EVERY: Duration = Duration::from_secs(10);

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
}

impl Policy {
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
        if engine.adapter != adapter::NAME {
            return Err(format!("the only engine adapter is {}", adapter::NAME));
        }
        if !engine.controller.is_absolute() {
            return Err("the controller path must be absolute".into());
        }
        // The same checks the task owner makes at admission, so a policy
        // that could never start a task refuses now.
        self.configuration().validate().map_err(|e| e.to_string())?;
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

    fn configuration(&self) -> adapter::Configuration {
        let engine = &self.engine;
        adapter::Configuration {
            schema: adapter::CONFIG_SCHEMA.into(),
            provider: "codex".into(),
            model: engine.model.clone(),
            effort: engine.effort.clone(),
            generation_endpoint: "https://chatgpt.com/backend-api/codex".into(),
            decision_endpoint: engine.decision_endpoint.clone(),
            decision_model: engine.decision_model.clone(),
            max_steps: engine.max_steps,
            acceptance: false,
            route: "never".into(),
            knowledge: "off".into(),
            dollar_limit_micros: None,
            expected_controller_digest: None,
            container: None,
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
    /// `eligible`, `started`, `skipped`, `refused`, `policy_on`, or
    /// `policy_off`.
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
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
}

impl Entry {
    fn new(at: u64, event: &str) -> Self {
        Self {
            schema: ENTRY_SCHEMA.into(),
            at,
            event: event.into(),
            task: None,
            device: None,
            workspace: None,
            grant_digest: None,
            owner_process: None,
            detail: None,
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
}

/// Append one entry to `root/autostart.jsonl`, created `0600`.
///
/// # Errors
/// Reports a failed write.
pub fn record(root: &Path, entry: &Entry) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::create_dir_all(root).map_err(|_| "cannot create the host root".to_owned())?;
    let mut line = serde_json::to_vec(entry).map_err(|e| e.to_string())?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
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
    sweeping: Mutex<()>,
    background: bool,
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
            sweeping: Mutex::new(()),
            background: true,
        }
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
        let mut entry = Entry::new((self.now)(), "eligible").task(task);
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
        let mut decided: BTreeSet<String> = BTreeSet::new();
        let mut started: Vec<(String, u64)> = Vec::new();
        for entry in history {
            let Some(task) = entry.task.clone() else {
                continue;
            };
            match entry.event.as_str() {
                "eligible" if !waiting.iter().any(|w| w.task == entry.task) => {
                    waiting.push(entry);
                }
                "started" => {
                    started.push((task.clone(), entry.at));
                    decided.insert(task);
                }
                "skipped" | "refused" => {
                    decided.insert(task);
                }
                _ => {}
            }
        }
        waiting.retain(|entry| entry.task.as_ref().is_some_and(|t| !decided.contains(t)));
        if waiting.is_empty() {
            return Vec::new();
        }
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
            let store = match Store::open(&self.store) {
                Ok(store) => store,
                Err(error) => {
                    eprintln!("coder host: auto-start cannot open the task store: {error}");
                    return written;
                }
            };
            let mut active = started
                .iter()
                .filter(|(task, at)| match store.show(task) {
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
            let mut plans = Vec::new();
            for entry in &waiting {
                let id = entry.task.clone().unwrap_or_default();
                let workspace = entry.workspace.clone().unwrap_or_default();
                let task = match store.show(&id) {
                    Ok(task) => task,
                    Err(_) => {
                        write(
                            Entry::new(now, "skipped")
                                .task(&id)
                                .detail("the task is gone"),
                        );
                        continue;
                    }
                };
                if task.status != Status::Queued || task.run.is_some() {
                    write(
                        Entry::new(now, "skipped")
                            .task(&id)
                            .detail("the task is no longer queued"),
                    );
                    continue;
                }
                if !policy.admits(&workspace) || !self.workspaces.contains_key(&workspace) {
                    write(
                        Entry::new(now, "skipped")
                            .task(&id)
                            .detail("the policy no longer admits the workspace"),
                    );
                    continue;
                }
                if task.intent.configuration.model.as_deref() != Some(&policy.engine.model) {
                    write(
                        Entry::new(now, "skipped")
                            .task(&id)
                            .detail("the policy's model changed after the task was created"),
                    );
                    continue;
                }
                if active >= policy.max_running as usize {
                    break;
                }
                active += 1;
                plans.push((id, workspace, task.intent_digest.clone(), task.revision));
            }
            plans
        };
        for (id, workspace, intent_digest, revision) in plans {
            let entry = match self.start(&policy, &id, &intent_digest, revision) {
                Ok(launched) => {
                    let mut entry = Entry::new(now, "started").task(&id);
                    entry.workspace = Some(workspace);
                    entry.grant_digest = Some(launched.grant_digest);
                    entry.owner_process = Some(launched.owner_process);
                    entry.detail = Some(format!(
                        "{} {} max_steps {} wall_seconds {} write_workspace {}",
                        policy.engine.adapter,
                        policy.engine.model,
                        policy.engine.max_steps,
                        policy.engine.wall_seconds,
                        policy.engine.write_workspace
                    ));
                    entry
                }
                Err(why) => Entry::new(now, "refused").task(&id).detail(why),
            };
            write(entry);
        }
        written
    }

    fn start(
        &self,
        policy: &Policy,
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
            adapter_configuration: Some(policy.configuration()),
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
fn shell() -> std::result::Result<PathBuf, String> {
    ["/bin/bash", "/bin/sh"]
        .iter()
        .find_map(|path| Path::new(path).canonicalize().ok())
        .ok_or_else(|| "no system shell at /bin/bash or /bin/sh".into())
}

fn write_private(path: &Path, bytes: &[u8]) -> std::result::Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let parent = path.parent().ok_or("no parent directory")?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|_| format!("cannot create {}", parent.display()))?;
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
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
  show                 Print the policy and the latest decisions.
  on --workspace LABEL [--workspace LABEL]... [--max-running N] [--model ID]
     [--effort low|medium|high|xhigh] [--max-steps N] [--wall-seconds N]
     [--memory-mib N] [--read-only] [--controller PATH]
     [--decision-endpoint URL] [--decision-model ID]
                       Start tasks that enrolled devices with `operate` create
                       in these workspaces, at most N at once (default 1).
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
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--read-only" {
            read_only = true;
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
            let engine = Engine {
                adapter: adapter::NAME.into(),
                controller,
                model: take_one(&mut values, "--model")?.unwrap_or_else(|| "gpt-6-luna".into()),
                effort: Some(take_one(&mut values, "--effort")?.unwrap_or_else(|| "medium".into())),
                max_steps: usize::try_from(max_steps).map_err(|_| "--max-steps is too large")?,
                wall_seconds,
                memory_bytes: memory_mib.saturating_mul(1024 * 1024),
                write_workspace: !read_only,
                decision_endpoint: take_one(&mut values, "--decision-endpoint")?
                    .unwrap_or_else(|| "https://api.typesafe.ai".into()),
                decision_model: take_one(&mut values, "--decision-model")?
                    .unwrap_or_else(|| "jev-latest".into()),
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
                    "workspaces {} max_running {} model {} write_workspace {}",
                    workspaces.join(","),
                    policy.max_running,
                    policy.engine.model,
                    policy.engine.write_workspace
                )),
            )?;
            println!("on");
        }
        _ => return Err(format!("usage: unknown command `{command}`")),
    }
    Ok(())
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
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .map_err(|_| "cannot run git".to_owned())?;
    let common = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let root = path
        .canonicalize()
        .map_err(|_| "the workspace is missing".to_owned())?;
    if !output.status.success() || common.as_os_str().is_empty() {
        return Err(format!("{} is not a Git checkout", root.display()));
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

    fn setup() -> Setup {
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
            },
            changed_at: 1,
        }
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
                ("started".into(), Some(second.clone())),
            ]
        );
        assert_eq!(journal(&s.root)[0].device.as_deref(), Some("phone"));
        assert_eq!(journal(&s.root)[1].owner_process, Some(4242));
        assert!(s.autostart.sweep().is_empty(), "nothing starts twice");
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
}
