//! An admitted adapter uses the task owner's lease, evidence, and child boundary.
//!
//! The adapter supplies its algorithm. This host retains authority and effects;
//! it does not load a model or turn a model's completion into independent checks.

use super::*;
use std::cell::{Cell, RefCell};

use atif::{Log, Session, Source, Step};
use coder_boundary::{Boundary, Snapshot};
use serde_json::{Value, json};
use supervise::{Input, Job, Limits};

pub const NAME: &str = "microcoder-repository";
pub const CONFIG_SCHEMA: &str = "openagents.microcoder.repository-config.v1";
const TRACE_LIMIT: usize = 48 * 1024 * 1024;
const STEP_LIMIT: usize = 8 * 1024 * 1024;

/// The supported first repository profile is explicit about absent features.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub schema: String,
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
    pub generation_endpoint: String,
    pub decision_endpoint: String,
    pub decision_model: String,
    pub max_steps: usize,
    pub acceptance: bool,
    pub route: String,
    pub knowledge: String,
    pub dollar_limit_micros: Option<u64>,
    #[serde(default)]
    pub expected_controller_digest: Option<String>,
}

impl Configuration {
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != CONFIG_SCHEMA
            || !matches!(self.provider.as_str(), "codex" | "synthetic")
            || !identifier(&self.model, true)
            || !identifier(&self.decision_model, true)
            || self
                .effort
                .as_deref()
                .is_some_and(|effort| !matches!(effort, "low" | "medium" | "high" | "xhigh"))
            || !(1..=128).contains(&self.max_steps)
            || self.acceptance
            || self.route != "never"
            || self.knowledge != "off"
            || self.dollar_limit_micros.is_some()
            || self
                .expected_controller_digest
                .as_ref()
                .is_some_and(|digest| {
                    !digest.starts_with("sha256:")
                        || digest.len() != 71
                        || !digest[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
                })
        {
            return Err(Error::InvalidCommand(
                "unsupported repository adapter configuration",
            ));
        }
        for endpoint in [&self.generation_endpoint, &self.decision_endpoint] {
            if self.provider == "synthetic" {
                if endpoint != "in-process" {
                    return Err(Error::InvalidCommand(
                        "synthetic fixtures require in-process models",
                    ));
                }
            } else {
                let url = reqwest::Url::parse(endpoint)
                    .map_err(|_| Error::InvalidCommand("invalid model endpoint"))?;
                if url.scheme() != "https"
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.query().is_some()
                    || url.fragment().is_some()
                {
                    return Err(Error::InvalidCommand(
                        "model endpoints require HTTPS without credentials",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn capabilities(&self) -> Value {
        json!({
            "commands":"native-supervised", "reads":"native-confined",
            "cancellation":"emulated-host-stop", "atif":"native",
            "process_cleanup":"observed", "crash_resume":"unsupported",
            "model_written_acceptance":"unsupported", "routing":"unsupported",
            "knowledge":"unsupported", "hard_dollar_limit":"unsupported",
            "billing":"unknown", "effort_confirmation":"not_reported",
            "provider_artifact_attestation":"unsupported", "container_adapter":"unsupported"
        })
    }
}

/// Full bounded process observation; prompt summaries are a caller's projection.
#[derive(Debug)]
pub struct CommandObservation {
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub seconds: f64,
    pub output: String,
    pub group_clear: bool,
}

/// A single task owner. It is deliberately neither serializable nor cloneable.
pub struct Host {
    owner: owner::Owner,
    task: Task,
    admission: owner::Admission,
    before: Snapshot,
    boundary: Boundary,
    trace: RefCell<Log>,
    trace_bytes: Cell<usize>,
    started: Instant,
    sequence: Cell<usize>,
    stopped: Cell<bool>,
    group_clear: Cell<bool>,
    output_incomplete: Cell<bool>,
    fault: RefCell<Option<String>>,
}

impl Host {
    /// Admit exact operator bytes before any model call or workspace command.
    pub async fn admit(directory: &Path, bytes: &[u8]) -> Result<Self, Error> {
        let grant = owner::Grant::parse(bytes)?;
        let configuration = grant
            .adapter_configuration
            .as_ref()
            .ok_or(Error::InvalidCommand(
                "repository admission requires an explicit adapter configuration",
            ))?;
        configuration.validate()?;
        if !grant.arguments.is_empty() {
            return Err(Error::InvalidCommand(
                "repository admission supplies no fixed command arguments",
            ));
        }
        let (owner, task) = {
            let store = Store::open(directory)?;
            let owner = owner::Owner::acquire(&store, &grant.task_id)?;
            let task = store.show(&grant.task_id)?;
            if task.run.is_some() || task.status != Status::Queued {
                return Err(Error::InvalidTransition);
            }
            if grant.intent_digest != task.intent_digest || grant.expected_revision != task.revision
            {
                return Err(Error::RevisionMismatch);
            }
            if task.intent.configuration.adapter != NAME
                || task.intent.configuration.model.as_deref() != Some(&configuration.model)
            {
                return Err(Error::InvalidCommand(
                    "requested and granted adapter or model differ",
                ));
            }
            (owner, task)
        };
        let workspace = Path::new(&task.intent.workspace.path).canonicalize()?;
        if owner.dir.starts_with(&workspace) || workspace.starts_with(&owner.dir) {
            return Err(Error::UnsafePath);
        }
        let program = grant.program.canonicalize()?;
        if program != grant.program
            || ![Path::new("/bin/bash"), Path::new("/bin/sh")]
                .iter()
                .filter_map(|path| path.canonicalize().ok())
                .any(|path| path == program)
        {
            return Err(Error::InvalidCommand(
                "the granted program must be the canonical system shell",
            ));
        }
        let source_revision = owner::git(&workspace, &["rev-parse", "HEAD"]).await?;
        if task
            .intent
            .workspace
            .source_revision
            .as_ref()
            .is_some_and(|pin| pin != &source_revision)
        {
            return Err(Error::InvalidCommand(
                "the workspace source revision changed",
            ));
        }
        let git_directory = PathBuf::from(
            owner::git(
                &workspace,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )
            .await?,
        );
        let before = Snapshot::observe(&workspace);
        if !before.is_complete()
            || grant
                .expected_source_snapshot
                .as_ref()
                .is_some_and(|pin| pin != &before.digest())
        {
            return Err(Error::InvalidCommand(
                "the granted source snapshot is unavailable or changed",
            ));
        }
        let spec = if grant.write_workspace {
            Boundary::writing(&workspace)
        } else {
            Boundary::readonly()
        };
        let boundary = spec
            .readable(&workspace)
            .readable(&program)
            .sealed(&owner.dir)
            .sealed(&git_directory)
            .owned_scratch_under(std::env::temp_dir())
            .offline()
            .build()
            .map_err(|_| Error::InvalidCommand("the repository boundary cannot be enforced"))?;
        let context = checks::Context::capture(&task, &workspace, grant.requirements.as_ref())?;
        let controller = std::env::current_exe()?.canonicalize()?;
        let controller_digest = digest_bytes(&std::fs::read(&controller)?);
        if configuration
            .expected_controller_digest
            .as_ref()
            .is_some_and(|expected| expected != &controller_digest)
        {
            return Err(Error::InvalidCommand(
                "the repository controller differs from its grant",
            ));
        }
        let admission = owner::Admission {
            grant: grant.clone(),
            grant_digest: digest_bytes(bytes),
            grant_request: String::from_utf8(bytes.to_vec())
                .map_err(|_| Error::UnsupportedSchema)?,
            workspace: workspace.clone(),
            source_revision,
            source_snapshot: before.digest(),
            program_digest: digest_bytes(&std::fs::read(&program)?),
            adapter: NAME.into(),
            network: owner::network_policy().into(),
            read_scope: "workspace_and_system".into(),
            authority: "local_os_user".into(),
            trace_file: format!("{}.1.atif.jsonl", task.task_id),
            context,
        };
        owner.record(owner::Event::Admitted {
            admission: Box::new(admission.clone()),
        })?;
        let mut trace = Log::create_at(
            &owner.dir.join(&admission.trace_file),
            &Session::opening(
                &format!("{}-1", task.task_id),
                &configuration.model,
                NAME,
                &workspace.display().to_string(),
                env!("CARGO_PKG_VERSION"),
            ),
        )?;
        trace.append(&Step::said(Source::User, task.effective_prompt()))?;
        trace.append(
            &Step::said(
                Source::System,
                "Repository adapter admitted by the local operator.",
            )
            .noting("admission", json!(admission))
            .noting("controller",json!({"path":controller,"digest":controller_digest,"version":env!("CARGO_PKG_VERSION")}))
            .noting("capabilities", configuration.capabilities()),
        )?;
        if Snapshot::observe(&workspace).digest() != before.digest() {
            return Err(Error::InvalidCommand(
                "source changed after repository admission",
            ));
        }
        // The epoch effect is durable in the common journal. Individual effects
        // retain their exact intents and results in this same fsynced ATIF log.
        if Store::open(&owner.dir)?.show(&task.task_id)?.status != Status::CancelRequested {
            owner.record(owner::Event::EffectIntent {
                effect_id: format!("{}:1:command", task.task_id),
            })?;
        }
        let trace_bytes = std::fs::metadata(trace.path())?.len() as usize;
        Ok(Self {
            owner,
            task,
            admission,
            before,
            boundary,
            trace: RefCell::new(trace),
            trace_bytes: Cell::new(trace_bytes),
            started: Instant::now(),
            sequence: Cell::new(0),
            stopped: Cell::new(false),
            group_clear: Cell::new(true),
            output_incomplete: Cell::new(false),
            fault: RefCell::new(None),
        })
    }

    pub fn configuration(&self) -> &Configuration {
        self.admission
            .grant
            .adapter_configuration
            .as_ref()
            .expect("configuration admitted")
    }

    pub fn prompt(&self) -> &str {
        &self.admission.context.prompt
    }
    pub fn context(&self) -> &checks::Context {
        &self.admission.context
    }
    pub fn workspace(&self) -> &Path {
        &self.admission.workspace
    }
    pub fn wall_seconds(&self) -> u64 {
        self.admission.grant.wall_seconds
    }

    pub fn fail(&self, reason: impl Into<String>) {
        if self.fault.borrow().is_none() {
            *self.fault.borrow_mut() = Some(reason.into());
        }
    }

    /// Cancellation prevents future admissions; already sent model requests can
    /// still incur unknown charges after the client stops awaiting their replies.
    pub fn cancelled(&self) -> bool {
        if self.stopped.get() || self.fault.borrow().is_some() {
            return true;
        }
        if self.started.elapsed() >= Duration::from_secs(self.wall_seconds()) {
            self.stopped.set(true);
            return true;
        }
        match Store::open(&self.owner.dir).and_then(|store| store.show(&self.task.task_id)) {
            Ok(task) if task.status == Status::Running => false,
            Ok(_) => {
                self.stopped.set(true);
                true
            }
            Err(error) => {
                self.fail(error.to_string());
                true
            }
        }
    }

    pub async fn wait_cancelled(&self) {
        while !self.cancelled() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    pub fn append(&self, step: &Step) -> Result<(), Error> {
        let bytes = serde_json::to_vec(step)
            .map_err(|_| Error::UnsupportedSchema)?
            .len()
            + 256;
        if bytes > STEP_LIMIT || self.trace_bytes.get().saturating_add(bytes) > TRACE_LIMIT {
            self.output_incomplete.set(true);
            self.fail("the adapter evidence limit was reached; the attempted record was omitted");
            return Err(Error::LimitExceeded);
        }
        if let Err(error) = self.trace.borrow_mut().append(step) {
            self.fail("task evidence could not be retained");
            return Err(error.into());
        }
        self.trace_bytes.set(self.trace_bytes.get() + bytes);
        Ok(())
    }

    pub fn effect(&self, kind: &str, arguments: Value) -> Result<usize, Error> {
        if self.cancelled() {
            return Err(Error::InvalidTransition);
        }
        if self.sequence.get() >= self.configuration().max_steps * 32 {
            self.fail("repository effect bound reached");
            return Err(Error::LimitExceeded);
        }
        let sequence = self.sequence.get() + 1;
        self.append(
            &Step::said(
                Source::System,
                "Adapter effect intent retained before dispatch.",
            )
            .noting(
                "effect",
                json!({"sequence":sequence,"kind":kind,"arguments":arguments,
                "arguments_digest":atif::digest(&arguments)}),
            ),
        )?;
        self.sequence.set(sequence);
        Ok(sequence)
    }

    pub fn result(&self, sequence: usize, kind: &str, result: Value) -> Result<(), Error> {
        self.append(
            &Step::said(Source::System, "Adapter effect observation retained.").noting(
                "effect_result",
                json!({"sequence":sequence,"kind":kind,"result":result}),
            ),
        )
    }

    /// Reads only singly linked regular files below the admitted workspace.
    pub fn read(&self, requested: &str, cap: usize) -> Result<Option<Vec<u8>>, Error> {
        if cap == 0 || cap > 1024 * 1024 {
            return Err(Error::LimitExceeded);
        }
        let requested = Path::new(requested);
        let path = if requested.is_absolute() {
            requested
                .strip_prefix(self.workspace())
                .map_err(|_| Error::UnsafePath)?
        } else {
            requested
        };
        let sequence = self.effect("read", json!({"path":path,"max_bytes":cap}))?;
        let mut bytes = Vec::new();
        let result = match artifact::confined_file(self.workspace(), path) {
            Ok(file) => {
                if let Err(error) = file.take(cap as u64 + 1).read_to_end(&mut bytes) {
                    self.output_incomplete.set(true);
                    self.fail("an admitted file read failed before a complete observation");
                    self.result(
                        sequence,
                        "read",
                        json!({
                            "status":"read_error", "error":error.to_string(),
                            "partial_bytes":bytes, "partial_digest":digest_bytes(&bytes),
                            "observation_complete":false
                        }),
                    )?;
                    return Err(error.into());
                }
                Some(bytes)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                self.result(sequence, "read", json!({"refused":error.to_string()}))?;
                return Err(Error::UnsafePath);
            }
        };
        let result = result.map(|mut bytes| {
            let truncated = bytes.len() > cap;
            bytes.truncate(cap);
            (bytes, truncated)
        });
        self.result(
            sequence,
            "read",
            json!({"observation":result.as_ref().map(|(bytes,truncated)|
            json!({"bytes":bytes,"digest":digest_bytes(bytes),"truncated":truncated}))}),
        )?;
        Ok(result.map(|(bytes, _)| bytes))
    }

    /// Commands retain bounded full streams separately from the loop's prompt cut.
    pub async fn command(
        &self,
        script: &str,
        deadline: Duration,
    ) -> Result<CommandObservation, Error> {
        if script.len() > MAX_COMMAND_BYTES || script.contains('\0') {
            return Err(Error::LimitExceeded);
        }
        if digest_bytes(&std::fs::read(&self.admission.grant.program)?)
            != self.admission.program_digest
        {
            self.fail("the admitted shell changed");
            return Err(Error::InvalidTransition);
        }
        let sequence = self.effect("command", json!({"script":script}))?;
        let mut command = self
            .boundary
            .command(&self.admission.grant.program, ["-c", script])
            .map_err(|_| Error::UnsafePath)?;
        let scratch = self.boundary.scratch().ok_or(Error::UnsafePath)?;
        command
            .current_dir(self.workspace())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", scratch)
            .env("TMPDIR", scratch)
            .env("TMP", scratch)
            .env("TEMP", scratch);
        let live = {
            let store = Store::open(&self.owner.dir)?;
            if store.show(&self.task.task_id)?.status != Status::Running {
                self.stopped.set(true);
                return Err(Error::InvalidTransition);
            }
            self.group_clear.set(false);
            Job::from_command(command)
                .bounded(
                    Limits::within(
                        deadline.min(
                            Duration::from_secs(self.wall_seconds())
                                .saturating_sub(self.started.elapsed()),
                        ),
                    )
                    .keeping(self.admission.grant.stream_bytes)
                    .memory(Some(self.admission.grant.memory_bytes)),
                )
                .start(Input::Null)
                .map_err(|error| {
                    self.fail(error);
                    Error::InvalidTransition
                })?
        };
        if self
            .append(
                &Step::said(Source::System, "Supervised command started.")
                    .noting("process", json!({"effect":sequence,"pid":live.pid()})),
            )
            .is_err()
        {
            let ended = live.stop().await;
            self.group_clear.set(ended.group_clear);
            return Err(Error::LimitExceeded);
        }
        let mut stdout = Vec::new();
        let ended = loop {
            let delivery = live.take();
            if !delivery.gaps.is_empty() {
                self.output_incomplete.set(true);
            }
            stdout.extend_from_slice(&delivery.bytes);
            if !delivery.is_empty()
                && self.append(&Step::said(Source::System,&String::from_utf8_lossy(&delivery.bytes))
                    .noting("stream",json!({"effect":sequence,"name":"stdout","offset":delivery.offset,
                        "bytes":delivery.bytes,"gaps":delivery.gaps.iter().map(|gap|json!({"offset":gap.offset,"bytes":gap.bytes})).collect::<Vec<_>>()}))).is_err() {
                break live.stop().await;
            }
            if stdout.len() >= 2 * 1024 * 1024 {
                self.output_incomplete.set(true);
                break live.stop().await;
            }
            if live.finished() {
                break live.wait().await;
            }
            if self.cancelled() {
                break live.stop().await;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        stdout.extend_from_slice(&ended.rest.bytes);
        self.group_clear.set(ended.group_clear);
        self.output_incomplete.set(
            self.output_incomplete.get() || ended.stderr.truncated || !ended.rest.gaps.is_empty(),
        );
        self.result(sequence,"command",json!({"ending":ended.ending.to_string(),"exit":ended.ending.code(),
            "group_clear":ended.group_clear,"requested_stop":ended.requested,"stdout_tail":ended.rest.bytes,
            "stdout_tail_offset":ended.rest.offset,"stdout_bytes":ended.stdout_bytes,"stderr":ended.stderr.text,
            "stderr_bytes":ended.stderr.bytes,"stderr_truncated":ended.stderr.truncated,
            "seconds":ended.elapsed.as_secs_f64(),"memory":format!("{:?}",ended.memory)}))?;
        if !ended.group_clear {
            self.fail("supervised process cleanup is unknown");
        }
        let mut output = String::from_utf8_lossy(&stdout).into_owned();
        if !ended.stderr.text.is_empty() {
            output.push('\n');
            output.push_str(&ended.stderr.marked());
        }
        Ok(CommandObservation {
            exit: ended.ending.code(),
            timed_out: matches!(ended.ending, supervise::Ending::TimedOut),
            seconds: ended.elapsed.as_secs_f64(),
            output,
            group_clear: ended.group_clear,
        })
    }

    /// Seal the same task journal and trace; the adapter never sets checks passed.
    pub fn finish(self, ending: &str, completed: bool, summary: Value) -> Result<Task, Error> {
        let stopped = self.cancelled();
        let summary = if serde_json::to_vec(&summary)
            .map_err(|_| Error::UnsupportedSchema)?
            .len()
            > 1024 * 1024
        {
            self.output_incomplete.set(true);
            self.fail("the final adapter summary exceeded its retained size bound");
            json!({"omitted":true,"digest":atif::digest(&summary)})
        } else {
            summary
        };
        // Reserved headroom preserves the final disposition even when a prior
        // record exceeded the ordinary trace cap. The file remains below 64 MiB.
        self.trace.borrow_mut().append(
            &Step::said(
                Source::System,
                "Repository adapter ended; independent checks are separate.",
            )
            .noting("adapter_summary", summary)
            .noting("host_fault", json!(*self.fault.borrow())),
        )?;
        let after = Snapshot::observe(self.workspace());
        let (artifact_file, artifact_digest) =
            artifact::retain(&self.owner.dir, &self.before, &after)?;
        self.trace.borrow_mut().finish(atif::log::ENDED)?;
        let result = owner::ResultRecord {
            ending: ending.into(),
            exit_code: Some(if completed && self.fault.borrow().is_none() {
                0
            } else {
                1
            }),
            stop_requested: stopped,
            group_clear: self.group_clear.get(),
            elapsed_ms: self
                .started
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            trace_digest: digest_bytes(&std::fs::read(self.trace.borrow().path())?),
            candidate_snapshot: after.is_complete().then(|| after.digest()),
            artifact_file: Some(artifact_file),
            artifact_digest: Some(artifact_digest),
            output_incomplete: self.output_incomplete.get(),
            cost_status: "unknown".into(),
        };
        self.owner.record(owner::Event::Result { result })
    }
}
