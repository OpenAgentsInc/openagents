//! Durable, single-host execution ownership for an explicitly admitted command.
//!
//! The inbox lock protects journal updates. A separate OS-held lock outlives
//! each executor. Losing that owner never grants permission to replay an effect.

use super::*;
use atif::{Log, Session, Source, Step};
use coder_boundary::{Boundary, Snapshot};
use serde_json::json;
use supervise::{Input, Job, Limits};

/// A bounded command reads its instructions once, at admission, like any
/// task-owner run; its command itself takes no input.
pub const BOUNDED_COMMAND_STEERING: coder_delegate::steering::Steering =
    coder_delegate::steering::Steering {
        adapter: "bounded-command",
        native: coder_delegate::steering::Native::TurnBoundary,
        emulation: None,
        acknowledgment: coder_delegate::steering::Acknowledgment::NextTurnStart,
        limitations: &["The command's standard input is closed."],
    };

pub const GRANT_SCHEMA: &str = "openagents.coder.task-execution-grant.v1";
/// The result ending the store records for a run whose owner process ended
/// without recording one, killed or crashed or with the computer restarted
/// (#10124): its owner lock was free and no process group it recorded was
/// left. Its run is failed (stopped, when a stop was asked).
pub const OWNER_ENDED: &str = "owner_process_ended";
/// What a person reads for a run that ended as [`OWNER_ENDED`].
pub const OWNER_ENDED_TEXT: &str = "Coder's process ended unexpectedly";
/// The refusal an engine gives a grant whose fields it cannot read: one
/// written by a newer program, with a field the engine does not know or
/// without one it still requires (#10113).
pub const GRANT_SHAPE: &str = "the execution grant has an invalid shape";
/// The most owner events one task retains. No event is silently pruned.
pub(super) const MAX_HOST_EVENTS: usize = 8192;

/// The fixed, root-owned paths `git` is taken from, in order: where
/// distributions install it, then NixOS's system profile. Like the
/// boundary's `bwrap`, it is never searched for on `PATH`.
#[cfg(not(windows))]
pub const GIT_PATHS: [&str; 2] = ["/usr/bin/git", "/run/current-system/sw/bin/git"];

/// The fixed paths `git` is taken from on Windows: Git for Windows'
/// machine-wide install, never a search of `PATH`.
#[cfg(windows)]
pub const GIT_PATHS: [&str; 3] = [
    r"C:\Program Files\Git\cmd\git.exe",
    r"C:\Program Files\Git\ucrt64\bin\git.exe",
    r"C:\Program Files\Git\mingw64\bin\git.exe",
];

/// The `PATH` owned commands run with: the system directories, then NixOS's
/// root-owned system profile, which exists only there.
#[cfg(not(windows))]
pub const SYSTEM_PATH: &str = "/usr/bin:/bin:/run/current-system/sw/bin";

/// The `PATH` owned commands run with on Windows: Git for Windows' Unix
/// tools and `git`, then the system directories.
#[cfg(windows)]
pub const SYSTEM_PATH: &str = r"C:\Program Files\Git\usr\bin;C:\Program Files\Git\ucrt64\bin;C:\Program Files\Git\mingw64\bin;C:\Program Files\Git\cmd;C:\Windows\System32;C:\Windows;C:\Windows\System32\Wbem";

/// The canonical system shells a repository grant may name. On Windows
/// that is Git for Windows' `bash` (its MSYS build, then its launcher),
/// installed machine-wide: the engine writes bash scripts everywhere.
#[cfg(not(windows))]
pub const SYSTEM_SHELLS: [&str; 2] = ["/bin/bash", "/bin/sh"];

/// The canonical system shells a repository grant may name. On Windows
/// that is Git for Windows' `bash` (its MSYS build, then its launcher),
/// installed machine-wide: the engine writes bash scripts everywhere.
#[cfg(windows)]
pub const SYSTEM_SHELLS: [&str; 2] = [
    r"C:\Program Files\Git\usr\bin\bash.exe",
    r"C:\Program Files\Git\bin\bash.exe",
];

/// The variable a Windows shell command reads its script from; see
/// [`shell_arguments`].
pub const SCRIPT_VARIABLE: &str = "OPENAGENTS_SCRIPT";

/// What a bash started on Windows runs: the script from
/// [`SCRIPT_VARIABLE`], which it then unsets. It holds no backslash, so
/// the C runtime and Cygwin read its quoting the same way.
pub const SCRIPT_RUNNER: &str = r#"__openagents_script=$OPENAGENTS_SCRIPT; unset OPENAGENTS_SCRIPT; eval "$__openagents_script""#;

/// The variables [`shell_arguments`] sets for a command: the script, on
/// Windows.
pub type ScriptVariables = Vec<(&'static str, String)>;

/// The most UTF-16 units a Windows environment variable holds.
pub const WINDOWS_SCRIPT_MAX: usize = 32_000;

/// The arguments that make the system shell run `script`, and the
/// variables it needs set for that. On Unix it is `-c SCRIPT`. A Windows
/// program splits its own command line, and Git for Windows' `bash`
/// splits it by Cygwin's rules, which differ from the C runtime's for
/// backslashes before a quote; so there the script travels in
/// [`SCRIPT_VARIABLE`], which no one parses, and the command line holds
/// only [`SCRIPT_RUNNER`].
///
/// # Errors
///
/// A script too long for a Windows environment variable.
pub fn shell_arguments(script: &str) -> Result<(Vec<String>, ScriptVariables), Error> {
    if cfg!(windows) {
        if script.encode_utf16().count() > WINDOWS_SCRIPT_MAX {
            return Err(Error::InvalidCommand(
                "a command script on Windows is at most 32000 characters; split it",
            ));
        }
        Ok((
            vec!["-c".into(), SCRIPT_RUNNER.into()],
            vec![(SCRIPT_VARIABLE, script.into())],
        ))
    } else {
        Ok((vec!["-c".into(), script.into()], Vec::new()))
    }
}

/// The variables a Windows program cannot start without, from this
/// process's own environment, for a command whose environment is
/// otherwise cleared: `SystemRoot` (Winsock and much else load from it),
/// `windir`, `SystemDrive`, `ComSpec`, and `PATHEXT`. Unix needs none.
#[must_use]
pub fn base_environment() -> Vec<(&'static str, std::ffi::OsString)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    ["SystemRoot", "windir", "SystemDrive", "ComSpec", "PATHEXT"]
        .into_iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (name, value)))
        .collect()
}

/// Explicit local authority. This is supplied by the operator, never the model.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub schema: String,
    pub task_id: String,
    pub intent_digest: String,
    pub expected_revision: u64,
    #[serde(default)]
    pub expected_source_snapshot: Option<String>,
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub write_workspace: bool,
    /// How long a granted command may run, 1 to 3,600 seconds. A Coder run
    /// (a grant with an [`Grant::adapter_configuration`]) has no time
    /// limit: new ones leave this out (0), and an older grant's value is
    /// read and ignored.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wall_seconds: u64,
    pub stream_bytes: usize,
    pub memory_bytes: u64,
    #[serde(default)]
    pub requirements: Option<checks::Requirements>,
    #[serde(default)]
    pub adapter_configuration: Option<super::adapter::Configuration>,
}

impl Grant {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let value = parse_strict_bounded(bytes, MAX_COMMAND_BYTES).map_err(|_| {
            Error::InvalidCommand("the execution grant must be bounded strict JSON")
        })?;
        let grant: Self =
            serde_json::from_value(value).map_err(|_| Error::InvalidCommand(GRANT_SHAPE))?;
        grant.validate()?;
        Ok(grant)
    }

    fn validate(&self) -> Result<(), Error> {
        if self.schema != GRANT_SCHEMA
            || !identifier(&self.task_id, false)
            || !hex_digest(&self.intent_digest)
            || self
                .expected_source_snapshot
                .as_ref()
                .is_some_and(|digest| !hex_digest(digest))
            || !self.program.is_absolute()
            || self.arguments.len() > 128
            || self.arguments.iter().any(|arg| arg.contains('\0'))
            || (self.adapter_configuration.is_none() && !(1..=3600).contains(&self.wall_seconds))
            || !(1024..=1024 * 1024).contains(&self.stream_bytes)
            || !(64 * 1024 * 1024..=8 * 1024 * 1024 * 1024).contains(&self.memory_bytes)
        {
            return Err(Error::InvalidCommand(
                "the execution grant has invalid identities or bounds",
            ));
        }
        if let Some(requirements) = &self.requirements {
            requirements.validate()?;
        }
        if let Some(configuration) = &self.adapter_configuration {
            configuration.validate()?;
        }
        Ok(())
    }
}

pub(super) fn network_policy() -> &'static str {
    if cfg!(target_os = "macos") {
        "external_ip_denied_localhost_allowed"
    } else if cfg!(windows) {
        "appcontainer_without_network_capabilities"
    } else {
        "network_namespace_isolated"
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &u64) -> bool {
    *value == 0
}

fn hex_digest(value: &str) -> bool {
    let value = value.strip_prefix("sha256:").unwrap_or(value);
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Immutable source, configuration, and local-authority evidence for an attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub grant: Grant,
    pub grant_digest: String,
    pub grant_request: String,
    pub workspace: PathBuf,
    pub source_revision: String,
    pub source_snapshot: String,
    pub program_digest: String,
    pub adapter: String,
    pub network: String,
    pub read_scope: String,
    pub authority: String,
    pub trace_file: String,
    pub context: checks::Context,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub epoch: u64,
    pub admission: Admission,
    pub effect_id: Option<String>,
    pub result: Option<ResultRecord>,
    pub process_id: Option<u32>,
    pub recovery_reason: Option<String>,
    pub check_report: Option<checks::Report>,
}

/// An executor result, never an independent correctness or integration verdict.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultRecord {
    pub ending: String,
    pub exit_code: Option<i32>,
    pub stop_requested: bool,
    pub group_clear: bool,
    pub elapsed_ms: u64,
    pub trace_digest: String,
    pub candidate_snapshot: Option<String>,
    pub artifact_file: Option<String>,
    pub artifact_digest: Option<String>,
    pub output_incomplete: bool,
    /// `priced` when the run's whole cost is known, `partial` when only
    /// part of it is, `unknown` otherwise. Information about the run,
    /// never a limit on it.
    pub cost_status: String,
    /// The run's whole cost in micro-dollars, the engine and Jev
    /// together, when both are known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
    /// The engine's part (model calls, or a whole coding agent's turn).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_microusd: Option<u64>,
    /// Jev's part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jev_microusd: Option<u64>,
    /// Who paid for the run's model calls (BYOK, #10176): `ours` or
    /// `theirs`, as the run's process held it. Absent on records written
    /// before it was kept, and on a run its owner ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<model_access::Paid>,
    /// Under `theirs`, each of the person's keys that may have paid: the
    /// provider and the key's fingerprint, never the key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payer_keys: Vec<model_access::KeyPrint>,
}

/// What a run cost, by part, as its engine reported or priced it. A part
/// left `None` is unknown, never a stand-in zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    pub engine_microusd: Option<u64>,
    pub jev_microusd: Option<u64>,
}

impl Cost {
    /// A known zero: nothing spent yet.
    pub const ZERO: Self = Self {
        engine_microusd: Some(0),
        jev_microusd: Some(0),
    };

    /// A cost from dollar amounts, each `None` when unknown.
    #[must_use]
    pub fn from_usd(engine: Option<f64>, jev: Option<f64>) -> Self {
        let micro =
            |usd: f64| (usd.is_finite() && usd >= 0.0).then(|| (usd * 1_000_000.0).round() as u64);
        Self {
            engine_microusd: engine.and_then(micro),
            jev_microusd: jev.and_then(micro),
        }
    }

    /// The whole cost, when every part is known.
    #[must_use]
    pub fn total_microusd(&self) -> Option<u64> {
        Some(self.engine_microusd?.saturating_add(self.jev_microusd?))
    }

    /// This cost and `other` together: a part is known only when it is
    /// known in both.
    #[must_use]
    pub fn plus(self, other: Self) -> Self {
        let add = |a: Option<u64>, b: Option<u64>| Some(a?.saturating_add(b?));
        Self {
            engine_microusd: add(self.engine_microusd, other.engine_microusd),
            jev_microusd: add(self.jev_microusd, other.jev_microusd),
        }
    }

    /// `priced`, `partial`, or `unknown`.
    #[must_use]
    pub fn status(&self) -> &'static str {
        match (self.engine_microusd, self.jev_microusd) {
            (Some(_), Some(_)) => "priced",
            (None, None) => "unknown",
            _ => "partial",
        }
    }
}

impl ResultRecord {
    /// Records `cost` on the result: its status, its parts, and its total.
    pub fn priced(&mut self, cost: Cost) {
        self.cost_status = cost.status().into();
        self.cost_microusd = cost.total_microusd();
        self.engine_microusd = cost.engine_microusd;
        self.jev_microusd = cost.jev_microusd;
    }

    /// Records who pays for this process's model calls
    /// ([`model_access::current`]).
    pub fn paid_by(&mut self, access: &model_access::Access) {
        let (payer, keys) = access.paid();
        self.payer = Some(payer);
        self.payer_keys = keys;
    }

    /// The cost these fields record, when they agree with each other.
    fn cost_consistent(&self) -> bool {
        let cost = Cost {
            engine_microusd: self.engine_microusd,
            jev_microusd: self.jev_microusd,
        };
        self.cost_status == cost.status() && self.cost_microusd == cost.total_microusd()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Event {
    Admitted { admission: Box<Admission> },
    EffectIntent { effect_id: String },
    Result { result: ResultRecord },
    Spawned { process_id: u32 },
    OwnerLost,
    CheckIntent,
    Checked { report: checks::Report },
    CheckOwnerLost,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub sequence: u64,
    pub(super) task_id: String,
    epoch: u64,
    event: Event,
}

/// Whether `task` holds its workspace's tree: its execution is unresolved
/// (running, stopping, or unknown) or its checks run. Admission and checks
/// of another task in an overlapping tree wait for it.
fn reserves(task: &Task) -> bool {
    matches!(
        task.status,
        Status::Running | Status::CancelRequested | Status::Unknown
    ) || task.checks == Checks::Running
}

/// Whether [`Store::settle`] may end `task`'s run once its owner is gone:
/// a run with no result yet that is running, stopping, or unknown after a
/// lost owner, or checks that are running.
fn settleable(task: &Task) -> bool {
    let Some(run) = &task.run else {
        return false;
    };
    (run.result.is_none()
        && matches!(
            task.status,
            Status::Running | Status::CancelRequested | Status::Unknown
        ))
        || (task.checks == Checks::Running && run.result.is_some())
}

pub(super) fn transition(record: &Record, tasks: &mut BTreeMap<String, Task>) -> Result<(), Error> {
    let reserved_workspace = match &record.event {
        Event::Admitted { admission } => Some(&admission.workspace),
        Event::CheckIntent => tasks
            .get(&record.task_id)
            .and_then(|task| task.run.as_ref())
            .map(|run| &run.admission.workspace),
        _ => None,
    };
    if let Some(workspace) = reserved_workspace
        && tasks.values().any(|task| {
            task.task_id != record.task_id
                && reserves(task)
                && task.run.as_ref().is_some_and(|run| {
                    run.admission.workspace.starts_with(workspace)
                        || workspace.starts_with(&run.admission.workspace)
                })
        })
    {
        return Err(Error::WorkspaceBusy);
    }
    let task = tasks.get_mut(&record.task_id).ok_or(Error::NotFound)?;
    match &record.event {
        Event::Admitted { admission } => {
            admission.grant.validate()?;
            if task.run.is_some()
                || task.status != Status::Queued
                || record.epoch != 1
                || admission.grant.task_id != task.task_id
                || admission.grant.intent_digest != task.intent_digest
                || admission.grant.expected_revision != task.revision
                || task.intent.configuration.adapter != admission.adapter
                || match (
                    &admission.grant.adapter_configuration,
                    admission.adapter.as_str(),
                ) {
                    (None, "bounded-command") => task.intent.configuration.model.is_some(),
                    (Some(config), super::adapter::NAME) => !task
                        .intent
                        .configuration
                        .model
                        .as_deref()
                        .is_some_and(|model| config.admits_model(model)),
                    _ => true,
                }
                || if admission
                    .grant
                    .adapter_configuration
                    .as_ref()
                    .is_some_and(|config| config.container.is_some())
                {
                    admission.network != "container_network_none"
                        || admission.read_scope != "workspace_host_reads_and_pinned_container_image"
                } else if admission
                    .grant
                    .adapter_configuration
                    .as_ref()
                    .is_some_and(|config| config.access == super::adapter::Access::Full)
                {
                    // The owner's full access: the host user's reads and
                    // network, and nothing narrower claimed.
                    admission.network != "host_network" || admission.read_scope != "host_user"
                } else if admission
                    .grant
                    .adapter_configuration
                    .as_ref()
                    .is_some_and(|config| config.access == super::adapter::Access::Toolchains)
                {
                    // This computer's tools: the network, and reads of the
                    // workspace, the system, and the derived toolchains.
                    admission.network != "host_network"
                        || admission.read_scope != "workspace_system_and_toolchains"
                } else {
                    !matches!(
                        admission.network.as_str(),
                        "external_ip_denied_localhost_allowed" | "network_namespace_isolated"
                    ) || admission.read_scope != "workspace_and_system"
                }
                || admission.authority != "local_os_user"
                || !admission
                    .context
                    .valid(task, admission.grant.requirements.as_ref())
                || admission.trace_file != task.trace_file(task.turn())
                || !hex_digest(&admission.source_snapshot)
                || !hex_digest(&admission.program_digest)
                || Grant::parse(admission.grant_request.as_bytes())? != admission.grant
                || digest_bytes(admission.grant_request.as_bytes()) != admission.grant_digest
                || !hex_digest(&admission.grant_digest)
            {
                return Err(Error::InvalidTransition);
            }
            task.run = Some(Run {
                epoch: 1,
                admission: (**admission).clone(),
                effect_id: None,
                result: None,
                process_id: None,
                recovery_reason: None,
                check_report: None,
            });
            task.status = Status::Running;
            task.execution = Execution::Running;
        }
        Event::EffectIntent { effect_id } => {
            let expected = effect_id_for(task);
            let run = task.run.as_mut().ok_or(Error::InvalidTransition)?;
            if run.epoch != record.epoch
                || run.effect_id.is_some()
                || task.status != Status::Running
                || effect_id != &expected
            {
                return Err(Error::InvalidTransition);
            }
            run.effect_id = Some(effect_id.clone());
        }
        Event::Spawned { process_id } => {
            let run = task.run.as_mut().ok_or(Error::InvalidTransition)?;
            if run.epoch != record.epoch
                || run.effect_id.is_none()
                || run.process_id.is_some()
                || *process_id == 0
                || !matches!(task.status, Status::Running | Status::CancelRequested)
            {
                return Err(Error::InvalidTransition);
            }
            run.process_id = Some(*process_id);
        }
        Event::Result { result } => {
            let run = task.run.as_mut().ok_or(Error::InvalidTransition)?;
            // An unknown run takes a result only as the store settles it
            // after its owner ended (#10124).
            if run.epoch != record.epoch
                || run.result.is_some()
                || !(matches!(task.status, Status::Running | Status::CancelRequested)
                    || (task.status == Status::Unknown && result.ending == OWNER_ENDED))
                || (result.ending == OWNER_ENDED && !result.group_clear)
                || !hex_digest(&result.trace_digest)
                || !result.cost_consistent()
            {
                return Err(Error::InvalidTransition);
            }
            task.execution = if !result.group_clear {
                Execution::Unknown
            } else if result.stop_requested || task.status == Status::CancelRequested {
                Execution::Stopped
            } else if result.exit_code == Some(0) {
                Execution::Finished
            } else {
                Execution::Failed
            };
            task.status = if result.group_clear {
                Status::Finished
            } else {
                Status::Unknown
            };
            run.result = Some(result.clone());
        }
        Event::CheckIntent => {
            let run = task.run.as_ref().ok_or(Error::InvalidTransition)?;
            if run.epoch != record.epoch
                || task.status != Status::Finished
                || task.checks != Checks::NotRun
                || task.context_superseded()
                || run.admission.grant.requirements.is_none()
                || run.result.is_none()
            {
                return Err(Error::InvalidTransition);
            }
            task.checks = Checks::Running;
        }
        Event::Checked { report } => {
            let superseded = task.context_superseded();
            let run = task.run.as_mut().ok_or(Error::InvalidTransition)?;
            let requirements = run
                .admission
                .grant
                .requirements
                .as_ref()
                .ok_or(Error::InvalidTransition)?;
            report.validate(
                requirements,
                &run.admission.context,
                run.result
                    .as_ref()
                    .and_then(|result| result.candidate_snapshot.as_deref()),
            )?;
            if run.epoch != record.epoch
                || !matches!(task.checks, Checks::Running | Checks::Disputed)
                || report.schema != "openagents.coder.task-checks.v1"
                || report.requirements_digest != requirements.digest()
                || report.context_digest != run.admission.context.digest
                || run
                    .result
                    .as_ref()
                    .and_then(|result| result.candidate_snapshot.as_ref())
                    != report.candidate_snapshot.as_ref()
                || !matches!(
                    report.verdict,
                    Checks::Passed | Checks::Failed | Checks::Unavailable | Checks::Disputed
                )
            {
                return Err(Error::InvalidTransition);
            }
            task.checks = if superseded {
                Checks::Disputed
            } else {
                report.verdict
            };
            run.check_report = Some(report.clone());
        }
        Event::CheckOwnerLost => {
            let run = task.run.as_mut().ok_or(Error::InvalidTransition)?;
            if task.checks != Checks::Running || record.epoch != run.epoch + 1 {
                return Err(Error::InvalidTransition);
            }
            task.checks = Checks::Unavailable;
            run.epoch = record.epoch;
            run.recovery_reason = Some("checker_owner_lost".into());
        }
        Event::OwnerLost => {
            let run = task.run.as_mut().ok_or(Error::InvalidTransition)?;
            if record.epoch != run.epoch + 1
                || !matches!(task.status, Status::Running | Status::CancelRequested)
            {
                return Err(Error::InvalidTransition);
            }
            run.epoch = record.epoch;
            run.recovery_reason = Some("owner_lost_effects_not_replayed".into());
            task.status = Status::Unknown;
            task.execution = Execution::Unknown;
        }
    }
    task.revision += 1;
    Ok(())
}

impl Store {
    /// Record `event` on the owner's task under that task's lock. An event
    /// that reserves a workspace (admission, check intent) also holds the
    /// workspace lock and checks every other task, so two unresolved runs
    /// never share a tree across processes.
    fn record(&mut self, owner: &Owner, event: Event, epoch: u64) -> Result<Task, Error> {
        self.check_healthy()?;
        owner.verify(&self.dir)?;
        // Before a run takes its tree, end the runs there whose owners are
        // gone, so a dead one never holds the project (#10124).
        if let Event::Admitted { admission } = &event {
            let workspace = admission.workspace.clone();
            self.settle_all(Some(&workspace), Some(&owner.task_id));
        }
        let reserving = matches!(event, Event::Admitted { .. } | Event::CheckIntent);
        let _task = self.lock_task(&owner.task_id)?;
        let _workspaces = if reserving {
            Some(self.lock_workspaces()?)
        } else {
            None
        };
        let mut file = self.read_task(&owner.task_id)?.ok_or(Error::NotFound)?;
        if file.host_events.len() >= MAX_HOST_EVENTS {
            return Err(Error::LimitExceeded);
        }
        let mut tasks = BTreeMap::new();
        if reserving {
            for task in self.list()? {
                if task.task_id != owner.task_id {
                    tasks.insert(task.task_id.clone(), task);
                }
            }
        }
        tasks.insert(owner.task_id.clone(), file.task.clone());
        let record = Record {
            sequence: file.next_sequence(),
            task_id: owner.task_id.clone(),
            epoch,
            event,
        };
        transition(&record, &mut tasks)?;
        file.task = tasks
            .remove(&owner.task_id)
            .ok_or(Error::Corrupt("a transition lost its task"))?;
        file.host_events.push(record);
        if let Err(error) = self.write_task(&file) {
            self.healthy = false;
            return Err(error);
        }
        Ok(file.task)
    }
}

impl Store {
    /// End `id`'s unresolved run when nothing of it is left (#10124): its
    /// owner's OS lock is free, so its owner process is gone (killed,
    /// crashed, or the computer restarted), and no process group the run
    /// recorded is left. A run with no result gets one, ending
    /// [`OWNER_ENDED`], so the task is finished with its run failed
    /// (stopped, when a stop was asked); checks whose owner is gone become
    /// unavailable. Nothing is killed, rerun, or deleted: the journal keeps
    /// every earlier record, and the trace stays as the owner left it.
    ///
    /// Returns the task when this call ended it, `None` when there was
    /// nothing to end or something of it still runs.
    ///
    /// # Errors
    /// Store and lock I/O failures.
    pub fn settle(&mut self, id: &str) -> Result<Option<Task>, Error> {
        if !settleable(&self.show(id)?) {
            return Ok(None);
        }
        // A live owner holds this lock for as long as it runs; a recycled
        // process ID cannot hold it.
        let owner = match Owner::acquire(self, id) {
            Ok(owner) => owner,
            Err(Error::Busy) => return Ok(None),
            Err(error) => return Err(error),
        };
        let task = self.show(id)?;
        let Some(run) = task.run.as_ref().filter(|_| settleable(&task)) else {
            return Ok(None);
        };
        if run.process_id.is_some_and(group_alive) {
            return Ok(None);
        }
        let ended = if run.result.is_some() {
            self.record(&owner, Event::CheckOwnerLost, run.epoch + 1)?
        } else {
            let trace = std::fs::read(self.dir.join(&run.admission.trace_file)).unwrap_or_default();
            let result = ResultRecord {
                ending: OWNER_ENDED.into(),
                exit_code: None,
                stop_requested: task.status == Status::CancelRequested
                    || task.cancellation_reason.is_some(),
                // The owner's lock was free and no recorded group was left.
                group_clear: true,
                elapsed_ms: 0,
                trace_digest: digest_bytes(&trace),
                candidate_snapshot: None,
                artifact_file: None,
                artifact_digest: None,
                output_incomplete: true,
                cost_status: "unknown".into(),
                cost_microusd: None,
                engine_microusd: None,
                jev_microusd: None,
                payer: None,
                payer_keys: Vec::new(),
            };
            self.record(&owner, Event::Result { result }, run.epoch)?
        };
        Ok(Some(ended))
    }

    /// [`Store::settle`] every task but `except` whose run holds a tree
    /// overlapping `workspace` (every task, with `None`). Returns the tasks
    /// it ended; one it cannot end is left as it is.
    pub fn settle_all(&mut self, workspace: Option<&Path>, except: Option<&str>) -> Vec<Task> {
        let ids: Vec<String> = self
            .list()
            .unwrap_or_default()
            .iter()
            .filter(|task| Some(task.task_id.as_str()) != except && settleable(task))
            .filter(|task| {
                workspace.is_none_or(|workspace| {
                    task.run.as_ref().is_some_and(|run| {
                        run.admission.workspace.starts_with(workspace)
                            || workspace.starts_with(&run.admission.workspace)
                    })
                })
            })
            .map(|task| task.task_id.clone())
            .collect();
        ids.iter()
            .filter_map(|id| self.settle(id).ok().flatten())
            .collect()
    }
}

/// Whether process `pid` or the process group it leads still exists. A
/// supervised command leads its own group, so a descendant that outlived
/// it keeps the group alive. Anything but a clear "no such process" counts
/// as alive, so nothing live is ever taken for gone.
#[cfg(unix)]
fn group_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return true;
    };
    if pid <= 0 {
        return true;
    }
    let exists = |target: libc::pid_t| {
        // SAFETY: signal 0 checks for the process or group and delivers nothing.
        let sent = unsafe { libc::kill(target, 0) };
        sent == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    };
    exists(pid) || exists(-pid)
}

/// Without a way to look here, a recorded process counts as alive.
#[cfg(not(unix))]
fn group_alive(_pid: u32) -> bool {
    true
}

/// An exclusive OS-held owner capability. It cannot be deserialized or forged by a command.
pub(super) struct Owner {
    pub(super) dir: PathBuf,
    task_id: String,
    lock: File,
}

impl Owner {
    /// Take `id`'s owner lock now, or refuse with [`Error::Busy`] while
    /// another process holds it: how a reader asks whether an owner lives.
    pub(super) fn acquire(store: &Store, id: &str) -> Result<Self, Error> {
        Self::acquire_within(store, id, Duration::ZERO)
    }

    /// [`Owner::acquire`] for a process that is about to own `id`'s run:
    /// it waits out a holder for up to the store's own wait, so a reader
    /// probing the lock for a moment, or the last turn's owner letting go,
    /// no longer refuses the new turn's start (#10301).
    pub(super) fn acquire_waiting(store: &Store, id: &str) -> Result<Self, Error> {
        Self::acquire_within(store, id, store.wait)
    }

    fn acquire_within(store: &Store, id: &str, wait: Duration) -> Result<Self, Error> {
        let task = store.show(id)?;
        let path = store.dir.join(format!("owner-{id}.lock"));
        let exists = regular_or_absent(&path)?;
        if !exists && task.run.is_some() {
            return Err(Error::Corrupt("the execution owner lock is missing"));
        }
        let lock = private_open(&path, !exists, true)?;
        super::take_lock(&lock, wait)?;
        lock.sync_all()?;
        super::sync_directory(&store.dir)?;
        Ok(Self {
            dir: store.dir.clone(),
            task_id: id.into(),
            lock,
        })
    }

    fn verify(&self, dir: &Path) -> Result<(), Error> {
        if self.dir != dir {
            return Err(Error::UnsafePath);
        }
        verify_same_file(
            &dir.join(format!("owner-{}.lock", self.task_id)),
            &self.lock,
        )
    }

    /// Records `event`, waiting out a full disk ([`wait_out_full_disk`]):
    /// an owner that gave up there would leave its run to be ended as
    /// [`OWNER_ENDED`] while the host's cleanup frees space (#10237).
    pub(super) fn record(&self, event: Event) -> Result<Task, Error> {
        wait_out_full_disk(STORAGE_FULL_WAIT, STORAGE_FULL_STEP, || {
            Store::open_for_owner(&self.dir)?.record(self, event.clone(), 1)
        })
    }
}

/// How long an owner waits for disk space to record an event (#10237).
/// The host frees space when its disk is almost full, within a minute.
pub const STORAGE_FULL_WAIT: std::time::Duration = std::time::Duration::from_secs(600);
const STORAGE_FULL_STEP: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether `error` is a full disk or an exhausted quota: one the host's
/// disk cleanup can clear, so worth waiting out (#10237).
#[must_use]
pub fn storage_full(error: &Error) -> bool {
    matches!(error, Error::Io(error) if io_storage_full(error))
}

/// [`storage_full`] for an I/O error.
#[must_use]
pub fn io_storage_full(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded
    )
}

/// Runs `attempt` again every `step` while it fails for a full disk, for
/// up to `wait`; any other outcome, or the last one, is returned.
pub(super) fn wait_out_full_disk<T>(
    wait: std::time::Duration,
    step: std::time::Duration,
    mut attempt: impl FnMut() -> Result<T, Error>,
) -> Result<T, Error> {
    let deadline = std::time::Instant::now() + wait;
    loop {
        match attempt() {
            Err(error) if storage_full(&error) && std::time::Instant::now() < deadline => {
                std::thread::sleep(step);
            }
            outcome => return outcome,
        }
    }
}

/// Recover only after acquiring the abandoned owner's OS lock. Never dispatches work.
///
/// When nothing of the run is left, recovering ends it as
/// [`Store::settle`] does (#10124); while a process group it recorded is
/// still there, the run becomes `unknown` as before.
pub fn recover(directory: &Path, id: &str) -> Result<Task, Error> {
    let mut store = Store::open(directory)?;
    if let Some(task) = store.settle(id)? {
        return Ok(task);
    }
    let owner = Owner::acquire(&store, id)?;
    let task = store.show(id)?;
    if matches!(task.status, Status::Running | Status::CancelRequested) {
        let epoch = task.run.as_ref().ok_or(Error::InvalidTransition)?.epoch + 1;
        store.record(&owner, Event::OwnerLost, epoch)
    } else if task.checks == Checks::Running {
        let epoch = task.run.as_ref().ok_or(Error::InvalidTransition)?.epoch + 1;
        store.record(&owner, Event::CheckOwnerLost, epoch)
    } else {
        Ok(task)
    }
}

/// Run the checks frozen in the execution grant through the existing trusted
/// verification engine. Repeated or interrupted checks are never silently replayed.
pub async fn check(
    directory: &Path,
    id: &str,
    trust: &crate::capability::Trust,
) -> Result<Task, Error> {
    let (owner, task) = {
        let mut store = Store::open_for_owner(directory)?;
        let owner = Owner::acquire_waiting(&store, id)?;
        let task = store.record(&owner, Event::CheckIntent, 1)?;
        (owner, task)
    };
    let report = checks::execute(&task, trust).await?;
    owner.record(Event::Checked { report })
}

fn refused(message: impl std::fmt::Display) -> Error {
    Error::Io(std::io::Error::other(message.to_string()))
}

pub(super) async fn git(workspace: &Path, arguments: &[&str]) -> Result<String, Error> {
    let git = GIT_PATHS
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .unwrap_or(GIT_PATHS[0]);
    let mut command = std::process::Command::new(git);
    command
        .env_clear()
        .envs(base_environment())
        .env("PATH", SYSTEM_PATH)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(arguments)
        .current_dir(coder_boundary::plain_path(workspace));
    let ended = Job::from_command(command)
        .bounded(Limits::within(Duration::from_secs(10)).keeping(64 * 1024))
        .run()
        .await;
    if !ended.ending.success() || ended.truncated() {
        return Err(refused("source identity observation failed"));
    }
    Ok(ended.stdout.text.trim().into())
}

/// The one effect a turn's run records before dispatch:
/// `<task>:<turn>:command`.
pub(super) fn effect_id_for(task: &Task) -> String {
    format!("{}:{}:command", task.task_id, task.turn())
}

/// Execute a single explicitly granted bounded command. The calling process is
/// the owner, not a client connection. Use the detached CLI to outlive a client.
pub async fn execute(directory: &Path, bytes: &[u8]) -> Result<Task, Error> {
    let grant = Grant::parse(bytes)?;
    if grant.adapter_configuration.is_some() {
        return Err(Error::InvalidCommand(
            "use the explicitly configured adapter entry point",
        ));
    }
    let (owner, task) = {
        let store = Store::open_for_owner(directory)?;
        let owner = Owner::acquire_waiting(&store, &grant.task_id)?;
        let task = store.show(&grant.task_id)?;
        if task.run.is_some() || task.status != Status::Queued {
            return Err(Error::InvalidTransition);
        }
        if grant.intent_digest != task.intent_digest || grant.expected_revision != task.revision {
            return Err(Error::RevisionMismatch);
        }
        (owner, task)
    };
    let workspace = Path::new(&task.intent.workspace.path).canonicalize()?;
    if owner.dir.starts_with(&workspace) || workspace.starts_with(&owner.dir) {
        return Err(Error::UnsafePath);
    }
    let program = grant.program.canonicalize()?;
    if program != grant.program || !program.is_file() {
        return Err(Error::UnsafePath);
    }
    let source_revision = git(&workspace, &["rev-parse", "HEAD"]).await?;
    if task
        .intent
        .workspace
        .source_revision
        .as_ref()
        .is_some_and(|expected| *expected != source_revision)
    {
        return Err(Error::InvalidCommand(
            "the workspace source revision changed",
        ));
    }
    let git_directory = PathBuf::from(
        git(
            &workspace,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .await?,
    );
    let before = Snapshot::observe(&workspace);
    if !before.is_complete() {
        return Err(refused("the source snapshot is incomplete"));
    }
    if grant
        .expected_source_snapshot
        .as_ref()
        .is_some_and(|expected| *expected != before.digest())
    {
        return Err(refused("the granted source snapshot changed"));
    }
    let mut spec = if grant.write_workspace {
        Boundary::writing(&workspace)
    } else {
        Boundary::readonly()
    };
    spec = spec
        .readable(&workspace)
        .readable(&program)
        .sealed(&owner.dir)
        .sealed(&git_directory)
        .offline();
    let boundary = spec.build().map_err(refused)?;
    let context = checks::Context::capture(&task, &workspace, grant.requirements.as_ref())?;
    let admission = Admission {
        grant: grant.clone(),
        grant_digest: digest_bytes(bytes),
        grant_request: String::from_utf8(bytes.to_vec()).map_err(refused)?,
        workspace: workspace.clone(),
        source_revision,
        source_snapshot: before.digest(),
        program_digest: digest_bytes(&std::fs::read(&program)?),
        adapter: "bounded-command".into(),
        network: network_policy().into(),
        read_scope: "workspace_and_system".into(),
        authority: "local_os_user".into(),
        trace_file: task.trace_file(task.turn()),
        context,
    };
    owner.record(Event::Admitted {
        admission: Box::new(admission.clone()),
    })?;
    fault("after_admission")?;
    let trace_path = owner.dir.join(&admission.trace_file);
    let session = Session::opening(
        &format!("{}-{}", task.task_id, task.turn()),
        "none",
        "bounded-command",
        &workspace.display().to_string(),
        env!("CARGO_PKG_VERSION"),
    );
    let mut trace = Log::create_at(&trace_path, &session)?;
    trace.append(&Step::said(Source::User, task.effective_prompt()))?;
    for step in super::consumed_steers(&task, &BOUNDED_COMMAND_STEERING) {
        trace.append(&step)?;
    }
    trace.append(
        &Step::said(Source::System, "Execution admitted by the local operator.")
            .noting("admission", json!(admission)),
    )?;
    let effect_id = effect_id_for(&task);
    let state = Store::open_for_owner(&owner.dir)?.show(&task.task_id)?;
    // A cancellation accepted before dispatch permits no effect.
    let mut output_incomplete = false;
    let mut retained_stdout = 0usize;
    let stopped = if state.status == Status::CancelRequested {
        None
    } else {
        owner.record(Event::EffectIntent {
            effect_id: effect_id.clone(),
        })?;
        fault("after_intent")?;
        trace.append(
            &Step::said(Source::System, "Effect intent persisted before dispatch.")
                .noting("effect_id", json!(effect_id)),
        )?;
        if Snapshot::observe(&workspace).digest() != before.digest()
            || digest_bytes(&std::fs::read(&program)?) != admission.program_digest
        {
            return Err(refused(
                "source or executable changed after admission; recover the unresolved attempt",
            ));
        }
        let mut command = boundary
            .command(&program, &grant.arguments)
            .map_err(refused)?;
        command
            .current_dir(coder_boundary::plain_path(&workspace))
            .env_clear()
            .envs(base_environment())
            .env("PATH", SYSTEM_PATH);
        let live = {
            let mut dispatch = Store::open_for_owner(&owner.dir)?;
            if dispatch.show(&task.task_id)?.status == Status::CancelRequested {
                None
            } else {
                // Keep command admission serialized through spawn. A cancellation
                // acknowledged before this point cannot race a later dispatch.
                let live = Job::from_command(command)
                    .bounded(
                        Limits::within(Duration::from_secs(grant.wall_seconds))
                            .keeping(grant.stream_bytes)
                            .memory(Some(grant.memory_bytes)),
                    )
                    .start(Input::Null)
                    .map_err(refused)?;
                if let Some(process_id) = live.pid() {
                    dispatch.record(&owner, Event::Spawned { process_id }, 1)?;
                }
                Some(live)
            }
        };
        if let Some(live) = live {
            fault("after_dispatch")?;
            loop {
                let delivery = live.take();
                output_incomplete |= !delivery.gaps.is_empty();
                retained_stdout += delivery.bytes.len();
                record_delivery(&mut trace, &delivery)?;
                if retained_stdout >= 2 * 1024 * 1024 {
                    output_incomplete = true;
                    break Some(live.stop().await);
                }
                if live.finished() {
                    break Some(live.wait().await);
                }
                let state = Store::open_for_owner(&owner.dir)?.show(&task.task_id)?;
                if state.status == Status::CancelRequested {
                    break Some(live.stop().await);
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        } else {
            None
        }
    };
    let mut result = if let Some(stopped) = stopped {
        record_delivery(&mut trace, &stopped.rest)?;
        trace.append(
            &Step::said(Source::System, &stopped.stderr.marked())
                .noting("stream", json!("stderr"))
                .noting("bytes", json!(stopped.stderr.bytes))
                .noting("truncated", json!(stopped.stderr.truncated)),
        )?;
        ResultRecord {
            ending: stopped.ending.to_string(),
            exit_code: stopped.ending.code(),
            stop_requested: stopped.requested,
            group_clear: stopped.group_clear,
            elapsed_ms: stopped.elapsed.as_millis().try_into().unwrap_or(u64::MAX),
            trace_digest: String::new(),
            candidate_snapshot: None,
            artifact_file: None,
            artifact_digest: None,
            output_incomplete: output_incomplete
                || stopped.stderr.truncated
                || !stopped.rest.gaps.is_empty(),
            cost_status: "unknown".into(),
            cost_microusd: None,
            engine_microusd: None,
            jev_microusd: None,
            payer: None,
            payer_keys: Vec::new(),
        }
    } else {
        ResultRecord {
            ending: "cancelled_before_dispatch".into(),
            exit_code: None,
            stop_requested: true,
            group_clear: true,
            elapsed_ms: 0,
            trace_digest: String::new(),
            candidate_snapshot: None,
            artifact_file: None,
            artifact_digest: None,
            output_incomplete: false,
            cost_status: "unknown".into(),
            cost_microusd: None,
            engine_microusd: None,
            jev_microusd: None,
            payer: None,
            payer_keys: Vec::new(),
        }
    };
    result.paid_by(&model_access::current());
    let after = Snapshot::observe(&workspace);
    if after.is_complete() {
        result.candidate_snapshot = Some(after.digest());
    }
    let (artifact_file, artifact_digest) = artifact::retain(&owner.dir, &before, &after)?;
    result.artifact_file = Some(artifact_file);
    result.artifact_digest = Some(artifact_digest);
    trace.append(
        &Step::said(
            Source::System,
            "Executor stopped; independent checks have not run.",
        )
        .noting("result", json!(result)),
    )?;
    trace.finish(atif::log::ENDED)?;
    result.trace_digest = digest_bytes(&std::fs::read(&trace_path)?);
    fault("before_result")?;
    let task = owner.record(Event::Result { result })?;
    fault("after_result")?;
    Ok(task)
}

fn record_delivery(trace: &mut Log, delivery: &supervise::Delivery) -> Result<(), Error> {
    if !delivery.is_empty() {
        trace.append(
            &Step::said(Source::System, &String::from_utf8_lossy(&delivery.bytes))
                .noting("stream", json!("stdout"))
                .noting("offset", json!(delivery.offset))
                .noting("raw_bytes", json!(delivery.bytes))
                .noting(
                    "gaps",
                    json!(
                        delivery
                            .gaps
                            .iter()
                            .map(|gap| json!({"offset": gap.offset, "bytes": gap.bytes}))
                            .collect::<Vec<_>>()
                    ),
                ),
        )?;
    }
    Ok(())
}

fn fault(_point: &str) -> Result<(), Error> {
    #[cfg(test)]
    if OWNER_FAULT.with(|fault| fault.get() == Some(_point)) {
        OWNER_FAULT.with(|fault| fault.set(None));
        return Err(refused("injected owner failure"));
    }
    Ok(())
}

#[cfg(test)]
std::thread_local! { static OWNER_FAULT: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) }; }

// The owner tests run shell scripts as the task program.
#[cfg(all(test, unix))]
mod tests;
// Router phase 1's exit evidence (#10207): a routed message through this
// owner, with a retained patch and an independent check.
#[cfg(all(test, unix))]
mod route_tests;
// `chat send` following a task whose owner process died (#10248).
#[cfg(all(test, unix))]
mod follow_tests;
