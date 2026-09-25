//! Offline validity: does a green suite predict a verifier pass?
//!
//! For a Terminal-Bench task with retained, graded trials, [`task`] writes
//! a suite from the task's words alone with Microluna, in the task's own
//! environment image, then runs the frozen suite against every retained
//! workspace restored into a fresh container of that image. [`validity`]
//! joins the results with the check-truth label rows and counts how often
//! "suite green" agrees with the verifier, beside today's checks and the
//! combined verdict on the same trials. A trial whose verifier reward is
//! unknown is counted apart and never as a failure.
//!
//! [`trials`] finds four kinds of workspace ([`Kind`]):
//!
//! - **Snapshot.** A Coder One trial's post-executor snapshot
//!   (`agent/episode/snapshot/workspace.tar.gz`). The snapshot is the
//!   workspace right after the first executor. A trial whose later rounds
//!   changed the workspace was graded on a different candidate, so each
//!   trial says whether its check candidates all share one digest
//!   ([`Trial::snapshot_graded`]).
//! - **Final.** A Microluna trial's final workspace: the image's working
//!   directory with the deliverables Harbor collected at the end of the
//!   trial (`artifacts/manifest.json`) copied over it. A file the trial
//!   deleted is still there, and a file Harbor didn't collect is the
//!   image's. A deliverable Harbor couldn't collect was missing when the
//!   trial ended, so a trial whose collection found nothing is the image's
//!   working directory; a trial with no collection record isn't read.
//! - **Candidate.** A lean-loop candidate that Microluna retained under
//!   `agent/episode/artifacts/lean-*/session-N`, checked against the file
//!   identity `selection.json` recorded for it. A snapshot in the Git
//!   scope holds only the files Git lists, so it goes over the image's
//!   working directory instead of replacing it. Its reward is known when
//!   its files are the submitted workspace's, or when a grade record from
//!   `tbench candidates` or the candidate-evidence experiment matches its
//!   files ([`grades`]).
//! - **Reconstruction.** A final workspace with a reconstruction's source
//!   files over it, graded by the verifier on its own
//!   ([`reconstruction`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use microluna::{Config, Isolation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::runner::{Docker, docker};
use super::{AcceptanceSuite, Inputs, MicrolunaWriter, Options, RunResult, Task, define, run};
use crate::checks::truth::{self, Says};
use crate::checks::verdict;
use crate::component::jev::JevMode;
use crate::record::Recorder;

/// The schema of one task's validity record.
pub const SCHEMA: &str = "openagents.coder-one.acceptance-validity.v1";

/// Where a retained workspace comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A Coder One trial's post-executor snapshot.
    #[default]
    Snapshot,
    /// A Microluna trial's final workspace: the image's working directory
    /// with the trial's collected artifacts over it.
    Final,
    /// A lean-loop candidate that Microluna retained.
    Candidate,
    /// A final workspace with a reconstruction's source files over it.
    Reconstruction,
}

impl Kind {
    /// The kind a word names: `snapshot`, `final`, `candidate`, or
    /// `reconstruction`.
    ///
    /// # Errors
    ///
    /// A message for any other word.
    pub fn parse(word: &str) -> Result<Kind, String> {
        match word {
            "snapshot" => Ok(Kind::Snapshot),
            "final" => Ok(Kind::Final),
            "candidate" => Ok(Kind::Candidate),
            "reconstruction" => Ok(Kind::Reconstruction),
            other => Err(format!(
                "a kind is snapshot, final, candidate, or reconstruction, not {other}"
            )),
        }
    }
}

/// One retained workspace of a graded trial.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trial {
    /// The trial's directory name. A candidate or a reconstruction adds a
    /// dot and what tells it apart, such as `task__abc.lean-1.session-2`.
    pub trial: String,
    pub job: String,
    pub episode: PathBuf,
    /// The verifier's reward, when it's known.
    pub reward: Option<f64>,
    /// Every check candidate of the episode has the snapshot's digest, so
    /// the verifier graded the snapshot's workspace. Only a snapshot can
    /// have this.
    pub snapshot_graded: bool,
    pub workdir: String,
    #[serde(default)]
    pub kind: Kind,
    /// The directory whose files are the workspace, for a candidate, or
    /// that goes over the final workspace, for a reconstruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PathBuf>,
    /// The file identity the source must have: each path's SHA-256, as
    /// the lean loop's `selection.json` or a reconstruction recorded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<BTreeMap<String, String>>,
    /// Where a candidate's or a reconstruction's reward comes from:
    /// `submitted` when its files are the submitted workspace's, `grade`
    /// from a grade record, and `reconstruction` from the reconstruction's
    /// own verifier run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_source: Option<String>,
    /// A candidate's snapshot scope as the lean loop recorded it: `plain`,
    /// the whole working directory, or `git`, only the files Git lists as
    /// tracked or untracked and not ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

fn reward(trial_dir: &Path) -> Option<f64> {
    if let Ok(text) = std::fs::read_to_string(trial_dir.join("verifier/reward.txt")) {
        return text.trim().parse().ok();
    }
    let text = std::fs::read_to_string(trial_dir.join("result.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .pointer("/verifier_result/rewards/reward")
        .and_then(Value::as_f64)
}

fn snapshot_graded(episode: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(episode.join("artifacts/composition.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    let candidates: Vec<&str> = value["checks"]
        .as_array()
        .map(|checks| {
            checks
                .iter()
                .filter_map(|c| c.pointer("/summary/candidate").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    !candidates.is_empty() && candidates.iter().all(|c| *c == candidates[0])
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Whether the episode's executor was Microluna.
fn microluna(episode: &Path) -> bool {
    let Some(manifest) = read_json(&episode.join("manifest.json")) else {
        return false;
    };
    manifest.pointer("/delegate/agent").and_then(Value::as_str) == Some("microluna")
        || manifest
            .pointer("/policy/manifest/policy/executor/agent")
            .and_then(Value::as_str)
            == Some("microluna")
}

/// The artifacts Harbor collected from the trial's container: each
/// entry's host path under the trial and its path in the container.
/// Service artifacts and logs aren't the workspace.
fn collected(trial_dir: &Path) -> Vec<(PathBuf, String)> {
    let Some(Value::Array(entries)) = read_json(&trial_dir.join("artifacts/manifest.json")) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|e| e["status"] == "ok" && e["service"].is_null())
        .filter_map(|e| {
            let source = e["source"].as_str()?;
            let destination = e["destination"].as_str()?;
            (source.starts_with('/') && !source.starts_with("/logs/") && !source.contains(".."))
                .then(|| (trial_dir.join(destination), source.to_string()))
        })
        .collect()
}

/// A workspace's file identity: each path's SHA-256, with a link's target
/// as `link:TARGET`, skipping Git metadata, Python bytecode, and the cache
/// directories that `micro::parallel::UNMERGED` names. It matches the lean
/// loop's `evidence_tree`, so it compares with what `selection.json`
/// recorded.
///
/// # Errors
///
/// A message for an unreadable entry or a special file.
pub fn identity(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut tree = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).map_err(|e| format!("{}: {e}", at.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if kind.is_dir() {
                if !crate::micro::parallel::UNMERGED.contains(&name.as_ref()) {
                    stack.push(path);
                }
                continue;
            }
            if name.ends_with(".pyc") {
                continue;
            }
            let relative = path.strip_prefix(dir).map_err(|e| e.to_string())?;
            let relative = relative.to_str().ok_or("file path is not UTF-8")?;
            let bytes = if kind.is_symlink() {
                let target = std::fs::read_link(&path).map_err(|e| e.to_string())?;
                format!(
                    "link:{}",
                    target.to_str().ok_or("link target is not UTF-8")?
                )
                .into_bytes()
            } else if kind.is_file() {
                std::fs::read(&path).map_err(|e| e.to_string())?
            } else {
                return Err(format!("unsupported workspace entry: {}", path.display()));
            };
            tree.insert(relative.to_string(), super::sha256(&bytes));
        }
    }
    Ok(tree)
}

fn files_of(value: &Value) -> Option<BTreeMap<String, String>> {
    serde_json::from_value(value.clone()).ok()
}

/// The lean loop's retained candidates of one Microluna trial, from each
/// `lean-*/selection.json`: every session whose snapshot was kept, with
/// the file identity recorded for it. A candidate whose files are the
/// submitted workspace's takes the trial's reward.
fn candidates(base: &Trial) -> Vec<Trial> {
    let mut selections: Vec<PathBuf> = std::fs::read_dir(base.episode.join("artifacts"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("lean-"))
                && p.join("selection.json").is_file()
        })
        .collect();
    selections.sort();
    let mut out = Vec::new();
    for dir in selections {
        let Some(Value::Array(moves)) = read_json(&dir.join("selection.json")) else {
            continue;
        };
        let submitted = moves
            .iter()
            .rev()
            .find(|m| m["kind"] == "lean.submitted")
            .and_then(|m| files_of(&m["workspace_files"]));
        let scope = moves
            .iter()
            .find_map(|m| m["candidate_scope"].as_str())
            .map(str::to_string);
        let lean = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for item in &moves {
            let Some(number) = item["after_session"].as_u64() else {
                continue;
            };
            let name = format!("session-{number}");
            let recorded = item["candidate"]
                .as_str()
                .and_then(|c| Path::new(c).file_name())
                .map(|n| n.to_string_lossy().into_owned());
            if item["kind"] != "lean"
                || recorded.as_deref() != Some(name.as_str())
                || !item["snapshot_error"].is_null()
            {
                continue;
            }
            let files = files_of(&item["workspace_files"]);
            let is_submitted = files.is_some() && files == submitted;
            out.push(Trial {
                trial: format!("{}.{lean}.{name}", base.trial),
                reward: base.reward.filter(|_| is_submitted),
                reward_source: is_submitted.then(|| "submitted".to_string()),
                snapshot_graded: false,
                kind: Kind::Candidate,
                source: Some(dir.join(&name)),
                files,
                scope: scope.clone(),
                ..base.clone()
            });
        }
    }
    out
}

/// Every retained workspace of `task` under `jobs`, in name order: each
/// Coder One snapshot, each Microluna trial's final workspace, and each
/// candidate that a Microluna lean loop retained.
#[must_use]
pub fn trials(jobs: &Path, task: &str) -> Vec<Trial> {
    let mut out = Vec::new();
    let mut stack = vec![(jobs.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let episode = path.join("agent/episode");
            if !name.starts_with(&format!("{task}__")) || !episode.is_dir() {
                if depth < 3 {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            let job = path
                .strip_prefix(jobs)
                .ok()
                .and_then(|p| p.components().next())
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .unwrap_or_default();
            if episode.join("snapshot/workspace.tar.gz").is_file() {
                let manifest =
                    read_json(&episode.join("snapshot/snapshot.json")).unwrap_or(Value::Null);
                out.push(Trial {
                    trial: name,
                    job,
                    reward: reward(&path),
                    snapshot_graded: snapshot_graded(&episode),
                    workdir: manifest["workdir"].as_str().unwrap_or("/app").to_string(),
                    episode,
                    kind: Kind::Snapshot,
                    source: None,
                    files: None,
                    reward_source: None,
                    scope: None,
                });
            } else if microluna(&episode) && path.join("artifacts/manifest.json").is_file() {
                let manifest = read_json(&episode.join("manifest.json")).unwrap_or(Value::Null);
                let base = Trial {
                    trial: name,
                    job,
                    reward: reward(&path),
                    snapshot_graded: false,
                    workdir: manifest["workdir"].as_str().unwrap_or("/app").to_string(),
                    episode,
                    kind: Kind::Final,
                    source: None,
                    files: None,
                    reward_source: None,
                    scope: None,
                };
                out.extend(candidates(&base));
                out.push(base);
            }
        }
    }
    out.sort_by(|a, b| a.trial.cmp(&b.trial));
    out
}

/// One verifier grade of a retained candidate, from a record that names
/// the trial and the candidate's file identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Grade {
    pub trial: String,
    pub files: BTreeMap<String, String>,
    pub reward: f64,
    pub record: PathBuf,
}

fn complete_reward(grade: &Value) -> Option<f64> {
    let reward = grade["reward"].as_f64()?;
    (grade["exit"].as_i64() == Some(0)
        && grade["exception"].is_null()
        && (reward == 0.0 || (reward - 1.0).abs() < f64::EPSILON))
        .then_some(reward)
}

fn cached(path: &str) -> bool {
    path.ends_with(".pyc")
        || Path::new(path).parent().is_some_and(|parent| {
            parent.components().any(|c| {
                crate::micro::parallel::UNMERGED.contains(&c.as_os_str().to_string_lossy().as_ref())
            })
        })
}

/// Every candidate grade under `dir`: each `grade.json` that the
/// candidate-evidence experiment wrote (`trial`, `workspace_files`, and
/// `grade`) and each `candidate.json` that `tbench candidates` wrote
/// (`trial`, `inputs.candidate`, and `grade`). Only a complete binary
/// grade counts; a failed verifier run isn't a grade.
#[must_use]
pub fn grades(dir: &Path) -> Vec<Grade> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = entry.file_name();
            if name != "grade.json" && name != "candidate.json" {
                continue;
            }
            let Some(value) = read_json(&path) else {
                continue;
            };
            let Some(trial) = value["trial"]
                .as_str()
                .and_then(|t| Path::new(t).file_name())
                .map(|t| t.to_string_lossy().into_owned())
            else {
                continue;
            };
            let files = if name == "grade.json" {
                files_of(&value["workspace_files"])
            } else if value["error"].is_null() {
                // The inventory `tbench candidates` takes, reduced to the
                // files the lean loop's identity keeps.
                value["inputs"]["candidate"].as_object().map(|entries| {
                    entries
                        .iter()
                        .filter(|(path, e)| e["kind"] == "file" && !cached(path))
                        .filter_map(|(path, e)| {
                            Some((path.clone(), e["sha256"].as_str()?.to_string()))
                        })
                        .collect()
                })
            } else {
                None
            };
            if let (Some(files), Some(reward)) = (files, complete_reward(&value["grade"])) {
                out.push(Grade {
                    trial,
                    files,
                    reward,
                    record: path,
                });
            }
        }
    }
    out.sort_by(|a, b| a.record.cmp(&b.record));
    out
}

/// Gives each candidate with no known reward the reward of a grade of the
/// same trial with the same files.
pub fn apply_grades(trials: &mut [Trial], grades: &[Grade]) {
    for trial in trials.iter_mut().filter(|t| t.kind == Kind::Candidate) {
        if trial.reward.is_some() {
            continue;
        }
        let base = trial.trial.split('.').next().unwrap_or_default();
        if let Some(grade) = grades
            .iter()
            .find(|g| g.trial == base && trial.files.as_ref() == Some(&g.files))
        {
            trial.reward = Some(grade.reward);
            trial.reward_source = Some("grade".to_string());
        }
    }
}

/// The reconstruction in `dir`: `reconstruction.json`, which names the
/// trial it starts from, its `source_files`, and the verifier's `grade`,
/// beside `source-files/`.
///
/// # Errors
///
/// A message when the record doesn't read or its trial isn't among the
/// final workspaces in `finals`.
pub fn reconstruction(dir: &Path, finals: &[Trial]) -> Result<Trial, String> {
    let record = read_json(&dir.join("reconstruction.json"))
        .ok_or_else(|| format!("no readable reconstruction.json in {}", dir.display()))?;
    let trial = record["trial"].as_str().unwrap_or_default();
    let base = finals
        .iter()
        .find(|t| t.kind == Kind::Final && t.trial == trial)
        .ok_or_else(|| format!("{}: no final workspace of {trial}", dir.display()))?;
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let reward = complete_reward(&record["grade"]);
    Ok(Trial {
        trial: format!("{trial}.{name}"),
        reward,
        reward_source: reward.map(|_| "reconstruction".to_string()),
        snapshot_graded: false,
        kind: Kind::Reconstruction,
        source: Some(dir.join("source-files")),
        files: files_of(&record["source_files"]),
        ..base.clone()
    })
}

/// Copies `from` over `to`, file by file, skipping `__pycache__`.
fn overlay(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_file() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(from, to).map_err(|e| format!("{}: {e}", from.display()))?;
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name() == "__pycache__" {
            continue;
        }
        overlay(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

/// Lays `trial`'s workspace out under `into` as paths from `/`, as
/// [`Docker`] copies a candidate in.
///
/// - A snapshot unpacks its archive.
/// - A final workspace copies the image's working directory out of a
///   container that never starts, then copies the collected artifacts
///   over it.
/// - A reconstruction is the final workspace with its source files over
///   it, once they match the recorded identity.
/// - A candidate is its retained directory as the working directory, once
///   it matches the identity `selection.json` recorded.
///
/// # Errors
///
/// A message when a source is missing, Docker fails, or a source's files
/// differ from their recorded identity.
pub fn materialize(trial: &Trial, image: &str, into: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(into);
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    let workdir = into.join(trial.workdir.trim_start_matches('/'));
    let check = |source: &Path| -> Result<(), String> {
        let recorded = trial
            .files
            .as_ref()
            .ok_or_else(|| format!("{} has no recorded file identity", trial.trial))?;
        if &identity(source)? == recorded {
            Ok(())
        } else {
            Err(format!(
                "{} differs from its recorded file identity",
                source.display()
            ))
        }
    };
    let image_workdir = || -> Result<(), String> {
        let parent = workdir.parent().ok_or("the working directory is /")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let name = super::runner::container_name();
        let mut args = vec!["create"];
        if let Some(name) = &name {
            args.extend(["--name", name.as_str()]);
        }
        args.extend(["--entrypoint", "sleep", image, "infinity"]);
        let id = docker(&args)?;
        let copied = docker(&[
            "cp",
            &format!("{id}:{}", trial.workdir),
            &parent.display().to_string(),
        ]);
        let _ = docker(&["rm", "-f", &id]);
        copied.map(|_| ())
    };
    match trial.kind {
        Kind::Snapshot => untar(&trial.episode.join("snapshot/workspace.tar.gz"), into),
        Kind::Candidate => {
            let source = trial
                .source
                .as_deref()
                .ok_or("a candidate needs a source")?;
            check(source)?;
            if trial.scope.as_deref() == Some("git") {
                // A Git-scope snapshot holds only the files Git lists, so
                // ignored files come from the image.
                image_workdir()?;
                overlay(source, &workdir)
            } else {
                crate::handoff::copy_tree(source, &workdir)
            }
        }
        Kind::Final | Kind::Reconstruction => {
            if trial.kind == Kind::Reconstruction {
                check(
                    trial
                        .source
                        .as_deref()
                        .ok_or("a reconstruction needs a source")?,
                )?;
            }
            image_workdir()?;
            let trial_dir = trial
                .episode
                .parent()
                .and_then(Path::parent)
                .ok_or("the episode isn't under a trial")?;
            for (host, source) in collected(trial_dir) {
                overlay(&host, &into.join(source.trim_start_matches('/')))?;
            }
            if let (Kind::Reconstruction, Some(source)) = (trial.kind, &trial.source) {
                overlay(source, &workdir)?;
            }
            Ok(())
        }
    }
}

/// The task's environment image: the warm one,
/// `tbench-warm/<task>:environment-*`, or one built from the task's
/// `environment/` as `accept-env/<task>`.
#[must_use]
pub fn image(task: &str) -> Option<String> {
    let listed = docker(&["images", "--format", "{{.Repository}}:{{.Tag}}"]).ok()?;
    let warm = format!("tbench-warm/{task}:environment-");
    let built = format!("accept-env/{task}:");
    listed
        .lines()
        .find(|line| line.starts_with(&warm))
        .or_else(|| listed.lines().find(|line| line.starts_with(&built)))
        .map(str::to_string)
}

/// How to measure one task.
#[derive(Clone, Debug)]
pub struct TaskOptions {
    pub jobs: PathBuf,
    pub tasks_dir: PathBuf,
    pub out: PathBuf,
    /// The image, when it isn't the warm one.
    pub image: Option<String>,
    /// Reuse a suite already frozen under `out`.
    pub reuse: bool,
    /// Only these trials, when given. A name also selects its candidates
    /// and reconstructions.
    pub only: Vec<String>,
    /// Only these kinds of workspace, when given.
    pub kinds: Vec<Kind>,
    /// Directories of candidate grade records ([`grades`]).
    pub grades: Vec<PathBuf>,
    /// Reconstruction directories ([`reconstruction`]); one whose trial
    /// belongs to another task is skipped.
    pub reconstructions: Vec<PathBuf>,
    /// Workspaces whose suite runs overlap.
    pub workers: usize,
    pub define: Options,
    pub model: String,
    pub writer_turns: usize,
    pub writer_sec: u64,
    pub echo: bool,
    /// A task-anatomy JSON file whose decisive facts and test ideas for
    /// the task are given to the writer as evidence; only those the
    /// instruction or the workspace supports, never a verifier-only one.
    pub facts: Option<PathBuf>,
}

/// The decisive facts and test ideas a task-anatomy file holds for
/// `task`, keeping only those whose source the agent can see.
#[must_use]
pub fn anatomy_evidence(path: &Path, task: &str) -> Option<microluna::Evidence> {
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let entry = match &value["tasks"] {
        Value::Array(tasks) => tasks.iter().find(|t| t["task"] == task)?.clone(),
        Value::Object(tasks) => tasks.get(task)?.clone(),
        _ => return None,
    };
    let visible = |kind: &Value| matches!(kind.as_str(), Some("instruction" | "workspace"));
    let mut text = String::from(
        "Facts a reading of this task and its workspace found decisive. Each is stated in the \
         instruction or visible in the workspace. Check each against the task, then encode it \
         as a test that fails for the simpler reading.\n",
    );
    let mut kept = 0;
    for fact in entry["decisive_facts"].as_array().into_iter().flatten() {
        if visible(&fact["source_kind"]) {
            kept += 1;
            text.push_str(&format!(
                "\n- {}: {} (source: {})",
                fact["id"].as_str().unwrap_or_default(),
                fact["fact"].as_str().unwrap_or_default(),
                fact["source"].as_str().unwrap_or_default()
            ));
        }
    }
    let mut ideas = String::new();
    for idea in entry["test_ideas"].as_array().into_iter().flatten() {
        if visible(&idea["support"]) {
            ideas.push_str(&format!(
                "\n- {}: {} Asserts: {}",
                idea["id"].as_str().unwrap_or_default(),
                idea["command"].as_str().unwrap_or_default(),
                idea["assertion"].as_str().unwrap_or_default()
            ));
        }
    }
    if !ideas.is_empty() {
        text.push_str("\n\nTest ideas:\n");
        text.push_str(&ideas);
    }
    (kept > 0 || !ideas.is_empty()).then(|| microluna::Evidence {
        label: "Decisive facts from the task anatomy".to_string(),
        text,
    })
}

/// What the writer is told about reaching the workspace in the container.
pub const CONTAINER_NOTE: &str = "The solution workspace is WORKDIR inside the task's \
container, which has no network. Your file tools can't reach it: run commands in it with \
`sh env.sh 'COMMAND'`, for example `sh env.sh 'ls -la'` or `sh env.sh 'sed -n 1,80p FILE'`. \
The tests run there too, from WORKDIR, with this suite directory copied to /accept.";

/// What a candidate with a `requirements.txt` runs before its tests.
pub const SETUP: &str = "python3 -m pip install -q -r requirements.txt \
    || pip install -q -r requirements.txt";

fn untar(archive: &Path, into: &Path) -> Result<(), String> {
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    let status = std::process::Command::new("tar")
        .arg("xzf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .status()
        .map_err(|e| format!("cannot run tar: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tar could not unpack {}", archive.display()))
    }
}

/// The retained workspaces [`task`] measures: [`trials`], with the grades
/// and reconstructions `options` names, narrowed to `options.only` and
/// `options.kinds`.
///
/// # Errors
///
/// A message when a reconstruction of this task doesn't read.
pub fn found(name: &str, options: &TaskOptions) -> Result<Vec<Trial>, String> {
    let mut found = trials(&options.jobs, name);
    let graded: Vec<Grade> = options.grades.iter().flat_map(|d| grades(d)).collect();
    apply_grades(&mut found, &graded);
    for dir in &options.reconstructions {
        let belongs = read_json(&dir.join("reconstruction.json"))
            .and_then(|r| r["trial"].as_str().map(str::to_string))
            .is_some_and(|t| t.starts_with(&format!("{name}__")));
        if belongs {
            let rebuilt = reconstruction(dir, &found)?;
            found.push(rebuilt);
        }
    }
    found.sort_by(|a, b| a.trial.cmp(&b.trial));
    if !options.only.is_empty() {
        found.retain(|t| {
            options
                .only
                .iter()
                .any(|o| t.trial == *o || t.trial.starts_with(&format!("{o}.")))
        });
    }
    if !options.kinds.is_empty() {
        found.retain(|t| options.kinds.contains(&t.kind));
    }
    Ok(found)
}

/// Writes (or reuses) the suite for `task` and runs it on every retained
/// workspace [`found`] lists; the record is written to
/// `out/<task>/validity.json`.
///
/// # Errors
///
/// A message when the task, its image, or the Codex login is missing.
#[allow(clippy::too_many_lines)]
pub async fn task(name: &str, jev: &JevMode, options: &TaskOptions) -> Result<Value, String> {
    let found = found(name, options)?;
    if found.is_empty() {
        return Err(format!(
            "no retained workspace of {name} under {}",
            options.jobs.display()
        ));
    }
    let instruction = std::fs::read_to_string(options.tasks_dir.join(name).join("instruction.md"))
        .map_err(|e| format!("cannot read {name}'s instruction: {e}"))?;
    let image = options
        .image
        .clone()
        .or_else(|| image(name))
        .ok_or_else(|| format!("no tbench-warm/{name}:environment-* image; pass --image"))?;
    let workdir = found[0].workdir.clone();
    let out = options.out.join(name);
    let suite_dir = out.join("suite");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let record = AcceptanceSuite::record_path(&suite_dir);
    let recorder = Recorder::default();
    let suite = if options.reuse && record.is_file() {
        let mut suite = AcceptanceSuite::load(&record)?;
        if suite.dir != suite_dir && suite_dir.is_dir() {
            // A suite copied with its record, as the retained records are,
            // runs from where it is now; its digest still has to match.
            suite.dir.clone_from(&suite_dir);
            for test in &mut suite.tests {
                test.source =
                    std::fs::read_to_string(suite_dir.join(&test.path)).unwrap_or_default();
            }
        }
        suite
    } else {
        // The retained map was extracted from the same words, so the suite
        // and the trials' checks answer to the same requirement IDs.
        let map: crate::requirements::RequirementMap =
            std::fs::read_to_string(found[0].episode.join("artifacts/requirements.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_else(|| crate::requirements::mechanical(&instruction));
        let dev_runner = Docker {
            image: image.clone(),
            workdir: workdir.clone(),
            candidate: None,
            test_sec: options.define.test_sec,
            dev: None,
            setup: None,
        };
        let dev_name = format!("accept-dev-{name}-{}", atif::now_ms());
        let dev = dev_runner.start(Some(&dev_name))?;
        let runner = Docker {
            dev: Some(dev.clone()),
            ..dev_runner
        };
        let wire = crate::micro::codex_wire(&format!("accept-offline-{name}-{}", atif::now_ms()))?;
        let writer = MicrolunaWriter {
            transport: &wire,
            config: Config {
                max_turns: options.writer_turns,
                deadline: Some(Duration::from_secs(options.writer_sec)),
                model: options.model.clone(),
                ..Config::luna(&format!(
                    "accept-writer-{}",
                    &super::sha256(instruction.as_bytes())[..16]
                ))
            },
            isolation: Isolation::Boundary,
            seal: None,
            traces: Some(out.clone()),
            echo: options.echo,
        };
        let task = Task {
            title: name.to_string(),
            instruction: instruction.clone(),
        };
        let workspace = PathBuf::from(&workdir);
        let evidence: Vec<microluna::Evidence> = options
            .facts
            .as_deref()
            .and_then(|path| anatomy_evidence(path, name))
            .into_iter()
            .collect();
        let inputs = Inputs {
            task: &task,
            requirements: &map,
            evidence: &evidence,
            workspace: &workspace,
            suite_dir: &suite_dir,
            workspace_note: CONTAINER_NOTE.replace("WORKDIR", &workdir),
            target: None,
        };
        crate::say::line(&format!("accept ▸ {name}: writing the suite in {image}"));
        let suite = define(&inputs, &writer, &runner, jev, &recorder, &options.define).await;
        let _ = docker(&["rm", "-f", &dev]);
        suite
    };
    crate::say::line(&format!("accept ▸ {name}: {}", suite.headline()));
    let one = |trial: &Trial| {
        let candidate = out.join(format!("candidate-{}", trial.trial));
        let (image, suite, recorder) = (&image, &suite, &recorder);
        let trial = trial.clone();
        async move {
            if let Err(error) = materialize(&trial, image, &candidate) {
                let _ = std::fs::remove_dir_all(&candidate);
                crate::say::line(&format!("accept ▸ {}: {error}", trial.trial));
                return json!({ "trial": trial, "error": error });
            }
            // A candidate that names its packages gets them, as a verifier
            // that grades in a separate container installs them.
            let requirements = candidate
                .join(trial.workdir.trim_start_matches('/'))
                .join("requirements.txt");
            let runner = Docker {
                image: image.clone(),
                workdir: trial.workdir.clone(),
                candidate: Some(candidate.clone()),
                test_sec: options.define.test_sec,
                dev: None,
                setup: requirements.is_file().then(|| SETUP.to_string()),
            };
            let ran = run(
                suite,
                Path::new(&trial.workdir),
                &runner,
                Some(recorder),
                &trial.trial,
            )
            .await;
            let _ = std::fs::remove_dir_all(&candidate);
            match ran {
                Ok(result) => {
                    crate::say::line(&format!(
                        "accept ▸ {}: {} of {} tests pass; reward {:?}{}",
                        trial.trial,
                        result.passed,
                        result.total,
                        trial.reward,
                        if trial.kind != Kind::Snapshot || trial.snapshot_graded {
                            ""
                        } else {
                            " (the snapshot isn't the graded candidate)"
                        }
                    ));
                    json!({ "trial": trial, "run": result })
                }
                Err(tampered) => json!({ "trial": trial, "error": tampered.to_string() }),
            }
        }
    };
    let results: Vec<Value> = futures_util::stream::iter(found.iter().map(one))
        .buffered(options.workers.max(1))
        .collect()
        .await;
    let value = json!({
        "schema": SCHEMA,
        "task": name,
        "image": image,
        "suite": {
            "record": record,
            "status": suite.status,
            "headline": suite.headline(),
            "digest": suite.digest,
            "tests": suite.tests,
            "rejected": suite.rejected,
            "gaps": suite.gaps,
            "coverage": suite.coverage,
            "start": suite.start,
            "rounds": suite.rounds,
            "writer_usd": suite.writer_usd,
            "jev_usd": suite.jev_usd,
        },
        "trials": results,
    });
    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&out.join("validity.json"), format!("{text}\n").as_bytes())?;
    Ok(value)
}

/// One trial's three signals beside its label.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Joined {
    pub task: String,
    pub trial: String,
    pub kind: Kind,
    pub reward: Option<f64>,
    pub snapshot_graded: bool,
    /// The suite's call: pass when green.
    pub suite: Option<Says>,
    /// The suite's call when a gap counts as red: pass only when green
    /// and every requirement has a test.
    pub complete: Option<Says>,
    pub passed: usize,
    pub total: usize,
    /// Today's checks, from the label row.
    pub checks: Option<Says>,
    /// The combined verdict, from the label row.
    pub verdict: Option<Says>,
    /// The digest of the suite that ran.
    #[serde(default)]
    pub digest: String,
    /// Each test's ID and whether it was green, in run order.
    #[serde(default)]
    pub tests: Vec<(String, bool)>,
    /// Where a candidate's reward comes from ([`Trial::reward_source`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_source: Option<String>,
}

/// A signal's agreement with the verifier over some trials.
///
/// A trial whose verifier reward is unknown says nothing about the
/// signal: it's counted in `unknown` and in nothing else.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Agreement {
    /// Trials with a known reward.
    pub trials: usize,
    /// Trials left out because the verifier's reward is unknown.
    pub unknown: usize,
    /// Trials where the signal spoke.
    pub spoke: usize,
    /// Trials where what it said matched the verifier.
    pub agreed: usize,
    /// Of the trials it called failed, how many failed.
    pub fail_right: usize,
    pub fail_called: usize,
    /// Of the trials it called passed, how many passed.
    pub pass_right: usize,
    pub pass_called: usize,
    /// The verifier's failures; `fail_right` of them the signal called
    /// failed.
    pub failures: usize,
    /// The verifier's passes; `pass_right` of them the signal called
    /// passed.
    pub passes: usize,
}

impl Agreement {
    pub(crate) fn of(rows: &[&Joined], says: impl Fn(&Joined) -> Option<Says>) -> Agreement {
        let mut out = Agreement::default();
        for row in rows {
            let Some(reward) = row.reward else {
                out.unknown += 1;
                continue;
            };
            out.trials += 1;
            let passed = reward >= 1.0;
            if passed {
                out.passes += 1;
            } else {
                out.failures += 1;
            }
            match says(row) {
                Some(Says::Fail) => {
                    out.spoke += 1;
                    out.fail_called += 1;
                    if !passed {
                        out.fail_right += 1;
                        out.agreed += 1;
                    }
                }
                Some(Says::Pass) => {
                    out.spoke += 1;
                    out.pass_called += 1;
                    if passed {
                        out.pass_right += 1;
                        out.agreed += 1;
                    }
                }
                None => {}
            }
        }
        out
    }
}

/// Reads every `*/validity.json` under `dir` and joins each trial with its
/// label row in `rows`.
///
/// # Errors
///
/// A message when the rows don't read.
pub fn join(dir: &Path, rows: &Path) -> Result<Vec<Joined>, String> {
    join_all(&[dir.to_path_buf()], rows)
}

/// [`join`] over several directories of offline records, in order.
///
/// # Errors
///
/// A message when the rows or a directory don't read.
pub fn join_all(dirs: &[PathBuf], rows: &Path) -> Result<Vec<Joined>, String> {
    let rows = truth::read_rows(rows)?;
    let mut out = Vec::new();
    for dir in dirs {
        join_into(dir, &rows, &mut out)?;
    }
    Ok(out)
}

fn join_into(dir: &Path, rows: &[truth::Row], out: &mut Vec<Joined>) -> Result<(), String> {
    let mut tasks: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path().join("validity.json"))
        .filter(|p| p.is_file())
        .collect();
    tasks.sort();
    for path in tasks {
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let task = value["task"].as_str().unwrap_or_default().to_string();
        let digest = value["suite"]["digest"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        for entry in value["trials"].as_array().into_iter().flatten() {
            let Ok(trial) = serde_json::from_value::<Trial>(entry["trial"].clone()) else {
                continue;
            };
            let run: Option<RunResult> = serde_json::from_value(entry["run"].clone()).ok();
            // A label row describes the trial's own workspace, not a
            // candidate or a reconstruction of it.
            let row = rows
                .iter()
                .find(|r| r.trial == trial.trial)
                .filter(|_| matches!(trial.kind, Kind::Snapshot | Kind::Final));
            out.push(Joined {
                task: task.clone(),
                trial: trial.trial.clone(),
                kind: trial.kind,
                reward: trial.reward.or_else(|| row.map(|r| r.reward)),
                snapshot_graded: trial.snapshot_graded,
                suite: run
                    .as_ref()
                    .filter(|r| r.total > 0)
                    .map(|r| if r.green { Says::Pass } else { Says::Fail }),
                complete: run.as_ref().filter(|r| r.total > 0).map(|r| {
                    if r.green && r.gaps.is_empty() {
                        Says::Pass
                    } else {
                        Says::Fail
                    }
                }),
                passed: run.as_ref().map_or(0, |r| r.passed),
                total: run.as_ref().map_or(0, |r| r.total),
                checks: row.and_then(truth::todays_checks),
                verdict: row.and_then(|r| {
                    verdict::judge(&verdict::Evidence::of_row(r), &verdict::fitted()).says()
                }),
                digest: run
                    .as_ref()
                    .map_or_else(|| digest.clone(), |r| r.digest.clone()),
                tests: run
                    .as_ref()
                    .map(|r| r.tests.iter().map(|t| (t.id.clone(), t.green)).collect())
                    .unwrap_or_default(),
                reward_source: trial.reward_source.clone(),
            });
        }
    }
    Ok(())
}

/// The sets [`validity`] counts, by key, with a label and which joined
/// trials each holds.
pub const SETS: [(&str, &str); 4] = [
    ("snapshot_graded", "Coder One snapshots the verifier graded"),
    ("all_snapshots", "every Coder One snapshot"),
    (
        "microluna_finals",
        "Microluna final workspaces and reconstructions",
    ),
    ("microluna_candidates", "Microluna retained candidates"),
];

fn in_set(set: &str, j: &Joined) -> bool {
    match set {
        "snapshot_graded" => j.kind == Kind::Snapshot && j.snapshot_graded,
        "all_snapshots" => j.kind == Kind::Snapshot,
        "microluna_finals" => matches!(j.kind, Kind::Final | Kind::Reconstruction),
        "microluna_candidates" => j.kind == Kind::Candidate,
        _ => false,
    }
}

/// The three signals' agreement over the joined trials, in each of
/// [`SETS`]. A trial with an unknown reward is counted apart.
#[must_use]
pub fn validity(joined: &[Joined]) -> Value {
    let table = |rows: &[&Joined]| {
        json!({
            "suite_green": Agreement::of(rows, |j| j.suite),
            "suite_complete": Agreement::of(rows, |j| j.complete),
            "todays_checks": Agreement::of(rows, |j| j.checks),
            "combined_verdict": Agreement::of(rows, |j| j.verdict),
        })
    };
    let mut out = serde_json::Map::new();
    for (set, _) in SETS {
        let rows: Vec<&Joined> = joined.iter().filter(|j| in_set(set, j)).collect();
        out.insert(set.to_string(), table(&rows));
    }
    out.insert("trials".to_string(), json!(joined));
    Value::Object(out)
}
