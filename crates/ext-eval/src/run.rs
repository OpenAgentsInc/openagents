//! The runner: every planned run of a suite, each one `coder -p` turn in
//! its own confined directory, graded, scored, and written out.
//!
//! [`execute`] makes the runs: for each planned attempt it builds a
//! [`Sandbox`], starts a door proxy, spawns the agent inside the boundary
//! with the arm's environment, waits under the case's deadline and the
//! operator's [`Cancel`], and collects the ATIF trajectory, the created
//! files, cost, and time into a [`RunRecord`]. [`evaluate_runs`] grades
//! them with the live doors and concludes. [`write_results`] writes the
//! results directory. [`run_suite`] does all three, which is what
//! `openagents ext eval run` and the hosted runner call.
//!
//! Nothing here reads the operator's `CODER_*` environment or writes a
//! credential: the caller hands the pinned doors over as values, the
//! child sees only a per-run proxy token, and [`write_results`] refuses
//! to finish when the door key turns up under the results directory.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use crate::arms::{self, AgentPin, Subject};
use crate::artifact::{ArtifactRef, JSON, json_bytes};
use crate::case::{Case, Grant, RunFailure};
use crate::child::{self, ChildSpec, Ended};
use crate::discover::Suite;
use crate::door::{DecisionDoor, Doors};
use crate::evaluate::{EvalError, Evaluation, conclude, load_gate_named};
use crate::live::OpenResponsesJudge;
use crate::proxy::{Proxy, Secret, Upstream};
use crate::record::{Arm, RunOutcome, RunRecord, run_path};
use crate::replay::ModuleReplayer;
use crate::report::{ArmSetup, DoorNames, Identity};
use crate::sandbox::{self, Sandbox, SandboxError};
use crate::score::{Plan, grade_all};
use crate::signal::Cancel;
use crate::trajectory::Trajectory;

/// The fewest and most runs at once.
pub const CONCURRENCY: std::ops::RangeInclusive<usize> = 1..=8;
/// The largest created file copied into the results.
pub const MAX_COPIED_FILE: u64 = 1 << 20;
/// The schema of `run.json`, the runner's own record in a results dir.
pub const RUN_SCHEMA: &str = "openagents.ext-eval-run.v1";

/// The chat door both arms answer through, pinned.
#[derive(Clone, Debug)]
pub struct Door {
    /// The door's name in the runner's table, recorded in the report.
    pub name: String,
    /// Its base URL.
    pub url: String,
    /// Its key. Never handed to a child.
    pub key: Secret,
    /// The model it runs.
    pub model: String,
}

impl Door {
    /// The door as a report names it: `name/model`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}/{}", self.name, self.model)
    }
}

/// The decision door a child classifies its turns through.
///
/// Coder asks Jev whether a turn runs a program before it does anything
/// else, so a decision door that can't answer takes the extension's
/// programs away from the subject arm and both arms score alike
/// ([#10122](https://github.com/OpenAgentsInc/openagents/issues/10122)).
/// A pin may carry Jev's other doors ([`DecisionPin::fallbacks`]); a
/// decision then asks them first and TypeSafe last, the order every server
/// that holds the keys uses ([`jev::doors::Failover::primary_last`]).
#[derive(Clone, Debug)]
pub struct DecisionPin {
    /// Its base URL, such as `https://api.typesafe.ai`.
    pub url: String,
    /// Its key. Never handed to a child.
    pub key: Secret,
    /// Jev's other doors with their keys, asked before `url` (the Vercel
    /// AI Gateway, then OpenRouter). Empty asks `url` alone.
    pub fallbacks: Vec<jev::doors::Door>,
}

impl DecisionPin {
    /// TypeSafe's door at `url` under `key`, alone.
    #[must_use]
    pub fn new(url: impl Into<String>, key: Secret) -> Self {
        Self {
            url: url.into(),
            key,
            fallbacks: Vec::new(),
        }
    }

    /// The pin with Jev's other doors ([`jev::doors::FALLBACKS`]) whose
    /// keys `key_of` finds, by each door's key variable
    /// (`AI_GATEWAY_API_KEY`, `OPENROUTER_API_KEY`). A blank key is no
    /// door.
    #[must_use]
    pub fn with_fallbacks(mut self, key_of: &dyn Fn(&str) -> Option<String>) -> Self {
        for fallback in &jev::doors::FALLBACKS {
            if let Some(key) = key_of(fallback.key_var).filter(|key| !key.trim().is_empty()) {
                self.fallbacks
                    .push(jev::doors::Door::fallback(fallback, jev::ApiKey::new(key)));
            }
        }
        self
    }

    /// The doors a decision asks, in order, for a log line: each door's
    /// URL, never a key.
    #[must_use]
    pub fn doors(&self) -> Vec<String> {
        self.fallbacks
            .iter()
            .map(|door| door.door.clone())
            .chain(std::iter::once(self.url.trim_end_matches('/').to_string()))
            .collect()
    }

    /// The failover across the pin's doors, or `None` when it has one.
    #[must_use]
    pub fn failover(&self) -> Option<jev::doors::Failover> {
        if self.fallbacks.is_empty() {
            return None;
        }
        Some(
            jev::doors::Failover::new(
                jev::doors::Door::new(
                    self.url.trim_end_matches('/'),
                    self.url.clone(),
                    jev::doors::Naming::Canonical,
                    jev::ApiKey::new(self.key.expose()),
                ),
                self.fallbacks.clone(),
            )
            .primary_last(),
        )
    }

    /// Jev for the `decision` graders, through the same doors.
    ///
    /// # Errors
    ///
    /// Why there is no Jev, never a key.
    pub fn jev_door(&self, model: Option<String>) -> Result<crate::JevDoor, String> {
        let Some(failover) = self.failover() else {
            return crate::JevDoor::resolved(&self.url, Some(self.key.expose()), model);
        };
        let client = jev::Client::new(
            jev::Config::new()
                .exchange(jev::doors::exchange(failover))
                .base_url(&self.url)
                .default_model(model.as_deref().unwrap_or(jev::defaults::MODEL)),
        )
        .map_err(|error| format!("Jev: {error}"))?;
        crate::JevDoor::from_client(client, model)
    }
}

/// How a suite runs.
#[derive(Clone, Debug)]
pub struct Options {
    /// Runs per arm for every case, overriding each case's `runs`.
    pub runs: Option<u32>,
    /// Whether the baseline arm runs.
    pub baseline: bool,
    /// Runs at once, 1 to 8.
    pub concurrency: usize,
    /// Keep each run's directory and report its path.
    pub keep_temp: bool,
    /// The operator's grants; `read` is always among them.
    pub grants: BTreeSet<Grant>,
    /// Where run directories are made.
    pub temp_root: PathBuf,
    /// The confinement backend binary this run requires. `None` uses the
    /// host's; a path that isn't a file refuses every run as
    /// `unconfined_host`.
    pub backend: Option<PathBuf>,
    /// The Gym gate the suite is judged by and names in its acceptance:
    /// `None` is [`crate::evaluate::GATE_ID`] (`ext-eval-v2`, correctness
    /// first); `ext-eval-cost-v1` reads cost first.
    pub gate: Option<String>,
    /// Coder's defaults, admitted in both arms so the report is marginal:
    /// the candidate on top of the current defaults against the current
    /// defaults alone. `None` runs the baseline with nothing admitted.
    pub defaults: Option<arms::Defaults>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            runs: None,
            baseline: true,
            concurrency: 1,
            keep_temp: false,
            grants: BTreeSet::from([Grant::Read]),
            temp_root: std::env::temp_dir(),
            backend: None,
            gate: None,
            defaults: None,
        }
    }
}

impl Options {
    /// The gate ID the run is judged by.
    #[must_use]
    pub fn gate_id(&self) -> &str {
        self.gate.as_deref().unwrap_or(crate::evaluate::GATE_ID)
    }
}

/// What the runner reports as it goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    /// A run started.
    Started {
        /// The case.
        case: String,
        /// The arm.
        arm: Arm,
        /// The attempt.
        attempt: u32,
    },
    /// A run ended.
    Finished {
        /// The case.
        case: String,
        /// The arm.
        arm: Arm,
        /// The attempt.
        attempt: u32,
        /// `completed`, `failed`, `refused`, or `cancelled`, with a reason.
        outcome: String,
        /// Wall seconds.
        seconds: f64,
    },
    /// A run's directory was kept.
    Kept(PathBuf),
}

/// What went wrong running a suite.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The concurrency is outside 1 to 8.
    #[error("--concurrency must be from 1 to 8")]
    Concurrency,
    /// The engine refused to conclude.
    #[error(transparent)]
    Eval(#[from] EvalError),
    /// A results file couldn't be written.
    #[error("the results could not be written: {0}")]
    Io(String),
    /// The door key reached the results directory. Nothing is published
    /// from it.
    #[error("the door key was found in {0}; the results were removed")]
    Leak(String),
}

/// Everything one suite run needs.
pub struct Setup<'a> {
    /// The suite, filtered.
    pub suite: &'a Suite,
    /// The extension under test.
    pub subject: &'a Subject,
    /// The agent binary both arms run.
    pub agent: &'a AgentPin,
    /// The pinned chat door.
    pub door: &'a Door,
    /// The decision door the child classifies through, when there is one.
    pub decision: Option<&'a DecisionPin>,
    /// How the suite runs.
    pub options: &'a Options,
}

impl Setup<'_> {
    /// The plan the setup runs.
    #[must_use]
    pub fn plan(&self) -> Plan {
        Plan {
            baseline: self.options.baseline,
            runs: self.options.runs,
            extension_operations: self.subject.operations(),
        }
    }
}

/// One finished run and what the harness kept of it.
pub struct Made {
    /// The record the engine grades.
    pub record: RunRecord,
    /// The run's directory, alive until grading is done.
    pub sandbox: Option<Sandbox>,
    /// The trajectory's exact bytes, as the record's ArtifactRef digests.
    pub trajectory: Option<Vec<u8>>,
    /// The run's `out/` files, scrubbed, by name.
    pub outputs: BTreeMap<String, Vec<u8>>,
    /// The created files' contents, scrubbed, when small enough to keep.
    pub created: BTreeMap<String, Vec<u8>>,
}

/// Every run of a suite.
pub struct Runs {
    /// The runs, in plan order.
    pub made: Vec<Made>,
    /// Whether the operator stopped the run.
    pub cancelled: bool,
    /// When the first run started, Unix seconds.
    pub started_at: u64,
    /// When the last run ended, Unix seconds.
    pub ended_at: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |span| span.as_secs())
}

/// The grants a case runs with: `read`, and each operation the case asks
/// for that the operator granted.
#[must_use]
pub fn case_grants(case: &Case, operator: &BTreeSet<Grant>) -> BTreeSet<Grant> {
    let mut grants = BTreeSet::from([Grant::Read]);
    for grant in &case.run.allowed_operations {
        if operator.contains(grant) {
            grants.insert(*grant);
        }
    }
    grants
}

/// Makes every planned run.
///
/// # Errors
///
/// Returns [`RunError::Concurrency`] for a concurrency outside 1 to 8.
pub fn execute(
    setup: &Setup<'_>,
    cancel: &Cancel,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Runs, RunError> {
    if !CONCURRENCY.contains(&setup.options.concurrency) {
        return Err(RunError::Concurrency);
    }
    let plan = setup.plan();
    let attempts = plan.attempts(setup.suite);
    let confinement = match &setup.options.backend {
        Some(path) if !path.is_file() => Err(SandboxError::Unconfined(format!(
            "{} is not installed",
            path.display()
        ))),
        _ => sandbox::confinement_available(),
    };
    let started_at = now();
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Made)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..setup.options.concurrency.min(attempts.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some((name, arm, attempt)) = attempts.get(index) else {
                        break;
                    };
                    let Some(case) = setup.suite.case(name) else {
                        continue;
                    };
                    let made = if cancel.cancelled() {
                        Made::bare(RunRecord::new(name, *arm, *attempt, RunOutcome::Cancelled))
                    } else if confinement.is_err() {
                        Made::bare(RunRecord::new(
                            name,
                            *arm,
                            *attempt,
                            RunOutcome::Errored(RunFailure::UnconfinedHost),
                        ))
                    } else {
                        progress(Progress::Started {
                            case: name.clone(),
                            arm: *arm,
                            attempt: *attempt,
                        });
                        let made = one(setup, case, *arm, *attempt, cancel);
                        progress(Progress::Finished {
                            case: name.clone(),
                            arm: *arm,
                            attempt: *attempt,
                            outcome: outcome_words(made.record.outcome),
                            seconds: made.record.seconds.unwrap_or(0.0),
                        });
                        made
                    };
                    if let Some(sandbox) = &made.sandbox
                        && sandbox.kept()
                    {
                        progress(Progress::Kept(sandbox.root().to_path_buf()));
                    }
                    if let Ok(mut done) = done.lock() {
                        done.push((index, made));
                    }
                }
            });
        }
    });
    let mut made = done.into_inner().unwrap_or_default();
    made.sort_by_key(|(index, _)| *index);
    Ok(Runs {
        made: made.into_iter().map(|(_, made)| made).collect(),
        cancelled: cancel.cancelled(),
        started_at,
        ended_at: now().max(started_at),
    })
}

fn outcome_words(outcome: RunOutcome) -> String {
    match outcome {
        RunOutcome::Errored(failure) => format!("{} ({})", outcome.coverage_word(), failure.word()),
        other => other.coverage_word().to_string(),
    }
}

impl Made {
    fn bare(record: RunRecord) -> Self {
        Self {
            record,
            sandbox: None,
            trajectory: None,
            outputs: BTreeMap::new(),
            created: BTreeMap::new(),
        }
    }
}

/// One run: sandbox, proxy, child, collection.
fn one(setup: &Setup<'_>, case: &Case, arm: Arm, attempt: u32, cancel: &Cancel) -> Made {
    let mut record = RunRecord::new(&case.name, arm, attempt, RunOutcome::Completed);
    if case.check_env().is_err() {
        record.outcome = RunOutcome::Errored(RunFailure::EnvVarRejected);
        return Made::bare(record);
    }
    let refused = |mut record: RunRecord| {
        record.outcome = RunOutcome::Errored(RunFailure::Refused);
        Made::bare(record)
    };
    let grants = case_grants(case, &setup.options.grants);
    let Ok(sandbox) = Sandbox::create(&setup.options.temp_root, case, setup.options.keep_temp)
    else {
        return refused(record);
    };
    if std::fs::write(sandbox.root().join("prompt.md"), case.prompt.as_bytes()).is_err() {
        return refused(record);
    }
    let defaults = setup.options.defaults.as_ref();
    let guidance = arms::guidance_with(arm, setup.subject, defaults, case);
    let guidance_digest = match &guidance {
        Some(text) => {
            if std::fs::write(sandbox.guidance(), text.as_bytes()).is_err() {
                return refused(record);
            }
            Some(nostr::contracts::digest_bytes(text.as_bytes()))
        }
        None => None,
    };
    if !setup.agent.questions.is_empty() {
        if std::fs::create_dir_all(sandbox.questions()).is_err() {
            return refused(record);
        }
        for (name, bytes) in &setup.agent.questions {
            if std::fs::write(sandbox.questions().join(name), bytes).is_err() {
                return refused(record);
            }
        }
    }
    // The defaults' programs are admitted in both arms; the subject's in
    // the subject arm. A baseline with nothing admitted names no program.
    let admitted = arms::admitted_programs(arm, setup.subject, defaults);
    let programs = if admitted.is_empty() {
        None
    } else {
        if std::fs::create_dir_all(sandbox.programs()).is_err() {
            return refused(record);
        }
        for program in &admitted {
            if std::fs::write(
                sandbox.programs().join(format!("{}.json", program.slug)),
                &program.bytes,
            )
            .is_err()
            {
                return refused(record);
            }
        }
        Some((
            arms::program_grant_for(arm, setup.subject, defaults),
            arms::effects_ceiling(&grants),
        ))
    };
    let boundary = match sandbox.confine(&grants, std::slice::from_ref(&setup.agent.path)) {
        Ok(boundary) => boundary,
        Err(SandboxError::Unconfined(_)) => {
            record.outcome = RunOutcome::Errored(RunFailure::UnconfinedHost);
            return Made::bare(record);
        }
        Err(SandboxError::Io(_)) => return refused(record),
    };
    let Ok(door) = Proxy::start(Upstream {
        url: setup.door.url.clone(),
        key: setup.door.key.clone(),
        decisions: None,
    }) else {
        return refused(record);
    };
    let decision = match setup.decision {
        Some(pin) => match Proxy::start(Upstream {
            url: pin.url.clone(),
            key: pin.key.clone(),
            decisions: pin.failover(),
        }) {
            Ok(proxy) => Some(proxy),
            Err(_) => return refused(record),
        },
        None => None,
    };
    let env = child::environment(&ChildSpec {
        sandbox: &sandbox,
        door: &door,
        model: &setup.door.model,
        decision: decision.as_ref(),
        programs,
        guidance: guidance_digest,
        grants: &grants,
        env: &case.run.env,
    });
    let Ok(command) = boundary.command(&setup.agent.path, child::arguments(&sandbox)) else {
        return refused(record);
    };
    let before = sandbox.workspace_files();
    let deadline = Duration::from_secs(u64::from(case.run.deadline_seconds));
    let finished = child::run(command, &sandbox, &env, deadline, cancel);
    drop(boundary);
    let Ok(finished) = finished else {
        return refused(record);
    };
    let mut secrets: Vec<String> = vec![
        setup.door.key.expose().to_string(),
        door.token().expose().to_string(),
    ];
    if let Some(proxy) = &decision {
        secrets.push(proxy.token().expose().to_string());
    }
    if let Some(pin) = setup.decision {
        secrets.push(pin.key.expose().to_string());
        secrets.extend(
            pin.fallbacks
                .iter()
                .map(|door| door.key().expose().to_string()),
        );
    }
    let refused_credential =
        door.refused_credential() || decision.as_ref().is_some_and(Proxy::refused_credential);
    drop(door);
    drop(decision);
    let secret_refs: Vec<&str> = secrets.iter().map(String::as_str).collect();
    record.seconds = Some(finished.seconds);
    record.outcome = match finished.ended {
        Ended::TimedOut => RunOutcome::Errored(RunFailure::Timeout),
        Ended::Cancelled => RunOutcome::Cancelled,
        Ended::Exited(_) if refused_credential => RunOutcome::Errored(RunFailure::AuthFailed),
        Ended::Exited(_) => RunOutcome::Completed,
    };
    let mut outputs = BTreeMap::new();
    for name in ["stdout.jsonl", "stderr.txt"] {
        if let Ok(bytes) = std::fs::read(sandbox.out().join(name)) {
            outputs.insert(name.to_string(), sandbox::scrub(&bytes, &secret_refs));
        }
    }
    if let Some(summary) = outputs
        .get("stdout.jsonl")
        .and_then(|bytes| child::summary(bytes))
    {
        record.cost_usd = summary.get("cost_usd").and_then(Value::as_f64);
    }
    let mut trajectory_bytes = None;
    if let Ok(recording) = atif::log::read(&sandbox.trajectory()) {
        let bytes = sandbox::scrub(&json_bytes(&recording.document()), &secret_refs);
        if let Ok(trajectory) = Trajectory::from_bytes(&bytes) {
            record.trajectory = Some(trajectory);
            trajectory_bytes = Some(bytes);
        }
    }
    let after = sandbox.workspace_files();
    record.created_files = sandbox::created(&before, &after);
    let mut created = BTreeMap::new();
    for path in &record.created_files {
        let full = sandbox.cwd().join(path);
        if std::fs::metadata(&full)
            .is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_COPIED_FILE)
            && let Ok(bytes) = std::fs::read(&full)
        {
            created.insert(path.clone(), sandbox::scrub(&bytes, &secret_refs));
        }
    }
    record.workspace = Some(sandbox.cwd());
    Made {
        record,
        sandbox: Some(sandbox),
        trajectory: trajectory_bytes,
        outputs,
        created,
    }
}

/// Who a report is about and who wrote it, beyond what the runner knows.
#[derive(Clone, Debug)]
pub struct Author {
    /// The suite author's public key.
    pub author: String,
    /// The suite's package slug.
    pub package: String,
    /// The suite's component slug.
    pub component: String,
    /// The evaluator's public key.
    pub evaluator: String,
    /// The suite's NIP-EXT release, when it was published.
    pub suite_release: Option<Value>,
    /// A hosted run's signed request.
    pub requester: Option<Value>,
}

/// The run configuration an arm's `ArmSetup.run` records.
#[must_use]
pub fn run_configuration(setup: &Setup<'_>, arm: Arm) -> Value {
    json!({
        "agent": "coder -p",
        "arm": arm.word(),
        "grants": setup.options.grants.iter().map(|grant| grant.word()).collect::<Vec<_>>(),
        "network": sandbox::network_policy(&setup.options.grants),
        "confinement": coder_boundary::backend_path(),
        "runs": setup.options.runs,
        "baseline": setup.options.baseline,
        "concurrency": setup.options.concurrency,
        "classifier": setup.decision.is_some(),
        "gate": setup.options.gate_id(),
        "defaults": setup.options.defaults.as_ref().map(|d| d.release.clone()),
    })
}

/// The report's identity for `setup` and `runs`.
#[must_use]
pub fn identity(setup: &Setup<'_>, author: &Author, runs: &Runs) -> Identity {
    let defaults = setup.options.defaults.as_ref();
    let subject_lock = setup.subject.lock_document_with(setup.agent, defaults);
    let baseline_lock = arms::baseline_lock_with(setup.agent, defaults);
    Identity {
        author: author.author.clone(),
        package: author.package.clone(),
        component: author.component.clone(),
        evaluator: author.evaluator.clone(),
        subject: ArmSetup {
            definition: setup.subject.definition.clone(),
            lock: ArtifactRef::of(&subject_lock, JSON, Some(arms::LOCK_SCHEMA)),
            door: setup.door.label(),
            run: run_configuration(setup, Arm::Subject),
        },
        baseline: setup.options.baseline.then(|| ArmSetup {
            definition: arms::baseline_definition(&author.author, setup.agent),
            lock: ArtifactRef::of(&baseline_lock, JSON, Some(arms::LOCK_SCHEMA)),
            door: setup.door.label(),
            run: run_configuration(setup, Arm::Baseline),
        }),
        started_at: runs.started_at,
        ended_at: runs.ended_at,
        requester: author.requester.clone(),
        suite_release: author.suite_release.clone(),
        environment: None,
        defaults: defaults.map(|d| d.release.clone()),
        partial: runs
            .cancelled
            .then(|| "the operator stopped the run".to_string()),
    }
}

/// Grades every run with the live doors and concludes. A stopped run is
/// graded with the structural graders only: no door is asked after the
/// operator said stop.
///
/// # Errors
///
/// Returns [`RunError::Eval`] when the engine refuses the records, the
/// gate, or the identity.
pub fn evaluate_runs(
    setup: &Setup<'_>,
    author: &Author,
    runs: &Runs,
    decision: Option<&dyn DecisionDoor>,
) -> Result<Evaluation, RunError> {
    let plan = setup.plan();
    let (gate, gate_file) = load_gate_named(setup.options.gate_id())?;
    let judge =
        OpenResponsesJudge::new(&setup.door.url, setup.door.key.clone(), &setup.door.model).ok();
    let mut replayer = ModuleReplayer::new(setup.subject);
    for made in &runs.made {
        if let Some(workspace) = &made.record.workspace {
            replayer.add_run(
                (
                    made.record.case.clone(),
                    made.record.arm,
                    made.record.attempt,
                ),
                workspace.clone(),
                made.record.trajectory.clone(),
            );
        }
    }
    let doors = if runs.cancelled {
        Doors::default()
    } else {
        Doors {
            decision,
            judge: judge
                .as_ref()
                .map(|judge| judge as &dyn crate::door::JudgeDoor),
            replayer: Some(&replayer),
        }
    };
    let records: Vec<RunRecord> = runs.made.iter().map(|made| made.record.clone()).collect();
    let graded = grade_all(setup.suite, &plan, records, doors).map_err(EvalError::from)?;
    let names = DoorNames {
        decision: doors.decision.map(DecisionDoor::describe),
        judge: doors.judge.map(crate::door::JudgeDoor::describe),
    };
    let identity = identity(setup, author, runs);
    Ok(conclude(
        setup.suite,
        &plan,
        graded,
        &identity,
        (&gate, &gate_file),
        &names,
    )?)
}

/// A fresh results directory under `base`: `<base>/<UTC timestamp>`.
///
/// # Errors
///
/// Returns the I/O error when it can't be made.
pub fn results_dir(base: &Path) -> std::io::Result<PathBuf> {
    let stamp = atif::document::stamp(atif::document::now_ms());
    std::fs::create_dir_all(base)?;
    for suffix in 0..1000 {
        let name = if suffix == 0 {
            stamp.clone()
        } else {
            format!("{stamp}-{suffix}")
        };
        let dir = base.join(name);
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other("no free results directory name"))
}

/// Writes the results directory: `report.json`, `report.html`, and
/// `artifacts/` from the engine; `runs/<case>/<arm>-<n>/` with each run's
/// trajectory, created files, and output; `suite/` with the case files'
/// exact bytes (what `publish` releases); and `run.json`. Then checks that
/// the door key reached none of it.
///
/// # Errors
///
/// Returns [`RunError::Io`] for a file that can't be written, and
/// [`RunError::Leak`] (after removing the directory) when the key is
/// found in it.
pub fn write_results(
    dir: &Path,
    setup: &Setup<'_>,
    author: &Author,
    evaluation: &Evaluation,
    runs: &Runs,
) -> Result<(), RunError> {
    let io = |error: std::io::Error| RunError::Io(error.to_string());
    evaluation.write(dir).map_err(io)?;
    for made in &runs.made {
        let record = &made.record;
        let run_dir = dir.join(run_path(&record.case, record.arm, record.attempt));
        std::fs::create_dir_all(&run_dir).map_err(io)?;
        if let Some(bytes) = &made.trajectory {
            std::fs::write(run_dir.join("trajectory.json"), bytes).map_err(io)?;
        }
        for (name, bytes) in &made.outputs {
            std::fs::write(run_dir.join(name), bytes).map_err(io)?;
        }
        std::fs::write(
            run_dir.join("created.json"),
            json_bytes(&json!(record.created_files)),
        )
        .map_err(io)?;
        for (path, bytes) in &made.created {
            let target = run_dir.join("files").join(path);
            if !target.starts_with(run_dir.join("files")) {
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            std::fs::write(target, bytes).map_err(io)?;
        }
    }
    write_suite_files(&dir.join("suite"), setup.suite).map_err(io)?;
    let record = json!({
        "v": RUN_SCHEMA,
        "requires": [],
        "author": author.author,
        "package": author.package,
        "component": author.component,
        "evaluator": author.evaluator,
        "subject": setup.subject.slug,
        "subject_definition": setup.subject.definition,
        "subject_lock": String::from_utf8_lossy(&setup.subject.lock_document(setup.agent)),
        "door": setup.door.label(),
        "agent": {"digest": setup.agent.digest, "size": setup.agent.size},
        "runs": setup.options.runs,
        "baseline": setup.options.baseline,
        "grants": setup.options.grants.iter().map(|grant| grant.word()).collect::<Vec<_>>(),
        "verdict": evaluation.verdict.word(),
        "partial": evaluation.partial,
    });
    std::fs::write(dir.join("run.json"), json_bytes(&record)).map_err(io)?;
    let fallback_keys = setup
        .decision
        .into_iter()
        .flat_map(|pin| pin.fallbacks.iter().map(|door| door.key().expose()));
    for secret in [
        Some(setup.door.key.expose()),
        setup.decision.map(|pin| pin.key.expose()),
    ]
    .into_iter()
    .flatten()
    .chain(fallback_keys)
    {
        if let Some(found) = sandbox::holds(dir, secret) {
            let _ = std::fs::remove_dir_all(dir);
            return Err(RunError::Leak(found.display().to_string()));
        }
    }
    Ok(())
}

/// Writes every case's files under `dir/<case path>/`, byte for byte.
///
/// # Errors
///
/// Returns the first I/O error.
pub fn write_suite_files(dir: &Path, suite: &Suite) -> std::io::Result<()> {
    for case in &suite.cases {
        let case_dir = dir.join(&case.path);
        std::fs::create_dir_all(&case_dir)?;
        std::fs::write(case_dir.join("prompt.md"), &case.files.prompt)?;
        if let Some(bytes) = &case.files.case_toml {
            std::fs::write(case_dir.join("case.toml"), bytes)?;
        }
        for (name, bytes) in &case.files.graders {
            let path = case_dir.join("graders").join(name);
            std::fs::create_dir_all(case_dir.join("graders"))?;
            std::fs::write(path, bytes)?;
        }
        for (path, bytes) in &case.files.fixtures {
            let target = case_dir.join("fixtures").join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, bytes)?;
        }
    }
    Ok(())
}

/// A finished suite run.
pub struct Outcome {
    /// The evaluation.
    pub evaluation: Evaluation,
    /// The results directory.
    pub results: PathBuf,
    /// The exit code: the signal's (130 or 143) when stopped by one,
    /// otherwise [`Evaluation::exit_code`].
    pub exit_code: i32,
    /// Run directories kept with `--keep-temp`.
    pub kept: Vec<PathBuf>,
}

/// Runs, grades, and writes a whole suite: what `openagents ext eval run`
/// and the hosted runner call.
///
/// # Errors
///
/// Returns [`RunError`] when the suite can't be run, concluded, or
/// written.
pub fn run_suite(
    setup: &Setup<'_>,
    author: &Author,
    results_base: &Path,
    decision: Option<&dyn DecisionDoor>,
    cancel: &Cancel,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Outcome, RunError> {
    let runs = execute(setup, cancel, progress)?;
    let evaluation = evaluate_runs(setup, author, &runs, decision)?;
    let results = results_dir(results_base).map_err(|error| RunError::Io(error.to_string()))?;
    write_results(&results, setup, author, &evaluation, &runs)?;
    let kept: Vec<PathBuf> = runs
        .made
        .into_iter()
        .filter_map(|made| made.sandbox)
        .filter(Sandbox::kept)
        .map(Sandbox::keep)
        .collect();
    let exit_code = cancel
        .exit_code()
        .filter(|_| runs.cancelled)
        .unwrap_or_else(|| evaluation.exit_code());
    Ok(Outcome {
        evaluation,
        results,
        exit_code,
        kept,
    })
}
