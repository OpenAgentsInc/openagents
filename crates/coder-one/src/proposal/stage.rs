//! An approved proposal's two stages: the mini-task stage, which runs in
//! seconds with no model, and the live stage, a targeted Terminal-Bench
//! experiment on the cited tasks only.
//!
//! For a policy or check, the mini stage runs each cited task's mini-task
//! with its known-good and known-bad scripts, once under the base manifest
//! and once under the proposal's, with the checks, the briefing, and the
//! repair each manifest sets; a scripted repair stands in for the
//! executor's. The scripted executor doesn't read the briefing, so the
//! stage catches a change that breaks an episode, flags good work, or
//! misses bad work, not one that helps a model. When no mini-task covers
//! the cited tasks, it screens every mini-task instead and says so.
//!
//! For a mini-task, the stage writes the task's files with each candidate
//! and runs the grader inside a `coder-boundary` boundary: the mini-task
//! reproduces the failure when the grader passes the good candidate and
//! fails the bad one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Kind, MiniSpec};
use crate::minitask::run::{ExecutorChoice, Options, Repair, run};
use crate::policy::Manifest;

/// How long one mini-task episode or one grader run may take.
pub const MINI_DEADLINE: Duration = Duration::from_secs(60);

/// The attempts per task per arm in a live stage.
pub const LIVE_ATTEMPTS: u32 = 3;

/// The mini-task episode's options under `manifest`.
fn options(
    manifest: &Manifest,
    task: crate::minitask::MiniTask,
    script: &str,
    out: &Path,
) -> Options {
    let verify = manifest.policy.verify.as_ref();
    let checks = verify.is_some_and(|v| v.checks);
    let repair = verify
        .and_then(|v| v.repair.as_ref())
        .filter(|_| checks)
        .map(|r| Repair {
            profile: crate::repair::Profile::Scripted("fix-if-packet".to_string()),
            policy: crate::repair::Policy {
                kind: r.brief,
                trigger: r.trigger,
                allowance: MINI_DEADLINE,
            },
        });
    Options {
        task,
        executor: ExecutorChoice::Scripted {
            variant: script.to_string(),
            script: None,
        },
        out: out.to_path_buf(),
        jev: None,
        speed: 0.0,
        deadline: MINI_DEADLINE,
        controls: crate::session::Controls::default(),
        checks,
        brief: Some(manifest.policy.brief.clone()),
        monitor: None,
        repair,
    }
}

/// Runs a policy or check proposal's mini stage.
pub async fn mini_policy(
    proposal: &Value,
    base: &Manifest,
    candidate: &Manifest,
    out: &Path,
) -> Value {
    let started = Instant::now();
    let named: Vec<&str> = proposal["mini_tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let covered = !named.is_empty();
    let tasks: Vec<crate::minitask::MiniTask> = if covered {
        named
            .iter()
            .filter_map(|id| crate::minitask::find(id).ok())
            .collect()
    } else {
        crate::minitask::CATALOG.to_vec()
    };
    let mut rows = Vec::new();
    for task in &tasks {
        for script in ["good", "bad"] {
            for (arm, manifest) in [("base", base), ("proposal", candidate)] {
                let dir = out.join(arm);
                let _ = std::fs::create_dir_all(&dir);
                let began = Instant::now();
                let ran = run(options(manifest, *task, script, &dir)).await;
                let milliseconds = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
                rows.push(match ran {
                    Ok(ran) => json!({
                        "task": task.id,
                        "script": script,
                        "arm": arm,
                        "verdict": ran.grade.verdict,
                        "detail": ran.grade.detail,
                        "checks_failed": ran.manifest.pointer("/checks/verdicts/failed").and_then(Value::as_u64),
                        "checks_passed": ran.manifest.pointer("/checks/verdicts/passed").and_then(Value::as_u64),
                        "briefing_chars": ran.manifest.pointer("/delegation/briefing/chars").and_then(Value::as_u64),
                        "milliseconds": milliseconds,
                        "dir": ran.dir.display().to_string(),
                    }),
                    Err(error) => json!({
                        "task": task.id,
                        "script": script,
                        "arm": arm,
                        "verdict": "error",
                        "detail": error,
                        "milliseconds": milliseconds,
                    }),
                });
            }
        }
    }
    let (verdict, findings) = compare(&rows);
    json!({
        "kind": "policy",
        "covered": covered,
        "tasks": tasks.iter().map(|t| t.id).collect::<Vec<_>>(),
        "note": if covered {
            "the cited tasks' mini-tasks, good and bad scripts, base and proposal; the scripted executor doesn't read the briefing"
        } else {
            "no mini-task covers the cited tasks, so this screened every mini-task for breakage; the scripted executor doesn't read the briefing"
        },
        "runs": rows,
        "verdict": verdict,
        "findings": findings,
        "milliseconds": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// The mini stage's verdict from its rows: `regressed` when the proposal
/// fails good work the base passed, errs, or flags good work the base
/// didn't; `improved` when it passes bad work the base failed or flags bad
/// work the base didn't; `unchanged` otherwise.
#[must_use]
pub fn compare(rows: &[Value]) -> (&'static str, Vec<String>) {
    let mut cells: BTreeMap<(String, String), BTreeMap<String, &Value>> = BTreeMap::new();
    for row in rows {
        cells
            .entry((
                row["task"].as_str().unwrap_or_default().to_string(),
                row["script"].as_str().unwrap_or_default().to_string(),
            ))
            .or_default()
            .insert(row["arm"].as_str().unwrap_or_default().to_string(), row);
    }
    let (mut worse, mut better) = (Vec::new(), Vec::new());
    for ((task, script), arms) in &cells {
        let (Some(base), Some(new)) = (arms.get("base"), arms.get("proposal")) else {
            continue;
        };
        let passed = |row: &Value| row["verdict"] == "passed";
        let flags = |row: &Value| row["checks_failed"].as_u64().unwrap_or(0) > 0;
        if new["verdict"] == "error" {
            worse.push(format!(
                "{task} {script}: the proposal's episode failed to run: {}",
                new["detail"].as_str().unwrap_or("")
            ));
            continue;
        }
        if script == "good" {
            if passed(base) && !passed(new) {
                worse.push(format!(
                    "{task} good: the base passed and the proposal failed"
                ));
            }
            if flags(new) && !flags(base) {
                worse.push(format!(
                    "{task} good: the proposal's checks flag work the grader passes"
                ));
            }
        } else {
            if passed(new) && !passed(base) {
                better.push(format!(
                    "{task} bad: the proposal repaired work the base left failing"
                ));
            }
            if flags(new) && !flags(base) {
                better.push(format!(
                    "{task} bad: the proposal's checks flag the failure and the base's don't"
                ));
            }
            if passed(base) && !passed(new) {
                worse.push(format!(
                    "{task} bad: the base's repair passed and the proposal's didn't"
                ));
            }
        }
    }
    let verdict = if !worse.is_empty() {
        "regressed"
    } else if !better.is_empty() {
        "improved"
    } else {
        "unchanged"
    };
    let mut findings = worse;
    findings.extend(better);
    (verdict, findings)
}

/// Runs a mini-task proposal's mini stage: the grader on each candidate,
/// inside a boundary that lets it write only the candidate's directory.
///
/// # Errors
///
/// Returns a message when the host has no boundary.
pub async fn mini_task(spec: &MiniSpec, out: &Path) -> Result<Value, String> {
    let started = Instant::now();
    let mut rows = Vec::new();
    for (name, candidate) in [("good", &spec.good), ("bad", &spec.bad)] {
        let dir = out.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        for (path, body) in spec.files.iter().chain(candidate) {
            let file = dir.join(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            }
            std::fs::write(&file, body)
                .map_err(|error| format!("cannot write {}: {error}", file.display()))?;
        }
        let boundary = coder_boundary::Boundary::readonly()
            .writable(&dir)
            .build()
            .map_err(|error| format!("cannot bound the grader, so it doesn't run: {error}"))?;
        let mut command = boundary
            .command("/bin/sh", ["-c", spec.grader.as_str()])
            .map_err(|error| error.to_string())?;
        command.current_dir(&dir);
        let ended = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(MINI_DEADLINE).keeping(16 * 1024))
            .run()
            .await;
        rows.push(json!({
            "candidate": name,
            "passed": ended.ending.success(),
            "ending": ended.ending.to_string(),
            "stdout": crate::ask::clip(&ended.stdout.marked(), 2_000),
            "stderr": crate::ask::clip(&ended.stderr.marked(), 2_000),
            "dir": dir.display().to_string(),
        }));
    }
    let good = rows[0]["passed"] == true;
    let bad = rows[1]["passed"] == true;
    let verdict = if good && !bad {
        "reproduces"
    } else {
        "does-not-reproduce"
    };
    let mut findings = Vec::new();
    if !good {
        findings.push("the grader fails the good candidate".to_string());
    }
    if bad {
        findings.push(
            "the grader passes the bad candidate, so it doesn't show the failure".to_string(),
        );
    }
    Ok(json!({
        "kind": "minitask",
        "minitask": spec.id,
        "runs": rows,
        "verdict": verdict,
        "findings": findings,
        "milliseconds": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }))
}

/// A live stage's plan.
#[derive(Clone, Debug)]
pub struct Live {
    /// The repository, whose `bench/terminal-bench` runs the experiment.
    pub repo: PathBuf,
    /// The job profile, such as `tb4`.
    pub profile: String,
    pub quota_usd: f64,
    /// The tasks: the proposal's expected tasks, or a subset of them.
    pub tasks: Vec<String>,
    /// The agent profile the base manifest runs as; found when `None`.
    pub base_arm: Option<String>,
    pub min_free_disk_gb: Option<f64>,
    /// Print the schedule and check credentials without starting.
    pub plan_only: bool,
    /// The Coder One build both arms run, and its SHA-256.
    pub artifact: Option<(PathBuf, String)>,
}

/// A file's SHA-256, as the adapter pins an artifact.
///
/// # Errors
///
/// Returns a message when the file can't be read.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// The agent profile whose policy is `crates/coder-one/policies/<base>.json`.
///
/// # Errors
///
/// Returns a message when none, or more than one, does.
pub fn base_arm(repo: &Path, base: &str) -> Result<String, String> {
    let path = repo.join("bench/terminal-bench/profiles/agents.json");
    let agents: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{} isn't JSON: {error}", path.display()))?;
    let want = format!("crates/coder-one/policies/{base}.json");
    let found: Vec<String> = agents["agents"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, agent)| {
            agent.pointer("/kwargs/policy").and_then(Value::as_str) == Some(want.as_str())
        })
        .map(|(name, _)| name.clone())
        .collect();
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!(
            "no agent profile runs {want}; name one with --base-arm"
        )),
        many => Err(format!(
            "{} agent profiles run {want} ({}); name one with --base-arm",
            many.len(),
            many.join(", ")
        )),
    }
}

/// The `tbench experiment` arguments for a live stage.
#[must_use]
pub fn experiment_args(proposal_id: &str, base: &str, policy: &Path, live: &Live) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "tbench".to_string(),
        "experiment".to_string(),
        if live.plan_only { "plan" } else { "run" }.to_string(),
        "--id".to_string(),
        proposal_id.to_string(),
        "--profile".to_string(),
        live.profile.clone(),
        "--arm".to_string(),
        base.to_string(),
        "--arm".to_string(),
        format!("{proposal_id}={base}"),
        "--arm-kwarg".to_string(),
        format!("{proposal_id}:policy={}", policy.display()),
    ];
    if let Some((path, sha)) = &live.artifact {
        for arm in [base.to_string(), proposal_id.to_string()] {
            args.extend([
                "--arm-kwarg".to_string(),
                format!("{arm}:artifact_path={}", path.display()),
                "--arm-kwarg".to_string(),
                format!("{arm}:artifact_sha256={sha}"),
            ]);
        }
    }
    args.extend([
        "--tasks".to_string(),
        live.tasks.join(","),
        "--attempts".to_string(),
        LIVE_ATTEMPTS.to_string(),
        "--quota-usd".to_string(),
        format!("{}", live.quota_usd),
    ]);
    if let Some(gb) = live.min_free_disk_gb {
        args.extend(["--min-free-disk-gb".to_string(), format!("{gb}")]);
    }
    if !live.plan_only {
        args.push("--detach".to_string());
    }
    args
}

/// Starts the live stage: `uv run tbench experiment run … --detach` from
/// `bench/terminal-bench`, or `plan` with `plan_only`.
///
/// # Errors
///
/// Returns a message when the proposal can't run live or `uv` fails.
pub async fn live(proposal: &Value, dir: &Path, live: &Live) -> Result<Value, String> {
    let kind = Kind::parse(proposal["kind"].as_str().unwrap_or_default())?;
    if !matches!(kind, Kind::Policy | Kind::Check) {
        return Err(format!(
            "a {} proposal has no live stage; only policy and check proposals run on Terminal-Bench",
            kind.word()
        ));
    }
    let base = proposal["materialized"]["base"]
        .as_str()
        .ok_or("the proposal has no materialized manifest")?;
    let arm = match &live.base_arm {
        Some(arm) => arm.clone(),
        None => base_arm(&live.repo, base)?,
    };
    let id = proposal["id"].as_str().unwrap_or_default();
    let policy = dir.join(super::POLICY_FILE);
    let args = experiment_args(id, &arm, &policy, live);
    if live.artifact.is_none() && !live.plan_only {
        return Err(
            "the live stage needs --artifact: the Coder One build both arms run, such as \
             ./scripts/build-coder-one-linux.sh prints"
                .to_string(),
        );
    }
    let uv = crate::ask::allow::which("uv")
        .ok_or("uv isn't on PATH; the live stage runs `uv run tbench`")?;
    let bench = live.repo.join("bench/terminal-bench");
    let ended = supervise::Job::new(&uv)
        .args(&args)
        .in_directory(&bench)
        .bounded(supervise::Limits::within(Duration::from_secs(600)).keeping(256 * 1024))
        .run()
        .await;
    let output = format!("{}{}", ended.stdout.marked(), ended.stderr.marked());
    if !ended.ending.success() {
        return Err(format!(
            "`uv {}` ended {}:\n{}",
            args.join(" "),
            ended.ending,
            crate::ask::clip(&output, 4_000)
        ));
    }
    Ok(json!({
        "experiment": id,
        "profile": live.profile,
        "arms": [arm, id],
        "baseline": arm,
        "tasks": live.tasks,
        "attempts": LIVE_ATTEMPTS,
        "quota_usd": live.quota_usd,
        "plan_only": live.plan_only,
        "artifact": live.artifact.as_ref().map(|(path, sha)| json!({"path": path.display().to_string(), "sha256": sha})),
        "command": format!("uv {}", args.join(" ")),
        "started_at": atif::document::iso(atif::now_ms()),
        "output": crate::ask::clip(&output, 8_000),
        "report": format!("gym terminal-bench experiment report {id}"),
    }))
}
