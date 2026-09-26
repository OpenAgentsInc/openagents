//! Durable, single-host execution ownership for an explicitly admitted command.
//!
//! The inbox lock protects journal updates. A separate OS-held lock outlives
//! each executor. Losing that owner never grants permission to replay an effect.

use super::*;
use atif::{Log, Session, Source, Step};
use coder_boundary::{Boundary, Snapshot};
use serde_json::json;
use supervise::{Input, Job, Limits};

pub const GRANT_SCHEMA: &str = "openagents.coder.task-execution-grant.v1";
const MAX_HOST_EVENTS: usize = 8192;

/// The fixed, root-owned paths `git` is taken from, in order: where
/// distributions install it, then NixOS's system profile. Like the
/// boundary's `bwrap`, it is never searched for on `PATH`.
pub const GIT_PATHS: [&str; 2] = ["/usr/bin/git", "/run/current-system/sw/bin/git"];

/// The `PATH` owned commands run with: the system directories, then NixOS's
/// root-owned system profile, which exists only there.
pub const SYSTEM_PATH: &str = "/usr/bin:/bin:/run/current-system/sw/bin";

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
        let grant: Self = serde_json::from_value(value)
            .map_err(|_| Error::InvalidCommand("the execution grant has an invalid shape"))?;
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
            || !(1..=3600).contains(&self.wall_seconds)
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
    } else {
        "network_namespace_isolated"
    }
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
    pub cost_status: String,
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
    task_id: String,
    epoch: u64,
    event: Event,
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
                && (matches!(
                    task.status,
                    Status::Running | Status::CancelRequested | Status::Unknown
                ) || task.checks == Checks::Running)
                && task.run.as_ref().is_some_and(|run| {
                    run.admission.workspace.starts_with(workspace)
                        || workspace.starts_with(&run.admission.workspace)
                })
        })
    {
        return Err(Error::InvalidTransition);
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
                    (Some(config), super::adapter::NAME) => {
                        task.intent.configuration.model.as_deref() != Some(config.model.as_str())
                    }
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
                || admission.trace_file != format!("{}.1.atif.jsonl", task.task_id)
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
            let run = task.run.as_mut().ok_or(Error::InvalidTransition)?;
            if run.epoch != record.epoch
                || run.effect_id.is_some()
                || task.status != Status::Running
                || effect_id != &format!("{}:1:command", task.task_id)
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
            if run.epoch != record.epoch
                || run.result.is_some()
                || !matches!(task.status, Status::Running | Status::CancelRequested)
                || !hex_digest(&result.trace_digest)
                || result.cost_status != "unknown"
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
    fn record(&mut self, owner: &Owner, event: Event, epoch: u64) -> Result<Task, Error> {
        self.check_healthy()?;
        owner.verify(&self.dir)?;
        if self.document.host_events.len() >= MAX_HOST_EVENTS {
            return Err(Error::LimitExceeded);
        }
        let mut next = self.document.clone();
        next.schema = STORE_SCHEMA.into();
        next.sequence += 1;
        let record = Record {
            sequence: next.sequence,
            task_id: owner.task_id.clone(),
            epoch,
            event,
        };
        transition(&record, &mut next.tasks)?;
        next.host_events.push(record);
        if let Err(error) = self.commit(&next) {
            self.healthy = false;
            return Err(error);
        }
        self.document = next;
        self.show(&owner.task_id)
    }
}

/// An exclusive OS-held owner capability. It cannot be deserialized or forged by a command.
pub(super) struct Owner {
    pub(super) dir: PathBuf,
    task_id: String,
    lock: File,
}

impl Owner {
    pub(super) fn acquire(store: &Store, id: &str) -> Result<Self, Error> {
        let task = store.show(id)?;
        let path = store.dir.join(format!("owner-{id}.lock"));
        let exists = regular_or_absent(&path)?;
        if !exists && task.run.is_some() {
            return Err(Error::Corrupt("the execution owner lock is missing"));
        }
        let lock = private_open(&path, !exists, true)?;
        match lock.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
        lock.sync_all()?;
        File::open(&store.dir)?.sync_all()?;
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

    pub(super) fn record(&self, event: Event) -> Result<Task, Error> {
        Store::open(&self.dir)?.record(self, event, 1)
    }
}

/// Recover only after acquiring the abandoned owner's OS lock. Never dispatches work.
pub fn recover(directory: &Path, id: &str) -> Result<Task, Error> {
    let mut store = Store::open(directory)?;
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
        let mut store = Store::open(directory)?;
        let owner = Owner::acquire(&store, id)?;
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
        .env("PATH", SYSTEM_PATH)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(arguments)
        .current_dir(workspace);
    let ended = Job::from_command(command)
        .bounded(Limits::within(Duration::from_secs(10)).keeping(64 * 1024))
        .run()
        .await;
    if !ended.ending.success() || ended.truncated() {
        return Err(refused("source identity observation failed"));
    }
    Ok(ended.stdout.text.trim().into())
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
        let store = Store::open(directory)?;
        let owner = Owner::acquire(&store, &grant.task_id)?;
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
        trace_file: format!("{}.1.atif.jsonl", task.task_id),
        context,
    };
    owner.record(Event::Admitted {
        admission: Box::new(admission.clone()),
    })?;
    fault("after_admission")?;
    let trace_path = owner.dir.join(&admission.trace_file);
    let session = Session::opening(
        &format!("{}-1", task.task_id),
        "none",
        "bounded-command",
        &workspace.display().to_string(),
        env!("CARGO_PKG_VERSION"),
    );
    let mut trace = Log::create_at(&trace_path, &session)?;
    trace.append(&Step::said(Source::User, task.effective_prompt()))?;
    trace.append(
        &Step::said(Source::System, "Execution admitted by the local operator.")
            .noting("admission", json!(admission)),
    )?;
    let effect_id = format!("{}:1:command", task.task_id);
    let state = Store::open(&owner.dir)?.show(&task.task_id)?;
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
            .current_dir(&workspace)
            .env_clear()
            .env("PATH", SYSTEM_PATH);
        let live = {
            let mut dispatch = Store::open(&owner.dir)?;
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
                let state = Store::open(&owner.dir)?.show(&task.task_id)?;
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
        }
    };
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

#[cfg(test)]
mod tests;
