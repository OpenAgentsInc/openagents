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
    pub tasks: Vec<Prepared>,
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
        let mut reasons = BTreeMap::<String, String>::new();
        let snapshot = if !stop {
            github::fetch(&configuration.project, &configuration.repository).await
        } else {
            Err("supervisor admission bound reached; draining active work".into())
        };
        let mut eligible = catalog.clone();
        let mut states = ledger.statuses();
        states.insert("external-owner-reservation".into(), Status::Active);
        eligible.tasks.retain(|task| {
            if task.id == "external-owner-reservation"
                || states.get(&task.id) != Some(&Status::Queued)
            {
                return true;
            }
            let prepared = configuration
                .tasks
                .iter()
                .find(|p| p.scheduling.id == task.id)
                .expect("catalog contains prepared tasks");
            let reason = match &snapshot {
                Err(error) => Some(error.clone()),
                Ok(snapshot) if !snapshot.issues.contains_key(&task.issue) => {
                    Some("issue is no longer visible in the scoped project".into())
                }
                Ok(snapshot) => tracker_block(&configuration, prepared, snapshot),
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
                    ledger.settle(&id, &attempt, &owner, &result_digest).map_err(|e| e.to_string())?;
                    eprintln!("result {id} attempt {attempt}: pending independent review");
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(configuration.poll_seconds)) => {},
            _ = tokio::signal::ctrl_c() => { stop = true; eprintln!("stop requested; draining active attempts and preserving results"); },
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared() -> Prepared {
        let assignment = Assignment {
            id: "task-a".into(),
            base: "a".repeat(40),
            prompt: "Inspect a.rs".into(),
            writes: false,
            expected_text: Some("done".into()),
            minutes: 1,
        };
        let scheduling = Task {
            id: assignment.id.clone(),
            issue: 9507,
            base: assignment.base.clone(),
            input: String::new(),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec!["a.rs".into()],
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
            issue_updated: "version-1".into(),
            issue_body_digest: "body-1".into(),
            tracker_base: "a".repeat(40),
        };
        prepared.scheduling.input = prepared.input_digest();
        prepared
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
            tasks: vec![prepared()],
        }
    }

    fn snapshot() -> github::Snapshot {
        github::Snapshot {
            issues: BTreeMap::from([(
                9507,
                github::Issue {
                    number: 9507,
                    updated_at: "version-1".into(),
                    body_digest: "body-1".into(),
                    closed: false,
                    blockers: vec![],
                },
            )]),
            source_digest: "snapshot".into(),
            default_branch_revision: "a".repeat(40),
            ignored_non_issues: 0,
            ignored_other_repositories: 0,
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
}
