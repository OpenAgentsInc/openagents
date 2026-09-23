//! The tiers above replay: mini-task episodes, and the Terminal-Bench
//! screen, measurement, and held-out confirmation.
//!
//! Tier 1 runs every mini-task with its known-good script under the
//! candidate's briefing policy. The scripted executor doesn't read the
//! briefing, so the tier screens for gross failures only: a candidate
//! whose briefing breaks the episode or overruns its cap. A real executor
//! on mini-tasks is `coder-one minitask run --executor`, which costs money
//! and isn't part of a study's defaults.
//!
//! Tiers 2 to 4 run the Harbor harness in `bench/terminal-bench` with the
//! candidate's manifest as the arm's `policy`, and read each trial's
//! reward, cost, and time from the job's `tbench/attempts/*.json`. They run
//! only under `--allow-terminal-bench`, with the artifact the arm installs.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Spend, Tier, write_json};
use crate::policy::{BriefPolicy, Manifest};

/// One mini-task under a candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MiniRun {
    pub task: String,
    pub verdict: String,
    pub reward: Option<f64>,
    /// The briefing's characters and cap, from the run's manifest.
    pub chars: Option<u64>,
    pub cap: Option<u64>,
    pub packer: Option<String>,
    pub milliseconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A candidate's mini-task screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MiniScreen {
    pub runs: Vec<MiniRun>,
    /// Every task passed and no briefing overran its cap.
    pub passed: bool,
}

/// Runs every mini-task's known-good script under `brief`, recording runs
/// under `out`.
pub async fn mini(brief: &BriefPolicy, out: &Path) -> MiniScreen {
    let mut runs = Vec::new();
    for task in crate::minitask::CATALOG {
        let started = Instant::now();
        let ran = crate::minitask::run::run(crate::minitask::run::Options {
            task: *task,
            executor: crate::minitask::run::ExecutorChoice::Scripted {
                variant: "good".to_string(),
                script: None,
            },
            out: out.to_path_buf(),
            jev: None,
            speed: 0.0,
            deadline: Duration::from_secs(60),
            controls: crate::session::Controls::default(),
            checks: false,
            brief: Some(brief.clone()),
            monitor: None,
            repair: None,
        })
        .await;
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        runs.push(match ran {
            Ok(ran) => MiniRun {
                task: task.id.to_string(),
                verdict: ran.grade.verdict.clone(),
                reward: ran.grade.reward(),
                chars: ran
                    .manifest
                    .pointer("/delegation/briefing/chars")
                    .and_then(Value::as_u64),
                cap: ran
                    .manifest
                    .pointer("/delegation/briefing/cap")
                    .and_then(Value::as_u64),
                packer: ran
                    .manifest
                    .pointer("/delegation/packer")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                milliseconds,
                error: None,
            },
            Err(error) => MiniRun {
                task: task.id.to_string(),
                verdict: "error".to_string(),
                reward: None,
                chars: None,
                cap: None,
                packer: None,
                milliseconds,
                error: Some(error),
            },
        });
    }
    let passed = runs.iter().all(|run| {
        run.reward == Some(1.0)
            && matches!((run.chars, run.cap), (Some(chars), Some(cap)) if chars <= cap)
    });
    MiniScreen { runs, passed }
}

/// What a Terminal-Bench tier needs to run.
#[derive(Debug, Clone)]
pub struct TerminalBench {
    /// The `bench/terminal-bench` directory.
    pub harness: PathBuf,
    /// The command that runs the harness's CLI, such as
    /// `uv run python -m tbench`.
    pub command: Vec<String>,
    /// The agent profile the candidate's manifest overrides.
    pub arm: String,
    /// The Coder One Linux binary the arm installs, and its SHA-256.
    pub artifact: PathBuf,
    pub artifact_sha256: String,
    /// Where Harbor writes jobs.
    pub jobs: PathBuf,
}

impl TerminalBench {
    /// The checked-in harness and the default arm, for `artifact`.
    #[must_use]
    pub fn new(artifact: PathBuf, artifact_sha256: String) -> Self {
        TerminalBench {
            harness: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench"),
            command: ["uv", "run", "python", "-m", "tbench"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            arm: "coder-one-pack-luna".to_string(),
            artifact,
            artifact_sha256,
            jobs: std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".openagents/terminal-bench/jobs"))
                .unwrap_or_default(),
        }
    }
}

/// The tasks and trials a tier runs, from the frozen task pool.
#[must_use]
pub fn tier_tasks(tier: Tier) -> (Vec<String>, usize, &'static str) {
    let pool = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/profiles/task-pool.json");
    let split = if tier == Tier::Confirm {
        "held-out"
    } else {
        "development-current"
    };
    let tasks = std::fs::read_to_string(&pool)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|v| v["tasks"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t["split"] == split)
        .filter_map(|t| t["id"].as_str().map(str::to_string))
        .collect();
    let trials = match tier {
        Tier::Screen => 1,
        _ => 3,
    };
    (tasks, trials, "extended")
}

/// One trial read back from a Harbor job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trial {
    pub task: String,
    pub trial: String,
    pub reward: Option<f64>,
    pub cost_usd: Option<f64>,
    pub seconds: Option<f64>,
}

/// Reads every attempt record in a Harbor job directory.
#[must_use]
pub fn ingest(job: &Path) -> Vec<Trial> {
    let Ok(entries) = std::fs::read_dir(job.join("tbench/attempts")) else {
        return Vec::new();
    };
    let mut out: Vec<Trial> = entries
        .flatten()
        .filter_map(|entry| {
            let value: Value =
                serde_json::from_str(&std::fs::read_to_string(entry.path()).ok()?).ok()?;
            let trial = value.pointer("/attempt/trial")?.as_str()?.to_string();
            Some(Trial {
                task: trial.split("__").next().unwrap_or(&trial).to_string(),
                trial,
                reward: value.pointer("/outcome/reward").and_then(Value::as_f64),
                cost_usd: value.pointer("/cost/amount_usd").and_then(Value::as_f64),
                seconds: value
                    .pointer("/timing/total_ms")
                    .and_then(Value::as_f64)
                    .map(|ms| ms / 1000.0),
            })
        })
        .collect();
    out.sort_by(|a, b| a.trial.cmp(&b.trial));
    out
}

/// `path` with the home directory spelled `~`, which the harness expands,
/// so a recorded command names no account.
fn home_relative(path: &Path) -> String {
    std::env::var_os("HOME")
        .and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf))
        .map_or_else(
            || path.display().to_string(),
            |rest| format!("~/{}", rest.display()),
        )
}

/// Plans a Terminal-Bench tier for `candidates`, and runs it when `bench`
/// is given: one job per candidate and trial, each task in it. Without
/// `bench`, it records the commands it would run.
///
/// # Errors
///
/// Returns a message when a candidate's manifest can't be written.
pub async fn terminal_bench(
    tier: Tier,
    candidates: &[(String, Manifest)],
    dir: &Path,
    bench: Option<&TerminalBench>,
    spend: &mut Vec<Spend>,
) -> Result<Value, String> {
    let (tasks, trials, profile) = tier_tasks(tier);
    let mut jobs = Vec::new();
    for (digest, manifest) in candidates {
        let path = dir
            .join("manifests")
            .join(format!("{}.json", &digest[..12]));
        write_json(&path, manifest)?;
        for trial in 1..=trials {
            let job = format!("study-{}-{}-{}", tier.word(), &digest[..12], trial);
            let mut args: Vec<String> = vec![
                "run".into(),
                "--profile".into(),
                profile.into(),
                "--agent".into(),
                bench.map_or("coder-one-pack-luna".to_string(), |b| b.arm.clone()),
                "--agent-kwarg".into(),
                format!("policy={}", home_relative(&path)),
                "--job-name".into(),
                job.clone(),
            ];
            if let Some(bench) = bench {
                args.push("--agent-kwarg".into());
                args.push(format!("artifact_path={}", bench.artifact.display()));
                args.push("--agent-kwarg".into());
                args.push(format!("artifact_sha256={}", bench.artifact_sha256));
            }
            for task in &tasks {
                args.push("--task".into());
                args.push(task.clone());
            }
            jobs.push((digest.clone(), job, args));
        }
    }
    let Some(bench) = bench else {
        return Ok(json!({
            "tier": tier.word(),
            "run": false,
            "why": format!("gated: pass {} with --through {} to run it", tier.gate().unwrap_or(""), tier.word()),
            "candidates": candidates.iter().map(|(d, _)| d).collect::<Vec<_>>(),
            "tasks": tasks,
            "trials_per_task": trials,
            "commands": jobs.iter().map(|(_, _, args)| {
                format!("uv run python -m tbench {}", args.join(" "))
            }).collect::<Vec<_>>(),
        }));
    };
    let started = Instant::now();
    let mut results = Vec::new();
    for (digest, job, args) in &jobs {
        let (program, rest) = bench
            .command
            .split_first()
            .ok_or("the harness command is empty")?;
        let status = std::process::Command::new(program)
            .args(rest)
            .args(args)
            .current_dir(&bench.harness)
            .status()
            .map_err(|error| format!("cannot run the harness: {error}"))?;
        let trials = ingest(&bench.jobs.join(job));
        results.push(json!({
            "candidate": digest,
            "job": job,
            "exit": status.code(),
            "trials": trials,
        }));
    }
    let cost: Vec<Option<f64>> = results
        .iter()
        .flat_map(|r| r["trials"].as_array().cloned().unwrap_or_default())
        .map(|t| t["cost_usd"].as_f64())
        .collect();
    spend.push(Spend {
        category: "student".to_string(),
        tier,
        calls: None,
        wall_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        spend_microusd: cost
            .iter()
            .all(Option::is_some)
            .then(|| cost.iter().flatten().map(|c| (c * 1e6) as u64).sum()),
        note: format!("{} Terminal-Bench jobs", jobs.len()),
    });
    Ok(json!({
        "tier": tier.word(),
        "run": true,
        "candidates": candidates.iter().map(|(d, _)| d).collect::<Vec<_>>(),
        "tasks": tasks,
        "trials_per_task": trials,
        "results": results,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiers_read_their_tasks_from_the_frozen_pool() {
        let (dev, one, _) = tier_tasks(Tier::Screen);
        assert_eq!(dev.len(), 8);
        assert_eq!(one, 1);
        let (held, three, _) = tier_tasks(Tier::Confirm);
        assert!(held.len() >= 20);
        assert_eq!(three, 3);
        assert!(dev.iter().all(|t| !held.contains(t)));
    }

    #[test]
    fn a_job_reads_back_its_attempts() {
        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("tbench/attempts");
        std::fs::create_dir_all(&attempts).unwrap();
        std::fs::write(
            attempts.join("fix-git__abc.json"),
            json!({
                "attempt": { "trial": "fix-git__abc" },
                "outcome": { "reward": 1.0 },
                "cost": { "amount_usd": 0.02 },
                "timing": { "total_ms": 150000 },
            })
            .to_string(),
        )
        .unwrap();
        let trials = ingest(dir.path());
        assert_eq!(trials.len(), 1);
        assert_eq!(trials[0].task, "fix-git");
        assert_eq!(trials[0].reward, Some(1.0));
        assert_eq!(trials[0].seconds, Some(150.0));
    }

    #[tokio::test]
    async fn a_gated_tier_records_its_commands_and_spends_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = super::super::pack::baseline().unwrap();
        let digest = manifest.digest();
        let mut spend = Vec::new();
        let record = terminal_bench(
            Tier::Screen,
            &[(digest.clone(), manifest)],
            dir.path(),
            None,
            &mut spend,
        )
        .await
        .unwrap();
        assert_eq!(record["run"], false);
        assert!(spend.is_empty());
        let command = record["commands"][0].as_str().unwrap();
        assert!(command.contains("--agent coder-one-pack-luna"), "{command}");
        assert!(command.contains("--task fix-git"), "{command}");
        assert!(
            dir.path()
                .join("manifests")
                .join(format!("{}.json", &digest[..12]))
                .is_file()
        );
    }

    #[tokio::test]
    async fn the_mini_tier_passes_the_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = super::super::pack::baseline().unwrap();
        let screen = mini(&manifest.policy.brief, dir.path()).await;
        assert_eq!(screen.runs.len(), crate::minitask::CATALOG.len());
        assert!(screen.passed, "{screen:?}");
        assert!(
            screen
                .runs
                .iter()
                .all(|r| r.packer.as_deref() == Some("coverage"))
        );
    }
}
