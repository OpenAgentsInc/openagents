//! The contract, run on retained workspaces.
//!
//! For one Terminal-Bench task, [`task`] makes the plan in a fresh
//! container of the task's image, the untouched workspace, and runs it
//! there once as a baseline. It then restores every retained workspace
//! that `accept offline` reads ([`crate::accept::offline::found`]) into its
//! own fresh container with no network, runs the plan, and removes the
//! container. The record keeps the verifier's rewards apart from the
//! results, in `labels.json`, so the results can be read before the
//! labels are joined.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde_json::{Value, json};

use super::host::Container;
use super::{Plan, call, extract, report, run, score, tally};
use crate::accept::offline::{self as accept_offline, Kind as WorkspaceKind, Trial};
use crate::accept::runner::{Docker, docker};
use crate::component::jev::JevMode;
use crate::record::Recorder;

/// The schema of one task's results.
pub const SCHEMA: &str = "openagents.coder-one.contract-offline.v1";

/// The schema of one task's labels.
pub const LABELS_SCHEMA: &str = "openagents.coder-one.contract-labels.v1";

/// How to measure one task.
#[derive(Clone, Debug)]
pub struct Options {
    pub jobs: PathBuf,
    pub tasks_dir: PathBuf,
    pub out: PathBuf,
    /// The image, when it isn't the warm or built one.
    pub image: Option<String>,
    pub grades: Vec<PathBuf>,
    pub reconstructions: Vec<PathBuf>,
    /// Only these trials, when given.
    pub only: Vec<String>,
    /// Only these kinds of workspace, when given.
    pub kinds: Vec<WorkspaceKind>,
    /// Trials of a job whose name contains any of these are left out.
    pub exclude_jobs: Vec<String>,
    /// Workspaces run at once.
    pub workers: usize,
    /// Reuse `out/<task>/plan.json` when it's there.
    pub reuse_plan: bool,
}

fn now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn container_name(task: &str, what: &str) -> String {
    format!(
        "contract-check-{task}-{what}-{}-{}",
        std::process::id(),
        now()
    )
}

/// The retained workspaces of `name`, less the excluded jobs.
///
/// # Errors
///
/// A message when a reconstruction doesn't read.
pub fn workspaces(name: &str, options: &Options) -> Result<Vec<Trial>, String> {
    let accept = accept_offline::TaskOptions {
        jobs: options.jobs.clone(),
        tasks_dir: options.tasks_dir.clone(),
        out: options.out.clone(),
        image: options.image.clone(),
        reuse: false,
        only: options.only.clone(),
        kinds: options.kinds.clone(),
        grades: options.grades.clone(),
        reconstructions: options.reconstructions.clone(),
        workers: options.workers,
        define: crate::accept::Options::default(),
        model: String::new(),
        writer_turns: 0,
        writer_sec: 0,
        echo: false,
        facts: None,
    };
    let mut found = accept_offline::found(name, &accept)?;
    found.retain(|t| {
        !options
            .exclude_jobs
            .iter()
            .any(|x| t.job.contains(x.as_str()))
    });
    Ok(found)
}

/// Starts a container of `image` with no network, runs `f` against it,
/// and removes it whatever happens.
async fn with_container<T, F, Fut>(
    runner: Docker,
    name: &str,
    scratch: &Path,
    f: F,
) -> Result<T, String>
where
    F: FnOnce(Container) -> Fut,
    Fut: Future<Output = T>,
{
    let id = runner.start(Some(name))?;
    let host = Container {
        id: id.clone(),
        workdir: runner.workdir.clone(),
        scratch: scratch.to_path_buf(),
    };
    let result = f(host).await;
    let _ = docker(&["rm", "-f", &id]);
    let _ = std::fs::remove_dir_all(scratch);
    Ok(result)
}

/// Makes (or reuses) `name`'s plan and runs it on every retained
/// workspace; writes `out/<name>/plan.json`, `contract.json`, and
/// `labels.json`.
///
/// # Errors
///
/// A message when the task, its image, or its workspaces are missing.
#[allow(clippy::too_many_lines)]
pub async fn task(
    name: &str,
    mode: &JevMode,
    replay: Option<&JevMode>,
    options: &Options,
) -> Result<Value, String> {
    let found = workspaces(name, options)?;
    let instruction = std::fs::read_to_string(options.tasks_dir.join(name).join("instruction.md"))
        .map_err(|e| format!("cannot read {name}'s instruction: {e}"))?;
    let image = options
        .image
        .clone()
        .or_else(|| accept_offline::image(name))
        .ok_or_else(|| format!("no image for {name}; pass --image"))?;
    let workdir = found
        .first()
        .map_or_else(|| "/app".to_string(), |t| t.workdir.clone());
    let out = options.out.join(name);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let plan_path = out.join("plan.json");
    let pristine = |candidate: Option<PathBuf>, setup: Option<String>| Docker {
        image: image.clone(),
        workdir: workdir.clone(),
        candidate,
        test_sec: 0,
        dev: None,
        setup,
    };
    let recorder = Recorder::default();
    let (plan, untouched) = {
        let reused: Option<Plan> = options
            .reuse_plan
            .then(|| std::fs::read_to_string(&plan_path).ok())
            .flatten()
            .and_then(|t| serde_json::from_str(&t).ok());
        let scratch = out.join("scratch-untouched");
        let label = container_name(name, "untouched");
        let instruction = instruction.clone();
        let workdir = workdir.clone();
        with_container(pristine(None, None), &label, &scratch, |host| async move {
            let plan = match reused {
                Some(plan) => plan,
                None => {
                    extract::plan(name, &instruction, &workdir, &host, mode, replay, &recorder)
                        .await
                }
            };
            let results = run(&plan, &host).await;
            (plan, results)
        })
        .await?
    };
    let text = serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&plan_path, format!("{text}\n").as_bytes())?;
    crate::say::line(&format!(
        "contract ▸ {name}: {} items, {} checks; untouched workspace: {}",
        plan.items.len(),
        plan.checks().count(),
        tally(&untouched)
    ));
    let one = |(i, trial): (usize, &Trial)| {
        let (plan, image, out) = (&plan, &image, &out);
        let trial = trial.clone();
        async move {
            let started = now();
            let workspace = out.join(format!("ws-{i}"));
            if let Err(error) = accept_offline::materialize(&trial, image, &workspace) {
                let _ = std::fs::remove_dir_all(&workspace);
                crate::say::line(&format!("contract ▸ {}: {error}", trial.trial));
                return json!({ "trial": trial.trial, "error": error });
            }
            let requirements = workspace
                .join(trial.workdir.trim_start_matches('/'))
                .join("requirements.txt");
            let runner = Docker {
                image: image.clone(),
                workdir: trial.workdir.clone(),
                candidate: Some(workspace.clone()),
                test_sec: 0,
                dev: None,
                setup: requirements
                    .is_file()
                    .then(|| crate::accept::offline::SETUP.to_string()),
            };
            let network = if runner.setup.is_some() {
                "bridge"
            } else {
                "none"
            };
            let label = container_name(name, &i.to_string());
            let ran = with_container(
                runner,
                &label,
                &out.join(format!("scratch-{i}")),
                |host| async move { run(plan, &host).await },
            )
            .await;
            let _ = std::fs::remove_dir_all(&workspace);
            match ran {
                Ok(results) => {
                    crate::say::line(&format!(
                        "contract ▸ {}: {} ({})",
                        trial.trial,
                        call(&results).unwrap_or("no call"),
                        tally(&results)
                    ));
                    json!({
                        "trial": trial.trial,
                        "job": trial.job,
                        "kind": trial.kind,
                        "network": network,
                        "report": report(plan, &trial.trial, &results),
                        "seconds": (now() - started) / 1000,
                    })
                }
                Err(error) => json!({ "trial": trial.trial, "error": error }),
            }
        }
    };
    let results: Vec<Value> = futures_util::stream::iter(found.iter().enumerate().map(one))
        .buffered(options.workers.max(1))
        .collect()
        .await;
    let value = json!({
        "schema": SCHEMA,
        "task": name,
        "image": image,
        "plan": plan.digest,
        "untouched": {
            "call": call(&untouched),
            "score": score(&untouched),
            "tally": tally(&untouched),
            "items": untouched,
        },
        "trials": results,
    });
    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&out.join("contract.json"), format!("{text}\n").as_bytes())?;
    let labels = json!({
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
    });
    let text = serde_json::to_string_pretty(&labels).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&out.join("labels.json"), format!("{text}\n").as_bytes())?;
    Ok(value)
}
