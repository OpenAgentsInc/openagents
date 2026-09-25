//! The offline measurement of `accept.grade` (issue #9635).
//!
//! For every retained Microluna trial whose lean loop kept its frozen
//! score script (`artifacts/lean-*/evaluator/score.sh`), this grades the
//! script as the freeze would, then runs an instrumented copy of it on
//! every graded workspace of the same task: each trial's final workspace
//! (`produced/app`) and each retained lean-loop candidate whose reward is
//! known. From the runs it reports, per script and per task, whether
//! ranking on the lines graded `follows` separates passing from failing
//! workspaces where the raw score doesn't, and, per trial, whether the
//! graded ranking would have changed the loop's keep-best decision.
//!
//! The scripts run on this host, not in the task's image: `/app` in the
//! script is rewritten to a fresh copy of the workspace, and `python` is
//! `python3`. Each script's own trial's recorded host scores are the
//! check that this host reproduces the image; a row that doesn't is
//! reported.
//!
//! ```text
//! coder-one accept grade [--traces DIR] [--grades DIR] [--out DIR]
//!                        [--jev live|recorded|off] [--recorded FILE]
//!                        [--workers N] [--score-sec N]
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::support;
use super::{Freeze, Grade, Grades, Parsed, Split};
use crate::component::jev::{JevMode, Recorded};
use crate::record::Recorder;

/// The schema of one run row.
pub const ROW_SCHEMA: &str = "openagents.coder-one.check-grades-offline-row.v1";

/// The schema of the summary.
pub const SUMMARY_SCHEMA: &str = "openagents.coder-one.check-grades-offline.v1";

/// The command's usage.
pub const USAGE: &str = "usage: coder-one accept grade [--traces DIR] [--grades DIR] [--out DIR]
                              [--jev live|recorded|off] [--recorded FILE]
                              [--workers N] [--score-sec N]
                              [--image TASK=IMAGE]... [--exclude-task NAME]...

grade grades every retained frozen score script under --traces with
accept.grade, runs an instrumented copy of each on every graded workspace
of its task, in the task's image when --image names one and on this
host otherwise, and writes the rows and a summary under --out:
whether ranking on the lines graded `follows` separates passing from
failing workspaces where the raw score doesn't, and whether it would have
changed a keep-best decision. Live Jev answers are saved to --recorded
(default OUT/jev-recorded.json) so a later run with --jev recorded
replays them.

--exclude-task omits a task's trials before any of their records are read,
such as tasks whose outcomes a prospective protocol seals.";

/// One graded workspace of a task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    /// `job/trial` for a final, `job/trial/lean-1/session-2` for a
    /// candidate.
    pub id: String,
    pub task: String,
    pub job: String,
    pub trial: String,
    /// `final` or `candidate`.
    pub kind: String,
    /// The session, for a candidate.
    pub session: Option<u32>,
    pub dir: PathBuf,
    pub reward: Option<f64>,
    /// Where the reward comes from: `verifier`, `grade`, or `submitted`.
    pub reward_source: Option<String>,
    /// The digest of its Python source files, which dedupes a final and
    /// the candidate it was.
    pub source_digest: String,
    /// The workspace this one duplicates, when an earlier one has the
    /// same source.
    pub dup_of: Option<String>,
}

/// One retained trial.
#[derive(Clone, Debug)]
struct Trial {
    job: String,
    trial: String,
    task: String,
    episode: PathBuf,
    reward: Option<f64>,
    instruction: String,
    /// The frozen script, when the lean loop kept one.
    script: Option<PathBuf>,
    /// The lean loop's selection moves.
    moves: Vec<Value>,
    /// Whether keep-best took a later tie (`protect_candidates` off).
    ties: Option<bool>,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// The digest of a directory's Python source files, `__pycache__` aside.
#[must_use]
pub fn source_digest(dir: &Path) -> String {
    let mut files = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if name != "__pycache__" && !name.starts_with('.') {
                    stack.push(path);
                }
            } else if name.ends_with(".py")
                && let Ok(bytes) = std::fs::read(&path)
            {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                files.insert(rel, crate::accept::sha256(&bytes));
            }
        }
    }
    atif::digest(&json!(files))
}

/// The arm a job name carries: `tb4--coder-one-<arm>--<task>…`.
fn arm(job: &str) -> Option<&str> {
    job.strip_prefix("tb4--coder-one-")?.split("--").next()
}

/// Whether the arm's lean loop took later ties: `protect_candidates` off.
fn ties(policies: &Path, job: &str) -> Option<bool> {
    let policy = read_json(&policies.join(format!("{}.json", arm(job)?)))?;
    let lean = policy
        .pointer("/policy/executor/microluna/lean")
        .or_else(|| policy.pointer("/executor/microluna/lean"))?;
    Some(!lean["protect_candidates"].as_bool().unwrap_or(false))
}

fn trials(traces: &Path, policies: &Path, excluded: &[String]) -> Vec<Trial> {
    let mut out = Vec::new();
    let mut jobs: Vec<PathBuf> = std::fs::read_dir(traces)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    jobs.sort();
    for job_dir in jobs {
        let job = job_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !job.contains("microluna")
            || job.contains("truth-confirmation")
            || job.contains("truth-control")
        {
            continue;
        }
        let mut episodes: Vec<PathBuf> = std::fs::read_dir(&job_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "episode"))
            .collect();
        episodes.sort();
        for episode in episodes {
            let trial = episode
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let task = trial.split("__").next().unwrap_or_default().to_string();
            if excluded.contains(&task) {
                continue;
            }
            let reward = std::fs::read_to_string(episode.join("verifier/reward.txt"))
                .ok()
                .and_then(|t| t.trim().parse().ok());
            let instruction = read_json(&episode.join("artifacts/briefing-pack.json"))
                .and_then(|b| b["inputs"]["instruction"].as_str().map(str::to_string))
                .unwrap_or_default();
            let lean = std::fs::read_dir(episode.join("artifacts"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("lean-"))
                })
                .min();
            let script = lean
                .as_ref()
                .map(|l| l.join("evaluator/score.sh"))
                .filter(|p| p.is_file());
            let moves = lean
                .as_ref()
                .and_then(|l| read_json(&l.join("selection.json")))
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            out.push(Trial {
                ties: ties(policies, &job),
                job: job.clone(),
                trial,
                task,
                episode,
                reward,
                instruction,
                script,
                moves,
            });
        }
    }
    out
}

/// The lean moves after sessions, in order.
fn session_moves(moves: &[Value]) -> Vec<&Value> {
    moves
        .iter()
        .filter(|m| m["kind"] == "lean" && m["after_session"].as_u64().is_some())
        .collect()
}

fn workspaces(trials: &[Trial], grades: &Path) -> Vec<Workspace> {
    let mut out: Vec<Workspace> = Vec::new();
    for t in trials {
        let final_dir = t.episode.join("produced/app");
        if final_dir.is_dir() {
            out.push(Workspace {
                id: format!("{}/{}", t.job, t.trial),
                task: t.task.clone(),
                job: t.job.clone(),
                trial: t.trial.clone(),
                kind: "final".to_string(),
                session: None,
                source_digest: source_digest(&final_dir),
                dir: final_dir,
                reward: t.reward,
                reward_source: t.reward.map(|_| "verifier".to_string()),
                dup_of: None,
            });
        }
        let moves = session_moves(&t.moves);
        let submitted = moves
            .iter()
            .rev()
            .find(|m| m["kept"] == true)
            .map(|m| m["workspace_files"].clone());
        for m in &moves {
            let Some(session) = m["after_session"].as_u64() else {
                continue;
            };
            let Some(candidate) = m["candidate"].as_str() else {
                continue;
            };
            let name = Path::new(candidate)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let group = Path::new(candidate)
                .parent()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let dir = t.episode.join("artifacts").join(&group).join(&name);
            if !dir.is_dir() {
                continue;
            }
            let files = &m["workspace_files"];
            let graded = read_json(&grades.join(&t.job).join(&name).join("grade.json"))
                .filter(|g| !files.is_null() && g["workspace_files"] == *files)
                .and_then(|g| g["grade"]["reward"].as_f64());
            let (reward, source) = match graded {
                Some(r) => (Some(r), Some("grade")),
                None if !files.is_null() && submitted.as_ref() == Some(files) => {
                    (t.reward, t.reward.map(|_| "submitted"))
                }
                None => (None, None),
            };
            out.push(Workspace {
                id: format!("{}/{}/{group}/{name}", t.job, t.trial),
                task: t.task.clone(),
                job: t.job.clone(),
                trial: t.trial.clone(),
                kind: "candidate".to_string(),
                session: u32::try_from(session).ok(),
                source_digest: source_digest(&dir),
                dir,
                reward,
                reward_source: source.map(str::to_string),
                dup_of: None,
            });
        }
    }
    // Workspaces with the same source are one: the first with a known
    // reward leads, candidates before finals, so a final defers to the
    // candidate it was.
    out.sort_by_key(|w| (w.task.clone(), w.kind != "candidate", w.id.clone()));
    let mut leaders: BTreeMap<(String, String), String> = BTreeMap::new();
    for w in out.iter().filter(|w| w.reward.is_some()).chain(out.iter()) {
        leaders
            .entry((w.task.clone(), w.source_digest.clone()))
            .or_insert_with(|| w.id.clone());
    }
    for w in &mut out {
        let leader = &leaders[&(w.task.clone(), w.source_digest.clone())];
        if *leader != w.id {
            w.dup_of = Some(leader.clone());
        }
    }
    out
}

/// One run of one script on one workspace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub schema: String,
    pub task: String,
    /// The script's trial, `job/trial`.
    pub script: String,
    pub workspace: String,
    pub kind: String,
    pub session: Option<u32>,
    /// The script ran on its own trial's workspace.
    pub own: bool,
    pub dup_of: Option<String>,
    pub reward: Option<f64>,
    pub reward_source: Option<String>,
    pub score: Option<(u64, u64)>,
    pub supported: Option<(u64, u64)>,
    /// Each check line's result; empty when unknown.
    pub lines: BTreeMap<String, bool>,
    /// The host score the trial recorded for this candidate, for its own
    /// script.
    pub recorded: Option<(u64, u64)>,
    pub exit: Option<i32>,
    pub ms: u64,
    pub tail: String,
}

/// Rewrites `/app` in a script to `dir`.
fn rebased(script: &str, dir: &Path) -> String {
    let to = dir.display().to_string();
    script.replace("/app", &to)
}

async fn run_one(
    text: &str,
    workspace: &Path,
    scratch: &Path,
    shim: &Path,
    image: Option<&str>,
    wall: Duration,
) -> (Option<(u64, u64)>, String, Option<i32>, u64) {
    let copy = scratch.join("app");
    if let Err(error) = crate::handoff::copy_tree(workspace, &copy) {
        return (None, format!("copy failed: {error}"), None, 0);
    }
    let script = scratch.join("score.sh");
    let body = if image.is_some() {
        text.to_string()
    } else {
        rebased(text, &copy)
    };
    if let Err(error) = std::fs::write(&script, body) {
        return (None, error.to_string(), None, 0);
    }
    let mut command = if let Some(image) = image {
        // The task's image, with the workspace at `/app`, no network, and
        // the bound enforced inside the container too.
        let mut command = std::process::Command::new("docker");
        command.args(["run", "--rm", "--network", "none", "-w", "/app"]);
        command
            .arg("-v")
            .arg(format!("{}:/app", copy.display()))
            .arg("-v")
            .arg(format!("{}:/oa-score.sh:ro", script.display()))
            .args(["-e", "PYTHONDONTWRITEBYTECODE=1", image, "timeout"])
            .arg(wall.as_secs().to_string())
            .args(["/bin/sh", "/oa-score.sh"]);
        command
    } else {
        let path = format!(
            "{}:{}",
            shim.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut command = std::process::Command::new("/bin/sh");
        command
            .arg(&script)
            .current_dir(&copy)
            .env("PATH", path)
            .env("PYTHONDONTWRITEBYTECODE", "1");
        command
    };
    microluna::tools::withhold_credentials(&mut command);
    let wall = wall + Duration::from_secs(if image.is_some() { 30 } else { 0 });
    let started = std::time::Instant::now();
    let ended = supervise::Job::from_command(command)
        .bounded(
            supervise::Limits::within(wall)
                .keeping(256 * 1024)
                .memory(None),
        )
        .run()
        .await;
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let stdout = ended.stdout.marked();
    let stderr = ended.stderr.marked();
    let score = ended
        .ending
        .success()
        .then(|| crate::micro::lean::parse_score(&stdout))
        .flatten();
    let _ = std::fs::remove_dir_all(scratch);
    (
        score,
        format!("{stdout}\n{stderr}\n{:?}", ended.ending),
        ended.ending.code(),
        ms,
    )
}

/// Runs `text` in `image` on the image's own `/app`, the untouched
/// workspace, with no network.
async fn run_untouched(
    text: &str,
    scratch: &Path,
    image: &str,
    wall: Duration,
) -> (Option<(u64, u64)>, String, Option<i32>, u64) {
    let _ = std::fs::create_dir_all(scratch);
    let script = scratch.join("score.sh");
    if let Err(error) = std::fs::write(&script, text) {
        return (None, error.to_string(), None, 0);
    }
    let mut command = std::process::Command::new("docker");
    command
        .args(["run", "--rm", "--network", "none", "-w", "/app", "-v"])
        .arg(format!("{}:/oa-score.sh:ro", script.display()))
        .args(["-e", "PYTHONDONTWRITEBYTECODE=1", image, "timeout"])
        .arg(wall.as_secs().to_string())
        .args(["/bin/sh", "/oa-score.sh"]);
    microluna::tools::withhold_credentials(&mut command);
    let started = std::time::Instant::now();
    let ended = supervise::Job::from_command(command)
        .bounded(
            supervise::Limits::within(wall + Duration::from_secs(30))
                .keeping(256 * 1024)
                .memory(None),
        )
        .run()
        .await;
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let stdout = ended.stdout.marked();
    let stderr = ended.stderr.marked();
    let score = ended
        .ending
        .success()
        .then(|| crate::micro::lean::parse_score(&stdout))
        .flatten();
    let _ = std::fs::remove_dir_all(scratch);
    (
        score,
        format!("{stdout}\n{stderr}"),
        ended.ending.code(),
        ms,
    )
}

/// The share of (pass, fail) pairs a key orders correctly, ties counting
/// half: 1 is perfect separation, 0.5 is none.
#[must_use]
pub fn auc<K: PartialOrd>(passes: &[K], fails: &[K]) -> Option<f64> {
    if passes.is_empty() || fails.is_empty() {
        return None;
    }
    let mut sum = 0.0;
    for p in passes {
        for f in fails {
            sum += match p.partial_cmp(f) {
                Some(std::cmp::Ordering::Greater) => 1.0,
                Some(std::cmp::Ordering::Equal) => 0.5,
                _ => 0.0,
            };
        }
    }
    Some(sum / (passes.len() * fails.len()) as f64)
}

/// Wilson's 95% interval for `k` of `n`.
#[must_use]
pub fn wilson(k: usize, n: usize) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let z = 1.959_964_f64;
    let n = n as f64;
    let p = k as f64 / n;
    let d = 1.0 + z * z / n;
    let c = p + z * z / (2.0 * n);
    let m = z * ((p * (1.0 - p) + z * z / (4.0 * n)) / n).sqrt();
    Some(((c - m) / d, (c + m) / d))
}

fn frac(score: Option<(u64, u64)>) -> f64 {
    super::fraction(score)
}

/// The summary for one script over its task's deduplicated workspaces.
fn script_summary(rows: &[&Row]) -> Value {
    let known: Vec<&&Row> = rows
        .iter()
        .filter(|r| r.dup_of.is_none() && r.reward.is_some())
        .collect();
    let pass = |r: &&&Row| r.reward.is_some_and(|x| x >= 1.0);
    let raw_p: Vec<f64> = known
        .iter()
        .filter(|r| pass(r))
        .map(|r| frac(r.score))
        .collect();
    let raw_f: Vec<f64> = known
        .iter()
        .filter(|r| !pass(r))
        .map(|r| frac(r.score))
        .collect();
    let key_p: Vec<(f64, f64)> = known
        .iter()
        .filter(|r| pass(r))
        .map(|r| super::key(r.supported, r.score))
        .collect();
    let key_f: Vec<(f64, f64)> = known
        .iter()
        .filter(|r| !pass(r))
        .map(|r| super::key(r.supported, r.score))
        .collect();
    let full = |r: &&&Row| r.score.is_some_and(|(p, t)| t > 0 && p == t);
    let supported_full = |r: &&&Row| r.supported.is_some_and(|(p, t)| p == t);
    let fails = known.iter().filter(|r| !pass(r)).count();
    let passes = known.len() - fails;
    let raw_auc = auc(&raw_p, &raw_f);
    let graded_auc = auc(&key_p, &key_f);
    json!({
        "workspaces": known.len(),
        "passes": passes,
        "fails": fails,
        "full_on_fails": known.iter().filter(|r| !pass(r) && full(r)).count(),
        "full_on_passes": known.iter().filter(|r| pass(r) && full(r)).count(),
        "supported_full_on_fails": known.iter().filter(|r| !pass(r) && supported_full(r)).count(),
        "supported_full_on_passes": known.iter().filter(|r| pass(r) && supported_full(r)).count(),
        "raw_auc": raw_auc,
        "graded_auc": graded_auc,
        "raw_separates": raw_auc.is_some_and(|a| a >= 1.0),
        "graded_separates": graded_auc.is_some_and(|a| a >= 1.0),
        "graded_separates_where_raw_did_not": graded_auc.is_some_and(|a| a >= 1.0)
            && !raw_auc.is_some_and(|a| a >= 1.0),
    })
}

/// Replays one trial's keep-best decisions with the raw and the graded
/// key, from the trial's own script on its own candidates.
fn keep_best(trial: &Trial, rows: &[&Row]) -> Value {
    let ties = trial.ties.unwrap_or(true);
    let mut sessions: Vec<&&Row> = rows
        .iter()
        .filter(|r| r.own && r.kind == "candidate")
        .collect();
    sessions.sort_by_key(|r| r.session);
    let mut raw: Option<(u32, f64)> = None;
    let mut graded: Option<(u32, (f64, f64))> = None;
    let mut decisions = Vec::new();
    for r in &sessions {
        let session = r.session.unwrap_or(0);
        let f = frac(r.score);
        let k = super::key(r.supported, r.score);
        let raw_keep = raw.is_none_or(|(_, b)| if ties { f >= b } else { f > b });
        let graded_keep = graded.is_none_or(|(_, b)| super::ahead(k, b, ties));
        if raw_keep {
            raw = Some((session, f));
        }
        if graded_keep {
            graded = Some((session, k));
        }
        decisions.push(json!({
            "session": session,
            "score": r.score,
            "supported": r.supported,
            "raw_keeps": raw_keep,
            "graded_keeps": graded_keep,
        }));
    }
    let recorded: Vec<Value> = session_moves(&trial.moves)
        .iter()
        .map(|m| json!({"session": m["after_session"], "kept": m["kept"]}))
        .collect();
    json!({
        "trial": format!("{}/{}", trial.job, trial.trial),
        "task": trial.task,
        "ties": ties,
        "decisions": decisions,
        "recorded": recorded,
        "raw_final": raw.map(|(s, _)| s),
        "graded_final": graded.map(|(s, _)| s),
        "changed": raw.map(|(s, _)| s) != graded.map(|(s, _)| s),
        "any_decision_changed": decisions.iter().any(|d| d["raw_keeps"] != d["graded_keeps"]),
    })
}

/// Options for [`measure`].
pub struct Options {
    pub traces: PathBuf,
    pub grades: PathBuf,
    pub policies: PathBuf,
    pub out: PathBuf,
    pub workers: usize,
    pub score_sec: u64,
    /// The image each task runs in; a task without one runs on this host.
    pub images: BTreeMap<String, String>,
    /// Tasks whose trials are never read, given on the command line so no
    /// benchmark task name sits in the source the contamination guard
    /// scans.
    pub excluded_tasks: Vec<String>,
}

/// Grades every frozen script, runs each on its task's workspaces, and
/// writes the rows, the grade records, and the summary under `out`.
///
/// # Errors
///
/// A message when `out` can't be written.
#[allow(clippy::too_many_lines)]
pub async fn measure(
    jev: &JevMode,
    recorder: &Recorder,
    options: &Options,
) -> Result<Value, String> {
    std::fs::create_dir_all(options.out.join("grades")).map_err(|e| e.to_string())?;
    let shim = std::env::temp_dir().join(format!("accept-grade-bin-{}", std::process::id()));
    std::fs::create_dir_all(&shim).map_err(|e| e.to_string())?;
    let python3 = std::process::Command::new("/bin/sh")
        .args(["-c", "command -v python3"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|p| !p.is_empty())
        .ok_or("python3 isn't on PATH")?;
    let link = shim.join("python");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&python3, &link).map_err(|e| e.to_string())?;
    let trials = trials(&options.traces, &options.policies, &options.excluded_tasks);
    let spaces = workspaces(&trials, &options.grades);
    let mut graded: Vec<(Trial, Grades, Parsed, String)> = Vec::new();
    let mut jev_usd = 0.0;
    for t in trials.iter().filter(|t| t.script.is_some()) {
        let Some(path) = &t.script else { continue };
        let script = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let group = path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        // Each line on the untouched workspace, the image's own `/app`,
        // for its authority class (#9629).
        let split = super::split(&script);
        let start = match (
            super::instrument(&script, &split),
            options.images.get(&t.task),
        ) {
            (Some(text), Some(image)) => {
                let scratch = std::env::temp_dir().join(format!(
                    "accept-grade-start-{}-{}",
                    std::process::id(),
                    t.trial
                ));
                let (score, output, _, _) = run_untouched(
                    &text,
                    &scratch,
                    image,
                    Duration::from_secs(options.score_sec),
                )
                .await;
                score.map(|_| super::results(&output, &split))
            }
            _ => None,
        };
        let (grades, parsed, _) = super::grade(
            jev,
            recorder,
            &support::Context {
                component: super::COMPONENT,
                name: super::DECISION,
                id: format!("grade-{}", t.trial),
                deadline: None,
            },
            &Freeze {
                check: format!("{group}/evaluator/score.sh"),
                script: &script,
                frozen_after_session: 1,
                task: &t.instruction,
                baseline: None,
                start: start.as_ref(),
            },
        )
        .await;
        jev_usd += grades.jev_usd;
        let text = serde_json::to_string_pretty(&grades).map_err(|e| e.to_string())?;
        crate::record::write_atomic(
            &options
                .out
                .join("grades")
                .join(format!("{}--{}.json", t.job, t.trial)),
            format!("{text}\n").as_bytes(),
        )?;
        let instrumented = super::instrument(&script, &parsed).unwrap_or(script);
        graded.push((t.clone(), grades, parsed, instrumented));
    }
    // Every script on every workspace of its task: deduplicated
    // workspaces with a known reward, and its own trial's candidates.
    let mut jobs = Vec::new();
    for (n, (t, _, _, _)) in graded.iter().enumerate() {
        for w in &spaces {
            if w.task != t.task {
                continue;
            }
            let own = w.job == t.job && w.trial == t.trial;
            let counted = w.dup_of.is_none() && w.reward.is_some();
            if counted || (own && w.kind == "candidate") {
                jobs.push((n, w.clone(), own));
            }
        }
    }
    let scratch_root = std::env::temp_dir().join(format!("accept-grade-{}", std::process::id()));
    let wall = Duration::from_secs(options.score_sec);
    let total = jobs.len();
    let mut rows: Vec<Row> = futures_util::stream::iter(jobs.into_iter().enumerate())
        .map(|(k, (n, w, own))| {
            let (t, grades, parsed, instrumented) = &graded[n];
            let scratch = scratch_root.join(format!("{k}"));
            let shim = shim.clone();
            async move {
                let image = options.images.get(&t.task).map(String::as_str);
                let (score, output, exit, ms) =
                    run_one(instrumented, &w.dir, &scratch, &shim, image, wall).await;
                let lines = if score.is_some() {
                    super::results(&output, parsed)
                } else {
                    BTreeMap::new()
                };
                let recorded = own
                    .then(|| {
                        session_moves(&t.moves)
                            .iter()
                            .find(|m| m["after_session"].as_u64() == w.session.map(u64::from))
                            .and_then(|m| {
                                Some((
                                    m["score"]["passed"].as_u64()?,
                                    m["score"]["total"].as_u64()?,
                                ))
                            })
                    })
                    .flatten();
                eprintln!(
                    "  accept.grade ▸ {k}/{total} {} on {}: {:?}",
                    t.trial, w.id, score
                );
                Row {
                    schema: ROW_SCHEMA.to_string(),
                    task: t.task.clone(),
                    script: format!("{}/{}", t.job, t.trial),
                    workspace: w.id.clone(),
                    kind: w.kind.clone(),
                    session: w.session,
                    own,
                    dup_of: w.dup_of.clone(),
                    reward: w.reward,
                    reward_source: w.reward_source.clone(),
                    supported: grades.supported(&lines),
                    score,
                    lines,
                    recorded,
                    exit,
                    ms,
                    tail: crate::judge::clip(
                        super::without_marks(&output)
                            .lines()
                            .rev()
                            .take(6)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect::<Vec<_>>()
                            .join("\n")
                            .trim(),
                        600,
                    ),
                }
            }
        })
        .buffer_unordered(options.workers.max(1))
        .collect()
        .await;
    let _ = std::fs::remove_dir_all(&scratch_root);
    let _ = std::fs::remove_dir_all(&shim);
    rows.sort_by(|a, b| {
        (&a.task, &a.script, &a.workspace).cmp(&(&b.task, &b.script, &b.workspace))
    });
    // Each trial's record as the loop would have written it: its own
    // candidates' line results, in session order.
    for (t, grades, _, _) in &mut graded {
        let id = format!("{}/{}", t.job, t.trial);
        let mut own: Vec<&Row> = rows
            .iter()
            .filter(|r| r.script == id && r.own && r.kind == "candidate")
            .collect();
        own.sort_by_key(|r| r.session);
        for r in own {
            grades.record(r.session.unwrap_or(0), &r.lines);
        }
        let text = serde_json::to_string_pretty(grades).map_err(|e| e.to_string())?;
        crate::record::write_atomic(
            &options
                .out
                .join("grades")
                .join(format!("{}--{}.json", t.job, t.trial)),
            format!("{text}\n").as_bytes(),
        )?;
    }
    let mut text = String::new();
    for row in &rows {
        text.push_str(&serde_json::to_string(row).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    crate::record::write_atomic(&options.out.join("rows.jsonl"), text.as_bytes())?;
    // Summaries.
    let tasks: BTreeSet<&str> = graded.iter().map(|(t, ..)| t.task.as_str()).collect();
    let mut by_task = serde_json::Map::new();
    for task in &tasks {
        let mut scripts = Vec::new();
        for (t, grades, _, _) in graded.iter().filter(|(t, ..)| t.task == *task) {
            let id = format!("{}/{}", t.job, t.trial);
            let mine: Vec<&Row> = rows.iter().filter(|r| r.script == id).collect();
            let reproduced = mine
                .iter()
                .filter(|r| r.recorded.is_some())
                .map(|r| r.recorded == r.score)
                .collect::<Vec<_>>();
            let count = |g: Grade| grades.lines.iter().filter(|l| l.grade == g).count();
            let mut summary = script_summary(&mine);
            summary["script"] = json!(id);
            summary["split"] = json!(grades.split);
            summary["lines"] = json!(grades.lines.len());
            summary["follows"] = json!(count(Grade::Follows));
            summary["advisory"] = json!(count(Grade::Advisory));
            summary["unknown"] = json!(count(Grade::Unknown));
            summary["reproduced"] = json!(format!(
                "{} of {}",
                reproduced.iter().filter(|x| **x).count(),
                reproduced.len()
            ));
            scripts.push(summary);
        }
        let workspaces: Vec<&Workspace> = spaces
            .iter()
            .filter(|w| w.task == *task && w.dup_of.is_none() && w.reward.is_some())
            .collect();
        let passes = workspaces
            .iter()
            .filter(|w| w.reward.is_some_and(|r| r >= 1.0))
            .count();
        let with_both = scripts.iter().filter(|s| s["raw_auc"].is_number()).count();
        let improved = scripts
            .iter()
            .filter(|s| s["graded_separates_where_raw_did_not"] == true)
            .count();
        let decisions: Vec<Value> = graded
            .iter()
            .filter(|(t, ..)| t.task == *task)
            .map(|(t, ..)| {
                let id = format!("{}/{}", t.job, t.trial);
                let mine: Vec<&Row> = rows.iter().filter(|r| r.script == id).collect();
                keep_best(t, &mine)
            })
            .collect();
        by_task.insert(
            (*task).to_string(),
            json!({
                "workspaces": workspaces.len(),
                "passes": passes,
                "fails": workspaces.len() - passes,
                "scripts": scripts,
                "scripts_with_both_outcomes": with_both,
                "scripts_where_graded_separates_and_raw_did_not": improved,
                "keep_best": decisions,
                "keep_best_changed": decisions.iter().filter(|d| d["changed"] == true).count(),
            }),
        );
    }
    let missing: Vec<Value> = trials
        .iter()
        .filter(|t| t.script.is_none())
        .map(|t| json!({"trial": format!("{}/{}", t.job, t.trial), "task": t.task, "reward": t.reward}))
        .collect();
    let summary = json!({
        "schema": SUMMARY_SCHEMA,
        "threshold": super::THRESHOLD,
        "question_set": support::support_set().id,
        "question_digest": support::support_set().digest,
        "baseline": false,
        "jev": jev.word(),
        "jev_usd": jev_usd,
        "trials": trials.len(),
        "scripts": graded.len(),
        "one_unit": graded.iter().filter(|(_, g, ..)| g.split == Split::OneUnit).count(),
        "runs": rows.len(),
        "workspaces": spaces.iter().filter(|w| w.dup_of.is_none() && w.reward.is_some()).count(),
        "without_script": missing,
        "tasks": by_task,
    });
    let text = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    crate::record::write_atomic(
        &options.out.join("summary.json"),
        format!("{text}\n").as_bytes(),
    )?;
    // Directories relative to --traces, so the record names no host path.
    let shown: Vec<Workspace> = spaces
        .iter()
        .cloned()
        .map(|mut w| {
            w.dir = w
                .dir
                .strip_prefix(&options.traces)
                .map(Path::to_path_buf)
                .unwrap_or(w.dir);
            w
        })
        .collect();
    let text = serde_json::to_string_pretty(&shown).map_err(|e| e.to_string())?;
    crate::record::write_atomic(
        &options.out.join("workspaces.json"),
        format!("{text}\n").as_bytes(),
    )?;
    Ok(summary)
}

/// Runs `coder-one accept grade`.
///
/// # Errors
///
/// A message for bad arguments or a run that can't write its records.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut options = Options {
        traces: root.join("bench/terminal-bench/traces"),
        grades: root.join(
            "bench/terminal-bench/experiments/2026-09-24-candidate-evidence/records/candidate-grades",
        ),
        policies: root.join("crates/coder-one/policies"),
        out: std::env::temp_dir().join("accept-grade"),
        workers: 4,
        score_sec: 120,
        images: BTreeMap::new(),
        excluded_tasks: Vec::new(),
    };
    let mut jev = "live".to_string();
    let mut recorded_path = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--traces" => options.traces = PathBuf::from(value()?),
            "--grades" => options.grades = PathBuf::from(value()?),
            "--policies" => options.policies = PathBuf::from(value()?),
            "--out" => options.out = PathBuf::from(value()?),
            "--jev" => jev = value()?,
            "--image" => {
                let v = value()?;
                let (task, image) = v
                    .split_once('=')
                    .ok_or_else(|| format!("--image takes TASK=IMAGE, not {v}"))?;
                options.images.insert(task.to_string(), image.to_string());
            }
            "--exclude-task" => options.excluded_tasks.push(value()?),
            "--recorded" => recorded_path = Some(PathBuf::from(value()?)),
            "--workers" => {
                options.workers = value()?.parse().map_err(|e| format!("--workers: {e}"))?
            }
            "--score-sec" => {
                options.score_sec = value()?.parse().map_err(|e| format!("--score-sec: {e}"))?;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(0);
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
    }
    let recorded_path = recorded_path.unwrap_or_else(|| options.out.join("jev-recorded.json"));
    let mode = match jev.as_str() {
        "live" => JevMode::Live(crate::component::cli::live_client()?),
        "recorded" => JevMode::Recorded(Recorded::load(&recorded_path)?),
        "off" => JevMode::Off,
        other => return Err(format!("--jev takes live, recorded, or off, not {other}")),
    };
    let recorder = Recorder::default();
    let summary = measure(&mode, &recorder, &options).await?;
    if jev == "live" {
        let mut recorded = if recorded_path.is_file() {
            Recorded::load(&recorded_path)?
        } else {
            Recorded::empty()
        };
        let added = crate::component::jev::record_answers(
            &recorder.steps(),
            "live accept.grade offline",
            &mut recorded,
        );
        recorded.save(&recorded_path)?;
        eprintln!("recorded {added} answers in {}", recorded_path.display());
    }
    for (task, t) in summary["tasks"].as_object().into_iter().flatten() {
        println!(
            "{task}: {} workspaces ({} pass, {} fail); keep-best changed on {} of {} trials",
            t["workspaces"],
            t["passes"],
            t["fails"],
            t["keep_best_changed"],
            t["keep_best"].as_array().map_or(0, Vec::len)
        );
        for s in t["scripts"].as_array().into_iter().flatten() {
            println!(
                "  {}: {} lines ({} follows, {} advisory, {} unknown), raw AUC {}, graded AUC {}, reproduced {}",
                s["script"].as_str().unwrap_or_default(),
                s["lines"],
                s["follows"],
                s["advisory"],
                s["unknown"],
                s["raw_auc"],
                s["graded_auc"],
                s["reproduced"].as_str().unwrap_or_default(),
            );
        }
    }
    println!("records in {}", options.out.display());
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auc_counts_ties_half() {
        assert_eq!(auc(&[1.0], &[0.5]), Some(1.0));
        assert_eq!(auc(&[1.0], &[1.0]), Some(0.5));
        assert_eq!(auc(&[(1.0, 0.5)], &[(0.5, 1.0)]), Some(1.0));
        assert_eq!(auc::<f64>(&[], &[1.0]), None);
    }

    #[test]
    fn wilson_brackets_the_rate() {
        let (lo, hi) = wilson(13, 18).expect("an interval");
        assert!(lo < 13.0 / 18.0 && 13.0 / 18.0 < hi);
        assert!(wilson(0, 0).is_none());
    }

    #[test]
    fn the_arm_comes_from_the_job_name() {
        assert_eq!(
            arm("tb4--coder-one-microluna-v13-retained--embedding-drift-monitor-2"),
            Some("microluna-v13-retained")
        );
        assert_eq!(arm("smoke--nop"), None);
    }
}
