//! `verify.method_conformance` on retained workspaces (issue #9653).
//!
//! For one Terminal-Bench task, [`task`] restores every retained workspace
//! that `accept offline` reads ([`crate::accept::offline::found`]) under
//! each jobs directory, finds the candidate functions in each by code,
//! and has Jev identify each distinct function once, from recorded
//! answers where they exist. For each workspace with an identified
//! function, it starts a fresh container of the task's image with no
//! network, copies the workspace in, runs the entries' checks, and
//! removes the container. The verifier's rewards go to `labels.json`,
//! apart from the results, so the results can be read before the labels
//! are joined.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{
    Candidate, Context, Row, candidates, check, failures, identify_with, question_set, registry,
};
use crate::accept::offline::{self as accept_offline, Kind as WorkspaceKind, Trial};
use crate::accept::runner::{Docker, docker};
use crate::checks::contract::host::Container;
use crate::component::jev::JevMode;
use crate::record::Recorder;

/// The schema of one task's results.
pub const SCHEMA: &str = "openagents.coder-one.method-conformance-offline.v1";

/// The schema of one task's labels.
pub const LABELS_SCHEMA: &str = "openagents.coder-one.method-conformance-labels.v1";

/// How to measure one task.
#[derive(Clone, Debug)]
pub struct Options {
    /// Directories of retained jobs, read in order; a trial found under an
    /// earlier one isn't read again from a later one.
    pub jobs: Vec<PathBuf>,
    pub tasks_dir: PathBuf,
    pub out: PathBuf,
    /// The image, when it isn't the warm or built one.
    pub image: Option<String>,
    /// Only these trials, when given.
    pub only: Vec<String>,
    /// Only these kinds of workspace, when given.
    pub kinds: Vec<WorkspaceKind>,
    /// Trials of a job whose name contains any of these are left out.
    pub exclude_jobs: Vec<String>,
}

fn now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// The retained workspaces of `name` under every jobs directory.
///
/// # Errors
///
/// A message when a jobs directory's trials don't read.
pub fn workspaces(name: &str, options: &Options) -> Result<Vec<Trial>, String> {
    let mut out: Vec<Trial> = Vec::new();
    for jobs in &options.jobs {
        let accept = accept_offline::TaskOptions {
            jobs: jobs.clone(),
            tasks_dir: options.tasks_dir.clone(),
            out: options.out.clone(),
            image: options.image.clone(),
            reuse: false,
            only: options.only.clone(),
            kinds: options.kinds.clone(),
            grades: Vec::new(),
            reconstructions: Vec::new(),
            workers: 1,
            define: crate::accept::Options::default(),
            model: String::new(),
            writer_turns: 0,
            writer_sec: 0,
            echo: false,
            facts: None,
        };
        for trial in accept_offline::found(name, &accept)? {
            let excluded = options
                .exclude_jobs
                .iter()
                .any(|x| trial.job.contains(x.as_str()));
            if !excluded && !out.iter().any(|t| t.trial == trial.trial) {
                out.push(trial);
            }
        }
    }
    out.sort_by(|a, b| a.trial.cmp(&b.trial));
    Ok(out)
}

/// The task's image: the warm or built one `accept offline` uses, or the
/// public image the #9584 archive built, `truth9584-archive/<task>:public`.
#[must_use]
pub fn image(task: &str) -> Option<String> {
    accept_offline::image(task).or_else(|| {
        let archived = format!("truth9584-archive/{task}:public");
        docker(&["image", "inspect", "--format", "{{.Id}}", &archived])
            .ok()
            .map(|_| archived)
    })
}

/// One workspace, restored.
struct Restored {
    trial: Trial,
    dir: PathBuf,
    found: Vec<Candidate>,
    error: Option<String>,
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::record::write_atomic(path, format!("{text}\n").as_bytes())
}

/// Measures `name`: writes `out/<name>/conformance.json` and
/// `labels.json`, and returns the Jev calls it made or replayed.
///
/// # Errors
///
/// A message when the task has no retained workspace or the output can't
/// be written.
#[allow(clippy::too_many_lines)]
pub async fn task(
    name: &str,
    mode: &JevMode,
    replay: Option<&JevMode>,
    options: &Options,
) -> Result<Vec<Value>, String> {
    let found = workspaces(name, options)?;
    if found.is_empty() {
        return Err(format!("no retained workspace of {name}"));
    }
    let image = options.image.clone().or_else(|| image(name));
    let out = options.out.join(name);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    write_labels(name, &found, &out)?;
    // Restore every workspace and find its candidates by code.
    let mut restored = Vec::new();
    for (i, trial) in found.iter().enumerate() {
        let dir = out.join(format!("ws-{i}"));
        let needs_image = match trial.kind {
            WorkspaceKind::Snapshot => false,
            WorkspaceKind::Candidate => trial.scope.as_deref() == Some("git"),
            WorkspaceKind::Final | WorkspaceKind::Reconstruction => true,
        };
        let materialized = match (&image, needs_image) {
            (Some(image), _) => accept_offline::materialize(trial, image, &dir),
            (None, false) => accept_offline::materialize(trial, "", &dir),
            (None, true) => Err(format!("no image for {name} to restore this workspace")),
        };
        let local = dir.join(trial.workdir.trim_start_matches('/'));
        let (found, error) = match materialized {
            Ok(()) => (candidates(&local), None),
            Err(error) => (Vec::new(), Some(error)),
        };
        // Restored again only if it holds a function to check, to keep
        // one workspace on disk at a time.
        let _ = std::fs::remove_dir_all(&dir);
        restored.push(Restored {
            trial: trial.clone(),
            dir,
            found,
            error,
        });
    }
    // Every distinct function once, in digest order, so a replay asks
    // the same requests.
    let mut unique: BTreeMap<String, Candidate> = BTreeMap::new();
    for r in &restored {
        for c in &r.found {
            unique.entry(c.digest.clone()).or_insert_with(|| c.clone());
        }
    }
    let distinct: Vec<Candidate> = unique.into_values().collect();
    let recorder = Recorder::default();
    let identified = identify_with(
        mode,
        replay,
        &recorder,
        &Context {
            component: super::COMPONENT,
            id: format!("jev-conformance-{name}"),
            deadline: None,
        },
        &distinct,
    )
    .await;
    let methods: BTreeMap<String, Option<String>> = distinct
        .iter()
        .zip(&identified.methods)
        .map(|(c, m)| (c.digest.clone(), m.clone()))
        .collect();
    let answered = identified.answered.iter().filter(|a| **a).count();
    crate::say::line(&format!(
        "conformance ▸ {name}: {} workspaces, {} distinct functions, {answered} answered, {} tied to an entry",
        restored.len(),
        distinct.len(),
        identified.methods.iter().flatten().count()
    ));
    // Check each workspace that has an identified function.
    let mut trials = Vec::new();
    for r in &restored {
        let tied: Vec<(&Candidate, &String)> = r
            .found
            .iter()
            .filter_map(|c| {
                methods
                    .get(&c.digest)
                    .and_then(Option::as_ref)
                    .map(|m| (c, m))
            })
            .collect();
        let mut entry = json!({
            "trial": r.trial.trial,
            "job": r.trial.job,
            "kind": r.trial.kind,
            "candidates": r.found.len(),
            "tied": tied.len(),
            "error": r.error,
        });
        if !tied.is_empty() {
            let started = now();
            match &image {
                None => {
                    entry["check_error"] = json!(format!("no image for {name}"));
                }
                Some(image) => 'check: {
                    if let Err(error) = accept_offline::materialize(&r.trial, image, &r.dir) {
                        entry["check_error"] = json!(error);
                        break 'check;
                    }
                    let runner = Docker {
                        image: image.clone(),
                        workdir: r.trial.workdir.clone(),
                        candidate: Some(r.dir.clone()),
                        test_sec: 0,
                        dev: None,
                        setup: None,
                    };
                    let label =
                        format!("method-conformance-{name}-{}-{}", std::process::id(), now());
                    match runner.start(Some(&label)) {
                        Err(error) => entry["check_error"] = json!(error),
                        Ok(id) => {
                            let host = Container {
                                id: id.clone(),
                                workdir: r.trial.workdir.clone(),
                                scratch: out.join("scratch"),
                            };
                            let mut rows = Vec::new();
                            for (candidate, slug) in &tied {
                                let Some(loaded) = registry().get(slug) else {
                                    continue;
                                };
                                let checked =
                                    check(&host, &r.trial.workdir, candidate, &loaded.entry).await;
                                rows.push(Row {
                                    file: candidate.file.clone(),
                                    line: candidate.line,
                                    qualname: candidate.qualname.clone(),
                                    method: (*slug).clone(),
                                    checked,
                                });
                            }
                            let _ = docker(&["rm", "-f", &id]);
                            let failed = failures(&rows);
                            crate::say::line(&format!(
                                "conformance ▸ {}: {} checked, {} failed properties",
                                r.trial.trial,
                                rows.len(),
                                failed.len()
                            ));
                            entry["rows"] = json!(rows);
                            entry["failures"] = json!(failed);
                        }
                    }
                    entry["seconds"] = json!((now() - started) / 1000);
                }
            }
        }
        trials.push(entry);
        let _ = std::fs::remove_dir_all(&r.dir);
    }
    let _ = std::fs::remove_dir_all(out.join("scratch"));
    let value = json!({
        "schema": SCHEMA,
        "task": name,
        "image": image,
        "registry": registry().digest,
        "question_set": question_set().digest,
        "implementation": super::implementation(),
        "distinct": distinct.iter().zip(&identified.methods).map(|(c, m)| json!({
            "digest": c.digest,
            "file": c.file,
            "line": c.line,
            "qualname": c.qualname,
            "method": m,
        })).collect::<Vec<_>>(),
        "jev_usd": identified.usd,
        "trials": trials,
    });
    write_json(&out.join("conformance.json"), &value)?;
    Ok(identified
        .calls
        .into_iter()
        .map(|mut c| {
            c["task"] = json!(name);
            c
        })
        .collect())
}

fn write_labels(name: &str, found: &[Trial], out: &Path) -> Result<(), String> {
    write_json(
        &out.join("labels.json"),
        &json!({
            "schema": LABELS_SCHEMA,
            "task": name,
            "trials": found.iter().map(|t| json!({
                "trial": t.trial,
                "job": t.job,
                "kind": t.kind,
                "reward": t.reward,
                "reward_source": t.reward_source,
                "snapshot_graded": t.snapshot_graded,
            })).collect::<Vec<_>>(),
        }),
    )
}
