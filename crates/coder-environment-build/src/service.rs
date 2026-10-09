//! The build owner: admission, the fresh builder computer, install,
//! sanitization, quiesce, capture, reconciliation, usage, and cleanup.
//!
//! [`Builder::advance`] holds the job's lease for a whole visit. Each step
//! reads the retained job, performs at most one provider effect, and
//! retains what it observed before the next step. Intent (a command spec,
//! a capture name and issue count) is retained before the effect, so a
//! crash at any point resumes by reading the provider, not by repeating
//! the effect.

use crate::sanitize::{self, MAX_REPORT_BYTES, Plan, Report};
use crate::store::{Lease, Store as JobStore, StoreError};
use crate::*;
use coder_environment::evidence::{
    CallIdentity, CallResult, EvidenceError, Recorder, Redactor, StreamName,
};
use coder_environment::store::{Store as EnvStore, StoreError as EnvStoreError};
use coder_environment::transition::BuildObservation;
use coder_environment::{
    BuildState, Command as EnvCommand, Effect, ImageIdentity, RunLink, Stage, digest,
};
use coder_environment_setup::{GIT_CREDENTIALS, embeds_url_credential, git_auth_env};
use coder_working_computer::boat::COMMAND_DIR;
use coder_working_computer::driver::{Driver, Settled};
use coder_working_computer::provider::{
    CommandProgress, CommandSpec, Commands, ImageRecord, ImageState, Images, Outcome,
};
use coder_working_computer::{
    Bounds, Computer, Fact, Phase as ComputerPhase, Principal, Spec, credential_name_allowed,
};
use serde_json::json;
use std::{collections::BTreeMap, fmt, fs, path::PathBuf, sync::Mutex};

/// Bytes of each stream one provider read takes.
pub const MAX_READ_BYTES: u64 = 128 * 1024;
const MAX_STEPS: usize = 64;

#[derive(Debug)]
pub enum BuildError {
    Refused(&'static str),
    /// The request ID was used for a different build.
    RequestConflict(String),
    Environment(EnvStoreError),
    Job(StoreError),
    Computer(coder_working_computer::store::StoreError),
    Evidence(EvidenceError),
    Custody(String),
}
impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(m) => f.write_str(m),
            Self::RequestConflict(id) => {
                write!(f, "Request {id} was already used for a different build.")
            }
            Self::Environment(e) => e.fmt(f),
            Self::Job(e) => e.fmt(f),
            Self::Computer(e) => e.fmt(f),
            Self::Evidence(e) => e.fmt(f),
            Self::Custody(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for BuildError {}
impl From<EnvStoreError> for BuildError {
    fn from(e: EnvStoreError) -> Self {
        Self::Environment(e)
    }
}
impl From<StoreError> for BuildError {
    fn from(e: StoreError) -> Self {
        Self::Job(e)
    }
}
impl From<coder_working_computer::store::StoreError> for BuildError {
    fn from(e: coder_working_computer::store::StoreError) -> Self {
        Self::Computer(e)
    }
}
impl From<EvidenceError> for BuildError {
    fn from(e: EvidenceError) -> Self {
        Self::Evidence(e)
    }
}
pub type Result<T> = std::result::Result<T, BuildError>;

/// Builds the redactor for the recipe's named credentials from operator
/// custody. Values never enter a job record.
pub type Custody = Box<
    dyn Fn(&std::collections::BTreeSet<String>) -> std::result::Result<Redactor, String>
        + Send
        + Sync,
>;

/// What a caller asks for when it starts a clean build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRequest {
    pub request_id: String,
    pub environment: String,
    pub owner: Principal,
    /// The draft revision the caller saw; a later edit refuses the build.
    pub expected_draft_revision: u64,
    pub size: String,
}

/// A job with the facts a client needs beside it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BuildView {
    pub job: BuildJob,
    pub build_state: BuildState,
    /// A later recipe edit made this build stale.
    pub stale: bool,
}

pub struct Builder<P> {
    pub jobs: JobStore,
    pub environments: EnvStore,
    pub computers: Driver<P>,
    root: PathBuf,
    blobs: PathBuf,
    custody: Custody,
    live: Mutex<BTreeMap<String, Recorder>>,
}

fn reason(text: impl Into<String>) -> String {
    let mut text: String = text.into();
    if text.is_empty() {
        text.push_str("unspecified");
    }
    if text.len() > MAX_REASON_BYTES {
        let mut end = MAX_REASON_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}
fn fail(job: &mut BuildJob, why: impl Into<String>) {
    job.phase = Phase::Failed;
    job.reason = Some(reason(why));
    job.unresolved = None;
}
fn run_link(job: &BuildJob) -> RunLink {
    RunLink {
        cloud_job: job.computer.clone(),
        task: Some(job.id.clone()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind2 {
    Source,
    Install,
    Sanitize,
}
impl Kind2 {
    fn id(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Install => "install",
            Self::Sanitize => "sanitize",
        }
    }
    fn tool(self) -> &'static str {
        match self {
            Self::Source => "environment.build.source",
            Self::Install => "environment.build.install",
            Self::Sanitize => "environment.build.sanitize",
        }
    }
    fn get(self, job: &BuildJob) -> Option<&Step> {
        match self {
            Self::Source => job.source.as_ref(),
            Self::Install => job.install.as_ref(),
            Self::Sanitize => job.sanitize.as_ref(),
        }
    }
    fn get_mut(self, job: &mut BuildJob) -> &mut Option<Step> {
        match self {
            Self::Source => &mut job.source,
            Self::Install => &mut job.install,
            Self::Sanitize => &mut job.sanitize,
        }
    }
}

impl<P: Commands + Images> Builder<P> {
    /// `root` holds evidence (`evidence/<job>/<segment>`); `blobs` is the
    /// setup owner's script blob directory (`<digest>` files).
    pub fn new(
        root: impl Into<PathBuf>,
        blobs: impl Into<PathBuf>,
        jobs: JobStore,
        environments: EnvStore,
        computers: Driver<P>,
        custody: Custody,
    ) -> Self {
        Self {
            jobs,
            environments,
            computers,
            root: root.into(),
            blobs: blobs.into(),
            custody,
            live: Mutex::new(BTreeMap::new()),
        }
    }

    /// Forget live recorders, as an owner restart would.
    #[cfg(test)]
    pub(crate) fn restart(&self) {
        self.live.lock().expect("recorders").clear();
    }

    pub fn evidence_dir(&self, job: &str, segment: &str) -> PathBuf {
        self.root.join("evidence").join(job).join(segment)
    }

    fn read_script(&self, d: &str) -> Result<String> {
        if !coder_environment::valid_digest(d) {
            return Err(BuildError::Refused("Invalid install script digest."));
        }
        let bytes = fs::read(self.blobs.join(d))
            .map_err(|_| BuildError::Refused("The install script blob is missing."))?;
        if digest(&bytes) != d {
            return Err(BuildError::Refused(
                "The install script blob does not match its digest.",
            ));
        }
        String::from_utf8(bytes)
            .map_err(|_| BuildError::Refused("The install script is not UTF-8."))
    }

    /// Start (or continue) a clean build of the environment's current
    /// draft. Repeating the same request returns the same job.
    pub async fn start(&self, request: &BuildRequest, now_ms: u64) -> Result<BuildJob> {
        if !valid_id(&request.request_id)
            || !valid_id(&request.environment)
            || !valid_id(&request.size)
        {
            return Err(BuildError::Refused(
                "The request, environment, and size need opaque identities.",
            ));
        }
        let fp = digest(&serde_json::to_vec(request).expect("request encodes"));
        let id = format!(
            "bj-{}",
            &digest(format!("{}\0{}", request.environment, request.request_id).as_bytes())[..24]
        );
        let lease = self.jobs.lease(&id)?;
        if lease.exists() {
            if lease.read()?.fingerprint != fp {
                return Err(BuildError::RequestConflict(request.request_id.clone()));
            }
            return self.advance_locked(&lease, now_ms).await;
        }
        let env = self.environments.read(&request.environment)?;
        if request.owner.workspace != env.project.workspace {
            return Err(BuildError::Refused(
                "The owner is not in the environment's workspace.",
            ));
        }
        // Unknown cleanup of an earlier builder blocks new allocations.
        if self
            .jobs
            .list()?
            .iter()
            .any(|j| j.inputs.environment == env.id && j.id != id && j.cleanup.uncertain())
        {
            return Err(BuildError::Refused(
                "An earlier builder's cleanup is unknown; reconcile it first.",
            ));
        }
        let draft = env.draft();
        if draft.revision != request.expected_draft_revision {
            return Err(BuildError::Environment(EnvStoreError::Refused(
                coder_environment::Refusal::StaleDraft {
                    expected: request.expected_draft_revision,
                    current: draft.revision,
                },
            )));
        }
        let recipe = draft.recipe.clone();
        if !recipe
            .credential_names
            .iter()
            .all(|n| credential_name_allowed(n))
        {
            return Err(BuildError::Refused(
                "The recipe names a credential a builder may never carry.",
            ));
        }
        // The builder boots exactly the recipe's pinned base on this provider.
        self.computers
            .provider
            .admits_base(&recipe.base)
            .map_err(BuildError::Refused)?;
        let script = self.read_script(&recipe.install.digest)?;
        if embeds_url_credential(&script) {
            return Err(BuildError::Refused(
                "The install script embeds a credential in a URL.",
            ));
        }
        let build_id = match self.environments.apply(
            &env.id,
            &EnvCommand::StartBuild {
                request_id: request.request_id.clone(),
                expected_draft_revision: request.expected_draft_revision,
            },
            now_ms,
        )? {
            coder_environment::Applied::Changed(_, Effect::BuildRequested { build_id, .. })
            | coder_environment::Applied::Replayed(Effect::BuildRequested { build_id, .. }) => {
                build_id
            }
            _ => return Err(BuildError::Refused("Unexpected environment effect.")),
        };
        let env = self.environments.read(&env.id)?;
        let build = env.build(&build_id).expect("build exists");
        let computer_id = format!("builder-{id}");
        let window = recipe.limits.deadline_seconds.min(24 * 3600) * 1000;
        let git_credential = GIT_CREDENTIALS
            .iter()
            .find(|g| recipe.credential_names.contains(**g))
            .map(|g| g.to_string());
        let inputs = Inputs {
            environment: env.id.clone(),
            build_id: build_id.clone(),
            recipe_revision: build.recipe_revision,
            recipe_digest: build.recipe_digest.clone(),
            source: build.source.clone(),
            base: recipe.base.clone(),
            runtime: recipe.runtime.clone(),
            platform: recipe.platform.clone(),
            install_digest: recipe.install.digest.clone(),
            install_cwd: recipe.install.cwd.clone(),
            credential_names: recipe.credential_names.clone(),
            git_credential,
            plan: Plan::new(&recipe.capture, &format!("{COMMAND_DIR}/sanitize")),
            image_name: image_name(&env.id, &build_id, &build.recipe_digest),
            evidence_budget: recipe.limits.output_bytes,
        };
        let spec = Spec {
            id: computer_id.clone(),
            owner: request.owner.clone(),
            chat: id.clone(),
            project: env.project.clone(),
            source: env.source.clone(),
            base: None,
            size: request.size.clone(),
            credential_names: recipe.credential_names.clone(),
            services: vec![],
            bounds: Bounds {
                idle_ms: window,
                observed_extension_ms: 0,
                absolute_ms: window,
            },
        };
        let mut computer =
            Computer::for_build(spec, &env.id, &build_id, now_ms).map_err(BuildError::Refused)?;
        computer.provider = self.computers.provider.kind();
        match self.computers.store.read(&computer_id) {
            Ok(existing) if existing.purpose == computer.purpose => {}
            Ok(_) => {
                return Err(BuildError::Refused(
                    "That computer is not this build's dedicated builder.",
                ));
            }
            Err(coder_working_computer::store::StoreError::NotFound) => {
                self.computers.store.create(&computer)?
            }
            Err(e) => return Err(e.into()),
        }
        lease.create(&BuildJob {
            schema: SCHEMA.into(),
            id: id.clone(),
            revision: 1,
            created_ms: now_ms,
            updated_ms: now_ms,
            request_id: request.request_id.clone(),
            fingerprint: fp,
            inputs,
            computer: computer_id,
            deadline_ms: now_ms + window,
            phase: Phase::Provisioning,
            unresolved: None,
            generation: None,
            source: None,
            checkout: None,
            install: None,
            sanitize: None,
            report: None,
            capture: None,
            image: None,
            reason: None,
            usage: Usage::default(),
            cleanup: Cleanup::NotStarted,
            segments: vec![],
            history: vec![Transition {
                phase: Phase::Provisioning,
                at_ms: now_ms,
                reason: None,
            }],
        })?;
        self.advance_locked(&lease, now_ms).await
    }

    /// A visit: move the build as far as it can go now, reconciling any
    /// unknown outcome first.
    pub async fn advance(&self, id: &str, now_ms: u64) -> Result<BuildJob> {
        let lease = self.jobs.lease(id)?;
        self.advance_locked(&lease, now_ms).await
    }

    /// The job, its projected build state, and whether it is stale.
    pub fn view(&self, id: &str) -> Result<BuildView> {
        let job = self.jobs.read(id)?;
        let env = self.environments.read(&job.inputs.environment)?;
        let build = env
            .build(&job.inputs.build_id)
            .ok_or(BuildError::Refused("The build attempt is missing."))?;
        Ok(BuildView {
            build_state: build.state,
            stale: env.is_stale(build),
            job,
        })
    }

    /// Cancel a build; its builder is still cleaned up.
    pub async fn cancel(&self, id: &str, why: &str, now_ms: u64) -> Result<BuildJob> {
        let lease = self.jobs.lease(id)?;
        let job = lease.read()?;
        if !job.phase.terminal() {
            self.stop_active(&job).await;
            lease.update(now_ms, |j| {
                j.phase = Phase::Cancelled;
                j.reason = Some(reason(why));
                j.unresolved = None;
            })?;
        }
        self.advance_locked(&lease, now_ms).await
    }

    /// Retry the builder's cleanup (and its usage check) after the build
    /// ended.
    pub async fn cleanup(&self, id: &str, now_ms: u64) -> Result<BuildJob> {
        let lease = self.jobs.lease(id)?;
        if !lease.read()?.phase.terminal() {
            return Err(BuildError::Refused(
                "Finish or cancel the build before cleanup.",
            ));
        }
        self.advance_locked(&lease, now_ms).await
    }

    async fn advance_locked(&self, lease: &Lease, now_ms: u64) -> Result<BuildJob> {
        for _ in 0..MAX_STEPS {
            let job = lease.read()?;
            self.sync_environment(&job, now_ms)?;
            if job.phase.terminal() {
                return self.finish(lease, now_ms).await;
            }
            self.ensure_segment(lease, now_ms)?;
            let job = lease.read()?;
            if now_ms >= job.deadline_ms {
                self.stop_active(&job).await;
                lease.update(now_ms, |j| fail(j, "The build deadline passed."))?;
                continue;
            }
            let moved = match job.phase {
                Phase::Provisioning => self.provision(lease, &job, now_ms).await?,
                Phase::Materializing => self.step(lease, &job, Kind2::Source, now_ms).await?,
                Phase::Installing => self.step(lease, &job, Kind2::Install, now_ms).await?,
                Phase::Sanitizing => self.step(lease, &job, Kind2::Sanitize, now_ms).await?,
                Phase::Quiescing => self.quiesce(lease, &job, now_ms).await?,
                Phase::Capturing => self.capture(lease, &job, now_ms).await?,
                Phase::Ready | Phase::Failed | Phase::Cancelled => true,
            };
            if !moved {
                let job = lease.read()?;
                self.sync_environment(&job, now_ms)?;
                return Ok(job);
            }
        }
        Ok(lease.read()?)
    }

    /// Project the job onto its `BuildAttempt`. Every observation here is
    /// idempotent, so a crash between the two records repeats it safely.
    fn sync_environment(&self, job: &BuildJob, now_ms: u64) -> Result<()> {
        let env = self.environments.read(&job.inputs.environment)?;
        let build = env
            .build(&job.inputs.build_id)
            .ok_or(BuildError::Refused("The build attempt is missing."))?;
        let mut observations = vec![];
        if build.run.is_none() {
            observations.push(BuildObservation::Linked { run: run_link(job) });
        }
        if !build.state.terminal() {
            let want = job.phase.build_state();
            if let Some(why) = job.unresolved.as_ref().filter(|_| !job.phase.terminal()) {
                if build.unresolved.is_none() {
                    observations.push(BuildObservation::Unknown {
                        reason: reason(why.clone()),
                    });
                }
            } else {
                match job.phase {
                    Phase::Ready => observations.push(BuildObservation::ImageReady {
                        image: job.image.clone().expect("ready has an image"),
                    }),
                    Phase::Failed => observations.push(BuildObservation::Failed {
                        reason: reason(job.reason.clone().unwrap_or_default()),
                    }),
                    Phase::Cancelled => observations.push(BuildObservation::Cancelled),
                    _ if build.unresolved.is_some() || build.state.rank() < want.rank() => {
                        observations.push(BuildObservation::Progress { state: want })
                    }
                    _ => {}
                }
            }
        }
        for observation in observations {
            self.environments.apply(
                &env.id,
                &EnvCommand::ObserveBuild {
                    build_id: build.id.clone(),
                    observation,
                },
                now_ms,
            )?;
        }
        Ok(())
    }

    /// Open a new evidence segment when this owner holds no live recorder
    /// for the job (first use, or after a restart). An active command
    /// continues in a new call that says where it resumes.
    fn ensure_segment(&self, lease: &Lease, now_ms: u64) -> Result<()> {
        let mut live = self.live.lock().expect("recorders");
        let job = lease.read()?;
        if live.contains_key(&job.id) {
            return Ok(());
        }
        let seg = format!("seg-{}", job.segments.len() + 1);
        let redactor = (self.custody)(&job.inputs.credential_names).map_err(BuildError::Custody)?;
        let mut recorder = Recorder::create(
            self.evidence_dir(&job.id, &seg),
            &seg,
            Some(run_link(&job)),
            redactor,
            job.inputs.evidence_budget,
        )?;
        let mut continued = vec![];
        for kind in [Kind2::Source, Kind2::Install, Kind2::Sanitize] {
            if let Some(step) = kind.get(&job)
                && matches!(step.run, Run::Requested | Run::Started { .. })
            {
                let call = format!("{}-{seg}", kind.id());
                recorder.start_call(
                    identity(&job, &call, kind.tool()),
                    &json!({"continues": step.spec.id, "cursor": step.cursor}),
                    now_ms,
                )?;
                continued.push((kind, call));
            }
        }
        lease.update(now_ms, |j| {
            j.segments.push(Segment {
                id: seg.clone(),
                sealed: None,
            });
            for (kind, call) in continued {
                if let Some(step) = kind.get_mut(j) {
                    step.call = call;
                }
            }
        })?;
        live.insert(job.id.clone(), recorder);
        Ok(())
    }

    fn record<T>(
        &self,
        job: &str,
        f: impl FnOnce(&mut Recorder) -> std::result::Result<T, EvidenceError>,
    ) -> Result<T> {
        let mut live = self.live.lock().expect("recorders");
        let recorder = live
            .get_mut(job)
            .ok_or(BuildError::Refused("No evidence segment is open."))?;
        Ok(f(recorder)?)
    }

    async fn provision(&self, lease: &Lease, job: &BuildJob, now_ms: u64) -> Result<bool> {
        let settled = self.computers.prompt(&job.computer, now_ms).await?;
        let c = settled.computer();
        let generation = match (&settled, &c.phase) {
            (Settled::Dispatch { generation, .. }, _) => Some(*generation),
            // Dispatched before a crash, before the job retained it.
            (_, ComputerPhase::Turn { generation }) => Some(*generation),
            _ => None,
        };
        if let Some(generation) = generation {
            lease.update(now_ms, |j| {
                j.generation = Some(generation);
                j.unresolved = None;
                j.phase = Phase::Materializing;
            })?;
            return Ok(true);
        }
        match (&settled, &c.phase) {
            (_, ComputerPhase::Unknown { reason: why }) => {
                let why = format!("The builder's provisioning outcome is unknown: {why}");
                lease.update(now_ms, |j| j.unresolved = Some(reason(why)))?;
                Ok(false)
            }
            (Settled::Stuck(..), _) => {
                lease.update(now_ms, |j| {
                    j.unresolved = Some("The builder did not become ready.".into())
                })?;
                Ok(false)
            }
            (Settled::Refused(m, _), _) => {
                let m = format!("The builder was refused: {m}");
                lease.update(now_ms, |j| fail(j, m))?;
                Ok(true)
            }
            _ => {
                let why = match &c.phase {
                    ComputerPhase::Failed { reason } => {
                        format!("The builder did not boot: {reason}")
                    }
                    other => format!("The builder did not boot ({other:?})."),
                };
                lease.update(now_ms, |j| fail(j, why))?;
                Ok(true)
            }
        }
    }

    fn command_spec(&self, job: &BuildJob, kind: Kind2, now_ms: u64) -> Result<CommandSpec> {
        let timeout_seconds = (job.deadline_ms.saturating_sub(now_ms) / 1000).max(1);
        let mut spec = match kind {
            // The exact pinned commit, fetched with ephemeral Git auth.
            Kind2::Source => {
                let (command, credential_names, env) = coder_environment_setup::source::command(
                    &job.inputs.source,
                    coder_environment_setup::source::Mode::Materialize,
                    job.inputs.git_credential.as_deref(),
                )
                .map_err(BuildError::Refused)?;
                CommandSpec {
                    id: kind.id().into(),
                    command,
                    cwd: ".".into(),
                    credential_names,
                    env,
                    timeout_seconds: timeout_seconds.min(3600),
                    digest: String::new(),
                }
            }
            Kind2::Install => {
                let command = self.read_script(&job.inputs.install_digest)?;
                let env = job
                    .inputs
                    .git_credential
                    .as_deref()
                    .map(git_auth_env)
                    .unwrap_or_default();
                CommandSpec {
                    id: kind.id().into(),
                    command,
                    cwd: job.inputs.install_cwd.clone(),
                    credential_names: job.inputs.credential_names.clone(),
                    env,
                    timeout_seconds,
                    digest: String::new(),
                }
            }
            // Sanitization sees no credential at all.
            Kind2::Sanitize => CommandSpec {
                id: kind.id().into(),
                command: sanitize::script(&job.inputs.plan),
                cwd: ".".into(),
                credential_names: Default::default(),
                env: Default::default(),
                timeout_seconds: timeout_seconds.min(1800),
                digest: String::new(),
            },
        };
        spec.digest = coder_environment_setup::transition::spec_digest(&spec);
        Ok(spec)
    }

    /// Run one identified command to its end, reading by identity first so
    /// a lost start reply never runs it twice.
    async fn step(&self, lease: &Lease, job: &BuildJob, kind: Kind2, now_ms: u64) -> Result<bool> {
        let Some(step) = kind.get(job).cloned() else {
            let spec = self.command_spec(job, kind, now_ms)?;
            let call = kind.id().to_owned();
            self.record(&job.id, |r| {
                r.start_call(
                    identity(job, &call, kind.tool()),
                    &json!({
                        "command": spec.command,
                        "cwd": spec.cwd,
                        "credential_names": spec.credential_names,
                        "env": spec.env,
                        "timeout_seconds": spec.timeout_seconds,
                        "digest": spec.digest,
                        "recipe_revision": job.inputs.recipe_revision,
                        "recipe_digest": job.inputs.recipe_digest,
                    }),
                    now_ms,
                )
            })?;
            lease.update(now_ms, |j| {
                *kind.get_mut(j) = Some(Step {
                    spec,
                    run: Run::Requested,
                    cursor: Default::default(),
                    call,
                    report: String::new(),
                });
            })?;
            return Ok(true);
        };
        let computer = self.computers.store.read(&job.computer)?;
        let Some(resource) = computer.resource().map(str::to_owned) else {
            lease.update(now_ms, |j| {
                j.unresolved = Some("The builder has no machine.".into())
            })?;
            return Ok(false);
        };
        let provider = &self.computers.provider;
        let read = match provider
            .read_command(
                &computer,
                &resource,
                &step.spec.id,
                step.cursor,
                MAX_READ_BYTES,
            )
            .await
        {
            Outcome::Done { value } => value,
            Outcome::Failed { reason: why } | Outcome::Unknown { reason: why } => {
                let why = format!("Reading {} failed: {why}", kind.id());
                lease.update(now_ms, |j| j.unresolved = Some(reason(why)))?;
                return Ok(false);
            }
        };
        if let Some(d) = &read.digest
            && d != &step.spec.digest
        {
            lease.update(now_ms, |j| {
                fail(j, "A different command holds this step's identity.")
            })?;
            return Ok(true);
        }
        if read.progress == CommandProgress::Absent {
            if matches!(step.run, Run::Started { .. }) && step.cursor != Default::default() {
                lease.update(now_ms, |j| fail(j, "The builder lost a started command."))?;
                return Ok(true);
            }
            // Never started (or the start never landed): one identity runs
            // at most once, so starting it now is safe.
            let outcome = provider
                .start_command(&computer, &resource, &step.spec)
                .await;
            lease.update(now_ms, |j| match outcome {
                Outcome::Done { value } => {
                    kind.get_mut(j).as_mut().expect("step").run = Run::Started { operation: value };
                    j.unresolved = None;
                }
                Outcome::Unknown { reason: why } => {
                    j.unresolved = Some(reason(format!("Starting {}: {why}", kind.id())));
                }
                Outcome::Failed { reason: why } => {
                    fail(j, format!("Starting {} failed: {why}", kind.id()));
                }
            })?;
            return Ok(lease.read()?.unresolved.is_none());
        }
        let full = read.stdout.len() as u64 >= MAX_READ_BYTES
            || read.stderr.len() as u64 >= MAX_READ_BYTES;
        let progress = match read.progress {
            CommandProgress::Exited { .. } if full => CommandProgress::Running,
            other => other,
        };
        let call = step.call.clone();
        self.record(&job.id, |r| {
            r.output(&call, StreamName::Stdout, &read.stdout)?;
            r.output(&call, StreamName::Stderr, &read.stderr)?;
            match progress {
                CommandProgress::Exited { code } => {
                    r.close_stream(&call, StreamName::Stdout)?;
                    r.close_stream(&call, StreamName::Stderr)?;
                    r.result(
                        &call,
                        CallResult::Exited {
                            code: Some(code),
                            success: code == 0,
                        },
                        now_ms,
                    )?;
                }
                CommandProgress::Lost => {
                    r.result(
                        &call,
                        CallResult::EngineError {
                            message: "The process ended without a recorded exit.".into(),
                        },
                        now_ms,
                    )?;
                }
                CommandProgress::Absent | CommandProgress::Running => {}
            }
            Ok(())
        })?;
        let mut report_overflow = false;
        let stale =
            matches!(progress, CommandProgress::Exited { code: 0 }) && kind == Kind2::Sanitize && {
                let env = self.environments.read(&job.inputs.environment)?;
                env.build(&job.inputs.build_id)
                    .is_some_and(|b| env.is_stale(b))
            };
        lease.update(now_ms, |j| {
            j.unresolved = None;
            let s = kind.get_mut(j).as_mut().expect("step");
            s.cursor.stdout += read.stdout.len() as u64;
            s.cursor.stderr += read.stderr.len() as u64;
            if matches!(kind, Kind2::Sanitize | Kind2::Source) {
                if s.report.len() + read.stdout.len() > MAX_REPORT_BYTES {
                    report_overflow = true;
                } else {
                    s.report.push_str(&String::from_utf8_lossy(&read.stdout));
                }
            }
            match progress {
                CommandProgress::Exited { code } => {
                    s.run = Run::Exited { code };
                    let text = s.report.clone();
                    match (kind, code) {
                        (Kind2::Source, code) => {
                            let report =
                                coder_environment_setup::source::Report::parse(&text)
                                    .unwrap_or_default();
                            let ok = code == 0
                                && !report_overflow
                                && report.verified(&j.inputs.source);
                            let detail = if report.timed_out() {
                                format!(
                                    "Downloading the pinned source timed out (exit {code}); it was stopped, not failed."
                                )
                            } else {
                                format!(
                                    "The pinned source was not materialized (exit {code}; head {:?}; error {:?}).",
                                    report.head, report.error
                                )
                            };
                            j.checkout = Some(report);
                            if ok {
                                j.phase = Phase::Installing;
                            } else {
                                fail(j, detail);
                            }
                        }
                        (Kind2::Install, 0) => j.phase = Phase::Sanitizing,
                        (Kind2::Install, code) => fail(j, format!("The install exited {code}.")),
                        (Kind2::Sanitize, _) => {
                            let report = Report::parse(&text);
                            let clean = code == 0
                                && !report_overflow
                                && report.clean_for(&j.inputs.plan);
                            let detail = format!(
                                "Sanitization did not pass (exit {code}; left: {:?}; missing: {:?}).",
                                report.residue, report.missing
                            );
                            j.report = Some(report);
                            if !clean {
                                fail(j, detail);
                            } else if stale {
                                j.phase = Phase::Cancelled;
                                j.reason = Some(
                                    "The recipe was revised during the build; this build is stale."
                                        .into(),
                                );
                            } else {
                                j.phase = Phase::Quiescing;
                            }
                        }
                    }
                }
                CommandProgress::Lost => {
                    s.run = Run::Lost;
                    fail(j, format!("The {} process ended without an exit.", kind.id()));
                }
                CommandProgress::Running | CommandProgress::Absent => {}
            }
        })?;
        Ok(full || lease.read()?.phase != job.phase)
    }

    /// Stop the builder so the capture sees a quiet filesystem. The stop
    /// takes no checkpoint: a builder is never restored.
    async fn quiesce(&self, lease: &Lease, job: &BuildJob, now_ms: u64) -> Result<bool> {
        let settled = self.computers.stop(&job.computer, now_ms).await?;
        let c = settled.computer();
        let stopped = matches!(c.phase, ComputerPhase::Stopped)
            && c.boot()
                .is_some_and(|b| b.resource_stop.as_ref().is_some_and(Fact::is_done));
        lease.update(now_ms, |j| {
            if stopped {
                j.unresolved = None;
                j.phase = Phase::Capturing;
            } else {
                j.unresolved = Some(reason(format!(
                    "The builder has not stopped ({:?}).",
                    c.phase
                )));
            }
        })?;
        Ok(stopped)
    }

    /// Capture under the owned name, reading the name before any capture
    /// call so a crash or lost reply is reconciled, never retried blindly.
    async fn capture(&self, lease: &Lease, job: &BuildJob, now_ms: u64) -> Result<bool> {
        let Some(cap) = job.capture.clone() else {
            lease.update(now_ms, |j| {
                j.capture = Some(Capture {
                    name: j.inputs.image_name.clone(),
                    issued: 0,
                    record: None,
                })
            })?;
            return Ok(true);
        };
        let computer = self.computers.store.read(&job.computer)?;
        let Some(resource) = computer.resource().map(str::to_owned) else {
            lease.update(now_ms, |j| fail(j, "The builder is gone before capture."))?;
            return Ok(true);
        };
        let provider = &self.computers.provider;
        let found = match provider.read_image(&cap.name).await {
            Outcome::Done { value } => value,
            Outcome::Failed { reason: why } | Outcome::Unknown { reason: why } => {
                let why = format!("Reading image {}: {why}", cap.name);
                lease.update(now_ms, |j| j.unresolved = Some(reason(why)))?;
                return Ok(false);
            }
        };
        let record = match found {
            Some(r) => r,
            None if cap.issued >= MAX_CAPTURE_ISSUES => {
                lease.update(now_ms, |j| {
                    j.unresolved = Some(format!(
                        "Image {} is still absent after {} captures.",
                        cap.name, cap.issued
                    ))
                })?;
                return Ok(false);
            }
            None => {
                // Retain the issue before the call.
                lease.update(now_ms, |j| j.capture.as_mut().expect("capture").issued += 1)?;
                match provider
                    .capture_image(&computer, &resource, &cap.name)
                    .await
                {
                    Outcome::Done { value } => value,
                    Outcome::Unknown { reason: why } => {
                        let why = format!("Capturing {}: {why}", cap.name);
                        lease.update(now_ms, |j| j.unresolved = Some(reason(why)))?;
                        return Ok(false);
                    }
                    Outcome::Failed { reason: why } => {
                        let why = format!("Capturing {} failed: {why}", cap.name);
                        lease.update(now_ms, |j| fail(j, why))?;
                        return Ok(true);
                    }
                }
            }
        };
        self.observe_image(lease, job, &resource, record, now_ms)
    }

    fn observe_image(
        &self,
        lease: &Lease,
        job: &BuildJob,
        resource: &str,
        record: ImageRecord,
        now_ms: u64,
    ) -> Result<bool> {
        if record.name != job.inputs.image_name || record.source != resource {
            lease.update(now_ms, |j| {
                fail(
                    j,
                    "The image name belongs to another source; it is never replaced.",
                )
            })?;
            return Ok(true);
        }
        let sealed = match (&record.state, &record.snapshot) {
            (ImageState::Ready, Some(snapshot)) => {
                let manifest = ImageManifest {
                    schema: MANIFEST_SCHEMA.into(),
                    environment: job.inputs.environment.clone(),
                    build_id: job.inputs.build_id.clone(),
                    recipe_revision: job.inputs.recipe_revision,
                    recipe_digest: job.inputs.recipe_digest.clone(),
                    source: job.inputs.source.clone(),
                    base: job.inputs.base.clone(),
                    runtime: job.inputs.runtime.clone(),
                    platform: job.inputs.platform.clone(),
                    plan_digest: job.inputs.plan.digest(),
                    checkout: job.checkout.clone().unwrap_or_default(),
                    report: job.report.clone().unwrap_or_default(),
                    name: record.name.clone(),
                    snapshot: snapshot.clone(),
                    builder: resource.into(),
                };
                Some(ImageIdentity {
                    provider: self.computers.provider.kind(),
                    image_id: record.name.clone(),
                    snapshot_id: Some(snapshot.clone()),
                    manifest_digest: manifest.digest(),
                })
            }
            _ => None,
        };
        if let ImageState::Ready | ImageState::Failed { .. } = record.state {
            let call = "capture".to_owned();
            let result = serde_json::to_vec(&record).expect("record encodes");
            let ok = sealed.is_some();
            self.record(&job.id, |r| {
                r.start_call(
                    identity(job, &call, "environment.build.capture"),
                    &json!({"name": record.name, "builder": resource}),
                    now_ms,
                )?;
                r.output(
                    &call,
                    if ok {
                        StreamName::Stdout
                    } else {
                        StreamName::Stderr
                    },
                    &result,
                )?;
                r.close_stream(&call, StreamName::Stdout)?;
                r.close_stream(&call, StreamName::Stderr)?;
                r.result(
                    &call,
                    CallResult::Exited {
                        code: Some(if ok { 0 } else { 1 }),
                        success: ok,
                    },
                    now_ms,
                )?;
                Ok(())
            })?;
        }
        lease.update(now_ms, |j| {
            let state = record.state.clone();
            j.capture.as_mut().expect("capture").record = Some(record);
            j.unresolved = None;
            match (state, sealed) {
                (_, Some(image)) => {
                    j.image = Some(image);
                    j.phase = Phase::Ready;
                }
                (ImageState::Failed { reason: why }, _) => {
                    fail(j, format!("The image capture failed: {why}"))
                }
                // Pending, or ready without an immutable snapshot yet.
                _ => {}
            }
        })?;
        Ok(lease.read()?.phase.terminal())
    }

    async fn stop_active(&self, job: &BuildJob) {
        let Ok(computer) = self.computers.store.read(&job.computer) else {
            return;
        };
        let Some(resource) = computer.resource() else {
            return;
        };
        for kind in [Kind2::Source, Kind2::Install, Kind2::Sanitize] {
            if let Some(step) = kind.get(job)
                && matches!(step.run, Run::Requested | Run::Started { .. })
            {
                let _ = self
                    .computers
                    .provider
                    .stop_command(&computer, resource, &step.spec.id)
                    .await;
            }
        }
    }

    /// Seal evidence, delete the builder, and retain usage and cleanup.
    async fn finish(&self, lease: &Lease, now_ms: u64) -> Result<BuildJob> {
        let job = lease.read()?;
        let recorder = self.live.lock().expect("recorders").remove(&job.id);
        if let Some(mut recorder) = recorder {
            let sealed = recorder.finish(now_ms)?;
            lease.update(now_ms, |j| {
                if let Some(seg) = j.segments.iter_mut().find(|s| s.id == sealed.id) {
                    seg.sealed = Some(sealed);
                }
            })?;
        }
        let job = lease.read()?;
        if matches!(job.cleanup, Cleanup::Complete { .. }) {
            return Ok(job);
        }
        lease.update(now_ms, |j| j.cleanup = Cleanup::Requested)?;
        let settled = self.computers.delete(&job.computer, now_ms).await?;
        let c = settled.computer().clone();
        let (usage, cleanup) = facts(&c);
        lease.update(now_ms, |j| {
            j.usage = usage;
            j.cleanup = cleanup;
        })?;
        Ok(lease.read()?)
    }
}

/// Usage and cleanup facts from the builder computer's record.
fn facts(c: &Computer) -> (Usage, Cleanup) {
    let mut usage = Usage::default();
    for b in &c.boots {
        match &b.meter_stop {
            Some(Fact::Done { evidence, .. }) => usage.evidence.push(evidence.clone()),
            Some(Fact::Unknown { reason, .. }) | Some(Fact::Failed { reason, .. }) => {
                usage.uncertain = Some(format!("Boot {} meter: {reason}", b.number));
            }
            Some(Fact::Requested { .. }) | None => {
                usage.uncertain = Some(format!("Boot {} has no meter-stop fact.", b.number));
            }
        }
    }
    let cleanup = match &c.phase {
        ComputerPhase::Deleted => Cleanup::Complete {
            evidence: c
                .creates
                .iter()
                .filter_map(|a| a.deletion.as_ref().and_then(Fact::evidence))
                .collect::<Vec<_>>()
                .join(","),
        },
        ComputerPhase::Unknown { reason: why } => Cleanup::Unknown {
            reason: reason(why.clone()),
        },
        other => Cleanup::Unknown {
            reason: reason(format!("The builder is not deleted ({other:?}).")),
        },
    };
    if matches!(cleanup, Cleanup::Complete { .. }) && c.boots.is_empty() {
        usage.uncertain = None;
    }
    (usage, cleanup)
}

fn identity(job: &BuildJob, call: &str, tool: &str) -> CallIdentity {
    CallIdentity {
        id: call.into(),
        parent: None,
        run: run_link(job),
        tool: tool.into(),
        request: Some(job.request_id.clone()),
        operation: None,
    }
}
