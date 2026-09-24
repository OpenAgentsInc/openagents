//! Offline validity: does a green suite predict a verifier pass?
//!
//! For a Terminal-Bench task with retained, graded Coder One trials that
//! kept a post-executor snapshot (`verify.snapshot`), [`task`] writes a
//! suite from the task's words alone with Microluna, in the task's own
//! environment image, then runs the frozen suite against every trial's
//! snapshot restored into a fresh container of that image. [`validity`]
//! joins the results with the check-truth label rows and counts how often
//! "suite green" agrees with the verifier, beside today's checks and the
//! combined verdict on the same trials.
//!
//! The snapshot is the workspace right after the first executor. A trial
//! whose later rounds changed the workspace was graded on a different
//! candidate, so each trial says whether its check candidates all share
//! one digest ([`Trial::snapshot_graded`]); only those count toward
//! agreement.

use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// One retained trial with a snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trial {
    pub trial: String,
    pub job: String,
    pub episode: PathBuf,
    /// The verifier's reward.
    pub reward: Option<f64>,
    /// Every check candidate of the episode has the snapshot's digest, so
    /// the verifier graded the snapshot's workspace.
    pub snapshot_graded: bool,
    pub workdir: String,
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

/// Every retained trial of `task` under `jobs` with a snapshot, in name
/// order.
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
            if name.starts_with(&format!("{task}__"))
                && episode.join("snapshot/workspace.tar.gz").is_file()
            {
                let manifest: Value =
                    std::fs::read_to_string(episode.join("snapshot/snapshot.json"))
                        .ok()
                        .and_then(|t| serde_json::from_str(&t).ok())
                        .unwrap_or(Value::Null);
                let job = path
                    .strip_prefix(jobs)
                    .ok()
                    .and_then(|p| p.components().next())
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push(Trial {
                    trial: name,
                    job,
                    reward: reward(&path),
                    snapshot_graded: snapshot_graded(&episode),
                    workdir: manifest["workdir"].as_str().unwrap_or("/app").to_string(),
                    episode,
                });
            } else if depth < 3 {
                stack.push((path, depth + 1));
            }
        }
    }
    out.sort_by(|a, b| a.trial.cmp(&b.trial));
    out
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
    /// Only these trials, when given.
    pub only: Vec<String>,
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

/// Writes (or reuses) the suite for `task` and runs it on every trial's
/// snapshot; the record is written to `out/<task>/validity.json`.
///
/// # Errors
///
/// A message when the task, its image, or the Codex login is missing.
#[allow(clippy::too_many_lines)]
pub async fn task(name: &str, jev: &JevMode, options: &TaskOptions) -> Result<Value, String> {
    let mut found = trials(&options.jobs, name);
    if !options.only.is_empty() {
        found.retain(|t| options.only.contains(&t.trial));
    }
    if found.is_empty() {
        return Err(format!(
            "no retained trial of {name} under {} has a snapshot",
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
        AcceptanceSuite::load(&record)?
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
    let mut results = Vec::new();
    for trial in &found {
        let candidate = out.join(format!("candidate-{}", trial.trial));
        let _ = std::fs::remove_dir_all(&candidate);
        if let Err(error) = untar(&trial.episode.join("snapshot/workspace.tar.gz"), &candidate) {
            results.push(json!({ "trial": trial, "error": error }));
            continue;
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
            &suite,
            Path::new(&trial.workdir),
            &runner,
            Some(&recorder),
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
                    if trial.snapshot_graded {
                        ""
                    } else {
                        " (the snapshot isn't the graded candidate)"
                    }
                ));
                results.push(json!({ "trial": trial, "run": result }));
            }
            Err(tampered) => results.push(json!({ "trial": trial, "error": tampered.to_string() })),
        }
    }
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
}

/// A signal's agreement with the verifier over some trials.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Agreement {
    pub trials: usize,
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
    /// Of the verifier's failures, how many it called failed.
    pub failures: usize,
}

impl Agreement {
    fn of(rows: &[&Joined], says: impl Fn(&Joined) -> Option<Says>) -> Agreement {
        let mut out = Agreement {
            trials: rows.len(),
            ..Agreement::default()
        };
        for row in rows {
            let passed = row.reward.is_some_and(|r| r >= 1.0);
            if !passed {
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
    let rows = truth::read_rows(rows)?;
    let mut out = Vec::new();
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
        for entry in value["trials"].as_array().into_iter().flatten() {
            let Ok(trial) = serde_json::from_value::<Trial>(entry["trial"].clone()) else {
                continue;
            };
            let run: Option<RunResult> = serde_json::from_value(entry["run"].clone()).ok();
            let row = rows.iter().find(|r| r.trial == trial.trial);
            out.push(Joined {
                task: task.clone(),
                trial: trial.trial.clone(),
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
            });
        }
    }
    Ok(out)
}

/// The three signals' agreement over the joined trials: over the trials
/// whose snapshot was graded, and over all of them.
#[must_use]
pub fn validity(joined: &[Joined]) -> Value {
    let graded: Vec<&Joined> = joined.iter().filter(|j| j.snapshot_graded).collect();
    let all: Vec<&Joined> = joined.iter().collect();
    let table = |rows: &[&Joined]| {
        json!({
            "suite_green": Agreement::of(rows, |j| j.suite),
            "suite_complete": Agreement::of(rows, |j| j.complete),
            "todays_checks": Agreement::of(rows, |j| j.checks),
            "combined_verdict": Agreement::of(rows, |j| j.verdict),
        })
    };
    json!({
        "snapshot_graded": table(&graded),
        "all_snapshots": table(&all),
        "trials": joined,
    })
}
