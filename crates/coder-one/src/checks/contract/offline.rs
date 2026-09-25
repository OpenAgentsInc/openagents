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

pub(crate) fn container_name(task: &str, what: &str) -> String {
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
pub(crate) async fn with_container<T, F, Fut>(
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
    write_labels(name, &found, &out)?;
    Ok(value)
}

fn write_labels(name: &str, found: &[Trial], out: &Path) -> Result<(), String> {
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
    crate::record::write_atomic(&out.join("labels.json"), format!("{text}\n").as_bytes())
}

/// The schema of one task's `verify.executed` results.
pub const EXECUTED_SCHEMA: &str = "openagents.coder-one.executed-offline.v1";

/// The files under the container's working directory, relative to it, for
/// [`executed::compile`]: at most three levels, with the directories the
/// evidence file list skips left out.
async fn container_files(host: &Container) -> Vec<String> {
    let ran = super::host::Host::run(
        host,
        "find . -maxdepth 3 \\( -name .git -o -name node_modules -o -name .venv -o -name venv \
         -o -name target -o -name __pycache__ \\) -prune -o -type f -print",
        std::time::Duration::from_secs(60),
    )
    .await;
    ran.stdout
        .lines()
        .filter_map(|l| l.strip_prefix("./"))
        .map(str::to_string)
        .collect()
}

/// `verify.executed` on retained workspaces (issue #9636). In a fresh
/// container of the task's image, the untouched workspace, it plans the
/// commands with no model (`evidence.baseline`'s entry points, found on a
/// copy of the image's working directory, the commands the instruction
/// names, and a compile or import of the package) and runs each once. It then restores
/// every retained workspace that `accept offline` reads into its own
/// container, runs the same commands, and judges each against its
/// untouched outcome by [`executed::verdict`]. Writes
/// `out/<name>/executed.json` and `labels.json`.
///
/// # Errors
///
/// A message when the task, its image, or its workspaces are missing.
pub async fn executed_task(
    name: &str,
    options: &Options,
    wall: std::time::Duration,
    budget: std::time::Duration,
) -> Result<Value, String> {
    use super::executed::{self, At, Stage};
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
    let untouched_at = At {
        stage: Some(Stage::Baseline),
        session: None,
        candidate: None,
        cwd: workdir.clone(),
    };
    // `evidence.baseline`'s entry points, found by code on a copy of the
    // image's working directory that the instruction calls `workdir`.
    let entries = {
        let local = out.join("untouched-workdir");
        let _ = std::fs::remove_dir_all(&local);
        std::fs::create_dir_all(&local).map_err(|e| e.to_string())?;
        let id = docker(&["create", "--entrypoint", "sleep", &image, "infinity"])?;
        let copied = docker(&[
            "cp",
            &format!("{id}:{workdir}/."),
            &local.display().to_string(),
        ]);
        let _ = docker(&["rm", "-f", &id]);
        copied?;
        let found = super::entry::find(&instruction, &local, &workdir).await;
        let _ = std::fs::remove_dir_all(&local);
        found
    };
    let (commands, untouched_runs) = {
        let runner = Docker {
            image: image.clone(),
            workdir: workdir.clone(),
            candidate: None,
            test_sec: 0,
            dev: None,
            setup: None,
        };
        let instruction = instruction.clone();
        let workdir = workdir.clone();
        let entries: Vec<executed::Planned> = entries
            .iter()
            .filter(|e| e.refused.is_none())
            .map(|e| executed::Planned {
                kind: e.kind.word().to_string(),
                command: e.command.clone(),
                requirements: Vec::new(),
            })
            .collect();
        with_container(
            runner,
            &container_name(name, "untouched"),
            &out.join("scratch-untouched"),
            |host| async move {
                let pristine = extract::gather(&host, &instruction, &workdir).await;
                let files = container_files(&host).await;
                let named = executed::named(&instruction, &workdir, &pristine);
                let commands = executed::commands(
                    &[],
                    &[],
                    entries.into_iter().chain(named).collect(),
                    executed::compile(&files),
                );
                let runs =
                    executed::run_each(&host, &commands, wall, budget, &|c| c.to_string()).await;
                (commands, runs)
            },
        )
        .await?
    };
    let untouched: Vec<executed::Record> = untouched_runs
        .iter()
        .map(|run| executed::record(&untouched_at, run, None))
        .collect();
    let outcomes = executed::untouched(&untouched);
    crate::say::line(&format!(
        "executed ▸ {name}: {} commands; {} exited 0 on the untouched workspace",
        commands.len(),
        outcomes.values().filter(|o| o.passed()).count()
    ));
    let one = |(i, trial): (usize, &Trial)| {
        let (commands, outcomes, image, out, workdir) =
            (&commands, &outcomes, &image, &out, &workdir);
        let trial = trial.clone();
        async move {
            let started = now();
            let workspace = out.join(format!("ws-{i}"));
            if let Err(error) = accept_offline::materialize(&trial, image, &workspace) {
                let _ = std::fs::remove_dir_all(&workspace);
                crate::say::line(&format!("executed ▸ {}: {error}", trial.trial));
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
            let ran = with_container(
                runner,
                &container_name(name, &i.to_string()),
                &out.join(format!("scratch-{i}")),
                |host| async move {
                    executed::run_each(&host, commands, wall, budget, &|c| c.to_string()).await
                },
            )
            .await;
            let _ = std::fs::remove_dir_all(&workspace);
            match ran {
                Ok(runs) => {
                    let at = At {
                        stage: Some(Stage::AfterSession),
                        session: None,
                        candidate: Some(trial.trial.clone()),
                        cwd: workdir.clone(),
                    };
                    let records = executed::judge(&at, &runs, outcomes);
                    let rejected = executed::rejects(&records);
                    crate::say::line(&format!(
                        "executed ▸ {}: {}",
                        trial.trial,
                        if rejected { "rejected" } else { "not rejected" }
                    ));
                    json!({
                        "trial": trial.trial,
                        "job": trial.job,
                        "kind": trial.kind,
                        "network": network,
                        "rejected": rejected,
                        "records": records,
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
        "schema": EXECUTED_SCHEMA,
        "task": name,
        "image": image,
        "commands": commands,
        "untouched": untouched,
        "trials": results,
    });
    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&out.join("executed.json"), format!("{text}\n").as_bytes())?;
    write_labels(name, &found, &out)?;
    Ok(value)
}
