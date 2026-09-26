//! Frozen, source-separated comparisons of a knowledge snapshot with no base.
//!
//! This local NIP-OPT profile supports one fixed candidate and a declared paired
//! assignment schedule. It does not search, promote an entry, or execute a model.
//! The Microcoder runner owns dispatch and binds its receipts to these records.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Entry, digest};

pub const SCHEMA: &str = "openagents.kb-study.v1";
const MAX_BYTES: usize = 16 * 1024 * 1024;

/// Effective settings shared by both arms. The runner supports no hidden fallback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub provider: String,
    pub cost_basis: String,
    pub retrieval_mode: String,
    pub decision_base_url: String,
    pub decision_model: String,
    pub model: String,
    pub effort: String,
    pub strong_model: String,
    pub max_steps: u32,
    pub max_seconds: u64,
    pub max_usd: f64,
    pub command_seconds: u64,
    pub test_seconds: u64,
    pub prompt: String,
    pub network: String,
    pub acceptance: bool,
}

/// One task family in a frozen partition. Digests name exact retained manifests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub task: String,
    pub group: String,
    pub partition: Partition,
    pub workload_digest: String,
    pub environment_digest: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Partition {
    Development,
    Confirmation,
}

/// One fixed candidate, exact executable, bounded assignments, and source lineage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub study: String,
    pub owner: String,
    pub binary: PathBuf,
    pub binary_digest: String,
    pub tasks_root: PathBuf,
    pub candidate_digest: String,
    pub configuration: Configuration,
    pub cases: Vec<Case>,
    pub repetitions: u32,
    /// Fixed alternating assignment order; no outcome-dependent scheduling.
    pub first_subject: bool,
    /// All sources considered while constructing the candidate, beyond its entries.
    pub source_tasks: Vec<String>,
    pub max_total_usd: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arm {
    Subject,
    Baseline,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub id: String,
    pub task: String,
    pub repetition: u32,
    pub arm: Arm,
    pub partition: Partition,
}

/// One durable reservation. Its existence forbids dispatching that assignment again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Started {
    pub schema: String,
    pub plan_digest: String,
    pub assignment: Assignment,
    pub started_at_ms: u64,
    pub argv: Vec<String>,
    pub output: PathBuf,
}

/// A terminal attempt receipt. Unknown model costs remain unknown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finished {
    pub schema: String,
    pub plan_digest: String,
    pub assignment: String,
    pub started_digest: String,
    pub finished_at_ms: u64,
    pub exit_code: Option<i32>,
    pub ending: String,
    pub wall_seconds: f64,
    pub summary_digest: Option<String>,
    pub events_digest: Option<String>,
    pub reward: Option<f64>,
    pub costs: BTreeMap<String, Option<f64>>,
    pub known_cost_lower_bound_usd: f64,
    pub evidence: BTreeMap<String, String>,
    pub served_models: Vec<String>,
    pub problems: Vec<String>,
}

/// A single-owner study store. Losing the owner leaves started attempts unknown.
pub struct Store {
    pub root: PathBuf,
    pub plan: Plan,
    pub plan_digest: String,
    pub assignments: Vec<Assignment>,
    _owner: File,
}

fn fail(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    })
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    nostr::contracts::jcs(&serde_json::to_value(value).map_err(fail)?).map_err(fail)
}
fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(fail)?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
        return Err("study record must be a bounded regular file".into());
    }
    let value =
        nostr::contracts::parse_strict_bounded(&std::fs::read(path).map_err(fail)?, MAX_BYTES)
            .map_err(fail)?;
    serde_json::from_value(value).map_err(fail)
}
fn create(path: &Path, value: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(fail)?;
    file.write_all(value).map_err(fail)?;
    file.sync_all().map_err(fail)?;
    File::open(path.parent().ok_or("record has no parent")?)
        .map_err(fail)?
        .sync_all()
        .map_err(fail)
}

impl Plan {
    /// Check the supported fixed-candidate profile and source exclusions.
    ///
    /// # Errors
    /// Refuses unbounded settings, invalid pins, duplicate tasks, or partition leakage.
    pub fn validate(&self, documents: &[Entry]) -> Result<(), String> {
        if self.schema != SCHEMA
            || !id(&self.study)
            || self.owner.trim().is_empty()
            || !self.binary.is_absolute()
            || !self.tasks_root.is_absolute()
            || !valid_digest(&self.binary_digest)
            || !valid_digest(&self.candidate_digest)
            || !(1..=20).contains(&self.repetitions)
            || self.cases.is_empty()
            || self.cases.len() > 100
        {
            return Err("invalid study identity, pins, or assignment bounds".into());
        }
        let c = &self.configuration;
        if c.provider != "codex"
            || c.cost_basis != "list_price"
            || c.retrieval_mode != "lexical"
            || c.decision_base_url != "https://api.typesafe.ai"
            || c.decision_model.trim().is_empty()
            || c.model.is_empty()
            || c.effort.is_empty()
            || c.strong_model.is_empty()
            || c.prompt.is_empty()
            || c.network.is_empty()
            || c.max_steps == 0
            || c.max_steps > 1000
            || c.max_seconds == 0
            || c.max_seconds > 86400
            || c.command_seconds == 0
            || c.test_seconds == 0
            || !c.max_usd.is_finite()
            || c.max_usd <= 0.0
            || !self.max_total_usd.is_finite()
            || self.max_total_usd
                < c.max_usd * 2.0 * f64::from(self.repetitions) * self.cases.len() as f64
        {
            return Err("invalid study configuration or insufficient aggregate reservation".into());
        }
        let mut excluded: BTreeSet<String> = self
            .source_tasks
            .iter()
            .flat_map(|source| [source.clone(), crate::evidence::task_of(source)])
            .collect();
        for entry in documents {
            excluded.extend(
                entry
                    .written_from
                    .iter()
                    .filter(|source| source.as_str() != "reference")
                    .flat_map(|source| [source.clone(), crate::evidence::task_of(source)]),
            );
        }
        let mut tasks = BTreeSet::new();
        let mut groups = BTreeMap::new();
        for case in &self.cases {
            if !id(&case.task)
                || !id(&case.group)
                || !valid_digest(&case.workload_digest)
                || !valid_digest(&case.environment_digest)
                || !tasks.insert(&case.task)
            {
                return Err("invalid or repeated study task identity".into());
            }
            if case.partition == Partition::Confirmation && excluded.contains(&case.task) {
                return Err(format!(
                    "confirmation task {} contributed to the candidate",
                    case.task
                ));
            }
            if let Some(partition) = groups.insert(&case.group, case.partition)
                && partition != case.partition
            {
                return Err(format!(
                    "source group {} crosses study partitions",
                    case.group
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn assignments(&self) -> Vec<Assignment> {
        let mut assignments = Vec::new();
        for (case_index, case) in self.cases.iter().enumerate() {
            for repetition in 1..=self.repetitions {
                let subject_first =
                    self.first_subject ^ ((case_index + repetition as usize).is_multiple_of(2));
                let order = if subject_first {
                    [Arm::Subject, Arm::Baseline]
                } else {
                    [Arm::Baseline, Arm::Subject]
                };
                for arm in order {
                    assignments.push(Assignment {
                        id: format!("trial-{:04}", assignments.len() + 1),
                        task: case.task.clone(),
                        repetition,
                        arm,
                        partition: case.partition,
                    });
                }
            }
        }
        assignments
    }
}

impl Store {
    /// Freeze a new plan and exact snapshot before any assignment can begin.
    ///
    /// # Errors
    /// Existing directories, invalid snapshots, or source leakage refuse.
    pub fn freeze(root: &Path, plan: &Plan, candidate: &[u8]) -> Result<Self, String> {
        let value = nostr::contracts::parse_strict_bounded(candidate, MAX_BYTES).map_err(fail)?;
        let bundle: crate::snapshot::Bundle = serde_json::from_value(value).map_err(fail)?;
        let verified = crate::snapshot::verify(&bundle)?;
        plan.validate(&verified.base.entries)?;
        if digest(candidate) != plan.candidate_digest {
            return Err("candidate bytes do not match the frozen digest".into());
        }
        std::fs::create_dir(root).map_err(fail)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
        }
        create(&root.join("candidate.json"), candidate)?;
        create(&root.join("plan.json"), &bytes(plan)?)?;
        create(&root.join("assignments.json"), &bytes(&plan.assignments())?)?;
        create(
            &root.join("frozen.json"),
            &bytes(&json!({
                "schema":"openagents.kb-study-freeze.v1", "plan_digest":digest(&bytes(plan)?),
                "candidate_digest":plan.candidate_digest, "frozen_at_ms":now(),
                "profile":"NIP-OPT fixed candidate; no search, no automatic promotion",
                "cost_limit":"model-reported loop budget plus one-call overshoot; unknown charges are not zero",
            }))?,
        )?;
        Self::open(root)
    }

    /// Reopen a frozen study, taking its single execution-owner lock.
    ///
    /// # Errors
    /// Concurrent ownership, altered plans or assignments, or missing pins refuse.
    pub fn open(root: &Path) -> Result<Self, String> {
        let metadata = std::fs::symlink_metadata(root).map_err(fail)?;
        if !metadata.is_dir() {
            return Err("study root is not a directory".into());
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let owner_path = root.join("owner.lock");
        if let Ok(metadata) = std::fs::symlink_metadata(&owner_path)
            && !metadata.is_file()
        {
            return Err("study owner lock is not a regular file".into());
        }
        let owner = options.open(owner_path).map_err(fail)?;
        owner
            .try_lock()
            .map_err(|_| "study already has an execution owner".to_string())?;
        let plan: Plan = read(&root.join("plan.json"))?;
        let frozen: Value = read(&root.join("frozen.json"))?;
        let plan_digest = digest(&bytes(&plan)?);
        if frozen["plan_digest"] != plan_digest
            || frozen["candidate_digest"] != plan.candidate_digest
            || digest(&std::fs::read(root.join("candidate.json")).map_err(fail)?)
                != plan.candidate_digest
        {
            return Err("frozen study identity changed".into());
        }
        let verified = crate::snapshot::read(&root.join("candidate.json"))?;
        plan.validate(&verified.base.entries)?;
        let assignments: Vec<Assignment> = read(&root.join("assignments.json"))?;
        if assignments != plan.assignments() {
            return Err("frozen assignments changed".into());
        }
        Ok(Self {
            root: root.canonicalize().map_err(fail)?,
            plan,
            plan_digest,
            assignments,
            _owner: owner,
        })
    }

    /// Reserve one assignment durably. A started assignment is never dispatched twice.
    ///
    /// # Errors
    /// Unknown assignment IDs, duplicate starts, and an early confirmation refuse.
    pub fn begin(&self, assignment: &Assignment, argv: Vec<String>) -> Result<Started, String> {
        if !self.assignments.contains(assignment) {
            return Err("assignment is not in the frozen schedule".into());
        }
        let index = self
            .assignments
            .iter()
            .position(|a| a == assignment)
            .ok_or("assignment missing")?;
        // Sequence is frozen. A crash remains a visible unknown and blocks automatic continuation.
        if self.results()?[..index]
            .iter()
            .any(|row| row["state"] != "finished")
        {
            return Err(
                "an earlier assignment is unfinished; retain and resolve it before continuing"
                    .into(),
            );
        }
        let started = Started {
            schema: "openagents.kb-study-start.v1".into(),
            plan_digest: self.plan_digest.clone(),
            assignment: assignment.clone(),
            started_at_ms: now(),
            argv,
            output: self.root.join(format!("{}.run", assignment.id)),
        };
        create(
            &self.root.join(format!("{}.start.json", assignment.id)),
            &bytes(&started)?,
        )?;
        Ok(started)
    }

    /// Seal one result under its exact start receipt, preserving unknowns.
    ///
    /// # Errors
    /// Replacement results, a mismatched plan/start, or invalid metrics refuse.
    pub fn finish(&self, finished: &Finished) -> Result<(), String> {
        if !self
            .assignments
            .iter()
            .any(|assignment| assignment.id == finished.assignment)
        {
            return Err("result assignment is not in the frozen study".into());
        }
        let started: Started = read(
            &self
                .root
                .join(format!("{}.start.json", finished.assignment)),
        )?;
        self.check_started(&started)?;
        self.check_finished(&started, finished)?;
        self.check_evidence(&started, finished)?;
        create(
            &self
                .root
                .join(format!("{}.finish.json", finished.assignment)),
            &bytes(finished)?,
        )
    }

    fn check_started(&self, started: &Started) -> Result<(), String> {
        if started.schema != "openagents.kb-study-start.v1"
            || started.plan_digest != self.plan_digest
            || !self.assignments.contains(&started.assignment)
            || started.output != self.root.join(format!("{}.run", started.assignment.id))
        {
            return Err("start receipt does not match the frozen assignment".into());
        }
        Ok(())
    }

    fn check_finished(&self, started: &Started, finished: &Finished) -> Result<(), String> {
        if finished.schema != "openagents.kb-study-finish.v1"
            || finished.plan_digest != self.plan_digest
            || finished.assignment != started.assignment.id
            || !self.assignments.contains(&started.assignment)
            || started.plan_digest != self.plan_digest
            || finished.started_digest != digest(&bytes(started)?)
            || finished.finished_at_ms < started.started_at_ms
            || !finished.wall_seconds.is_finite()
            || finished.wall_seconds < 0.0
            || !finished.known_cost_lower_bound_usd.is_finite()
            || finished.known_cost_lower_bound_usd < 0.0
            || ["model_usd", "jev_usd", "embedding_usd"]
                .iter()
                .any(|key| !finished.costs.contains_key(*key))
            || finished
                .reward
                .is_some_and(|r| !r.is_finite() || !(0.0..=1.0).contains(&r))
            || finished
                .costs
                .values()
                .flatten()
                .any(|cost| !cost.is_finite() || *cost < 0.0)
        {
            return Err(
                "result does not bind the started assignment or has invalid metrics".into(),
            );
        }
        let subtotal: f64 = finished.costs.values().flatten().sum();
        if !subtotal.is_finite()
            || finished.known_cost_lower_bound_usd + 1e-9 < subtotal
            || (finished.costs.values().all(Option::is_some)
                && (finished.known_cost_lower_bound_usd - subtotal).abs() > 1e-9)
        {
            return Err("cost lower bound contradicts the recorded components".into());
        }
        Ok(())
    }

    fn check_evidence(&self, started: &Started, finished: &Finished) -> Result<(), String> {
        for (name, expected) in [
            ("summary.json", &finished.summary_digest),
            ("events.jsonl", &finished.events_digest),
        ] {
            if let Some(expected) = expected
                && file_digest(&started.output.join(name))? != *expected
            {
                return Err(format!("retained {name} does not match the result"));
            }
        }
        for (name, expected) in &finished.evidence {
            if !["stdout.txt", "stderr.txt", "cleanup.json"].contains(&name.as_str()) {
                return Err("unknown result evidence path".into());
            }
            if file_digest(&self.root.join(format!("{}.{}", finished.assignment, name)))?
                != *expected
            {
                return Err(format!("retained {name} does not match the result"));
            }
        }
        Ok(())
    }

    /// Complete assigned population, including not-started and interrupted attempts.
    ///
    /// # Errors
    /// A changed or unreadable receipt refuses; it never disappears from the population.
    pub fn results(&self) -> Result<Vec<Value>, String> {
        let mut results = Vec::new();
        for assignment in &self.assignments {
            let start = self.root.join(format!("{}.start.json", assignment.id));
            let finish = self.root.join(format!("{}.finish.json", assignment.id));
            let started: Option<Started> = if start.exists() {
                Some(read(&start)?)
            } else {
                None
            };
            let finished: Option<Finished> = if finish.exists() {
                Some(read(&finish)?)
            } else {
                None
            };
            if let Some(start) = &started {
                self.check_started(start)?;
                if start.assignment != *assignment {
                    return Err("start receipt assignment differs from its path".into());
                }
            }
            if let Some(result) = &finished {
                let start = started.as_ref().ok_or("result without start receipt")?;
                if result.plan_digest != self.plan_digest
                    || result.assignment != assignment.id
                    || start.assignment != *assignment
                    || start.plan_digest != self.plan_digest
                    || result.started_digest != digest(&bytes(start)?)
                {
                    return Err("result chain does not match the frozen study".into());
                }
                self.check_finished(start, result)?;
                self.check_evidence(start, result)?;
            }
            let state = if finished.is_some() {
                "finished"
            } else if started.is_some() {
                "unknown"
            } else {
                "not_started"
            };
            results.push(json!({"assignment":assignment, "started":started, "finished":finished, "state":state}));
        }
        Ok(results)
    }
}
/// Canonical identity of a retained start receipt.
///
/// # Errors
/// Serialization failures refuse.
pub fn started_digest(started: &Started) -> Result<String, String> {
    Ok(digest(&bytes(started)?))
}

/// SHA-256 of an exact regular file, streamed without loading an executable in memory.
///
/// # Errors
/// Nonregular, missing, or unreadable files refuse.
pub fn file_digest(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    if !std::fs::symlink_metadata(path).map_err(fail)?.is_file() {
        return Err("pin is not a regular file".into());
    }
    let mut file = File::open(path).map_err(fail)?;
    let mut hash = Sha256::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut chunk).map_err(fail)?;
        if count == 0 {
            break;
        }
        hash.update(&chunk[..count]);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

/// Digest of a bounded directory's relative names and exact file digests.
///
/// # Errors
/// Symlinks, unreadable paths, or more than 10000 files refuse.
pub fn tree_digest(root: &Path) -> Result<String, String> {
    fn walk(root: &Path, at: &Path, files: &mut BTreeMap<String, String>) -> Result<(), String> {
        if !std::fs::symlink_metadata(at).map_err(fail)?.is_dir() {
            return Err("tree pin is not a directory".into());
        }
        for entry in std::fs::read_dir(at).map_err(fail)? {
            let entry = entry.map_err(fail)?;
            let kind = entry.file_type().map_err(fail)?;
            if kind.is_dir() {
                walk(root, &entry.path(), files)?;
            } else if kind.is_file() {
                if files.len() >= 10000 {
                    return Err("tree pin exceeds 10000 files".into());
                }
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .map_err(fail)?
                    .to_str()
                    .ok_or("non-UTF-8 tree path")?
                    .to_string();
                files.insert(relative, file_digest(&path)?);
            } else {
                return Err("tree pin contains a nonregular path".into());
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files)?;
    Ok(digest(&bytes(&files)?))
}

#[cfg(test)]
mod tests;
