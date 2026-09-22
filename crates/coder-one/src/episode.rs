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
//! | `CODER_ONE_DELEGATE` | `off`, `always`, or `auto`; `off` when unset. |
//! | `CODER_ONE_DELEGATE_AGENT` | `claude-code` or `codex`; `claude-code` when unset. |
//! | `CODER_ONE_DELEGATE_MODEL` | The delegate's model; `claude-opus-5-5` for Claude Code and `gpt-6-luna` for Codex when unset. |
//! | `CODER_ONE_DELEGATE_TOOLS` | Claude Code's built-in tools for the delegate, such as `Bash,Read,Edit,Write`; the full default set when unset. |
//! | `CODER_ONE_DELEGATE_TIMEOUT` | Seconds the delegate may run; 600 when unset. |
//! | `CODER_ONE_EXPLORE_STEPS` | The explore phase's step bound; 8 when unset. |
//! | `CODER_ONE_BRIEFING_CAP` | The briefing's length cap in characters; 12,000 when unset. |
//! | `CODER_ONE_CLAUDE_BIN` | The `claude` binary; the first on `PATH` when unset. |
//! | `CODER_ONE_CODEX_BIN` | The `codex` binary; the first on `PATH` when unset. |
//! | `CLAUDE_CODE_OAUTH_TOKEN` | The Claude Code delegate's subscription token; or `ANTHROPIC_API_KEY`. |
//! | `CODEX_HOME` | Where the Codex delegate finds `auth.json`; `~/.codex` when unset. |
//! | `CODER_ONE_DEEP` | `on` runs deep Jev mode: a parallel survey before the first step, a readiness question each step, and repeated-command hints. |
//! | `CODER_ONE_PROBES` | `on`, with deep mode, runs a battery of read-only probes (listing, git state, README, tests, versions) and lets Jev pick the outputs that go into the survey and the briefing. |
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
use crate::delegate::{self, Agent, Cli, Credential, Delegated, Explorer, Mode, Plan, Policy};
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
    deep: bool,
    max_steps: usize,
    command_timeout: Duration,
    delegate: Mode,
    delegate_agent: Agent,
    delegate_model: String,
    delegate_timeout: Duration,
    policy: Policy,
    briefing_cap: usize,
    delegate_bin: Option<PathBuf>,
    credential: Credential,
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
        let delegate_agent = Agent::parse(
            env("CODER_ONE_DELEGATE_AGENT")
                .as_deref()
                .unwrap_or("claude-code"),
        )?;
        let (delegate_bin, credential) = delegate::resolve(delegate_agent, |name| env(name));
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
            deep: jev && matches!(env("CODER_ONE_DEEP").as_deref(), Some("on" | "1" | "true")),
            max_steps: usize::try_from(number("CODER_ONE_MAX_STEPS", 50)?).unwrap_or(50),
            command_timeout: Duration::from_secs(number("CODER_ONE_COMMAND_TIMEOUT", 300)?),
            delegate: Mode::parse(env("CODER_ONE_DELEGATE").as_deref().unwrap_or("off"))?,
            delegate_agent,
            delegate_model: env("CODER_ONE_DELEGATE_MODEL")
                .unwrap_or_else(|| delegate_agent.default_model().to_string()),
            delegate_timeout: Duration::from_secs(number("CODER_ONE_DELEGATE_TIMEOUT", 600)?),
            policy: Policy {
                explore_steps: usize::try_from(number("CODER_ONE_EXPLORE_STEPS", 8)?).unwrap_or(8),
                ..Policy::default()
            },
            briefing_cap: usize::try_from(number(
                "CODER_ONE_BRIEFING_CAP",
                delegate::BRIEFING_CAP as u64,
            )?)
            .unwrap_or(delegate::BRIEFING_CAP),
            delegate_bin,
            credential,
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

    if settings.delegate == Mode::Off {
        println!("delegate: off (CODER_ONE_DELEGATE)");
    } else {
        println!(
            "delegate: {} to {} ({}), explore {} steps, deadline {}s",
            settings.delegate.word(),
            settings.delegate_agent.word(),
            settings.delegate_model,
            settings.policy.explore_steps,
            settings.delegate_timeout.as_secs()
        );
        problems.extend(check_delegate(&settings).await);
    }

    if problems.is_empty() {
        println!("ok");
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// The oldest Claude Code the delegate accepts: the API refuses Opus 5.5
/// to 2.1.278.
const CLAUDE_MIN: (u64, u64, u64) = (2, 1, 280);

/// Checks that the delegate's `--version` runs, that Claude Code is new
/// enough, and that a credential is present, without any inference.
async fn check_delegate(settings: &Settings) -> Vec<String> {
    let mut problems = Vec::new();
    let agent = settings.delegate_agent;
    match &settings.delegate_bin {
        None => problems.push(format!(
            "no {} binary: set {} or put it on PATH",
            agent.word(),
            agent.binary_variable()
        )),
        Some(binary) => {
            let mut command = std::process::Command::new(binary);
            command.arg("--version").env_remove("CLAUDECODE");
            let ended = supervise::Job::from_command(command)
                .bounded(supervise::Limits::within(Duration::from_secs(30)))
                .run()
                .await;
            let text = ended.stdout.text.trim().to_string();
            // `codex-cli 0.155.1` names the program first; Claude Code
            // prints the version first.
            let version_text = match agent {
                Agent::Codex => text
                    .split_whitespace()
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join(" "),
                Agent::ClaudeCode => text.clone(),
            };
            match (ended.ending.success(), parse_version(&version_text)) {
                (true, Some(_)) if agent == Agent::Codex => {
                    println!("codex: {} at {}", text, binary.display());
                }
                (true, Some(version)) if version >= CLAUDE_MIN => {
                    println!("claude: {} at {}", text, binary.display());
                }
                (true, Some(_)) => problems.push(format!(
                    "claude {text} is older than {}.{}.{}",
                    CLAUDE_MIN.0, CLAUDE_MIN.1, CLAUDE_MIN.2
                )),
                _ => problems.push(format!(
                    "{} --version failed: {} {}",
                    binary.display(),
                    ended.ending,
                    crate::judge::clip(ended.stderr.text.trim(), 200)
                )),
            }
        }
    }
    match (settings.credential, agent) {
        (Credential::Missing, Agent::ClaudeCode) => problems.push(
            "no delegate credential: set CLAUDE_CODE_OAUTH_TOKEN or ANTHROPIC_API_KEY".to_string(),
        ),
        (Credential::Missing, Agent::Codex) => problems.push(
            "no delegate credential: put auth.json under CODEX_HOME or set OPENAI_API_KEY"
                .to_string(),
        ),
        (found, _) => println!("{} credential: {}", agent.word(), found.word()),
    }
    problems
}

/// `(major, minor, patch)` from `2.1.280 (Claude Code)`.
fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let word = text.split_whitespace().next()?;
    let mut parts = word.split('.').map(|part| part.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// Whether `CODER_ONE_PROBES` asks for the probe battery. It runs with the
/// deep survey, so it needs `CODER_ONE_DEEP` too.
fn probes_on() -> bool {
    matches!(
        std::env::var("CODER_ONE_PROBES").as_deref().map(str::trim),
        Ok("on" | "1" | "true")
    )
}

/// The arguments `episode run` takes.
pub struct RunArgs {
    pub instruction_file: PathBuf,
    pub output_dir: PathBuf,
    pub contract: String,
    pub model: Option<String>,
}

/// `episode run`: one headless episode in the current directory. Returns
/// the process exit code: 0 when the agent or its delegate finished, 3 when
/// the step limit ran out, 4 when generation failed, and 5 when the
/// delegate did not answer.
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
        "workdir {} · lane {} · jev {}{} · {} steps · {}s per command · delegate {}",
        workdir.display(),
        settings.lane,
        if settings.jev {
            credentials::JEV_MODEL
        } else {
            "off"
        },
        if settings.deep { " (deep)" } else { "" },
        settings.max_steps,
        settings.command_timeout.as_secs(),
        settings.delegate.word()
    );

    let mut judge = JevJudge::new(jev_client, workdir.clone(), &state.issue, recorder.clone())
        .deep(settings.deep)
        .probing(settings.deep && probes_on());
    judge.survey(&mut state).await;
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
    )?
    .caching_under(&bundle.session.id);
    let mut shell = Checkout {
        workdir: workdir.clone(),
        deadline: settings.command_timeout,
        recorder: recorder.clone(),
        commands: 0,
    };

    let (ended, delegated) = if settings.delegate == Mode::Off {
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
        (ended, None)
    } else {
        let mut executor = Cli {
            agent: settings.delegate_agent,
            binary: settings.delegate_bin.clone(),
            model: settings.delegate_model.clone(),
            deadline: settings.delegate_timeout,
            workdir: workdir.clone(),
            artifacts: args.output_dir.join("artifacts"),
            artifacts_label: "artifacts".to_string(),
            // The task container is the boundary, and the agent often runs
            // as root there, where the CLI refuses to bypass permissions
            // unless told it is in a sandbox.
            env: vec![("IS_SANDBOX".to_string(), "1".to_string())],
            credential: settings.credential,
            runs: 0,
        };
        let plan = Plan {
            mode: settings.delegate,
            policy: settings.policy,
            max_steps: settings.max_steps,
            prompt: "Complete this task.",
            instruction: &instruction,
            directions: EPISODE_DIRECTIONS,
            cap: settings.briefing_cap,
            isolation: "none",
            base: bundle.base.as_deref(),
        };
        let bundle_ref = &bundle;
        let steps_recorder = recorder.clone();
        let mut checkpoint = |state: &State| {
            if let Err(error) =
                bundle_ref.write(state, &steps_recorder.steps(), "running", None, None)
            {
                eprintln!("coder-one: cannot write the bundle: {error}");
            }
        };
        delegate::explore_then_delegate(
            &mut state,
            &plan,
            &mut judge,
            &mut door,
            &mut shell,
            &mut executor,
            &recorder,
            &mut checkpoint,
        )
        .await
    };

    let (outcome, code) = match &ended {
        Ended::Finished { title, summary, .. } => {
            recorder.push(Step::said(
                Source::Agent,
                &format!("finished: {title}\n\n{summary}"),
            ));
            ("finished", 0)
        }
        Ended::Delegated {
            answered: true,
            title,
            summary,
            ..
        } => {
            recorder.push(Step::said(
                Source::Agent,
                &format!("finished by the delegate: {title}\n\n{summary}"),
            ));
            ("delegated", 0)
        }
        Ended::Delegated { .. } => ("delegate_failed", 5),
        Ended::StepLimit { .. } | Ended::Stopped { .. } => ("step_limit", 3),
        Ended::GenerationFailed { .. } => ("generation_failed", 4),
    };
    println!("\n── {outcome} ──");
    bundle.write(
        &state,
        &recorder.steps(),
        outcome,
        Some(&ended),
        delegated.as_ref(),
    )?;
    Ok(code)
}

/// The delegate's closing directions in an episode.
const EPISODE_DIRECTIONS: &str = "Complete the task in the current working \
directory. Nobody answers questions, so decide from the task and the \
environment. An automated checker grades the final state of the environment \
against the task, so verify every requirement, including exact paths, names, \
and formats, before you stop. The files and command outputs in this briefing \
were gathered just before you started and are current: use them instead of \
re-running those commands, and go straight to the work. End with a short \
summary of what you changed and how you checked it.";

/// A judge that rewrites the bundle before every step, so a killed
/// episode leaves its evidence behind.
struct Snapshots<'a> {
    inner: JevJudge,
    bundle: &'a Bundle,
    recorder: Recorder,
}

impl Explorer for Snapshots<'_> {
    fn jev(&self) -> &JevJudge {
        &self.inner
    }
    fn jev_mut(&mut self) -> &mut JevJudge {
        &mut self.inner
    }
}

impl Judge for Snapshots<'_> {
    async fn judge(&mut self, state: &State) -> Judgments {
        if let Err(error) = self
            .bundle
            .write(state, &self.recorder.steps(), "running", None, None)
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
            "delegate": if settings.delegate == Mode::Off {
                json!({ "mode": "off" })
            } else {
                json!({
                    "mode": settings.delegate.word(),
                    "agent": settings.delegate_agent.word(),
                    "model": settings.delegate_model,
                    "executor_path": settings.delegate_bin.as_ref().map(|path| path.to_string_lossy()),
                    "credential": settings.credential.word(),
                    "deadline_sec": settings.delegate_timeout.as_secs(),
                    "briefing_cap": settings.briefing_cap,
                    "policy": settings.policy.record(),
                    "isolation": "none: the task container is the boundary",
                })
            },
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
            "jev_mode": if !settings.jev { "off" } else if settings.deep { "deep" } else { "step" },
            "prompt_layout": "cache-stable-prefix",
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
        delegated: Option<&Delegated>,
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

        let delegating = self.header["delegate"]["mode"].as_str() != Some("off");
        let usage = usage(steps, delegating);
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
        // The delegate writes its briefing and stream itself; the manifest
        // names each one that exists.
        if let Ok(entries) = std::fs::read_dir(self.dir.join("artifacts")) {
            let mut names: Vec<String> = entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with("delegate-"))
                .collect();
            names.sort();
            for name in names {
                let relative = format!("artifacts/{name}");
                if let Ok(bytes) = std::fs::read(self.dir.join(&relative)) {
                    files.insert(
                        name.replace('.', "_"),
                        json!({
                            "path": relative,
                            "bytes": bytes.len(),
                            "sha256": hex(&Sha256::digest(&bytes)),
                        }),
                    );
                }
            }
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
        if let Some(Ended::Delegated {
            title,
            summary,
            status,
            ..
        }) = ended
        {
            manifest["result"] =
                json!({ "title": title, "summary": summary, "delegate_status": status });
        }
        if let Some(Ended::Stopped { reason, .. }) = ended {
            manifest["stopped"] = json!({ "code": reason.code(), "reason": reason.to_string() });
        }
        if let Some(delegated) = delegated {
            manifest["delegate"]["delegation"] = delegated.record();
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
/// thing. Unreported values are null, never zero. `delegating` adds the
/// delegate component, which a run with delegation off leaves out.
pub fn usage(steps: &[Step], delegating: bool) -> Value {
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
    // No generation at all is a known cost of zero, as in a Jev-brief
    // episode that delegates before the explorer runs; one unreported call
    // leaves the generation cost unknown.
    let priced = costs.iter().all(Option::is_some);
    let gen_cost = costs.iter().flatten().sum::<u64>() as f64 / 1_000_000.0;

    let jev_input: Option<u64> = steps
        .iter()
        .filter_map(|step| step.extensions.get("jev_usage"))
        .map(|usage| usage.get("input_tokens").and_then(Value::as_u64))
        .sum();
    let jev_cost = jev_input.map(|tokens| tokens as f64 * JEV_USD_PER_MILLION_INPUT / 1_000_000.0);

    let cached: Vec<Option<u64>> = generations
        .iter()
        .map(|step| step.extensions.get("cached_tokens").and_then(Value::as_u64))
        .collect();
    let gen_cached = if !cached.is_empty() && cached.iter().all(Option::is_some) {
        json!(cached.iter().flatten().sum::<u64>())
    } else {
        Value::Null
    };

    let delegate = delegate_usage(steps);
    let delegations = delegate.calls.len();
    let delegate_failed = delegate
        .calls
        .iter()
        .filter(|call| call.outcome != atif::document::Outcome::Completed)
        .count();

    let gen_cost_known = priced.then_some(gen_cost);
    let delegate_cost_known = if delegations == 0 {
        Some(0.0)
    } else {
        delegate.cost_usd
    };
    let total = match (gen_cost_known, jev_cost, delegate_cost_known) {
        (Some(generation), Some(jev), Some(delegated)) => json!(generation + jev + delegated),
        _ => Value::Null,
    };
    let delegate_input = if delegations == 0 {
        Some(0)
    } else {
        delegate.total_input()
    };

    let mut components = json!({
        "generation": {
            "input_tokens": gen_input,
            "cached_input_tokens": gen_cached,
            "output_tokens": gen_output,
            "cost_usd": gen_cost_known,
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
    });
    if delegating || delegations > 0 {
        components["delegate"] = delegate.record();
    }

    json!({
        "tokens": {
            "input": match delegate_input {
                Some(delegated) => json!(gen_input + jev_input.unwrap_or(0) + delegated),
                None => Value::Null,
            },
            "cache": if delegations > 0 { json!(delegate.cache_read) } else { gen_cached.clone() },
            "output": match (delegations, delegate.output) {
                (0, _) => json!(gen_output),
                (_, Some(delegated)) => json!(gen_output + delegated),
                _ => Value::Null,
            },
            "note": "input sums generation, Jev, and delegate input tokens, the delegate's cache reads and writes included; cache is the delegate's cache reads when a delegation ran, and generation's reported cached input otherwise",
        },
        "cost": {
            "amount_usd": total,
            "provenance": if total.is_null() { "unknown" } else { "mixed" },
            "covers": "generation (provider_reported), jev (price_estimate), and delegate (cli_list_price or cli_reported for Claude Code, price_estimate for Codex); each is under components",
        },
        "calls": {
            "generation": generations.len(),
            "decisions": decisions.len(),
            "delegates": delegations,
            "failed": failed_generations + decisions_failed + delegate_failed,
            "retries": retries,
        },
        "components": components,
    })
}

/// The delegate component, summed over every `delegate` call.
#[derive(Default)]
struct DelegateUsage<'a> {
    calls: Vec<&'a atif::document::Call>,
    turns: Option<u64>,
    api_calls: Option<u64>,
    input: Option<u64>,
    cache_read: Option<u64>,
    cache_creation: Option<u64>,
    output: Option<u64>,
    cost_usd: Option<f64>,
    per_call: Vec<u64>,
}

impl DelegateUsage<'_> {
    fn total_input(&self) -> Option<u64> {
        Some(self.input? + self.cache_read? + self.cache_creation?)
    }

    fn record(&self) -> Value {
        let extra = |key: &str| -> Value {
            self.calls
                .first()
                .and_then(|call| call.extra.get(key))
                .cloned()
                .unwrap_or(Value::Null)
        };
        let provenance = if self.cost_usd.is_some() && !self.calls.is_empty() {
            extra("cost_provenance")
        } else if self.calls.is_empty() {
            json!("none")
        } else {
            json!("unknown")
        };
        json!({
            "agent": extra("capability"),
            "model": extra("model"),
            "credential": extra("credential"),
            "delegations": self.calls.len(),
            "turns": self.turns,
            "api_calls": self.api_calls,
            "input_tokens": self.input,
            "cache_read_input_tokens": self.cache_read,
            "cache_creation_input_tokens": self.cache_creation,
            "total_input_tokens": self.total_input(),
            "output_tokens": self.output,
            "input_tokens_per_call": self.per_call,
            "max_input_tokens_per_call": self.per_call.iter().max(),
            "cost_usd": if self.calls.is_empty() { json!(0.0) } else { json!(self.cost_usd) },
            "cost_provenance": provenance,
            "cost_note": extra("cost_note"),
        })
    }
}

fn delegate_usage(steps: &[Step]) -> DelegateUsage<'_> {
    let calls: Vec<&atif::document::Call> = steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .filter(|call| {
            call.name == "delegate"
                && call.extra.get("schema").and_then(Value::as_str) == Some(delegate::CALL_SCHEMA)
        })
        .collect();
    let sum = |read: &dyn Fn(&atif::document::Call) -> Option<u64>| -> Option<u64> {
        calls.iter().map(|call| read(call)).sum()
    };
    let usage = |key: &'static str| {
        move |call: &atif::document::Call| call.extra.get("usage")?.get(key)?.as_u64()
    };
    DelegateUsage {
        turns: sum(&|call| call.extra.get("num_turns")?.as_u64()),
        api_calls: sum(&|call| call.extra.get("api_calls")?.as_u64()),
        input: sum(&usage("input_tokens")),
        cache_read: sum(&usage("cache_read_input_tokens")),
        cache_creation: sum(&usage("cache_creation_input_tokens")),
        output: sum(&usage("output_tokens")),
        cost_usd: calls
            .iter()
            .map(|call| call.extra.get("total_cost_usd")?.as_f64())
            .sum(),
        per_call: calls
            .iter()
            .filter_map(|call| call.extra.get("input_tokens_per_call")?.as_array().cloned())
            .flatten()
            .filter_map(|value| value.as_u64())
            .collect(),
        calls,
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn an_episode_with_no_generation_has_a_known_generation_cost_of_zero() {
        let usage = super::usage(&[], false);
        assert_eq!(usage["components"]["generation"]["cost_usd"], 0.0);
        assert_eq!(usage["cost"]["amount_usd"], 0.0);
    }

    use atif::document::Decision;

    use super::*;
    use crate::delegate::tests::{FakeExecutor, report};
    use crate::delegate::{Briefing, BriefingInputs, Delegation, Reason, Status};

    fn generation(input: u64, output: u64, microusd: u64) -> Step {
        let mut step = Step::said(Source::Agent, "").noting("cost_microusd", json!(microusd));
        step.tokens = Some((input, output));
        step
    }

    fn jev(input_tokens: u64) -> Step {
        let decision = Decision {
            id: "jev-1".to_string(),
            name: "jev_step".to_string(),
            door: credentials::JEV_BASE_URL.to_string(),
            model: credentials::JEV_MODEL.to_string(),
            request: json!({}),
            answers: json!({}),
            route: None,
            error: None,
            attempts: Vec::new(),
            review: None,
            milliseconds: 5,
        };
        Step::called(decision.call()).noting("jev_usage", json!({ "input_tokens": input_tokens }))
    }

    fn delegation(status: Status) -> Step {
        let executor = FakeExecutor {
            reports: vec![],
            sent: vec![],
        };
        let inputs = BriefingInputs {
            instruction: "Fix the parser.".to_string(),
            requirements: vec![],
            files: vec![],
            spans: vec![],
            commands: vec![],
            last_output: None,
            conclusion: String::new(),
            directions: "Go.".to_string(),
        };
        let briefing = Briefing::build(&inputs, 1_000);
        delegate::record(
            &executor,
            &briefing,
            &Delegation {
                mode: Mode::Always,
                reason: &Reason::Always,
                isolation: "none",
            },
            &report(status),
            1,
        )
    }

    #[test]
    fn the_total_covers_generation_jev_and_the_delegate() {
        let steps = [
            generation(1_000, 100, 20_000),
            jev(1_000_000),
            delegation(Status::Answered),
        ];
        let usage = usage(&steps, true);
        // $0.02 generation + $0.042 Jev + $0.25 delegate.
        let total = usage["cost"]["amount_usd"].as_f64().unwrap();
        assert!((total - 0.312).abs() < 1e-9, "{total}");
        assert_eq!(usage["calls"]["delegates"], 1);
        assert_eq!(usage["calls"]["generation"], 1);
        assert_eq!(usage["calls"]["decisions"], 1);
        let delegate = &usage["components"]["delegate"];
        assert_eq!(delegate["cost_usd"], 0.25);
        assert_eq!(delegate["cost_provenance"], "cli_list_price");
        assert_eq!(delegate["turns"], 3);
        assert_eq!(delegate["api_calls"], 2);
        assert_eq!(delegate["total_input_tokens"], 38_508);
        assert_eq!(delegate["max_input_tokens_per_call"], 19_505);
        assert_eq!(usage["tokens"]["input"], 1_000 + 1_000_000 + 38_508);
        assert_eq!(usage["tokens"]["cache"], 29_000);
        assert_eq!(usage["tokens"]["output"], 100 + 110);
    }

    #[test]
    fn an_unreported_delegate_cost_is_null_never_zero() {
        let steps = [generation(1_000, 100, 20_000), delegation(Status::TimedOut)];
        let usage = usage(&steps, true);
        assert_eq!(usage["cost"]["amount_usd"], Value::Null);
        assert_eq!(usage["cost"]["provenance"], "unknown");
        let delegate = &usage["components"]["delegate"];
        assert_eq!(delegate["cost_usd"], Value::Null);
        assert_eq!(delegate["cost_provenance"], "unknown");
        assert_eq!(delegate["turns"], Value::Null);
        assert_eq!(usage["tokens"]["input"], Value::Null);
        assert_eq!(usage["calls"]["failed"], 1);
    }

    #[test]
    fn delegation_off_leaves_the_component_out() {
        let recorded = usage(&[generation(10, 1, 5), jev(100)], false);
        assert!(recorded["components"].get("delegate").is_none());
        assert_eq!(recorded["calls"]["delegates"], 0);
        assert!(recorded["cost"]["amount_usd"].as_f64().is_some());
        // With delegation on but never escalated, the delegate cost is a
        // true zero.
        let recorded = usage(&[generation(10, 1, 5)], true);
        assert_eq!(recorded["components"]["delegate"]["delegations"], 0);
        assert_eq!(recorded["components"]["delegate"]["cost_usd"], 0.0);
    }

    #[test]
    fn claude_versions_parse() {
        assert_eq!(parse_version("2.1.280 (Claude Code)"), Some((2, 1, 280)));
        assert!(parse_version("2.1.278 (Claude Code)").unwrap() < CLAUDE_MIN);
        assert_eq!(parse_version("claude"), None);
    }
}
