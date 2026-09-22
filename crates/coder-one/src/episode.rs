//! The headless episode contract, `openagents.coder.episode.v1`, as
//! Coder One implements it for the Terminal-Bench harness.
//!
//! The harness's Harbor adapter installs this binary inside a task
//! environment, runs `episode doctor` before any inference, then runs
//! `episode run` in the task's working directory and collects the bundle
//! it writes. `docs/coder/terminal-bench-contract.md` is the contract.
//!
//! Configuration comes from the environment the adapter forwards:
//!
//! | Variable | Meaning |
//! | --- | --- |
//! | `OPENAGENTS_API_KEY` | Bearer for the generation door. Required. |
//! | `OPENAGENTS_DOOR_URL` | The generation door's base URL; `https://openagents.com` when unset. |
//! | `OPENAGENTS_MODEL` | The lane when `--model` is absent; `free` when both are. |
//! | `TYPESAFE_API_KEY` | The Jev key. Required unless `CODER_ONE_JEV=off`. |
//! | `CODER_ONE_JEV` | `off` runs the search-hit baseline without Jev. |
//! | `CODER_ONE_MAX_STEPS` | The step limit; 50 when unset. |
//! | `CODER_ONE_COMMAND_TIMEOUT` | Seconds each command may run; 300 when unset. |
//!
//! The bundle is rewritten at the start of every step, so a deadline that
//! kills the process still leaves the evidence up to the last step.

use std::path::{Path, PathBuf};
use std::time::Duration;

use atif::document::{Session, Source, Step};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::agent::{EPISODE_INSTRUCTIONS, Judge, Judgments};
use crate::credentials::{self, Secret};
use crate::generate::Door;
use crate::judge::JevJudge;
use crate::record::Recorder;
use crate::shell::Checkout;
use crate::state::{Environment, Issue, State};
use crate::{Bounds, Ended, run};

/// The contract this binary implements.
pub const CONTRACT: &str = "openagents.coder.episode.v1";

/// The version `--version` prints and every record carries: the crate
/// version and the commit the build script stamped, or `dev`.
pub fn version() -> String {
    format!(
        "coder-one {} ({})",
        env!("CARGO_PKG_VERSION"),
        option_env!("CODER_ONE_COMMIT").unwrap_or("dev")
    )
}

/// Jev's published rate for `jev-1.13.0`, in dollars per million input
/// tokens, retrieved 2026-09-22. Used only for a labeled estimate.
const JEV_USD_PER_MILLION_INPUT: f64 = 0.042;

/// Everything the episode reads from its environment, resolved once.
struct Settings {
    bearer: Option<Secret>,
    door_url: String,
    lane: String,
    jev_key: Option<Secret>,
    jev: bool,
    max_steps: usize,
    command_timeout: Duration,
}

impl Settings {
    fn from_env(model: Option<&str>) -> Result<Self, String> {
        let env = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let dir = credentials::openagents_dir().unwrap_or_else(|| PathBuf::from("/nonexistent"));
        let jev = !matches!(env("CODER_ONE_JEV").as_deref(), Some("off" | "0" | "false"));
        let number = |name: &str, default: u64| -> Result<u64, String> {
            env(name).map_or(Ok(default), |value| {
                value
                    .parse()
                    .map_err(|_| format!("{name} must be a whole number"))
            })
        };
        Ok(Settings {
            bearer: credentials::bearer(|name| env(name), &dir)
                .ok()
                .map(|found| found.secret),
            door_url: env("OPENAGENTS_DOOR_URL")
                .unwrap_or_else(|| credentials::GENERATION_BASE_URL.to_string()),
            lane: model
                .map(str::to_string)
                .or_else(|| env("OPENAGENTS_MODEL"))
                .unwrap_or_else(|| "free".to_string()),
            jev_key: credentials::jev_key(|name| env(name), &dir)
                .ok()
                .map(|found| found.secret),
            jev,
            max_steps: usize::try_from(number("CODER_ONE_MAX_STEPS", 50)?).unwrap_or(50),
            command_timeout: Duration::from_secs(number("CODER_ONE_COMMAND_TIMEOUT", 300)?),
        })
    }
}

/// `episode doctor`: every requirement checked without spending inference.
pub async fn doctor(contract: &str) -> Result<(), String> {
    if contract != CONTRACT {
        return Err(format!("this binary implements {CONTRACT}, not {contract}"));
    }
    let settings = Settings::from_env(None)?;
    let mut problems = Vec::new();
    println!("version: {}", version());
    println!("contract: {CONTRACT}");

    match &settings.bearer {
        None => problems.push("OPENAGENTS_API_KEY is not set".to_string()),
        Some(_) => {
            let url = format!("{}/v1/models", settings.door_url.trim_end_matches('/'));
            match reqwest::Client::new()
                .get(&url)
                .timeout(Duration::from_secs(20))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    println!("generation door: {url} answered {}", response.status());
                }
                Ok(response) => problems.push(format!("{url} answered {}", response.status())),
                Err(error) => problems.push(format!("{url} is unreachable: {error}")),
            }
        }
    }

    if settings.jev {
        match &settings.jev_key {
            None => problems
                .push("TYPESAFE_API_KEY is not set and CODER_ONE_JEV is not off".to_string()),
            Some(key) => {
                let client = credentials::jev_client(key)?;
                match client.models().list(jev::ListOptions::default()).await {
                    Ok(models) => println!(
                        "jev door: {} answered with {} models",
                        credentials::JEV_BASE_URL,
                        models.len()
                    ),
                    Err(error) => problems.push(format!("jev door: {error}")),
                }
            }
        }
    } else {
        println!("jev: off (CODER_ONE_JEV)");
    }

    if problems.is_empty() {
        println!("ok");
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// The arguments `episode run` takes.
pub struct RunArgs {
    pub instruction_file: PathBuf,
    pub output_dir: PathBuf,
    pub contract: String,
    pub model: Option<String>,
}

/// `episode run`: one headless episode in the current directory. Returns
/// the process exit code: 0 when the agent finished, 3 when the step limit
/// ran out, and 4 when generation failed.
pub async fn run_episode(args: RunArgs) -> Result<i32, String> {
    if args.contract != CONTRACT {
        return Err(format!(
            "this binary implements {CONTRACT}, not {}",
            args.contract
        ));
    }
    let settings = Settings::from_env(args.model.as_deref())?;
    let bearer = settings
        .bearer
        .clone()
        .ok_or("OPENAGENTS_API_KEY is not set")?;
    let jev_client = if settings.jev {
        let key = settings
            .jev_key
            .as_ref()
            .ok_or("TYPESAFE_API_KEY is not set and CODER_ONE_JEV is not off")?;
        Some(credentials::jev_client(key)?)
    } else {
        None
    };
    let instruction = std::fs::read_to_string(&args.instruction_file)
        .map_err(|error| format!("cannot read {}: {error}", args.instruction_file.display()))?;
    let workdir = std::env::current_dir().map_err(|error| error.to_string())?;
    let bundle = Bundle::create(&args.output_dir, &settings, &workdir)?;

    let recorder = Recorder::default();
    recorder.push(Step::said(Source::System, EPISODE_INSTRUCTIONS));
    recorder.push(Step::said(Source::User, &instruction));

    let first_line = instruction.lines().find(|line| !line.trim().is_empty());
    let mut state = State::new(
        Environment {
            repository: String::new(),
            workdir: workdir.to_string_lossy().into_owned(),
            os: std::env::consts::OS.to_string(),
        },
        Issue {
            url: String::new(),
            title: crate::judge::clip(first_line.unwrap_or("Task").trim(), 120),
            body: instruction.clone(),
            labels: Vec::new(),
        },
    );
    println!("{} · {CONTRACT}", version());
    println!(
        "workdir {} · lane {} · jev {} · {} steps · {}s per command",
        workdir.display(),
        settings.lane,
        if settings.jev {
            credentials::JEV_MODEL
        } else {
            "off"
        },
        settings.max_steps,
        settings.command_timeout.as_secs()
    );

    let judge = JevJudge::new(jev_client, workdir.clone(), &state.issue, recorder.clone());
    let mut judge = Snapshots {
        inner: judge,
        bundle: &bundle,
        recorder: recorder.clone(),
    };
    let mut door = Door::new(
        &settings.door_url,
        bearer,
        &settings.lane,
        EPISODE_INSTRUCTIONS,
        Box::new(|delta| {
            use std::io::Write as _;
            print!("{delta}");
            let _ = std::io::stdout().flush();
        }),
        recorder.clone(),
    )?;
    let mut shell = Checkout {
        workdir: workdir.clone(),
        deadline: settings.command_timeout,
        recorder: recorder.clone(),
        commands: 0,
    };

    let ended = run(
        &mut state,
        "Complete this task.",
        Bounds {
            max_steps: settings.max_steps,
        },
        &mut judge,
        &mut door,
        &mut shell,
    )
    .await;

    let (outcome, code) = match &ended {
        Ended::Finished { title, summary, .. } => {
            recorder.push(Step::said(
                Source::Agent,
                &format!("finished: {title}\n\n{summary}"),
            ));
            ("finished", 0)
        }
        Ended::StepLimit { .. } => ("step_limit", 3),
        Ended::GenerationFailed { .. } => ("generation_failed", 4),
    };
    println!("\n── {outcome} ──");
    bundle.write(&state, &recorder.steps(), outcome, Some(&ended))?;
    Ok(code)
}

/// A judge that rewrites the bundle before every step, so a killed
/// episode leaves its evidence behind.
struct Snapshots<'a> {
    inner: JevJudge,
    bundle: &'a Bundle,
    recorder: Recorder,
}

impl Judge for Snapshots<'_> {
    async fn judge(&mut self, state: &State) -> Judgments {
        if let Err(error) = self
            .bundle
            .write(state, &self.recorder.steps(), "running", None)
        {
            eprintln!("coder-one: cannot write the bundle: {error}");
        }
        self.inner.judge(state).await
    }
}

/// The episode bundle under the output directory.
struct Bundle {
    dir: PathBuf,
    started: u64,
    header: Value,
    workdir: PathBuf,
    /// The workdir's Git HEAD at the start, when it is a work tree, so the
    /// diff covers what the episode changed.
    base: Option<String>,
    session: Session,
}

impl Bundle {
    fn create(dir: &Path, settings: &Settings, workdir: &Path) -> Result<Self, String> {
        for sub in ["artifacts", "verification", "evaluation"] {
            std::fs::create_dir_all(dir.join(sub))
                .map_err(|error| format!("cannot create {}: {error}", dir.join(sub).display()))?;
        }
        let started = atif::document::now_ms();
        let id = format!("coder-one-{started}");
        let mut session = Session::opening(
            &id,
            &settings.lane,
            &settings.door_url,
            &workdir.to_string_lossy(),
            &version(),
        );
        session.directive = "Complete this task.".to_string();
        let base = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(workdir)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());
        let header = json!({
            "contract": CONTRACT,
            "artifact": {
                "name": "coder-one",
                "version": version(),
                "sha256": self_digest(),
            },
            "doors": {
                "generation": {
                    "url": settings.door_url,
                    "lane_requested": settings.lane,
                },
                "jev": if settings.jev {
                    json!({ "url": credentials::JEV_BASE_URL, "model": credentials::JEV_MODEL })
                } else {
                    json!({ "enabled": false })
                },
            },
            "bounds": {
                "max_steps": settings.max_steps,
                "command_timeout_sec": settings.command_timeout.as_secs(),
                "episode_deadline": "owned by the harness's exec timeout",
            },
        });
        Ok(Bundle {
            dir: dir.to_path_buf(),
            started,
            header,
            workdir: workdir.to_path_buf(),
            base,
            session,
        })
    }

    fn write(
        &self,
        state: &State,
        steps: &[Step],
        outcome: &str,
        ended: Option<&Ended>,
    ) -> Result<(), String> {
        let now = atif::document::now_ms();
        let mut session = self.session.clone();
        session.state = if outcome == "running" {
            "interrupted"
        } else {
            "ended"
        }
        .to_string();
        session.seconds = (now - self.started) / 1000;
        let mut trajectory = atif::document::document(&session, steps);
        trajectory["agent"]["name"] = json!("coder-one");

        let usage = usage(steps);
        let state_json = serde_json::to_value(state).unwrap_or(Value::Null);
        let diff = self.diff();

        let mut files = serde_json::Map::new();
        for (key, relative, value) in [
            ("trajectory", "trajectory.atif.json", &trajectory),
            ("usage", "evaluation/usage.json", &usage),
            ("state", "artifacts/state.json", &state_json),
        ] {
            let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
            files.insert(key.to_string(), self.put(relative, text.as_bytes())?);
        }
        if let Some(diff) = diff {
            files.insert(
                "diff".to_string(),
                self.put("artifacts/diff.patch", diff.as_bytes())?,
            );
        }

        let mut manifest = self.header.clone();
        manifest["started_at"] = json!(atif::document::iso(self.started));
        manifest["updated_at"] = json!(atif::document::iso(now));
        manifest["outcome"] = json!(outcome);
        manifest["steps"] = json!(state.history.len());
        manifest["workdir"] = json!(self.workdir.to_string_lossy());
        manifest["git_base"] = json!(self.base);
        manifest["doors"]["generation"]["models_served"] = trajectory_models(steps);
        manifest["files"] = Value::Object(files);
        manifest["verification"] = json!({
            "note": "Coder One runs no independent verification of its own; the task's verifier is the grader.",
        });
        if let Some(Ended::Finished { title, summary, .. }) = ended {
            manifest["result"] = json!({ "title": title, "summary": summary });
        }
        if let Some(Ended::GenerationFailed { error, .. }) = ended {
            manifest["error"] = json!(error);
        }
        let text = serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?;
        self.put("manifest.json", text.as_bytes())?;
        Ok(())
    }

    /// Writes one file and returns its reference: path, bytes, and sha256.
    fn put(&self, relative: &str, bytes: &[u8]) -> Result<Value, String> {
        let path = self.dir.join(relative);
        std::fs::write(&path, bytes)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        Ok(json!({
            "path": relative,
            "bytes": bytes.len(),
            "sha256": hex(&Sha256::digest(bytes)),
        }))
    }

    /// The working tree's change against the base commit, untracked files
    /// included, when the workdir is a Git work tree.
    fn diff(&self) -> Option<String> {
        let base = self.base.as_deref()?;
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&self.workdir)
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        };
        // Intent-to-add makes untracked files show in the diff without
        // staging their content.
        let _ = git(&["add", "-N", "."]);
        git(&["diff", "--binary", base])
    }
}

/// The usage record, derived from the trajectory so both say the same
/// thing. Unreported values are null, never zero.
fn usage(steps: &[Step]) -> Value {
    let generations: Vec<&Step> = steps
        .iter()
        .filter(|step| step.source == Source::Agent && step.call.is_none() && step.tokens.is_some())
        .collect();
    let failed_generations = steps
        .iter()
        .filter(|step| {
            step.source == Source::System && step.message.starts_with("generation failed")
        })
        .count();
    let retries: u64 = generations
        .iter()
        .filter_map(|step| step.extensions.get("attempts").and_then(Value::as_u64))
        .map(|attempts| attempts.saturating_sub(1))
        .sum();
    let decisions: Vec<&atif::document::Call> = steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .filter(|call| call.is_decision())
        .collect();
    let decisions_failed = decisions
        .iter()
        .filter(|call| call.outcome == atif::document::Outcome::Failed)
        .count();

    let gen_input: u64 = generations
        .iter()
        .filter_map(|s| s.tokens)
        .map(|t| t.0)
        .sum();
    let gen_output: u64 = generations
        .iter()
        .filter_map(|s| s.tokens)
        .map(|t| t.1)
        .sum();
    let costs: Vec<Option<u64>> = generations
        .iter()
        .map(|step| step.extensions.get("cost_microusd").and_then(Value::as_u64))
        .collect();
    let priced = costs.iter().all(Option::is_some) && !costs.is_empty();
    let gen_cost = costs.iter().flatten().sum::<u64>() as f64 / 1_000_000.0;

    let jev_input: Option<u64> = steps
        .iter()
        .filter_map(|step| step.extensions.get("jev_usage"))
        .map(|usage| usage.get("input_tokens").and_then(Value::as_u64))
        .sum();
    let jev_cost = jev_input.map(|tokens| tokens as f64 * JEV_USD_PER_MILLION_INPUT / 1_000_000.0);

    json!({
        "tokens": {
            "input": gen_input + jev_input.unwrap_or(0),
            "cache": Value::Null,
            "output": gen_output,
        },
        "cost": {
            "amount_usd": if priced { json!(gen_cost) } else { Value::Null },
            "provenance": if priced { "provider_reported" } else { "unknown" },
            "covers": "generation only; the Jev estimate is under components.jev",
        },
        "calls": {
            "generation": generations.len(),
            "decisions": decisions.len(),
            "failed": failed_generations + decisions_failed,
            "retries": retries,
        },
        "components": {
            "generation": {
                "input_tokens": gen_input,
                "output_tokens": gen_output,
                "cost_usd": if priced { json!(gen_cost) } else { Value::Null },
                "cost_provenance": if priced { "provider_reported" } else { "unknown" },
                "unpriced_calls": costs.iter().filter(|cost| cost.is_none()).count(),
            },
            "jev": {
                "model": credentials::JEV_MODEL,
                "requests": decisions.len(),
                "input_tokens": jev_input,
                "output_tokens_billed": false,
                "cost_usd": jev_cost,
                "cost_provenance": if jev_cost.is_some() { "price_estimate" } else { "unknown" },
                "rate": "$0.042 per million input tokens, retrieved 2026-09-22",
            },
        },
    })
}

fn trajectory_models(steps: &[Step]) -> Value {
    let mut models: Vec<&str> = Vec::new();
    for step in steps {
        if let Some(model) = step.model.as_deref()
            && step.call.is_none()
            && !models.contains(&model)
        {
            models.push(model);
        }
    }
    json!(models)
}

/// The sha256 of this executable, so the manifest names exactly what ran.
fn self_digest() -> Value {
    std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .map_or(Value::Null, |bytes| json!(hex(&Sha256::digest(&bytes))))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
