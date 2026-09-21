//! A single durable writer refills admitted Coder work and waits for host review.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coder::Survey;
use coder_scheduler::{
    catalog::{Catalog, Footprint, Task},
    ledger::Ledger,
    plan::{self, Policy, Status},
    resources::{Capacity, Resources},
};
use futures_util::{StreamExt, stream::FuturesUnordered};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Assignment, dispatch, github, protect_evidence};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    pub scheduling: Task,
    pub assignment: Assignment,
    pub issue_updated: String,
    pub issue_body_digest: String,
    pub tracker_base: String,
}

impl Prepared {
    #[must_use]
    pub fn input_digest(&self) -> String {
        atif::digest(
            &json!({"assignment":self.assignment,"issue_updated":self.issue_updated,"issue_body_digest":self.issue_body_digest,"tracker_base":self.tracker_base}),
        )
    }
}

/// Host-owned configuration. It must be outside every executor write grant.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub v: u32,
    pub repository: PathBuf,
    pub project: github::Scope,
    pub capacity: Capacity,
    pub external_reservation: Resources,
    pub excluded_issues: BTreeSet<u64>,
    #[serde(default)]
    pub external_owners: Vec<plan::Exclusion>,
    pub accepted_closed_issues: BTreeSet<u64>,
    pub review_cap: u32,
    pub dispatch_limit: u32,
    pub admission_minutes: u64,
    pub poll_seconds: u64,
    /// How long a task whose attempt ended on an executor capacity
    /// refusal stays unadmitted — the host's stated delay before the
    /// task is offered again, not a guess at the provider's clock.
    /// Default is five minutes; zero asks for a hot retry and is
    /// refused.
    #[serde(default = "default_quota_backoff_seconds")]
    pub quota_backoff_seconds: u64,
    pub tasks: Vec<Prepared>,
}

/// The default capacity-refusal delay — five minutes. Deliberately not
/// zero: a hot retry against a full lane or a quota is exactly the
/// retry storm the bound exists to prevent.
fn default_quota_backoff_seconds() -> u64 {
    300
}

/// The refusal causes that mean the executor could not take the work
/// now — a full lane, a quota, a rate limit, a door that went away —
/// as the words the refusal or execution status actually carries. A
/// refusal matching none of these is a wrong-answer or authority
/// problem and still lands in review for a human.
const CAPACITY_WORDS: &[&str] = &["quota", "rate", "busy", "capacity", "door_unavailable"];

/// Whether a dispatch report is an executor capacity refusal — the
/// attempt ran nothing, so requeueing under a stated delay is the
/// safe, declared no-effect retry. Returns the matched status text as
/// the recorded cause.
fn capacity_cause(report: &crate::DispatchReport) -> Option<String> {
    [
        report.execution_status.as_deref(),
        report.refusal.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find(|status| {
        CAPACITY_WORDS
            .iter()
            .any(|word| status.to_lowercase().contains(word))
    })
    .map(str::to_string)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("supervisor input exceeds 4 MiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn write_new(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

impl Configuration {
    pub fn load(path: &Path) -> Result<Self, String> {
        let configuration: Self = read_json(path)?;
        configuration.validate()?;
        Ok(configuration)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.project.validate()?;
        if self.v != 1 || !self.repository.is_absolute() || self.tasks.len() > 256 {
            return Err("supervisor requires version 1, an absolute repository, and at most 256 prepared tasks".into());
        }
        if !(1..=64).contains(&self.capacity.executor_slots)
            || !(1..=128).contains(&self.review_cap)
            || !(1..=1000).contains(&self.dispatch_limit)
            || !(1..=720).contains(&self.admission_minutes)
            || !(10..=3600).contains(&self.poll_seconds)
            || !(1..=86400).contains(&self.quota_backoff_seconds)
            || self.capacity.integration_lanes != 1
        {
            return Err(
                "supervisor capacity, review, dispatch, polling, or wall bounds are invalid".into(),
            );
        }
        if self.external_reservation.quiet_host || self.external_reservation.integration {
            return Err("external reservations must describe ordinary occupied capacity".into());
        }
        for owner in &self.external_owners {
            if owner.owner.trim().is_empty() || owner.writes.is_empty() {
                return Err("external ownership needs an owner and declared paths".into());
            }
            for path in &owner.writes {
                crate::artifact::relative(path)?;
            }
        }
        for prepared in &self.tasks {
            prepared.assignment.validate()?;
            let task = &prepared.scheduling;
            if task.id != prepared.assignment.id
                || task.base != prepared.assignment.base
                || task.input != prepared.input_digest()
                || task.resources.executor_slots != 1
                || task.resources.integration
                || self.excluded_issues.contains(&task.issue)
                || prepared.issue_updated.is_empty()
                || prepared.issue_body_digest.is_empty()
                || prepared.tracker_base.len() != 40
                || !prepared.tracker_base.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(format!(
                    "task {} is excluded or its pinned assignment does not match its scheduling record",
                    task.id
                ));
            }
            if let Footprint::Declared { writes, .. } = &task.footprint
                && prepared.assignment.writes == writes.is_empty()
            {
                return Err(format!("task {} has inconsistent write authority", task.id));
            }
        }
        self.catalog()?.validate().map_err(|e| e.to_string())
    }

    fn catalog(&self) -> Result<Catalog, String> {
        let mut tasks: Vec<_> = self.tasks.iter().map(|p| p.scheduling.clone()).collect();
        if self.external_reservation != Resources::default() {
            tasks.push(Task {
                id: "external-owner-reservation".into(),
                issue: u64::MAX,
                base: "external".into(),
                input: "operator-owned".into(),
                depends_on: vec![],
                footprint: Footprint::Declared {
                    reads: vec![],
                    writes: vec![],
                },
                priority: 0,
                resources: self.external_reservation,
                estimate_ticks: 1,
            });
        }
        let mut catalog = Catalog::new("operator-scoped-project", tasks);
        catalog.seal().map_err(|e| e.to_string())?;
        Ok(catalog)
    }
}

/// Pin host-prepared tasks to a fresh tracker observation and the local commit.
/// The caller reviews the template's authority and supplies a new output path.
pub async fn pin_configuration(input: &Path, output: &Path) -> Result<(), String> {
    let mut configuration: Configuration = read_json(input)?;
    configuration.repository = configuration
        .repository
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let survey = Survey::read(Some(&configuration.repository), &configuration.repository);
    protect_evidence(&configuration.repository, output, &survey)?;
    let snapshot = github::fetch(&configuration.project, &configuration.repository).await?;
    let base = crate::git(&configuration.repository, &["rev-parse", "HEAD"])
        .await?
        .trim()
        .to_string();
    crate::git(
        &configuration.repository,
        &[
            "merge-base",
            "--is-ancestor",
            &snapshot.default_branch_revision,
            &base,
        ],
    )
    .await
    .map_err(|_| {
        "fetch and integrate the current default branch before pinning project work".to_string()
    })?;
    for prepared in &mut configuration.tasks {
        let issue = snapshot
            .issues
            .get(&prepared.scheduling.issue)
            .ok_or("prepared task is absent from the scoped project")?;
        prepared.issue_updated = issue.updated_at.clone();
        prepared.issue_body_digest = issue.body_digest.clone();
        prepared.tracker_base = snapshot.default_branch_revision.clone();
        prepared.assignment.base = base.clone();
        prepared.scheduling.base = base.clone();
        prepared.scheduling.input = prepared.input_digest();
    }
    configuration.validate()?;
    write_new(output, &configuration)
}

/// A reviewed host decision placed in the protected control directory.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub task: String,
    pub attempt: String,
    pub task_digest: String,
    pub result_digest: String,
    pub accepted: bool,
    pub evidence: String,
}

fn consume_reviews(state: &Path, ledger: &mut Ledger) -> Result<(), String> {
    let mut paths: Vec<_> = std::fs::read_dir(state.join("control"))
        .map_err(|e| e.to_string())?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    paths.sort();
    for path in paths {
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        if std::fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("host review records cannot be symlinks".into());
        }
        let review: Review = read_json(&path)?;
        let record = ledger
            .record(&review.task)
            .ok_or("review names an unknown task")?
            .clone();
        if review.evidence.trim().is_empty()
            || record.attempt != review.attempt
            || record.task_digest != review.task_digest
            || record.result_digest.as_deref() != Some(&review.result_digest)
        {
            return Err(
                "review does not match the recorded attempt, task, result, and evidence".into(),
            );
        }
        if record.status == Status::Review {
            if review.accepted {
                ledger
                    .accept(
                        &review.task,
                        &review.attempt,
                        &record.owner,
                        &review.task_digest,
                    )
                    .map_err(|e| e.to_string())?;
            } else {
                ledger
                    .reject(
                        &review.task,
                        &review.attempt,
                        &record.owner,
                        &review.evidence,
                    )
                    .map_err(|e| e.to_string())?;
            }
        } else if record.status
            != if review.accepted {
                Status::Completed
            } else {
                Status::Rejected
            }
        {
            return Err("review is incompatible with the current task state".into());
        }
        let name = path.file_name().ok_or("control record has no file name")?;
        std::fs::rename(&path, state.join("reviewed").join(name)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn tracker_block(
    configuration: &Configuration,
    prepared: &Prepared,
    snapshot: &github::Snapshot,
) -> Option<String> {
    if snapshot.default_branch_revision != prepared.tracker_base {
        return Some("default branch changed; integrate and repin queued work".into());
    }
    let Some(issue) = snapshot.issues.get(&prepared.scheduling.issue) else {
        return Some("issue is absent from the scoped project".into());
    };
    if issue.closed {
        return Some("issue is closed".into());
    }
    if issue.updated_at != prepared.issue_updated {
        return Some("issue version changed; refresh its prepared assignment".into());
    }
    if issue.body_digest != prepared.issue_body_digest {
        return Some("issue content changed; refresh its prepared assignment".into());
    }
    for blocker in &issue.blockers {
        if blocker.repository != configuration.project.repository_name() || !blocker.closed {
            return Some(format!(
                "open or out-of-scope prerequisite {}#{}",
                blocker.repository, blocker.number
            ));
        }
        if !configuration
            .accepted_closed_issues
            .contains(&blocker.number)
        {
            return Some(format!(
                "closed prerequisite #{} still needs its acceptance evidence recorded",
                blocker.number
            ));
        }
    }
    None
}

/// One admission round's answer: the plan, the status view it was drawn
/// from, and the cause every kept-out task names.
struct Round {
    plan: plan::Plan,
    states: BTreeMap<String, Status>,
    reasons: BTreeMap<String, String>,
}

/// Decide one round's admissions.
///
/// The tracker answer filters queued tasks before the planner sees
/// them: a failed fetch blocks everything it cannot observe, an issue
/// the fetch did not carry blocks its task, and a present-but-moved
/// issue fails its pins through [`tracker_block`]. The planner then
/// applies dependencies, footprints, exclusions, capacity, and the
/// review cap over what remains. Every task the round does not admit
/// lands in `reasons`, so the round snapshot records a cause rather
/// than a silence.
fn plan_round(
    configuration: &Configuration,
    catalog: &Catalog,
    ledger: &Ledger,
    snapshot: &Result<github::Snapshot, String>,
) -> Round {
    let mut reasons = BTreeMap::<String, String>::new();
    let mut states = ledger.statuses();
    states.insert("external-owner-reservation".into(), Status::Active);
    let mut eligible = catalog.clone();
    eligible.tasks.retain(|task| {
        if task.id == "external-owner-reservation" || states.get(&task.id) != Some(&Status::Queued)
        {
            return true;
        }
        let prepared = configuration
            .tasks
            .iter()
            .find(|p| p.scheduling.id == task.id)
            .expect("catalog contains prepared tasks");
        let now = atif::now_ms() / 1000;
        let reason = match ledger.record(&task.id).and_then(|r| r.backoff_until) {
            Some(until) if until > now => Some(format!("executor capacity backoff until {until}")),
            _ => match snapshot {
                Err(error) => Some(error.clone()),
                Ok(snapshot) if !snapshot.issues.contains_key(&task.issue) => {
                    Some("issue is no longer visible in the scoped project".into())
                }
                Ok(snapshot) => tracker_block(configuration, prepared, snapshot),
            },
        };
        if let Some(reason) = reason {
            reasons.insert(task.id.clone(), reason);
            false
        } else {
            true
        }
    });
    let policy = Policy {
        review_cap: configuration.review_cap,
    };
    let completed: BTreeSet<_> = states
        .iter()
        .filter(|(_, status)| **status == Status::Completed)
        .map(|(id, _)| id.clone())
        .collect();
    let plan = plan::select(&plan::Input {
        catalog: &eligible,
        capacity: &configuration.capacity,
        states: &states,
        externally_completed: &completed,
        exclusions: &configuration.external_owners,
        policy: &policy,
    });
    for blocked in &plan.blocked {
        reasons.insert(
            blocked.task.clone(),
            blocked
                .reasons
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
    Round {
        plan,
        states,
        reasons,
    }
}

/// Watch a project and dispatch only prepared, current, unblocked work.
/// The ledger lock is held until every local dispatch has settled.
pub async fn run(configuration_path: &Path, state: &Path, watch: bool) -> Result<(), String> {
    let initial = Configuration::load(configuration_path)?;
    if !state.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(state)
            .map_err(|e| e.to_string())?;
    }
    let survey = Survey::read(Some(&initial.repository), &initial.repository);
    let executor_limit = survey
        .capability("devin-local")
        .and_then(|found| found.manifest.concurrent_max)
        .filter(|limit| *limit > 0)
        .ok_or("the executor has no declared concurrency limit")?;
    if !coder::program_authority::Grant::operator(None).authorizes("project-task") {
        return Err("project-task requires an explicit CODER_PROGRAMS grant".into());
    }
    protect_evidence(&initial.repository, &state.join("ledger"), &survey)?;
    protect_evidence(&initial.repository, configuration_path, &survey)?;
    for directory in ["control", "reviewed", "attempts", "snapshots"] {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(state.join(directory))
            .map_err(|e| e.to_string())?;
    }
    let mut ledger = Ledger::open(state).map_err(|e| e.to_string())?;
    let owner = format!(
        "project-supervisor-{}-{}",
        std::process::id(),
        atif::now_ms()
    );
    let mut jobs = FuturesUnordered::new();
    let started = Instant::now();
    let deadline = Duration::from_secs(initial.admission_minutes * 60);
    let mut launched = 0;
    let mut round = 0_u64;
    let mut stop = false;
    loop {
        consume_reviews(state, &mut ledger)?;
        let configuration = Configuration::load(configuration_path)?;
        if configuration.capacity.executor_slots > executor_limit {
            return Err("project capacity exceeds the approved executor's declared limit".into());
        }
        if configuration.repository != initial.repository
            || configuration.project.repository_name() != initial.project.repository_name()
            || configuration.project.project != initial.project.project
        {
            return Err("a running supervisor cannot change repository or project scope".into());
        }
        let catalog = configuration.catalog()?;
        ledger.register(&catalog).map_err(|e| e.to_string())?;
        let drift = ledger.drift(&catalog);
        let missing = ledger.records().iter().any(|(id, record)| {
            matches!(
                record.status,
                Status::Active | Status::Unknown | Status::Review
            ) && catalog.task(id).is_none()
        });
        if missing || !drift.is_empty() {
            return Err("pinned in-flight or reviewed tasks changed or disappeared; restore their catalog before resuming".into());
        }
        if started.elapsed() >= deadline || launched >= initial.dispatch_limit {
            stop = true;
        }
        let snapshot = if !stop {
            github::fetch(&configuration.project, &configuration.repository).await
        } else {
            Err("supervisor admission bound reached; draining active work".into())
        };
        let Round {
            plan,
            states,
            reasons,
        } = plan_round(&configuration, &catalog, &ledger, &snapshot);
        round += 1;
        let unprepared: Vec<_> = snapshot
            .as_ref()
            .ok()
            .into_iter()
            .flat_map(|s| s.issues.values())
            .filter(|issue| {
                !issue.closed
                    && !configuration.excluded_issues.contains(&issue.number)
                    && !configuration
                        .tasks
                        .iter()
                        .any(|p| p.scheduling.issue == issue.number)
            })
            .map(|issue| issue.number)
            .collect();
        write_new(
            &state
                .join("snapshots")
                .join(format!("{owner}-{round:06}.json")),
            &json!({"round":round,"states":states,"blocked":reasons,"unprepared_issues":unprepared,"excluded_issues":configuration.excluded_issues,"snapshot":snapshot.as_ref().ok(),"error":snapshot.as_ref().err()}),
        )?;
        for admission in plan.admit {
            if stop || launched >= initial.dispatch_limit {
                break;
            }
            let prepared = configuration
                .tasks
                .iter()
                .find(|p| p.scheduling.id == admission.task)
                .ok_or("planner admitted an unprepared task")?;
            let attempt = ledger
                .claim(&admission.task, &owner, &prepared.scheduling.digest())
                .map_err(|e| e.to_string())?;
            let directory = state.join("attempts").join(&attempt);
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .map_err(|e| e.to_string())?;
            write_new(&directory.join("assignment.json"), &prepared)?;
            let repository = configuration.repository.clone();
            let assignment = prepared.assignment.clone();
            let survey = survey.clone();
            let id = admission.task;
            let owned = owner.clone();
            eprintln!("dispatch {id} attempt {attempt}");
            jobs.push(async move {
                let result = dispatch(
                    &repository,
                    survey,
                    &assignment,
                    &directory.join("trace.atif.jsonl"),
                )
                .await;
                (id, attempt, owned, directory, result)
            });
            launched += 1;
        }
        if jobs.is_empty() && (!watch || stop) {
            break;
        }
        tokio::select! {
            result = jobs.next(), if !jobs.is_empty() => {
                if let Some((id, attempt, owner, directory, result)) = result {
                    let value = match &result { Ok(report) => json!(report), Err(error) => json!({"error":error,"artifact_verified":false}) };
                    write_new(&directory.join("result.json"), &value)?;
                    let result_digest = atif::digest(&value);
                    let capacity = result.as_ref().ok().and_then(capacity_cause);
                    if let Some(cause) = capacity {
                        // The executor refused for capacity — the attempt
                        // ran nothing, so it requeues under the stated
                        // backoff rather than waiting on human review or
                        // retrying hot.
                        let until = atif::now_ms() / 1000 + configuration.quota_backoff_seconds;
                        ledger.backoff(&id, &attempt, &owner, &result_digest, until).map_err(|e| e.to_string())?;
                        eprintln!("result {id} attempt {attempt}: capacity refusal ({cause}); requeued under backoff until {until}");
                    } else {
                        ledger.settle(&id, &attempt, &owner, &result_digest).map_err(|e| e.to_string())?;
                        eprintln!("result {id} attempt {attempt}: pending independent review");
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(configuration.poll_seconds)) => {},
            _ = tokio::signal::ctrl_c() => { stop = true; eprintln!("stop requested; draining active attempts and preserving results"); },
        }
    }
    Ok(())
}

/// The adversarial exercise matrix: the controller's dispatch hazards
/// run against the real admission, ledger, and review paths. The matrix
/// exists so these failures are found here, not in a live run.
///
/// `run` itself needs a live tracker and an approved executor, so the
/// tests drive the same seams the loop drives: [`plan_round`] for the
/// admission decision, `Ledger` for the durable record, and
/// `consume_reviews` for host decisions. Two hazards have no direct
/// expression at this layer and are simulated at their real mechanism:
/// process death is a dropped ledger writer, because reopening is the
/// recovery a crashed run actually gets, and a second coordinator is a
/// second thread on the same state directory, because the
/// cross-process guard is the same OS file lock either way.
#[cfg(test)]
mod tests {
    use super::*;
    use coder_scheduler::ledger::LedgerError;
    use coder_scheduler::plan::Reason;
    use coder_scheduler::resources::Bound;

    fn prepared_named(id: &str, issue: u64) -> Prepared {
        let assignment = Assignment {
            id: id.into(),
            base: "a".repeat(40),
            prompt: format!("Inspect {id}'s files."),
            writes: false,
            expected_text: Some("done".into()),
            minutes: 1,
        };
        let scheduling = Task {
            id: assignment.id.clone(),
            issue,
            base: assignment.base.clone(),
            input: String::new(),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec![format!("src/{id}.rs")],
                writes: vec![],
            },
            priority: 1,
            resources: Resources {
                executor_slots: 1,
                cpu_units: 1,
                memory_mib: 256,
                quiet_host: false,
                integration: false,
            },
            estimate_ticks: 1,
        };
        let mut prepared = Prepared {
            scheduling,
            assignment,
            issue_updated: format!("version-{issue}"),
            issue_body_digest: format!("body-{issue}"),
            tracker_base: "a".repeat(40),
        };
        prepared.scheduling.input = prepared.input_digest();
        prepared
    }

    fn prepared() -> Prepared {
        prepared_named("task-a", 9507)
    }

    fn configuration() -> Configuration {
        Configuration {
            v: 1,
            repository: PathBuf::from("/operator/repository"),
            project: github::Scope {
                owner: "Example".into(),
                repository: "public".into(),
                project: 16,
            },
            capacity: Capacity {
                executor_slots: 6,
                cpu_units: 8,
                memory_mib: 8192,
                integration_lanes: 1,
            },
            external_reservation: Resources::default(),
            excluded_issues: BTreeSet::from([9476]),
            external_owners: vec![],
            accepted_closed_issues: BTreeSet::new(),
            review_cap: 3,
            dispatch_limit: 10,
            admission_minutes: 30,
            poll_seconds: 30,
            quota_backoff_seconds: 300,
            tasks: vec![prepared()],
        }
    }

    fn issue(number: u64) -> github::Issue {
        github::Issue {
            number,
            updated_at: format!("version-{number}"),
            body_digest: format!("body-{number}"),
            closed: false,
            blockers: vec![],
        }
    }

    fn snapshot() -> github::Snapshot {
        github::Snapshot {
            issues: BTreeMap::from([(9507, issue(9507))]),
            source_digest: "snapshot".into(),
            default_branch_revision: "a".repeat(40),
            ignored_non_issues: 0,
            ignored_other_repositories: 0,
        }
    }

    fn ledger_at(state: &Path, configuration: &Configuration) -> (Ledger, Catalog) {
        let catalog = configuration.catalog().unwrap();
        let mut ledger = Ledger::open(state).unwrap();
        ledger.register(&catalog).unwrap();
        (ledger, catalog)
    }

    fn control_dirs(state: &Path) {
        for name in ["control", "reviewed"] {
            std::fs::create_dir(state.join(name)).unwrap();
        }
    }

    #[test]
    fn exclusions_and_changed_inputs_cannot_dispatch() {
        let mut configuration = configuration();
        configuration.validate().unwrap();
        configuration.tasks[0].scheduling.issue = 9476;
        assert!(configuration.validate().is_err());
        configuration = self::configuration();
        configuration.tasks[0]
            .assignment
            .prompt
            .push_str(" changed");
        assert!(configuration.validate().is_err());
        configuration = self::configuration();
        configuration.tasks[0].issue_body_digest = "new body".into();
        assert!(configuration.validate().is_err());
    }

    #[test]
    fn tracker_closure_is_not_acceptance_and_missing_items_are_blocked() {
        let mut configuration = configuration();
        let prepared = prepared();
        let mut snapshot = snapshot();
        assert!(tracker_block(&configuration, &prepared, &snapshot).is_none());
        snapshot.default_branch_revision = "b".repeat(40);
        assert!(
            tracker_block(&configuration, &prepared, &snapshot)
                .unwrap()
                .contains("default branch")
        );
        snapshot.default_branch_revision = "a".repeat(40);
        snapshot
            .issues
            .get_mut(&9507)
            .unwrap()
            .blockers
            .push(github::Blocker {
                repository: "Example/public".into(),
                number: 9427,
                closed: true,
            });
        assert!(
            tracker_block(&configuration, &prepared, &snapshot)
                .unwrap()
                .contains("acceptance evidence")
        );
        configuration.accepted_closed_issues.insert(9427);
        assert!(tracker_block(&configuration, &prepared, &snapshot).is_none());
        snapshot.issues.get_mut(&9507).unwrap().blockers[0].closed = false;
        assert!(tracker_block(&configuration, &prepared, &snapshot).is_some());
        snapshot.issues.clear();
        assert!(tracker_block(&configuration, &prepared, &snapshot).is_some());
    }

    #[test]
    fn a_review_must_bind_the_attempt_and_exact_result() {
        let state = tempfile::tempdir().unwrap();
        for name in ["control", "reviewed"] {
            std::fs::create_dir(state.path().join(name)).unwrap();
        }
        let configuration = configuration();
        let catalog = configuration.catalog().unwrap();
        let task = catalog.task("task-a").unwrap();
        let mut ledger = Ledger::open(state.path()).unwrap();
        ledger.register(&catalog).unwrap();
        let attempt = ledger
            .claim(&task.id, "supervisor", &task.digest())
            .unwrap();
        ledger
            .settle(&task.id, &attempt, "supervisor", "actual-result")
            .unwrap();
        let review = Review {
            task: task.id.clone(),
            attempt,
            task_digest: task.digest(),
            result_digest: "stale-result".into(),
            accepted: true,
            evidence: "independent test record".into(),
        };
        let path = state.path().join("control/review.json");
        write_new(&path, &review).unwrap();
        assert!(consume_reviews(state.path(), &mut ledger).is_err());
        assert_eq!(ledger.record(&task.id).unwrap().status, Status::Review);
        std::fs::remove_file(&path).unwrap();
        write_new(
            &path,
            &Review {
                result_digest: "actual-result".into(),
                ..review
            },
        )
        .unwrap();
        consume_reviews(state.path(), &mut ledger).unwrap();
        assert_eq!(ledger.record(&task.id).unwrap().status, Status::Completed);
        assert!(state.path().join("reviewed/review.json").exists());
    }

    #[test]
    fn externally_owned_or_excluded_work_never_dispatches() {
        // An excluded issue cannot even stay in the configuration.
        let mut configuration = configuration();
        configuration.tasks[0].scheduling.issue = 9476;
        let error = configuration.validate().unwrap_err();
        assert!(error.contains("excluded"), "{error}");

        // An externally owned path refuses at the plan however free the
        // host is — a free slot is not authority.
        let mut configuration = self::configuration();
        configuration.external_owners = vec![plan::Exclusion {
            owner: "root-integration".into(),
            writes: vec!["src".into()],
        }];
        configuration.validate().unwrap();
        let state = tempfile::tempdir().unwrap();
        let (ledger, catalog) = ledger_at(state.path(), &configuration);
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot()));
        assert!(round.plan.admit.is_empty());
        assert!(
            round.reasons["task-a"].contains("owned by `root-integration`"),
            "{:?}",
            round.reasons
        );
        // The polling board applies the same rule: an exclusion the
        // document itself carries never enters the ready set.
        let board = crate::poll::Board::from_snapshot(&json!({
            "exclusions": [9507],
            "items": [{"number": 9507, "title": "Ready work", "status": "Ready",
                       "updatedAt": "version-9507", "state": "open"}]
        }))
        .unwrap();
        assert!(board.ready(&BTreeSet::new()).is_empty());
        assert!(
            board.refill(6, &BTreeSet::new()).is_empty(),
            "an excluded issue never enters a refill"
        );
    }

    #[test]
    fn a_cycle_among_prepared_tasks_is_detected_before_dispatch() {
        let mut configuration = configuration();
        configuration.tasks[0].scheduling.depends_on = vec!["task-b".into()];
        let mut second = prepared_named("task-b", 9508);
        second.scheduling.depends_on = vec!["task-a".into()];
        configuration.tasks.push(second);
        let error = configuration.validate().unwrap_err();
        assert!(error.contains("cycle"), "{error}");
        assert!(
            error.contains("task-a") && error.contains("task-b"),
            "the refusal names the cycle's members: {error}"
        );
        // The catalog never seals, so no round can plan over the cycle:
        // none of its tasks can dispatch.
        assert!(configuration.catalog().is_err());
    }

    #[test]
    fn a_partial_or_failed_fetch_blocks_what_it_cannot_see() {
        let mut configuration = configuration();
        configuration.tasks.push(prepared_named("task-b", 9508));
        configuration.validate().unwrap();
        let state = tempfile::tempdir().unwrap();
        let (ledger, catalog) = ledger_at(state.path(), &configuration);

        // The fetch carried 9507 but not 9508: the missing one blocks
        // rather than being assumed done or ready.
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot()));
        assert_eq!(round.plan.admit.len(), 1);
        assert_eq!(round.plan.admit[0].task, "task-a");
        assert_eq!(
            round.reasons["task-b"],
            "issue is no longer visible in the scoped project"
        );

        // A failed fetch blocks every queued task — nothing dispatches
        // on an observation that never arrived.
        let failed: Result<github::Snapshot, String> = Err("tracker fetch refused".into());
        let round = plan_round(&configuration, &catalog, &ledger, &failed);
        assert!(round.plan.admit.is_empty());
        assert!(
            round
                .reasons
                .values()
                .all(|reason| reason == "tracker fetch refused"),
            "{:?}",
            round.reasons
        );

        // The polling board reads the same rule: an edge into an item
        // the fetch did not carry is missing observation, never a
        // satisfied dependency — even when completion evidence is
        // supplied for it.
        let board = crate::poll::Board::from_snapshot(&json!({
            "items": [{"number": 1, "title": "Dependent", "status": "Ready",
                       "updatedAt": "v1",
                       "blockedBy": [{"number": 2, "state": "closed"}]}]
        }))
        .unwrap();
        assert_eq!(
            board.blocked_reason(1, &BTreeSet::from([2])),
            Some(crate::poll::Blockage::MissingDependency { dependency: 2 })
        );
        assert!(board.refill(1, &BTreeSet::from([2])).is_empty());
    }

    #[test]
    fn a_second_coordinator_cannot_claim_the_same_task() {
        let state = tempfile::tempdir().unwrap();
        let configuration = configuration();
        let catalog = configuration.catalog().unwrap();
        let mut ledger = Ledger::open(state.path()).unwrap();
        ledger.register(&catalog).unwrap();
        let digest = catalog.task("task-a").unwrap().digest();

        let attempt = ledger.claim("task-a", "coordinator-one", &digest).unwrap();
        // The claim is compare-and-set: a second owner loses on the
        // live record, not on timing luck.
        assert!(matches!(
            ledger.claim("task-a", "coordinator-two", &digest),
            Err(LedgerError::Transition { .. })
        ));

        // A second writer to the same state directory waits on the OS
        // lock. When the first closes, it reads the in-flight attempt
        // as `unknown` — claimed work is observed, never duplicated.
        let dir = state.path().to_path_buf();
        let contender = std::thread::spawn(move || Ledger::open(&dir));
        std::thread::sleep(Duration::from_millis(200));
        drop(ledger);
        let mut second = contender.join().unwrap().unwrap();
        let record = second.record("task-a").unwrap();
        assert_eq!(record.status, Status::Unknown);
        assert_eq!(record.attempt, attempt);
        assert_eq!(record.owner, "coordinator-one");
        assert!(matches!(
            second.claim("task-a", "coordinator-two", &digest),
            Err(LedgerError::Transition { .. })
        ));
    }

    #[test]
    fn an_interrupted_attempt_recovers_unknown_and_is_never_replayed() {
        let state = tempfile::tempdir().unwrap();
        let configuration = configuration();
        let catalog = configuration.catalog().unwrap();
        let digest = catalog.task("task-a").unwrap().digest();
        let attempt;
        {
            let mut ledger = Ledger::open(state.path()).unwrap();
            ledger.register(&catalog).unwrap();
            attempt = ledger.claim("task-a", "coordinator", &digest).unwrap();
            // The writer dies mid-flight: the claim is durable and no
            // result ever arrives.
        }
        let mut ledger = Ledger::open(state.path()).unwrap();
        let record = ledger.record("task-a").unwrap();
        assert_eq!(record.status, Status::Unknown);
        assert_eq!(record.attempt, attempt);
        assert_eq!(record.owner, "coordinator");
        assert_eq!(record.attempts, 1);
        assert!(
            record
                .cause
                .as_deref()
                .is_some_and(|c| c.contains("unknown")),
            "{:?}",
            record.cause
        );
        // Unknown is not a clean failure: settlement refuses, the task
        // still occupies the plan, and nothing requeues it implicitly.
        assert!(matches!(
            ledger.settle("task-a", &attempt, "coordinator", "late-result"),
            Err(LedgerError::Transition { .. })
        ));
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot()));
        assert!(round.plan.admit.is_empty());
        assert_eq!(round.plan.occupying, ["task-a"]);
        // Only the operator's explicit requeue returns it — under a new
        // attempt identity, so the interrupted attempt stays
        // distinguishable.
        ledger.requeue("task-a").unwrap();
        let retry = ledger.claim("task-a", "coordinator", &digest).unwrap();
        assert_ne!(retry, attempt);
    }

    #[test]
    fn a_result_from_before_repinning_is_stale_never_applied() {
        let state = tempfile::tempdir().unwrap();
        control_dirs(state.path());
        let configuration = configuration();
        let catalog = configuration.catalog().unwrap();
        let mut ledger = Ledger::open(state.path()).unwrap();
        ledger.register(&catalog).unwrap();
        let bound = catalog.task("task-a").unwrap().digest();
        let attempt = ledger.claim("task-a", "coordinator", &bound).unwrap();
        ledger
            .settle("task-a", &attempt, "coordinator", "result-digest")
            .unwrap();

        // The issue moved; pinning it again produces a new input digest
        // and therefore a new task identity.
        let mut repinned = self::configuration();
        repinned.tasks[0].issue_updated = "version-2".into();
        repinned.tasks[0].scheduling.input = repinned.tasks[0].input_digest();
        repinned.validate().unwrap();
        let new_catalog = repinned.catalog().unwrap();
        ledger.register(&new_catalog).unwrap();
        let current = new_catalog.task("task-a").unwrap().digest();
        assert_ne!(bound, current);

        // The drift the run refuses on is reported against the live
        // record, and the old attempt's result cannot settle under the
        // new identity.
        let drift = ledger.drift(&new_catalog);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].task, "task-a");
        assert_eq!(drift[0].status, Status::Review);
        assert_eq!(drift[0].bound, bound);
        assert_eq!(drift[0].current, current);
        assert!(matches!(
            ledger.accept("task-a", &attempt, "coordinator", &current),
            Err(LedgerError::TaskChanged { .. })
        ));
        // A review presenting the new task digest mismatches the bound
        // record and is refused whole — the stale result stays
        // unapplied.
        write_new(
            &state.path().join("control/review.json"),
            &Review {
                task: "task-a".into(),
                attempt,
                task_digest: current,
                result_digest: "result-digest".into(),
                accepted: true,
                evidence: "independent review record".into(),
            },
        )
        .unwrap();
        assert!(consume_reviews(state.path(), &mut ledger).is_err());
        let record = ledger.record("task-a").unwrap();
        assert_eq!(record.status, Status::Review);
        assert_eq!(record.task_digest, bound);
    }

    #[test]
    fn a_cancelled_attempt_stays_on_the_record_for_the_reconciler() {
        let state = tempfile::tempdir().unwrap();
        let mut configuration = configuration();
        configuration.tasks.push(prepared_named("task-b", 9508));
        configuration.validate().unwrap();
        let (mut ledger, catalog) = ledger_at(state.path(), &configuration);
        let digest = catalog.task("task-a").unwrap().digest();
        let attempt = ledger.claim("task-a", "coordinator", &digest).unwrap();
        // task-b runs to a result while task-a's job is cancelled
        // before any result arrives — the settle arm never runs for it.
        let sibling_digest = catalog.task("task-b").unwrap().digest();
        let sibling = ledger
            .claim("task-b", "coordinator", &sibling_digest)
            .unwrap();
        ledger
            .settle("task-b", &sibling, "coordinator", "sibling-result")
            .unwrap();

        // On the live ledger the cancelled attempt still reads
        // dispatched: the record does not report a finish it never saw,
        // and the attempt keeps occupying the plan.
        let record = ledger.record("task-a").unwrap();
        assert_eq!(record.status, Status::Active);
        assert_eq!(record.attempt, attempt);
        assert_eq!(record.owner, "coordinator");
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot()));
        assert!(round.plan.admit.is_empty());
        assert!(round.plan.occupying.iter().any(|task| task == "task-a"));

        // Recovery is what propagates the cancellation: the next open
        // marks the unreported attempt unknown — never queued, never
        // done — while the sibling's settled result still awaits its
        // review.
        drop(ledger);
        let ledger = Ledger::open(state.path()).unwrap();
        assert_eq!(ledger.record("task-a").unwrap().status, Status::Unknown);
        assert_eq!(ledger.record("task-b").unwrap().status, Status::Review);
    }

    #[test]
    fn needs_beyond_capacity_refuse_with_a_named_bound() {
        // The task asks for more CPU than the host declares: the plan
        // refuses it with the bound's name every round rather than
        // admitting it or dropping it silently.
        let mut configuration = configuration();
        configuration.tasks[0].scheduling.resources.cpu_units =
            configuration.capacity.cpu_units + 1;
        configuration.validate().unwrap();
        let state = tempfile::tempdir().unwrap();
        let (ledger, catalog) = ledger_at(state.path(), &configuration);
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot()));
        assert!(round.plan.admit.is_empty());
        assert_eq!(
            round.plan.blocked[0].reasons,
            vec![Reason::Capacity(Bound::CpuUnits)]
        );
        assert!(
            round.reasons["task-a"].contains("cpu-units"),
            "the round record names the bound: {:?}",
            round.reasons
        );

        // The reservation book answers the same way at request time: a
        // need over the ceiling is a typed refusal that names the lane
        // and the numbers, not a grant that waits forever.
        let mut book = crate::reservations::Book::new(crate::reservations::Ceilings {
            executor_slots: 2,
            cpu_units: 8,
            memory_mib: 8192,
        });
        let needs = crate::reservations::Needs {
            holder: "task-a".into(),
            executor_slots: Some(1),
            cpu_units: Some(9),
            memory_mib: Some(256),
            quiet_accelerator: None,
            integration: false,
            isolation: crate::reservations::Isolation::Admission,
            for_seconds: None,
            at_unix: 1_000,
        };
        assert_eq!(
            book.request(needs),
            Err(crate::reservations::Refusal::Ceiling {
                lane: crate::reservations::Lane::CpuBuild,
                held: 0,
                requested: 9,
                ceiling: 8,
            })
        );
    }

    #[test]
    fn a_full_review_backlog_applies_backpressure_until_reviewed() {
        let state = tempfile::tempdir().unwrap();
        control_dirs(state.path());
        let mut configuration = configuration();
        configuration.review_cap = 1;
        configuration.tasks.push(prepared_named("task-b", 9508));
        configuration.validate().unwrap();
        let (mut ledger, catalog) = ledger_at(state.path(), &configuration);

        // task-a settles into review and fills the cap: task-b is told
        // to wait for review, not admitted unboundedly.
        let digest = catalog.task("task-a").unwrap().digest();
        let attempt = ledger.claim("task-a", "coordinator", &digest).unwrap();
        ledger
            .settle("task-a", &attempt, "coordinator", "a-result")
            .unwrap();
        let mut snapshot = snapshot();
        snapshot.issues.insert(9508, issue(9508));
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot.clone()));
        assert!(round.plan.admit.is_empty());
        assert!(
            round.reasons["task-b"].contains("review backlog is at its cap of 1"),
            "{:?}",
            round.reasons
        );

        // The host's accept drains the backlog; the next round admits
        // the queued task.
        write_new(
            &state.path().join("control/review.json"),
            &Review {
                task: "task-a".into(),
                attempt,
                task_digest: digest,
                result_digest: "a-result".into(),
                accepted: true,
                evidence: "independent review record".into(),
            },
        )
        .unwrap();
        consume_reviews(state.path(), &mut ledger).unwrap();
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot));
        assert_eq!(round.plan.admit.len(), 1);
        assert_eq!(round.plan.admit[0].task, "task-b");
    }

    #[test]
    fn a_capacity_refusal_requeues_under_backoff_never_retries_hot() {
        let state = tempfile::tempdir().unwrap();
        control_dirs(state.path());
        let configuration = configuration();
        let (mut ledger, catalog) = ledger_at(state.path(), &configuration);

        // A claimed attempt ends on a capacity refusal — the executor
        // said it could not take the work, so the attempt ran nothing.
        let digest = catalog.task("task-a").unwrap().digest();
        let attempt = ledger.claim("task-a", "coordinator", &digest).unwrap();
        let until = atif::now_ms() / 1000 + configuration.quota_backoff_seconds;
        ledger
            .backoff("task-a", &attempt, "coordinator", "quota-result", until)
            .unwrap();
        let record = ledger.record("task-a").unwrap();
        // The task is queued again — not reviewed, not rejected — and
        // the refused result's digest stays as evidence of the attempt.
        assert_eq!(record.status, Status::Queued);
        assert_eq!(record.result_digest.as_deref(), Some("quota-result"));
        assert_eq!(record.backoff_until, Some(until));

        // While the backoff stands the round blocks it by name rather
        // than silently hot-retrying.
        let round = plan_round(&configuration, &catalog, &ledger, &Ok(snapshot()));
        assert!(round.plan.admit.is_empty());
        assert!(
            round.reasons["task-a"].contains("executor capacity backoff"),
            "{:?}",
            round.reasons
        );
    }

    #[test]
    fn a_wrong_answer_is_not_a_capacity_refusal() {
        let report = crate::DispatchReport {
            schema: "openagents.project-dispatch.v1".into(),
            task_id: "task-a".into(),
            input_digest: "input".into(),
            base: "a".repeat(40),
            program_digest: "program".into(),
            trace: PathBuf::from("/tmp/trace"),
            answered: true,
            execution_status: Some("answered".into()),
            text_matched: Some(false),
            refusal: None,
            retained_worktree: None,
            elapsed_ms: 1000,
            executor_cost_usd: None,
            artifact_verified: false,
        };
        // An answered attempt that failed its text check is review
        // work for a human, not an executor capacity problem.
        assert_eq!(capacity_cause(&report), None);

        let mut refused = crate::DispatchReport { ..report };
        refused.answered = false;
        refused.execution_status = Some("refused: quota".into());
        assert_eq!(capacity_cause(&refused).as_deref(), Some("refused: quota"));
        refused.execution_status = Some("refused: untrusted_workspace".into());
        // An authority refusal is not capacity — it lands in review.
        assert_eq!(capacity_cause(&refused), None);
    }
}
