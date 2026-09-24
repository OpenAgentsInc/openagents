//! The labeled set: retained Terminal-Bench trials with the verifier's
//! outcome, the candidate the trial left, and what the checks say about it.
//!
//! Each graded Coder One trial under the Harbor jobs directory holds the
//! verifier's reward and test results, the requirement map, the first
//! executor's final report, the commands the sessions ran before the first
//! check, and the outputs Harbor collected from the task container. A replay rebuilds the task's filesystem under a replay
//! root: the files the task image copies in (the `COPY` lines of the
//! task's public `environment/Dockerfile`), with the collected outputs on
//! top. The checks then run against that root, in a sandbox, exactly as
//! they would in the episode, except that commands the task image
//! installed aren't there, which makes a scenario that needs them
//! `unavailable` rather than failed.
//!
//! A trial whose agent never ran (Harbor recorded an agent exception) is
//! excluded: its failure says nothing about a candidate. The verifier's
//! failed test names are kept in the label for postmortem reading only;
//! no check reads them, and a test asserts that no scenario carries one.
//!
//! The collected outputs are the trial's final state, after any repair,
//! which is also what the verifier graded. Recall is therefore measured on
//! the candidate the verifier saw.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::generic::{Options, Workspace};
use super::{Budget, Input, Observed, Report, TaskText, check, workspace_candidate};
use crate::record::Recorder;
use crate::requirements::RequirementMap;

/// The schema of one trial's label.
pub const LABEL_SCHEMA: &str = "openagents.coder-one.check-label.v1";

/// The schema of a recall summary.
pub const RECALL_SCHEMA: &str = "openagents.coder-one.check-recall.v1";

/// The Terminal-Bench 4.0 tasks some leaderboard row passes at least four
/// times in five that no Coder One version had passed on 2026-09-23
/// (`bench/terminal-bench/reference/tb4-leaderboard.json`, and the
/// `tb4--coder-one-tunable-v*` jobs).
pub const TARGETS: &[&str] = &[
    "heat-pump-warranty",
    "html-js-filter",
    "ks-solver-cpp",
    "production-planning",
    "protein-autointerp-disulfide",
    "risk-scorer-replay",
    "rs-archive-clone",
    "sglang-qwen-burst",
    "wal-recovery-ordering",
];

/// [`TARGETS`] as owned names.
#[must_use]
pub fn targets() -> Vec<String> {
    TARGETS.iter().map(|t| (*t).to_string()).collect()
}

/// The largest file a replay root copies.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// The most bytes one replay root holds.
const MAX_ROOT_BYTES: u64 = 1024 * 1024 * 1024;

/// One retained trial, labeled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub schema: String,
    pub job: String,
    pub trial: String,
    pub task: String,
    /// The arm, such as `coder-one-tunable-v2`.
    pub arm: String,
    /// The verifier's reward.
    pub reward: Option<f64>,
    /// Why the trial isn't in the set, when it isn't.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded: Option<String>,
    /// The verifier's failed tests, for postmortem reading only.
    #[serde(default)]
    pub failed_tests: Vec<String>,
    /// How many verifier tests passed.
    #[serde(default)]
    pub passed_tests: usize,
    /// Whether the episode's own first check failed a scenario.
    pub episode_flagged: Option<bool>,
    /// The output paths Harbor collected.
    #[serde(default)]
    pub collected: Vec<String>,
}

impl Label {
    /// Whether the verifier failed the trial.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.reward.is_some_and(|r| r < 1.0)
    }
}

/// A trial ready to check: its label and the check's input.
#[derive(Clone, Debug)]
pub struct Labeled {
    pub label: Label,
    /// The task's public directory: instruction and environment.
    pub task_dir: PathBuf,
    pub trial_dir: PathBuf,
    pub instruction: String,
    pub requirements: Option<RequirementMap>,
    pub report: Option<String>,
    /// The commands the executor sessions ran before the first check, as
    /// the episode's invocation log recorded them.
    pub claimed: Vec<super::generic::Claimed>,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// The instruction without its HTML comments.
#[must_use]
pub fn public_instruction(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// The verifier's test results: failed names and the passed count.
fn tests(trial: &Path) -> (Vec<String>, usize) {
    let Some(ctrf) = read_json(&trial.join("verifier/ctrf.json")) else {
        return (Vec::new(), 0);
    };
    let mut failed = Vec::new();
    let mut passed = 0;
    for test in ctrf["results"]["tests"].as_array().into_iter().flatten() {
        let name = test["name"].as_str().unwrap_or_default();
        let name = name.rsplit("::").next().unwrap_or(name).to_string();
        if test["status"] == "passed" {
            passed += 1;
        } else {
            failed.push(name);
        }
    }
    (failed, passed)
}

/// Reads one Harbor trial directory, `<jobs>/<job>/<trial>`.
///
/// # Errors
///
/// Returns a message when the trial isn't a finished Coder One trial.
pub fn load(job: &str, trial_dir: &Path) -> Result<Labeled, String> {
    let result = read_json(&trial_dir.join("result.json"))
        .ok_or_else(|| format!("{} hasn't finished", trial_dir.display()))?;
    let trial = trial_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let task = trial
        .split_once("__")
        .map_or_else(|| trial.clone(), |(t, _)| t.to_string());
    let arm = job.split("--").nth(1).unwrap_or_default().to_string();
    let reward = std::fs::read_to_string(trial_dir.join("verifier/reward.txt"))
        .ok()
        .and_then(|t| t.trim().parse::<f64>().ok());
    let excluded = result["exception_info"]["exception_type"]
        .as_str()
        .map(|kind| {
            let message = result["exception_info"]["exception_message"]
                .as_str()
                .unwrap_or_default();
            format!(
                "Harbor recorded {kind}: {}",
                crate::judge::clip(message.trim(), 160)
            )
        })
        .or_else(|| {
            reward
                .is_none()
                .then(|| "the verifier left no reward".to_string())
        });
    let config = read_json(&trial_dir.join("config.json")).unwrap_or(Value::Null);
    let task_dir = config["task"]["path"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{} names no task", trial_dir.display()))?;
    let instruction = public_instruction(
        &std::fs::read_to_string(task_dir.join("instruction.md")).unwrap_or_default(),
    );
    let episode = trial_dir.join("agent/episode");
    let requirements = read_json(&episode.join("artifacts/requirements.json"))
        .and_then(|v| serde_json::from_value(v).ok());
    let report = std::fs::read_to_string(episode.join("artifacts/delegate-1.stream.jsonl"))
        .ok()
        .and_then(|stream| super::replay::final_report(&stream));
    let episode_flagged = read_json(&episode.join(super::COVERAGE_FILE))
        .and_then(|v| serde_json::from_value::<Report>(v).ok())
        .map(|r| r.detected());
    let collected = read_json(&trial_dir.join("artifacts/manifest.json"))
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter(|a| a["status"] == "ok")
        .filter_map(|a| a["source"].as_str().map(str::to_string))
        .collect();
    let (failed_tests, passed_tests) = tests(trial_dir);
    let claimed = crate::snapshot::reconstruct(&episode)
        .ok()
        .and_then(|subject| subject.live)
        .map(|live| live.claimed)
        .unwrap_or_default();
    Ok(Labeled {
        label: Label {
            schema: LABEL_SCHEMA.to_string(),
            job: job.to_string(),
            trial,
            task,
            arm,
            reward,
            excluded,
            failed_tests,
            passed_tests,
            episode_flagged,
            collected,
        },
        task_dir,
        trial_dir: trial_dir.to_path_buf(),
        instruction,
        requirements,
        report,
        claimed,
    })
}

/// Every finished trial of the jobs under `jobs` whose names contain
/// `matching`, in job order.
#[must_use]
pub fn scan(jobs: &Path, matching: &str) -> Vec<Labeled> {
    let mut found = Vec::new();
    let mut job_dirs: Vec<PathBuf> = std::fs::read_dir(jobs)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().contains(matching))
        })
        .collect();
    job_dirs.sort();
    for job_dir in job_dirs {
        let job = job_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut trials: Vec<PathBuf> = std::fs::read_dir(&job_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .is_some_and(|n| n.to_string_lossy().contains("__"))
            })
            .collect();
        trials.sort();
        for trial in trials {
            if let Ok(labeled) = load(&job, &trial) {
                found.push(labeled);
            }
        }
    }
    found
}

/// One `COPY` or `ADD` of a Dockerfile: its sources and destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copy {
    pub sources: Vec<String>,
    pub destination: String,
}

/// The `COPY` and `ADD` lines of a Dockerfile that copy from the build
/// context, and the last `WORKDIR`.
#[must_use]
pub fn dockerfile(text: &str) -> (Vec<Copy>, Option<String>) {
    let mut joined = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim_start().starts_with('#') && current.is_empty() {
            continue;
        }
        if let Some(continued) = trimmed.strip_suffix('\\') {
            current.push_str(continued);
            current.push(' ');
        } else {
            current.push_str(trimmed);
            joined.push(std::mem::take(&mut current));
        }
    }
    let mut copies = Vec::new();
    let mut workdir = None;
    for line in joined {
        let mut words = line.split_whitespace();
        let Some(instruction) = words.next() else {
            continue;
        };
        match instruction.to_ascii_uppercase().as_str() {
            "WORKDIR" => workdir = words.next().map(str::to_string),
            "COPY" | "ADD" => {
                let rest: Vec<String> = if line.contains('[') && line.contains(']') {
                    let inside =
                        &line[line.find('[').unwrap_or(0) + 1..line.rfind(']').unwrap_or(0)];
                    inside
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .collect()
                } else {
                    words.map(str::to_string).collect()
                };
                if rest.iter().any(|w| w.starts_with("--from")) {
                    continue;
                }
                let mut paths: Vec<String> =
                    rest.into_iter().filter(|w| !w.starts_with("--")).collect();
                if paths.len() < 2 || paths.iter().any(|p| p.contains("://")) {
                    continue;
                }
                let destination = paths.pop().unwrap_or_default();
                copies.push(Copy {
                    sources: paths,
                    destination,
                });
            }
            _ => {}
        }
    }
    (copies, workdir)
}

/// Copies `from` to `to` (files and directories), within `budget` bytes.
fn copy_tree(from: &Path, to: &Path, budget: &mut u64) {
    let Ok(meta) = std::fs::symlink_metadata(from) else {
        return;
    };
    if meta.is_dir() {
        let _ = std::fs::create_dir_all(to);
        for entry in std::fs::read_dir(from).into_iter().flatten().flatten() {
            copy_tree(&entry.path(), &to.join(entry.file_name()), budget);
        }
    } else if meta.is_file() {
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if meta.len() <= MAX_FILE_BYTES && meta.len() <= *budget {
            if std::fs::copy(from, to).is_ok() {
                *budget -= meta.len();
                let _ = std::fs::set_permissions(to, meta.permissions());
            }
        } else if let Ok(file) = std::fs::File::create(to) {
            // Too large to copy: a sparse file of the same length keeps
            // its existence and size, which is what the output checks read.
            let _ = file.set_len(meta.len());
        }
    }
}

/// A `*` pattern's matches in one directory.
fn matches(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let mut rest = name;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            let Some(r) = rest.strip_prefix(part) else {
                return false;
            };
            rest = r;
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else if let Some(at) = rest.find(part) {
            rest = &rest[at + part.len()..];
        } else {
            return false;
        }
    }
    true
}

/// Rebuilds the task's filesystem under `root`: the public files the task
/// image copies in, then the trial's collected outputs on top. Returns the
/// task's working directory.
///
/// # Errors
///
/// Returns why the root can't be built.
pub fn build_root(labeled: &Labeled, root: &Path) -> Result<String, String> {
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let mut budget = MAX_ROOT_BYTES;
    let environment = labeled.task_dir.join("environment");
    let (copies, workdir) =
        dockerfile(&std::fs::read_to_string(environment.join("Dockerfile")).unwrap_or_default());
    let workdir = workdir.unwrap_or_else(|| "/app".to_string());
    for copy in &copies {
        let destination = if copy.destination.starts_with('/') {
            copy.destination.clone()
        } else {
            format!("{}/{}", workdir.trim_end_matches('/'), copy.destination)
        };
        let into_dir = destination.ends_with('/') || copy.sources.len() > 1;
        for source in &copy.sources {
            let source = source.trim_start_matches("./");
            let mut found: Vec<PathBuf> = Vec::new();
            if source.contains('*') {
                let (dir, pattern) = source.rsplit_once('/').unwrap_or(("", source));
                for entry in std::fs::read_dir(environment.join(dir))
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    if matches(pattern, &entry.file_name().to_string_lossy()) {
                        found.push(entry.path());
                    }
                }
            } else {
                found.push(environment.join(source));
            }
            for from in found {
                let target = if from.is_dir() {
                    PathBuf::from(&destination)
                } else if into_dir || source.contains('*') {
                    PathBuf::from(&destination).join(from.file_name().unwrap_or_default())
                } else {
                    PathBuf::from(&destination)
                };
                copy_tree(
                    &from,
                    &root.join(target.strip_prefix("/").unwrap_or(&target)),
                    &mut budget,
                );
            }
        }
    }
    let manifest = read_json(&labeled.trial_dir.join("artifacts/manifest.json"))
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    for artifact in manifest.iter().filter(|a| a["status"] == "ok") {
        let (Some(source), Some(destination)) = (
            artifact["source"].as_str(),
            artifact["destination"].as_str(),
        ) else {
            continue;
        };
        if source.starts_with("/logs") {
            continue;
        }
        let from = labeled.trial_dir.join(destination);
        let to = root.join(source.trim_start_matches('/'));
        copy_tree(&from, &to, &mut budget);
    }
    let _ = std::fs::create_dir_all(root.join(workdir.trim_start_matches('/')));
    Ok(workdir)
}

/// The check options a replay arm runs.
#[must_use]
pub fn arm_options(arm: &str) -> Option<Options> {
    match arm {
        "v6" => Some(Options {
            self_report: true,
            optional_outputs: true,
            behavior: false,
        }),
        "v7" => Some(Options {
            self_report: true,
            optional_outputs: true,
            behavior: true,
        }),
        _ => None,
    }
}

/// The check input for a labeled trial against its replay root.
#[must_use]
pub fn input(labeled: &Labeled, root: &Path, workdir: &str, options: Options) -> Input {
    let mut candidate = workspace_candidate(
        &labeled.label.trial,
        &root.join(workdir.trim_start_matches('/')),
    );
    candidate.origin = "replay".to_string();
    Input {
        task: TaskText {
            title: labeled.label.task.clone(),
            instruction: labeled.instruction.clone(),
        },
        requirements: labeled.requirements.clone(),
        candidate,
        observed: Observed::default(),
        budget: Budget {
            max_scenarios: 12,
            seconds: 1_800,
        },
        workspace: Some(Workspace {
            dir: workdir.to_string(),
            claimed: labeled.claimed.clone(),
            command_sec: 120,
            report: labeled.report.clone(),
            options,
            root: Some(root.to_string_lossy().into_owned()),
            collected: labeled.label.collected.clone(),
        }),
        distrust: Vec::new(),
    }
}

/// What one replay arm found on one trial.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub arm: String,
    pub flagged: bool,
    /// The failed scenarios, each with its first observation, clipped.
    pub failed: Vec<Value>,
    /// Scenario verdict counts.
    pub verdicts: BTreeMap<String, usize>,
}

/// One labeled trial's row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub label: Label,
    pub found: Vec<Found>,
}

fn found(arm: &str, report: &Report) -> Found {
    let mut verdicts = BTreeMap::new();
    for v in &report.verdicts {
        *verdicts.entry(v.verdict.clone()).or_default() += 1;
    }
    Found {
        arm: arm.to_string(),
        flagged: report.detected(),
        failed: report
            .verdicts
            .iter()
            .filter(|v| v.verdict == "failed")
            .map(|v| {
                json!({
                    "scenario": v.scenario,
                    "observation": v.observations.first().map(|o| crate::judge::clip(&o.to_string(), 300)),
                })
            })
            .collect(),
        verdicts,
    }
}

/// Checks one labeled trial under each arm, writing the reports under
/// `<out>/<job>/<trial>/`.
pub async fn check_trial(labeled: &Labeled, arms: &[String], out: &Path) -> Row {
    let dir = out.join(&labeled.label.job).join(&labeled.label.trial);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("label.json"),
        serde_json::to_string_pretty(&labeled.label).unwrap_or_default(),
    );
    let mut row = Row {
        label: labeled.label.clone(),
        found: Vec::new(),
    };
    if labeled.label.excluded.is_some() {
        return row;
    }
    let root = std::env::temp_dir().join(format!(
        "coder-one-replay-{}-{}",
        std::process::id(),
        labeled.label.trial
    ));
    for arm in arms {
        let Some(options) = arm_options(arm) else {
            continue;
        };
        // Each arm gets a fresh root: a scenario may rewrite outputs.
        let workdir = match build_root(labeled, &root) {
            Ok(workdir) => workdir,
            Err(_) => continue,
        };
        let input = input(labeled, &root, &workdir, options);
        let report = check(
            &input,
            &Recorder::default(),
            &dir.join(format!("scratch-{arm}")),
        )
        .await;
        let _ = std::fs::write(
            dir.join(format!("checks-{arm}.json")),
            serde_json::to_string_pretty(&report).unwrap_or_default(),
        );
        row.found.push(found(arm, &report));
    }
    let _ = std::fs::remove_dir_all(&root);
    row
}

/// Recall on failures and false alarms on passes, per arm, with the
/// episode's own first check as the baseline.
#[must_use]
pub fn summary(rows: &[Row], arms: &[String], targets: &[String]) -> Value {
    let graded: Vec<&Row> = rows.iter().filter(|r| r.label.excluded.is_none()).collect();
    let failures: Vec<&&Row> = graded.iter().filter(|r| r.label.failed()).collect();
    let passes: Vec<&&Row> = graded.iter().filter(|r| !r.label.failed()).collect();
    let rate = |n: usize, d: usize| if d == 0 { 0.0 } else { n as f64 / d as f64 };
    let flagged = |row: &Row, arm: &str| {
        if arm == "episode" {
            row.label.episode_flagged.unwrap_or(false)
        } else {
            row.found.iter().any(|f| f.arm == arm && f.flagged)
        }
    };
    let mut columns = vec!["episode".to_string()];
    columns.extend(arms.iter().cloned());
    let totals: Vec<Value> = columns
        .iter()
        .map(|arm| {
            let caught = failures.iter().filter(|r| flagged(r, arm)).count();
            let alarms = passes.iter().filter(|r| flagged(r, arm)).count();
            json!({
                "arm": arm,
                "failures": failures.len(),
                "flagged_failures": caught,
                "recall": rate(caught, failures.len()),
                "passes": passes.len(),
                "flagged_passes": alarms,
                "false_alarm_rate": rate(alarms, passes.len()),
            })
        })
        .collect();
    let target_rows: Vec<Value> = graded
        .iter()
        .filter(|r| targets.contains(&r.label.task))
        .map(|r| {
            json!({
                "task": r.label.task,
                "arm": r.label.arm,
                "reward": r.label.reward,
                "flagged": columns.iter().map(|a| (a.clone(), json!(flagged(r, a)))).collect::<serde_json::Map<_, _>>(),
            })
        })
        .collect();
    json!({
        "schema": RECALL_SCHEMA,
        "columns": columns,
        "totals": totals,
        "excluded": rows.iter().filter_map(|r| r.label.excluded.as_ref().map(|why| json!({ "job": r.label.job, "trial": r.label.trial, "why": why }))).collect::<Vec<_>>(),
        "targets": target_rows,
        "rows": graded.iter().map(|r| {
            json!({
                "job": r.label.job,
                "trial": r.label.trial,
                "task": r.label.task,
                "arm": r.label.arm,
                "reward": r.label.reward,
                "failed_tests": r.label.failed_tests.len(),
                "passed_tests": r.label.passed_tests,
                "flagged": columns.iter().map(|a| (a.clone(), json!(flagged(r, a)))).collect::<serde_json::Map<_, _>>(),
                "failed": r.found.iter().map(|f| (f.arm.clone(), json!(f.failed.iter().map(|x| x["scenario"].clone()).collect::<Vec<_>>()))).collect::<serde_json::Map<_, _>>(),
            })
        }).collect::<Vec<_>>(),
    })
}

/// The summary as text: one line per trial, then the totals.
#[must_use]
pub fn lines(summary: &Value) -> Vec<String> {
    let columns: Vec<String> = summary["columns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(str::to_string))
        .collect();
    let mut lines = vec![format!(
        "{:<30} {:<6} {:>6}  {}",
        "task",
        "arm",
        "reward",
        columns
            .iter()
            .map(|c| format!("{c:<8}"))
            .collect::<String>()
    )];
    for row in summary["rows"].as_array().into_iter().flatten() {
        let marks: String = columns
            .iter()
            .map(|c| {
                format!(
                    "{:<8}",
                    if row["flagged"][c] == true {
                        "flags"
                    } else {
                        "·"
                    }
                )
            })
            .collect();
        let last = columns.last().cloned().unwrap_or_default();
        let why: Vec<String> = row["failed"][&last]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect();
        lines.push(format!(
            "{:<30} {:<6} {:>6}  {marks}{}",
            row["task"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(30)
                .collect::<String>(),
            row["arm"]
                .as_str()
                .unwrap_or_default()
                .rsplit('-')
                .next()
                .unwrap_or_default(),
            row["reward"]
                .as_f64()
                .map_or("—".to_string(), |r| format!("{r:.1}")),
            why.join(" ")
        ));
    }
    lines.push(String::new());
    for total in summary["totals"].as_array().into_iter().flatten() {
        lines.push(format!(
            "{:<8} recall {}/{} ({:.0}%)   false alarms {}/{} ({:.0}%)",
            total["arm"].as_str().unwrap_or_default(),
            total["flagged_failures"],
            total["failures"],
            total["recall"].as_f64().unwrap_or(0.0) * 100.0,
            total["flagged_passes"],
            total["passes"],
            total["false_alarm_rate"].as_f64().unwrap_or(0.0) * 100.0,
        ));
    }
    let excluded = summary["excluded"].as_array().map_or(0, Vec::len);
    if excluded > 0 {
        lines.push(format!(
            "{excluded} trial(s) excluded: the agent never ran or the verifier left no reward."
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dockerfile_copies_come_from_the_build_context() {
        let (copies, workdir) = dockerfile(
            "FROM python:3.12\nCOPY --from=builder /x /usr/local/bin/x\nCOPY app/ /app/\nCOPY package.json tsconfig.json \\\n  /app/\nADD https://x.invalid/a.tgz /opt/\nWORKDIR /app\n",
        );
        assert_eq!(
            copies,
            vec![
                Copy {
                    sources: vec!["app/".to_string()],
                    destination: "/app/".to_string()
                },
                Copy {
                    sources: vec!["package.json".to_string(), "tsconfig.json".to_string()],
                    destination: "/app/".to_string()
                },
            ]
        );
        assert_eq!(workdir.as_deref(), Some("/app"));
    }

    #[test]
    fn the_instruction_loses_its_comments() {
        assert_eq!(
            public_instruction("<!-- canary -->\nDo the task.\n<!-- x -->"),
            "Do the task."
        );
    }

    #[test]
    fn a_root_holds_the_image_files_and_the_collected_outputs() {
        let base = std::env::temp_dir().join(format!("coder-one-labeled-{}", std::process::id()));
        let task = base.join("task");
        let trial = base.join("trial");
        std::fs::create_dir_all(task.join("environment/data")).unwrap();
        std::fs::write(
            task.join("environment/Dockerfile"),
            "FROM x\nWORKDIR /app\nCOPY data/ /app/data/\n",
        )
        .unwrap();
        std::fs::write(task.join("environment/data/in.csv"), "a\n").unwrap();
        std::fs::create_dir_all(trial.join("artifacts/results")).unwrap();
        std::fs::write(trial.join("artifacts/results/out.csv"), "b\n").unwrap();
        std::fs::write(
            trial.join("artifacts/manifest.json"),
            r#"[{"source": "/results/out.csv", "destination": "artifacts/results/out.csv", "type": "file", "status": "ok"}]"#,
        )
        .unwrap();
        let labeled = Labeled {
            label: Label {
                schema: LABEL_SCHEMA.to_string(),
                job: "j".to_string(),
                trial: "t".to_string(),
                task: "t".to_string(),
                arm: "a".to_string(),
                reward: Some(0.0),
                excluded: None,
                failed_tests: Vec::new(),
                passed_tests: 0,
                episode_flagged: None,
                collected: Vec::new(),
            },
            task_dir: task,
            trial_dir: trial,
            instruction: String::new(),
            requirements: None,
            report: None,
            claimed: Vec::new(),
        };
        let root = base.join("root");
        let workdir = build_root(&labeled, &root).unwrap();
        assert_eq!(workdir, "/app");
        assert_eq!(
            std::fs::read_to_string(root.join("app/data/in.csv")).unwrap(),
            "a\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("results/out.csv")).unwrap(),
            "b\n"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
