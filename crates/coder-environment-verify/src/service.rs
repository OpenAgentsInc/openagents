//! The verifier owner: admission, identity agreement, the two fresh
//! machines, frozen steps, the verdict, cleanup, and projection onto the
//! `VerificationAttempt`.
//!
//! [`Verifier::advance`] holds the job's lease for a whole visit. Each step
//! reads the retained job, performs at most one provider effect, and
//! retains what it observed before the next step. Command intent (its
//! spec) is retained before the start, so a lost reply reconciles by
//! reading the identity, never by running the command twice.

use crate::plan::{self, Action, CheckPlan, SourceStep};
use crate::store::{Lease, Store as JobStore, StoreError};
use crate::*;
use coder_environment::evidence::{
    CallIdentity, CallResult, EvidenceError, Recorder, Redactor, StreamName,
};
use coder_environment::store::{Store as EnvStore, StoreError as EnvStoreError};
use coder_environment::transition::VerificationObservation as Obs;
use coder_environment::{Command as EnvCommand, Effect, RunLink, Stage, digest};
use coder_environment_build::store::Store as BuildStore;
use coder_environment_setup::source;
use coder_working_computer::driver::{Driver, Settled};
use coder_working_computer::provider::{
    CommandProgress, CommandSpec, Commands, ImageState, Images, Outcome,
};
use coder_working_computer::{
    Bounds, Computer, Fact, Phase as ComputerPhase, Principal, Purpose, Spec, VerifyRole,
};
use serde_json::json;
use std::{collections::BTreeMap, fmt, fs, path::PathBuf, sync::Mutex};

/// Bytes of each stream one provider read takes.
pub const MAX_READ_BYTES: u64 = 128 * 1024;
const MAX_STEPS: usize = 256;

#[derive(Debug)]
pub enum VerifyError {
    Refused(&'static str),
    /// The request ID was used for a different verification.
    RequestConflict(String),
    /// The provider could not say whether the image is the sealed one.
    Unavailable(String),
    Environment(EnvStoreError),
    Job(StoreError),
    Build(coder_environment_build::store::StoreError),
    Computer(coder_working_computer::store::StoreError),
    Evidence(EvidenceError),
}
impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(m) => f.write_str(m),
            Self::RequestConflict(id) => {
                write!(
                    f,
                    "Request {id} was already used for a different verification."
                )
            }
            Self::Unavailable(m) => f.write_str(m),
            Self::Environment(e) => e.fmt(f),
            Self::Job(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
            Self::Computer(e) => e.fmt(f),
            Self::Evidence(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for VerifyError {}
impl From<EnvStoreError> for VerifyError {
    fn from(e: EnvStoreError) -> Self {
        Self::Environment(e)
    }
}
impl From<StoreError> for VerifyError {
    fn from(e: StoreError) -> Self {
        Self::Job(e)
    }
}
impl From<coder_environment_build::store::StoreError> for VerifyError {
    fn from(e: coder_environment_build::store::StoreError) -> Self {
        Self::Build(e)
    }
}
impl From<coder_working_computer::store::StoreError> for VerifyError {
    fn from(e: coder_working_computer::store::StoreError) -> Self {
        Self::Computer(e)
    }
}
impl From<EvidenceError> for VerifyError {
    fn from(e: EvidenceError) -> Self {
        Self::Evidence(e)
    }
}
pub type Result<T> = std::result::Result<T, VerifyError>;

/// What a caller asks for when it starts a verification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyRequest {
    pub request_id: String,
    pub environment: String,
    pub build_id: String,
    pub owner: Principal,
    /// The frozen plan the caller expects; it must be the recipe's.
    pub plan_digest: String,
    pub size: String,
}

/// A job with the attempt it projects to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VerifyView {
    pub job: VerifyJob,
    pub attempt: coder_environment::VerificationAttempt,
}

/// The live evidence of one run: the run record and the open machine's
/// child record.
struct Live {
    top: Recorder,
    child: Option<(VerifyRole, Recorder)>,
}

pub struct Verifier<P> {
    pub jobs: JobStore,
    pub environments: EnvStore,
    pub builds: BuildStore,
    pub computers: Driver<P>,
    root: PathBuf,
    blobs: PathBuf,
    live: Mutex<BTreeMap<String, Live>>,
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
fn role_call(role: VerifyRole) -> &'static str {
    match role {
        VerifyRole::Baseline => "baseline",
        VerifyRole::Fork => "fork",
    }
}
fn tool(action: &Action) -> &'static str {
    match action {
        Action::Source { .. } => "environment.verify.source",
        Action::Locks => "environment.verify.locks",
        Action::Service { .. } => "environment.verify.service",
        Action::Check { .. } => "environment.verify.check",
        Action::Inventory { .. } => "environment.verify.inventory",
        Action::Install => "environment.verify.install",
    }
}
/// End the run with `verdict` unless one is already decided.
fn decide(j: &mut VerifyJob, verdict: Verdict) {
    if j.verdict.is_none() {
        j.verdict = Some(verdict);
    }
    if matches!(j.phase, Phase::Baseline | Phase::Fork) {
        j.phase = Phase::Cleanup;
    }
}
fn failed(why: impl Into<String>) -> Verdict {
    Verdict::Failed {
        reason: reason(why),
    }
}

impl<P: Commands + Images> Verifier<P> {
    /// `root` holds evidence (`evidence/<job>`); `blobs` is the protected
    /// artifact store (`<digest>` files: check plans, check scripts, and
    /// install scripts), outside every setup and build write grant.
    pub fn new(
        root: impl Into<PathBuf>,
        blobs: impl Into<PathBuf>,
        jobs: JobStore,
        environments: EnvStore,
        builds: BuildStore,
        computers: Driver<P>,
    ) -> Self {
        Self {
            jobs,
            environments,
            builds,
            computers,
            root: root.into(),
            blobs: blobs.into(),
            live: Mutex::new(BTreeMap::new()),
        }
    }

    /// Forget live evidence, as an owner restart would.
    #[cfg(test)]
    pub(crate) fn restart(&self) {
        self.live.lock().expect("evidence").clear();
    }

    pub fn evidence_dir(&self, job: &str) -> PathBuf {
        self.root.join("evidence").join(job)
    }

    /// A protected artifact by digest; `None` when missing or altered.
    fn blob(&self, d: &str) -> Option<Vec<u8>> {
        if !coder_environment::valid_digest(d) {
            return None;
        }
        let bytes = fs::read(self.blobs.join(d)).ok()?;
        (digest(&bytes) == d).then_some(bytes)
    }
    fn text_blob(&self, d: &str) -> Option<String> {
        String::from_utf8(self.blob(d)?).ok()
    }

    /// Start (or continue) verifying one build's sealed image. Repeating
    /// the same request returns the same job.
    pub async fn start(&self, request: &VerifyRequest, now_ms: u64) -> Result<VerifyJob> {
        if !valid_id(&request.request_id)
            || !valid_id(&request.environment)
            || !valid_id(&request.build_id)
            || !valid_id(&request.size)
        {
            return Err(VerifyError::Refused(
                "The request, environment, build, and size need opaque identities.",
            ));
        }
        let fp = digest(&serde_json::to_vec(request).expect("request encodes"));
        let id = format!(
            "vj-{}",
            &digest(format!("{}\0{}", request.environment, request.request_id).as_bytes())[..24]
        );
        let lease = self.jobs.lease(&id)?;
        if lease.exists() {
            if lease.read()?.fingerprint != fp {
                return Err(VerifyError::RequestConflict(request.request_id.clone()));
            }
            return self.advance_locked(&lease, now_ms).await;
        }
        let env = self.environments.read(&request.environment)?;
        if request.owner.workspace != env.project.workspace {
            return Err(VerifyError::Refused(
                "The owner is not in the environment's workspace.",
            ));
        }
        // Unknown cleanup of an earlier verifier blocks new allocations.
        if self
            .jobs
            .list()?
            .iter()
            .any(|j| j.inputs.environment == env.id && j.cleanup_uncertain())
        {
            return Err(VerifyError::Refused(
                "An earlier verifier's cleanup is unknown; reconcile it first.",
            ));
        }
        let build = env
            .build(&request.build_id)
            .ok_or(VerifyError::Refused("The build attempt is missing."))?;
        let image = build
            .image
            .clone()
            .ok_or(VerifyError::Refused("The build has no sealed image."))?;
        let recipe = env
            .recipe(build.recipe_revision)
            .ok_or(VerifyError::Refused("The build names a missing recipe."))?
            .recipe
            .clone();
        // The frozen plan is a protected artifact; it must exist intact.
        let plan = self
            .blob(&request.plan_digest)
            .ok_or(VerifyError::Refused(
                "The frozen check plan artifact is missing or altered.",
            ))
            .and_then(|b| CheckPlan::parse(&b).map_err(VerifyError::Refused))?;
        if plan.profile != recipe.qualification.profile {
            return Err(VerifyError::Refused(
                "The check plan is for a different qualification profile.",
            ));
        }
        // Identity: the builder's retained manifest reproduces the
        // attempt's manifest digest.
        let manifest = self
            .builds
            .list()?
            .into_iter()
            .find(|j| j.inputs.environment == env.id && j.inputs.build_id == build.id)
            .and_then(|j| j.manifest())
            .ok_or(VerifyError::Refused(
                "The build's image manifest is missing.",
            ))?;
        if manifest.digest() != image.manifest_digest
            || manifest.recipe_digest != build.recipe_digest
            || manifest.source != build.source
            || manifest.name != image.image_id
            || image.snapshot_id.as_deref() != Some(manifest.snapshot.as_str())
        {
            return Err(VerifyError::Refused(
                "The builder and the attempt disagree on the image identity.",
            ));
        }
        if image.provider != self.computers.provider.kind() {
            return Err(VerifyError::Refused(
                "The image belongs to another provider than this verifier's.",
            ));
        }
        // The provider still holds exactly that immutable snapshot.
        match self.computers.provider.read_image(&image.image_id).await {
            Outcome::Done { value: Some(r) }
                if r.state == ImageState::Ready
                    && r.snapshot.as_deref() == Some(manifest.snapshot.as_str())
                    && r.source == manifest.builder => {}
            Outcome::Done { .. } => {
                return Err(VerifyError::Refused(
                    "The provider's image is not the sealed snapshot.",
                ));
            }
            Outcome::Failed { reason } | Outcome::Unknown { reason } => {
                return Err(VerifyError::Unavailable(format!(
                    "The image could not be read: {reason}"
                )));
            }
        }
        let verification_id = match self.environments.apply(
            &env.id,
            &EnvCommand::StartVerification {
                request_id: request.request_id.clone(),
                build_id: build.id.clone(),
                plan_digest: request.plan_digest.clone(),
            },
            now_ms,
        )? {
            coder_environment::Applied::Changed(
                _,
                Effect::VerificationRequested {
                    verification_id, ..
                },
            )
            | coder_environment::Applied::Replayed(Effect::VerificationRequested {
                verification_id,
                ..
            }) => verification_id,
            _ => return Err(VerifyError::Refused("Unexpected environment effect.")),
        };
        let window = recipe.limits.deadline_seconds.min(24 * 3600) * 1000;
        let mut machines = vec![];
        for role in [VerifyRole::Baseline, VerifyRole::Fork] {
            let computer_id = match role {
                VerifyRole::Baseline => format!("verifier-{id}"),
                VerifyRole::Fork => format!("verifier-fork-{id}"),
            };
            let spec = Spec {
                id: computer_id.clone(),
                owner: request.owner.clone(),
                chat: id.clone(),
                project: env.project.clone(),
                source: env.source.clone(),
                base: None,
                size: request.size.clone(),
                credential_names: Default::default(),
                services: vec![],
                bounds: Bounds {
                    idle_ms: window,
                    observed_extension_ms: 0,
                    absolute_ms: window,
                },
            };
            let purpose = Purpose::EnvironmentVerify {
                environment: env.id.clone(),
                build: build.id.clone(),
                verification: verification_id.clone(),
                image: image.image_id.clone(),
                role,
            };
            let mut computer =
                Computer::for_verify(spec, purpose, now_ms).map_err(VerifyError::Refused)?;
            computer.provider = self.computers.provider.kind();
            match self.computers.store.read(&computer_id) {
                Ok(existing) if existing.purpose == computer.purpose => {}
                Ok(_) => {
                    return Err(VerifyError::Refused(
                        "That computer is not this verification's dedicated verifier.",
                    ));
                }
                Err(coder_working_computer::store::StoreError::NotFound) => {
                    self.computers.store.create(&computer)?
                }
                Err(e) => return Err(e.into()),
            }
            machines.push(MachineRun::new(computer_id, role));
        }
        let fork = machines.pop().expect("fork");
        let baseline = machines.pop().expect("baseline");
        let steps = plan::steps(&plan, !recipe.inputs.locks.is_empty())
            .into_iter()
            .map(|p| StepRecord {
                id: p.id,
                role: p.role,
                action: p.action,
                spec: None,
                run: Run::NotStarted,
                cursor: Default::default(),
                deadline_ms: 0,
                tally: Default::default(),
                report: String::new(),
                outcome: None,
            })
            .collect();
        let inputs = Inputs {
            environment: env.id.clone(),
            build_id: build.id.clone(),
            verification_id,
            image,
            manifest,
            plan_digest: request.plan_digest.clone(),
            plan,
            install_digest: recipe.install.digest.clone(),
            install_cwd: recipe.install.cwd.clone(),
            locks: recipe.inputs.locks.clone(),
            size: request.size.clone(),
            evidence_budget: recipe.limits.output_bytes,
        };
        let job = VerifyJob {
            schema: SCHEMA.into(),
            id: id.clone(),
            revision: 1,
            created_ms: now_ms,
            updated_ms: now_ms,
            request_id: request.request_id.clone(),
            fingerprint: fp,
            inputs,
            deadline_ms: now_ms + window,
            phase: Phase::Baseline,
            baseline,
            fork,
            steps,
            unresolved: None,
            verdict: None,
            evidence: None,
            evidence_lost: false,
            history: vec![Transition {
                phase: Phase::Baseline,
                at_ms: now_ms,
                reason: None,
            }],
        };
        lease.create(&job)?;
        self.open_run(&job, now_ms)?;
        self.advance_locked(&lease, now_ms).await
    }

    /// A visit: move the verification as far as it can go now.
    pub async fn advance(&self, id: &str, now_ms: u64) -> Result<VerifyJob> {
        let lease = self.jobs.lease(id)?;
        self.advance_locked(&lease, now_ms).await
    }

    /// Cancel a verification; its machines are still cleaned up.
    pub async fn cancel(&self, id: &str, why: &str, now_ms: u64) -> Result<VerifyJob> {
        let lease = self.jobs.lease(id)?;
        let job = lease.read()?;
        if job.verdict.is_none() {
            self.end_run(
                &lease,
                &job,
                Verdict::Cancelled {
                    reason: reason(why),
                },
                now_ms,
            )
            .await?;
        }
        self.advance_locked(&lease, now_ms).await
    }

    /// The job and the attempt it projects to.
    pub fn view(&self, id: &str) -> Result<VerifyView> {
        let job = self.jobs.read(id)?;
        let env = self.environments.read(&job.inputs.environment)?;
        let attempt = env
            .verification(&job.inputs.verification_id)
            .cloned()
            .ok_or(VerifyError::Refused("The verification attempt is missing."))?;
        Ok(VerifyView { job, attempt })
    }

    async fn advance_locked(&self, lease: &Lease, now_ms: u64) -> Result<VerifyJob> {
        for _ in 0..MAX_STEPS {
            let job = lease.read()?;
            self.sync_environment(&job, now_ms)?;
            if job.phase == Phase::Done {
                return Ok(job);
            }
            if self.lost_evidence(lease, &job, now_ms).await? {
                continue;
            }
            if job.verdict.is_none() {
                if now_ms >= job.deadline_ms {
                    self.end_run(
                        lease,
                        &job,
                        failed("The verification deadline passed."),
                        now_ms,
                    )
                    .await?;
                    continue;
                }
                if let Some(why) = self.plan_changed(&job)? {
                    self.end_run(lease, &job, Verdict::Cancelled { reason: why }, now_ms)
                        .await?;
                    continue;
                }
            }
            let moved = match job.phase {
                Phase::Baseline => {
                    self.run_role(lease, &job, VerifyRole::Baseline, now_ms)
                        .await?
                }
                Phase::Fork => self.run_role(lease, &job, VerifyRole::Fork, now_ms).await?,
                Phase::Cleanup => self.cleanup(lease, &job, now_ms).await?,
                Phase::Done => true,
            };
            if !moved {
                let job = lease.read()?;
                self.sync_environment(&job, now_ms)?;
                return Ok(job);
            }
        }
        Ok(lease.read()?)
    }

    /// A recipe revision (which stales the build) or an altered plan
    /// artifact invalidates the run.
    fn plan_changed(&self, job: &VerifyJob) -> Result<Option<String>> {
        let env = self.environments.read(&job.inputs.environment)?;
        let build = env
            .build(&job.inputs.build_id)
            .ok_or(VerifyError::Refused("The build attempt is missing."))?;
        let frozen = env
            .recipe(build.recipe_revision)
            .map(|r| r.recipe.qualification.plan_digest.clone());
        if env.is_stale(build) || frozen.as_deref() != Some(job.inputs.plan_digest.as_str()) {
            return Ok(Some(
                "The recipe or its check plan changed; this run is invalid.".into(),
            ));
        }
        if self.blob(&job.inputs.plan_digest).is_none() {
            return Ok(Some(
                "The protected check plan artifact changed; this run is invalid.".into(),
            ));
        }
        Ok(None)
    }

    /// Project the job onto its `VerificationAttempt`. Every observation is
    /// idempotent, so a crash between the two records repeats it safely.
    fn sync_environment(&self, job: &VerifyJob, now_ms: u64) -> Result<()> {
        use coder_environment::VerificationState as V;
        let env = self.environments.read(&job.inputs.environment)?;
        let v = env
            .verification(&job.inputs.verification_id)
            .ok_or(VerifyError::Refused("The verification attempt is missing."))?;
        let mut observations = vec![];
        if v.run.is_none() {
            observations.push(Obs::Linked {
                run: RunLink {
                    cloud_job: job.baseline.computer.clone(),
                    task: Some(job.id.clone()),
                },
            });
        }
        if !v.state.terminal() {
            if job.phase == Phase::Done {
                let evidence = job.evidence.clone();
                observations.push(match job.verdict.clone().expect("done has a verdict") {
                    Verdict::Passed => match &evidence {
                        Some(sealed) => sealed.passed(),
                        None => Obs::Incomplete {
                            reason: "The checks passed without sealed evidence.".into(),
                            evidence: None,
                        },
                    },
                    Verdict::Failed { reason } => Obs::Failed { reason, evidence },
                    Verdict::Incomplete { reason } => Obs::Incomplete { reason, evidence },
                    Verdict::Cancelled { .. } => Obs::Cancelled,
                });
            } else if let Some(why) = &job.unresolved {
                if v.unresolved.is_none() {
                    observations.push(Obs::Unknown {
                        reason: reason(why.clone()),
                    });
                }
            } else {
                let want = job.verification_state();
                if want != V::NeedsReconciliation
                    && (v.unresolved.is_some() || v.state.rank() < want.rank())
                {
                    observations.push(Obs::Progress { state: want });
                }
            }
        }
        for observation in observations {
            self.environments.apply(
                &env.id,
                &EnvCommand::ObserveVerification {
                    verification_id: v.id.clone(),
                    observation,
                },
                now_ms,
            )?;
        }
        Ok(())
    }

    fn identity(
        &self,
        job: &VerifyJob,
        call: &str,
        tool: &str,
        role: Option<VerifyRole>,
    ) -> CallIdentity {
        let cloud_job = match role {
            Some(r) => job.machine(r).computer.clone(),
            None => job.baseline.computer.clone(),
        };
        CallIdentity {
            id: call.into(),
            parent: role.map(|r| role_call(r).to_owned()),
            run: RunLink {
                cloud_job,
                task: Some(job.id.clone()),
            },
            tool: tool.into(),
            request: Some(job.request_id.clone()),
            operation: None,
        }
    }

    /// Open the run's evidence record and retain the identity agreement.
    fn open_run(&self, job: &VerifyJob, now_ms: u64) -> Result<()> {
        let mut top = Recorder::create(
            self.evidence_dir(&job.id),
            "verify",
            Some(RunLink {
                cloud_job: job.baseline.computer.clone(),
                task: Some(job.id.clone()),
            }),
            // A verifier carries no credentials.
            Redactor::new(),
            job.inputs.evidence_budget,
        )?;
        top.start_call(
            self.identity(job, "identity", "environment.verify.identity", None),
            &json!({
                "image": job.inputs.image,
                "plan_digest": job.inputs.plan_digest,
                "build": job.inputs.build_id,
            }),
            now_ms,
        )?;
        let manifest = serde_json::to_vec(&job.inputs.manifest).expect("manifest encodes");
        top.output("identity", StreamName::Stdout, &manifest)?;
        top.close_stream("identity", StreamName::Stdout)?;
        top.close_stream("identity", StreamName::Stderr)?;
        top.result(
            "identity",
            CallResult::Exited {
                code: Some(0),
                success: true,
            },
            now_ms,
        )?;
        self.live
            .lock()
            .expect("evidence")
            .insert(job.id.clone(), Live { top, child: None });
        Ok(())
    }

    /// An owner restart loses the live record: the run can no longer
    /// prove complete evidence, so it ends incomplete and is cleaned up.
    async fn lost_evidence(&self, lease: &Lease, job: &VerifyJob, now_ms: u64) -> Result<bool> {
        if job.evidence.is_some()
            || job.evidence_lost
            || self.live.lock().expect("evidence").contains_key(&job.id)
        {
            return Ok(false);
        }
        self.stop_active(job).await;
        lease.update(now_ms, |j| {
            j.evidence_lost = true;
            let why =
                "The verifier restarted and lost its live evidence; start a new verification.";
            if matches!(j.verdict, None | Some(Verdict::Passed)) {
                j.verdict = Some(Verdict::Incomplete { reason: why.into() });
            }
            if matches!(j.phase, Phase::Baseline | Phase::Fork) {
                j.phase = Phase::Cleanup;
            }
        })?;
        Ok(true)
    }

    fn with_live<T>(
        &self,
        job: &str,
        f: impl FnOnce(&mut Live) -> std::result::Result<T, EvidenceError>,
    ) -> Result<T> {
        let mut live = self.live.lock().expect("evidence");
        let l = live
            .get_mut(job)
            .ok_or(VerifyError::Refused("No evidence record is open."))?;
        Ok(f(l)?)
    }
    fn child<T>(
        &self,
        job: &str,
        f: impl FnOnce(&mut Recorder) -> std::result::Result<T, EvidenceError>,
    ) -> Result<T> {
        self.with_live(job, |l| match &mut l.child {
            Some((_, r)) => f(r),
            None => Err(EvidenceError::Invalid("No machine evidence is open.")),
        })
    }

    /// Open the machine's child record under its call in the run record.
    fn open_child(&self, job: &VerifyJob, role: VerifyRole, now_ms: u64) -> Result<()> {
        let call = role_call(role);
        let run = RunLink {
            cloud_job: job.machine(role).computer.clone(),
            task: Some(job.id.clone()),
        };
        let budget = job.inputs.evidence_budget / 2;
        let identity = self.identity(job, call, "environment.verify.machine", None);
        let args = json!({
            "computer": job.machine(role).computer,
            "image": job.inputs.image.image_id,
            "role": role,
        });
        self.with_live(&job.id, |l| {
            if l.child.as_ref().is_some_and(|(r, _)| *r == role) || l.top.call(call).is_some() {
                return Ok(());
            }
            l.top.start_call(identity, &args, now_ms)?;
            let child = l.top.child(call, call, run, budget)?;
            l.child = Some((role, child));
            Ok(())
        })
    }

    /// Archive the machine's child record and close its call.
    fn close_child(&self, job: &str, role: VerifyRole, success: bool, now_ms: u64) -> Result<()> {
        let call = role_call(role);
        let mut live = self.live.lock().expect("evidence");
        let Some(l) = live.get_mut(job) else {
            return Ok(());
        };
        let Some((r, _)) = &l.child else {
            return Ok(());
        };
        if *r != role {
            return Ok(());
        }
        let (_, child) = l.child.take().expect("child");
        l.top.archive_child(child, now_ms)?;
        l.top.close_stream(call, StreamName::Stdout)?;
        l.top.close_stream(call, StreamName::Stderr)?;
        l.top.result(
            call,
            CallResult::Exited {
                code: Some(if success { 0 } else { 1 }),
                success,
            },
            now_ms,
        )?;
        Ok(())
    }

    /// Stop whatever runs, close the open machine record, and decide.
    async fn end_run(
        &self,
        lease: &Lease,
        job: &VerifyJob,
        verdict: Verdict,
        now_ms: u64,
    ) -> Result<()> {
        self.stop_active(job).await;
        if let Some(role) = match job.phase {
            Phase::Baseline => Some(VerifyRole::Baseline),
            Phase::Fork => Some(VerifyRole::Fork),
            _ => None,
        } {
            self.close_child(&job.id, role, false, now_ms)?;
        }
        lease.update(now_ms, |j| {
            j.unresolved = None;
            decide(j, verdict);
        })?;
        Ok(())
    }

    async fn run_role(
        &self,
        lease: &Lease,
        job: &VerifyJob,
        role: VerifyRole,
        now_ms: u64,
    ) -> Result<bool> {
        let m = job.machine(role);
        if m.generation.is_none() {
            self.open_child(job, role, now_ms)?;
            return self.provision(lease, job, role, now_ms).await;
        }
        if !m.hydrated {
            return self.hydrate(lease, job, role, now_ms).await;
        }
        if job.inputs.plan.checks.is_empty() {
            self.close_child(&job.id, role, false, now_ms)?;
            lease.update(now_ms, |j| {
                decide(
                    j,
                    failed("The plan declares no behavior check; an empty plan cannot pass."),
                )
            })?;
            return Ok(true);
        }
        if let Some(step) = job.next_step(role).cloned() {
            return match step.action {
                Action::Service { ref name } => self.service(lease, job, &step, name, now_ms).await,
                _ => self.command(lease, job, &step, now_ms).await,
            };
        }
        match role {
            VerifyRole::Baseline => {
                self.close_child(&job.id, role, true, now_ms)?;
                lease.update(now_ms, |j| j.phase = Phase::Fork)?;
            }
            VerifyRole::Fork => {
                let inventory = |after: bool| {
                    job.steps
                        .iter()
                        .find(|s| s.action == Action::Inventory { after })
                        .and_then(|s| plan::parse_inventory(&s.report))
                };
                let verdict = match (inventory(false), inventory(true)) {
                    (Some(before), Some(after)) if before == after => Verdict::Passed,
                    (Some(before), Some(after)) => {
                        let changed: Vec<&String> = before
                            .keys()
                            .chain(after.keys())
                            .filter(|k| before.get(*k) != after.get(*k))
                            .collect();
                        failed(format!(
                            "Rerunning the install and startup changed the image: {changed:?}."
                        ))
                    }
                    _ => failed("The idempotence inventory is missing."),
                };
                let ok = verdict == Verdict::Passed;
                self.close_child(&job.id, role, ok, now_ms)?;
                lease.update(now_ms, |j| decide(j, verdict))?;
            }
        }
        Ok(true)
    }

    async fn provision(
        &self,
        lease: &Lease,
        job: &VerifyJob,
        role: VerifyRole,
        now_ms: u64,
    ) -> Result<bool> {
        let id = &job.machine(role).computer;
        let settled = self.computers.prompt(id, now_ms).await?;
        let c = settled.computer();
        let generation = match (&settled, &c.phase) {
            (Settled::Dispatch { generation, .. }, _) => Some(*generation),
            (_, ComputerPhase::Turn { generation }) => Some(*generation),
            _ => None,
        };
        if let Some(generation) = generation {
            let resource = c.resource().map(str::to_owned);
            // A different machine: never the builder, never the baseline.
            let reused = resource.as_deref() == Some(job.inputs.manifest.builder.as_str())
                || (role == VerifyRole::Fork && resource == job.baseline.resource);
            lease.update(now_ms, |j| {
                let m = j.machine_mut(role);
                m.generation = Some(generation);
                m.resource = resource;
                j.unresolved = None;
                if reused {
                    decide(
                        j,
                        failed("The verifier was not a different, fresh machine."),
                    );
                }
            })?;
            if reused {
                self.close_child(&job.id, role, false, now_ms)?;
            }
            return Ok(true);
        }
        match (&settled, &c.phase) {
            (_, ComputerPhase::Unknown { reason: why }) => {
                let why = format!("The verifier's provisioning outcome is unknown: {why}");
                lease.update(now_ms, |j| j.unresolved = Some(reason(why)))?;
                Ok(false)
            }
            (Settled::Stuck(..), _) => {
                lease.update(now_ms, |j| {
                    j.unresolved = Some("The verifier did not become ready.".into())
                })?;
                Ok(false)
            }
            _ => {
                let why = match (&settled, &c.phase) {
                    (Settled::Refused(m, _), _) => format!("The verifier was refused: {m}"),
                    (_, ComputerPhase::Failed { reason }) => {
                        format!("The image did not boot on a fresh machine: {reason}")
                    }
                    (_, other) => format!("The image did not boot ({other:?})."),
                };
                self.close_child(&job.id, role, false, now_ms)?;
                lease.update(now_ms, |j| decide(j, failed(why)))?;
                Ok(true)
            }
        }
    }

    /// Restore readiness: the provider's typed hydration fact.
    async fn hydrate(
        &self,
        lease: &Lease,
        job: &VerifyJob,
        role: VerifyRole,
        now_ms: u64,
    ) -> Result<bool> {
        let computer = self.computers.store.read(&job.machine(role).computer)?;
        let Some(resource) = computer.resource().map(str::to_owned) else {
            lease.update(now_ms, |j| {
                decide(j, failed("The verifier has no machine."))
            })?;
            return Ok(true);
        };
        match self
            .computers
            .provider
            .hydration(&computer, &resource)
            .await
        {
            Outcome::Done { value: true } => {
                let call = "hydration".to_owned();
                let identity =
                    self.identity(job, &call, "environment.verify.hydration", Some(role));
                self.child(&job.id, |r| {
                    r.start_call(identity, &json!({"resource": resource}), now_ms)?;
                    r.output(&call, StreamName::Stdout, b"hydrated=true\n")?;
                    r.close_stream(&call, StreamName::Stdout)?;
                    r.close_stream(&call, StreamName::Stderr)?;
                    r.result(
                        &call,
                        CallResult::Exited {
                            code: Some(0),
                            success: true,
                        },
                        now_ms,
                    )?;
                    Ok(())
                })?;
                lease.update(now_ms, |j| {
                    j.unresolved = None;
                    j.machine_mut(role).hydrated = true;
                })?;
                Ok(true)
            }
            // Still copying restored files in: wait for a later visit.
            Outcome::Done { value: false } => Ok(false),
            Outcome::Failed { reason: why } => {
                self.close_child(&job.id, role, false, now_ms)?;
                lease.update(now_ms, |j| {
                    decide(j, failed(format!("The image did not restore: {why}")))
                })?;
                Ok(true)
            }
            Outcome::Unknown { reason: why } => {
                lease.update(now_ms, |j| {
                    j.unresolved = Some(reason(format!("Reading hydration: {why}")))
                })?;
                Ok(false)
            }
        }
    }

    /// Start a declared service under provider process ownership and
    /// require its health rule. An unknown readiness outcome cannot pass.
    async fn service(
        &self,
        lease: &Lease,
        job: &VerifyJob,
        step: &StepRecord,
        name: &str,
        now_ms: u64,
    ) -> Result<bool> {
        let role = step.role;
        let Some(decl) = job
            .inputs
            .plan
            .services()
            .iter()
            .find(|s| s.name == name)
            .cloned()
        else {
            return self.fail_step(lease, job, step, "The plan names no such service.", now_ms);
        };
        if step.run != Run::NotStarted {
            // Retained intent without an observed outcome: never start a
            // second copy to find out.
            return self.fail_step(
                lease,
                job,
                step,
                "The service's readiness outcome is unknown after a restart.",
                now_ms,
            );
        }
        let identity = self.identity(job, &step.id, tool(&step.action), Some(role));
        self.child(&job.id, |r| {
            r.start_call(identity, &json!({"service": decl}), now_ms)
        })?;
        lease.update(now_ms, |j| {
            j.step_mut(&step.id).expect("step").run = Run::Requested
        })?;
        let computer = self.computers.store.read(&job.machine(role).computer)?;
        let Some(resource) = computer.resource().map(str::to_owned) else {
            return self.fail_step(lease, job, step, "The verifier has no machine.", now_ms);
        };
        let outcome = self
            .computers
            .provider
            .start_service(&computer, &resource, &decl)
            .await;
        let (ok, text) = match &outcome {
            Outcome::Done { value } => (true, value.clone()),
            Outcome::Failed { reason } => (false, format!("not ready: {reason}")),
            Outcome::Unknown { reason } => (false, format!("readiness unknown: {reason}")),
        };
        self.child(&job.id, |r| {
            r.output(
                &step.id,
                if ok {
                    StreamName::Stdout
                } else {
                    StreamName::Stderr
                },
                format!("{text}\n").as_bytes(),
            )?;
            r.close_stream(&step.id, StreamName::Stdout)?;
            r.close_stream(&step.id, StreamName::Stderr)?;
            r.result(
                &step.id,
                CallResult::Exited {
                    code: Some(if ok { 0 } else { 1 }),
                    success: ok,
                },
                now_ms,
            )?;
            Ok(())
        })?;
        lease.update(now_ms, |j| {
            j.step_mut(&step.id).expect("step").run = Run::Exited {
                code: if ok { 0 } else { 1 },
            }
        })?;
        if ok {
            lease.update(now_ms, |j| {
                j.step_mut(&step.id).expect("step").outcome =
                    Some(StepOutcome::Passed { detail: text })
            })?;
            Ok(true)
        } else {
            self.fail_step(lease, job, step, &text, now_ms)
        }
    }

    /// Record a failed step and end the run with it.
    fn fail_step(
        &self,
        lease: &Lease,
        job: &VerifyJob,
        step: &StepRecord,
        why: &str,
        now_ms: u64,
    ) -> Result<bool> {
        self.close_child(&job.id, step.role, false, now_ms)?;
        let why = reason(why);
        lease.update(now_ms, |j| {
            j.unresolved = None;
            j.step_mut(&step.id).expect("step").outcome = Some(StepOutcome::Failed {
                reason: why.clone(),
            });
            decide(j, failed(format!("{}: {why}", step.id)));
        })?;
        Ok(true)
    }

    /// The exact command for a step; `Err` names a missing artifact.
    fn command_spec(
        &self,
        job: &VerifyJob,
        step: &StepRecord,
        now_ms: u64,
    ) -> std::result::Result<CommandSpec, String> {
        let remaining = (job.deadline_ms.saturating_sub(now_ms) / 1000).max(1);
        let plan = &job.inputs.plan;
        let (command, cwd, timeout) = match &step.action {
            Action::Source { step: s } => {
                let mode = match s {
                    SourceStep::Contained => source::Mode::Verify,
                    SourceStep::Materialize => source::Mode::Materialize,
                };
                let (text, _, _) = source::command(&job.inputs.manifest.source, mode, None)
                    .map_err(str::to_owned)?;
                (text, ".".to_owned(), remaining.min(3600))
            }
            Action::Locks => (
                plan::locks_script(&job.inputs.locks),
                ".".to_owned(),
                remaining.min(600),
            ),
            Action::Inventory { .. } => (
                plan::inventory_script(&plan.idempotence.inventory),
                ".".to_owned(),
                remaining.min(3600),
            ),
            Action::Install => {
                let text = self.text_blob(&job.inputs.install_digest).ok_or(
                    "The build's install script artifact is missing or altered.".to_owned(),
                )?;
                (text, job.inputs.install_cwd.clone(), remaining)
            }
            Action::Check { name } => {
                let check = plan
                    .check(name)
                    .ok_or("The plan names no such check.".to_owned())?;
                let text = self.text_blob(&check.script).ok_or(format!(
                    "Check {name}'s script artifact is missing or altered."
                ))?;
                (
                    text,
                    check.cwd.clone(),
                    check.timeout_seconds.min(remaining),
                )
            }
            Action::Service { .. } => return Err("A service is not a command.".into()),
        };
        let mut spec = CommandSpec {
            id: step.id.clone(),
            command,
            cwd,
            credential_names: Default::default(),
            env: if plan.offline {
                plan::offline_env()
            } else {
                Default::default()
            },
            timeout_seconds: timeout,
            digest: String::new(),
        };
        spec.digest = coder_environment_setup::transition::spec_digest(&spec);
        Ok(spec)
    }

    /// Run one identified command to its end, reading by identity first so
    /// a lost start reply never runs it twice. It is never repaired.
    async fn command(
        &self,
        lease: &Lease,
        job: &VerifyJob,
        step: &StepRecord,
        now_ms: u64,
    ) -> Result<bool> {
        let role = step.role;
        let Some(spec) = step.spec.clone() else {
            let spec = match self.command_spec(job, step, now_ms) {
                Ok(spec) => spec,
                Err(why) => return self.fail_step(lease, job, step, &why, now_ms),
            };
            let identity = self.identity(job, &step.id, tool(&step.action), Some(role));
            self.child(&job.id, |r| {
                r.start_call(
                    identity,
                    &json!({
                        "action": step.action,
                        "command": spec.command,
                        "cwd": spec.cwd,
                        "env": spec.env,
                        "timeout_seconds": spec.timeout_seconds,
                        "digest": spec.digest,
                    }),
                    now_ms,
                )
            })?;
            let deadline = (now_ms + spec.timeout_seconds * 1000).min(job.deadline_ms);
            lease.update(now_ms, |j| {
                let s = j.step_mut(&step.id).expect("step");
                s.spec = Some(spec);
                s.run = Run::Requested;
                s.deadline_ms = deadline;
            })?;
            return Ok(true);
        };
        let computer = self.computers.store.read(&job.machine(role).computer)?;
        let Some(resource) = computer.resource().map(str::to_owned) else {
            return self.fail_step(lease, job, step, "The verifier has no machine.", now_ms);
        };
        let provider = &self.computers.provider;
        let read = match provider
            .read_command(&computer, &resource, &step.id, step.cursor, MAX_READ_BYTES)
            .await
        {
            Outcome::Done { value } => value,
            Outcome::Failed { reason: why } | Outcome::Unknown { reason: why } => {
                let why = format!("Reading {}: {why}", step.id);
                lease.update(now_ms, |j| j.unresolved = Some(reason(why)))?;
                return Ok(false);
            }
        };
        if let Some(d) = &read.digest
            && d != &spec.digest
        {
            return self.fail_step(
                lease,
                job,
                step,
                "A different command holds this step's identity.",
                now_ms,
            );
        }
        if read.progress == CommandProgress::Absent {
            if matches!(step.run, Run::Started { .. }) && step.cursor != Default::default() {
                return self.fail_step(
                    lease,
                    job,
                    step,
                    "The verifier lost a started command.",
                    now_ms,
                );
            }
            // Never started, or the start never landed: one identity runs
            // at most once, so starting it now is safe.
            let outcome = provider.start_command(&computer, &resource, &spec).await;
            return match outcome {
                Outcome::Done { value } => {
                    lease.update(now_ms, |j| {
                        j.unresolved = None;
                        j.step_mut(&step.id).expect("step").run = Run::Started { operation: value };
                    })?;
                    Ok(true)
                }
                Outcome::Unknown { reason: why } => {
                    lease.update(now_ms, |j| {
                        j.unresolved = Some(reason(format!("Starting {}: {why}", step.id)))
                    })?;
                    Ok(false)
                }
                Outcome::Failed { reason: why } => self.fail_step(
                    lease,
                    job,
                    step,
                    &format!("It did not start: {why}"),
                    now_ms,
                ),
            };
        }
        let full = read.stdout.len() as u64 >= MAX_READ_BYTES
            || read.stderr.len() as u64 >= MAX_READ_BYTES;
        let mut progress = match read.progress {
            CommandProgress::Exited { .. } if full => CommandProgress::Running,
            other => other,
        };
        let timed_out = progress == CommandProgress::Running && now_ms >= step.deadline_ms && !full;
        if timed_out {
            let _ = provider.stop_command(&computer, &resource, &step.id).await;
            progress = CommandProgress::Lost;
        }
        self.child(&job.id, |r| {
            r.output(&step.id, StreamName::Stdout, &read.stdout)?;
            r.output(&step.id, StreamName::Stderr, &read.stderr)?;
            match &progress {
                CommandProgress::Exited { code } => {
                    r.close_stream(&step.id, StreamName::Stdout)?;
                    r.close_stream(&step.id, StreamName::Stderr)?;
                    r.result(
                        &step.id,
                        CallResult::Exited {
                            code: Some(*code),
                            success: *code == 0,
                        },
                        now_ms,
                    )?;
                }
                CommandProgress::Lost if timed_out => {
                    r.result(&step.id, CallResult::TimedOut { code: None }, now_ms)?;
                }
                CommandProgress::Lost => {
                    r.result(
                        &step.id,
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
        let assertions = match &step.action {
            Action::Check { name } => job.inputs.plan.check(name).map(|c| c.assertions),
            _ => None,
        };
        let mut verdict: Option<std::result::Result<String, String>> = None;
        let updated = lease.update(now_ms, |j| {
            j.unresolved = None;
            let source_pin = j.inputs.manifest.source.clone();
            let locks = j.inputs.locks.clone();
            let s = j.step_mut(&step.id).expect("step");
            s.cursor.stdout += read.stdout.len() as u64;
            s.cursor.stderr += read.stderr.len() as u64;
            let mut overflow = false;
            match assertions {
                Some(a) => s.tally.feed(a, &read.stdout),
                None => {
                    if s.report.len() + read.stdout.len() > MAX_REPORT_BYTES {
                        overflow = true;
                    } else {
                        s.report.push_str(&String::from_utf8_lossy(&read.stdout));
                    }
                }
            }
            verdict = match &progress {
                CommandProgress::Exited { code } => {
                    s.run = Run::Exited { code: *code };
                    Some(if overflow {
                        Err("The step's report exceeded its bound.".into())
                    } else {
                        evaluate(s, *code, assertions, &source_pin, &locks)
                    })
                }
                CommandProgress::Lost if timed_out => {
                    s.run = Run::TimedOut;
                    Some(Err("The check timed out.".into()))
                }
                CommandProgress::Lost => {
                    s.run = Run::Lost;
                    Some(Err("The process ended without an exit.".into()))
                }
                _ => None,
            };
            if let Some(Ok(detail)) = &verdict {
                s.outcome = Some(StepOutcome::Passed {
                    detail: detail.clone(),
                });
            }
        })?;
        match verdict {
            Some(Err(why)) => self.fail_step(lease, &updated, step, &why, now_ms),
            Some(Ok(_)) => Ok(true),
            None => Ok(full),
        }
    }

    async fn stop_active(&self, job: &VerifyJob) {
        for step in job
            .steps
            .iter()
            .filter(|s| matches!(s.run, Run::Requested | Run::Started { .. }) && s.spec.is_some())
        {
            let Ok(computer) = self.computers.store.read(&job.machine(step.role).computer) else {
                continue;
            };
            if let Some(resource) = computer.resource() {
                let _ = self
                    .computers
                    .provider
                    .stop_command(&computer, resource, &step.id)
                    .await;
            }
        }
    }

    /// Delete both machines and retain their usage; only confirmed cleanup
    /// lets the verdict be recorded and the evidence seal.
    async fn cleanup(&self, lease: &Lease, job: &VerifyJob, now_ms: u64) -> Result<bool> {
        for role in [VerifyRole::Baseline, VerifyRole::Fork] {
            let m = job.machine(role);
            if matches!(m.cleanup, Cleanup::Complete { .. }) {
                continue;
            }
            let computer = self.computers.store.read(&m.computer)?;
            if computer.creates.is_empty() {
                lease.update(now_ms, |j| {
                    j.machine_mut(role).cleanup = Cleanup::Complete {
                        evidence: "never allocated".into(),
                    }
                })?;
                continue;
            }
            lease.update(now_ms, |j| j.machine_mut(role).cleanup = Cleanup::Requested)?;
            let settled = self.computers.delete(&m.computer, now_ms).await?;
            let (usage, uncertain, cleanup) = facts(settled.computer());
            lease.update(now_ms, |j| {
                let m = j.machine_mut(role);
                m.usage = usage;
                m.usage_uncertain = uncertain;
                m.cleanup = cleanup;
            })?;
        }
        let job = lease.read()?;
        if job.cleanup_uncertain()
            || !matches!(job.baseline.cleanup, Cleanup::Complete { .. })
            || !matches!(job.fork.cleanup, Cleanup::Complete { .. })
        {
            lease.update(now_ms, |j| {
                j.unresolved = Some(
                    "A verifier machine's cleanup is unknown; the verdict waits for it.".into(),
                )
            })?;
            return Ok(false);
        }
        let sealed = {
            let facts = json!({
                "baseline": {"cleanup": job.baseline.cleanup, "usage": job.baseline.usage},
                "fork": {"cleanup": job.fork.cleanup, "usage": job.fork.usage},
            });
            let identity = self.identity(&job, "cleanup", "environment.verify.cleanup", None);
            let mut live = self.live.lock().expect("evidence");
            match live.remove(&job.id) {
                None => None,
                Some(mut l) => {
                    if let Some((_, child)) = l.child.take() {
                        l.top.archive_child(child, now_ms)?;
                    }
                    l.top.start_call(identity, &json!({}), now_ms)?;
                    l.top.output(
                        "cleanup",
                        StreamName::Stdout,
                        &serde_json::to_vec(&facts).expect("facts encode"),
                    )?;
                    l.top.close_stream("cleanup", StreamName::Stdout)?;
                    l.top.close_stream("cleanup", StreamName::Stderr)?;
                    l.top.result(
                        "cleanup",
                        CallResult::Exited {
                            code: Some(0),
                            success: true,
                        },
                        now_ms,
                    )?;
                    Some(l.top.finish(now_ms)?)
                }
            }
        };
        lease.update(now_ms, |j| {
            j.unresolved = None;
            if sealed.is_none() && j.verdict == Some(Verdict::Passed) {
                j.verdict = Some(Verdict::Incomplete {
                    reason: "The run's evidence could not be sealed.".into(),
                });
            }
            j.evidence = sealed;
            j.phase = Phase::Done;
        })?;
        Ok(true)
    }
}

/// Judge a finished command step.
fn evaluate(
    s: &mut StepRecord,
    code: i64,
    assertions: Option<plan::Assertions>,
    pin: &coder_environment::SourcePin,
    locks: &BTreeMap<String, String>,
) -> std::result::Result<String, String> {
    match &s.action {
        Action::Check { .. } => {
            let a = assertions.ok_or("The plan names no such check.")?;
            s.tally.finish(a);
            if code != 0 {
                return Err(format!("The check exited {code}."));
            }
            s.tally.verdict(a)
        }
        Action::Source { .. } => {
            let report = source::Report::parse(&s.report).unwrap_or_default();
            if code == 0 && report.verified(pin) {
                Ok(format!(
                    "checkout {} is the pinned, clean commit",
                    report.head
                ))
            } else {
                Err(format!(
                    "The checkout is not the pinned commit (exit {code}; error {:?}).",
                    report.error
                ))
            }
        }
        Action::Locks => {
            plan::locks_ok(&s.report, locks)?;
            if code != 0 {
                return Err(format!("The lock check exited {code}."));
            }
            Ok(format!("{} lock files match", locks.len()))
        }
        Action::Inventory { .. } => match plan::parse_inventory(&s.report) {
            Some(map) if code == 0 => Ok(digest(
                &serde_json::to_vec(&map).expect("inventory encodes"),
            )),
            _ => Err(format!("The inventory did not complete (exit {code}).")),
        },
        Action::Install => {
            if code == 0 {
                Ok("the exact install script reran".into())
            } else {
                Err(format!("Rerunning the install exited {code}."))
            }
        }
        Action::Service { .. } => Err("A service is not a command.".into()),
    }
}

/// Usage and cleanup facts from a verifier computer's record.
fn facts(c: &Computer) -> (Vec<String>, Option<String>, Cleanup) {
    let mut usage = vec![];
    let mut uncertain = None;
    for b in &c.boots {
        match &b.meter_stop {
            Some(Fact::Done { evidence, .. }) => usage.push(evidence.clone()),
            Some(Fact::Unknown { reason, .. }) | Some(Fact::Failed { reason, .. }) => {
                uncertain = Some(format!("Boot {} meter: {reason}", b.number));
            }
            Some(Fact::Requested { .. }) | None => {
                uncertain = Some(format!("Boot {} has no meter-stop fact.", b.number));
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
            reason: reason(format!("The verifier is not deleted ({other:?}).")),
        },
    };
    (usage, uncertain, cleanup)
}
