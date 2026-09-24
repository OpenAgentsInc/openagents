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
//! | `CODER_ONE_DELEGATE_EFFORT` | The delegate's reasoning effort (`low`, `medium`, `high`, …): Claude Code's `--effort` or Codex's `model_reasoning_effort`; each CLI's default when unset. |
//! | `CODER_ONE_DELEGATE_TIMEOUT` | Seconds the delegate may run; 600 when unset. |
//! | `CODER_ONE_EXPLORE_STEPS` | The explore phase's step bound; 8 when unset. |
//! | `CODER_ONE_BRIEFING_CAP` | The briefing's length cap in characters; 12,000 when unset. |
//! | `CODER_ONE_CLAUDE_BIN` | The `claude` binary; the first on `PATH` when unset. |
//! | `CODER_ONE_CODEX_BIN` | The `codex` binary; the first on `PATH` when unset. |
//! | `CLAUDE_CODE_OAUTH_TOKEN` | The Claude Code delegate's subscription token; or `ANTHROPIC_API_KEY`. |
//! | `CODEX_HOME` | Where the Codex delegate finds `auth.json`; `~/.codex` when unset. |
//! | `CODER_ONE_CODEX_LOGIN` | `take` makes `episode run` read the Codex login into memory for Microluna and remove the file before any command runs, so the model's commands can't read it; the Codex CLI can't run then. The file stays when unset. |
//! | `CODER_ONE_DEEP` | `on` runs deep Jev mode: a parallel survey before the first step, a readiness question each step, and repeated-command hints. |
//! | `CODER_ONE_PROBES` | `on`, with deep mode, runs a battery of read-only probes (listing, git state, README, tests, versions) and lets Jev pick the outputs that go into the survey and the briefing. |
//! | `CODER_ONE_PROBE_V2` | `v3` adds directions that test every changed code path. `on`, with probes and deep mode: a Jev-gated setup pack, git probes in named repositories, whole edit targets, a 40-file survey, and batch-mode directions. |
//! | `CODER_ONE_POLICY` | A policy manifest: a path, or the JSON itself. The switches above then override it. |
//! | `CODER_ONE_EXECUTOR_VERSION` | The delegate CLI version the harness installed; the doctor refuses another. |
//! | `CLAUDE_CODE_PROMPT_CACHE_TTL` | The Claude Code delegate's prompt-cache TTL, `5m` or `1h`. |
//! | `CODER_ONE_EPISODE_DEADLINE` | Seconds for the whole episode, one monotonic deadline. Every dispatch, Jev request, retry, wait, setup command, probe, and command is granted at most what is left after the reserve. No deadline when unset. |
//! | `CODER_ONE_SPEND_SOFT_USD` | A soft spend bound in dollars, checked before each dispatch starts. A running dispatch can pass it. |
//!
//! `coder_one::policy` resolves all of these once, before anything runs,
//! into the policy manifest the episode reads its configuration from. The
//! bundle's manifest records the resolved manifest, its digest, and each
//! override under `policy`.
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
use crate::deadline::Deadline;
use crate::delegate::{self, Agent, Credential, Delegated, Explorer, Mode, Plan};
use crate::generate::Door;
use crate::judge::JevJudge;
use crate::policy::{ExecutorHost, Manifest, Resolution};
use crate::record::{Finish, Implementation, Outcome as RecordOutcome, Recorder, Start};
use crate::shell::Checkout;
use crate::state::{Environment, Issue, State};
use crate::{Bounds, Ended, run};

/// The episode's first system step when the Coder One loop sends no
/// prompt: the policy delegates with no explore steps.
pub const NO_LOOP_PROMPT: &str = "No Coder One loop step runs under this policy \
(control.explore_steps is 0), so the loop sends no system prompt. Each executor \
session carries its own instructions, recorded with that session.";

/// The contract this binary implements.
pub const CONTRACT: &str = "openagents.coder.episode.v1";

/// The episode's durable log, relative to the output directory: every
/// trajectory step and component invocation, synced as it happens.
pub const INVOCATION_LOG: &str = "episode.atif.jsonl";

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

/// Everything the episode reads from its environment, resolved once: the
/// credentials and what the host found, beside the resolved policy.
struct Settings {
    bearer: Option<Secret>,
    door_url: String,
    jev_key: Option<Secret>,
    delegate_bin: Option<PathBuf>,
    credential: Credential,
    resolution: Resolution,
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
        let resolution = Resolution::resolve(env, model)?;
        let agent = resolution.manifest.policy.executor.agent.agent();
        let (delegate_bin, credential) = delegate::resolve(agent, |name| env(name));
        Ok(Settings {
            bearer: credentials::bearer(|name| env(name), &dir)
                .ok()
                .map(|found| found.secret),
            door_url: env("OPENAGENTS_DOOR_URL")
                .unwrap_or_else(|| credentials::GENERATION_BASE_URL.to_string()),
            jev_key: credentials::jev_key(|name| env(name), &dir)
                .ok()
                .map(|found| found.secret),
            delegate_bin,
            credential,
            resolution,
        })
    }

    /// The resolved policy.
    fn policy(&self) -> &Manifest {
        &self.resolution.manifest
    }
}

/// `episode doctor`: every requirement checked without spending inference.
pub async fn doctor(contract: &str) -> Result<(), String> {
    if contract != CONTRACT {
        return Err(format!("this binary implements {CONTRACT}, not {contract}"));
    }
    let settings = Settings::from_env(None)?;
    let policy = settings.policy();
    let mut problems = Vec::new();
    println!("version: {}", version());
    println!("contract: {CONTRACT}");
    println!(
        "policy: {} {} ({}, {} overrides)",
        policy.name.as_deref().unwrap_or("unnamed"),
        settings.resolution.digest(),
        settings.resolution.source,
        settings.resolution.overrides.len()
    );

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

    if policy.jev() {
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

    if policy.mode() == Mode::Off {
        println!("delegate: off (CODER_ONE_DELEGATE)");
    } else {
        let executor = &policy.policy.executor;
        println!(
            "delegate: {} to {} ({}), explore {} steps, deadline {}s",
            policy.mode().word(),
            executor.agent.agent().word(),
            executor.model,
            policy.policy.control.explore_steps,
            executor.deadline_sec
        );
        problems.extend(check_delegate(&settings).await);
        // A route or a handoff can dispatch to another CLI: check each one
        // the manifest names beyond its own executor.
        let own = executor.agent.agent();
        let mut seen = vec![own];
        for tier in crate::compose::tiers(policy) {
            let Ok(agent) = Agent::parse(&tier.agent) else {
                continue;
            };
            if seen.contains(&agent) {
                continue;
            }
            seen.push(agent);
            if agent == Agent::Microluna {
                problems.extend(crate::micro::check_login());
                continue;
            }
            let env = |name: &str| {
                std::env::var(name)
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            };
            let (binary, credential) = delegate::resolve(agent, env);
            problems.extend(
                check_cli(
                    agent,
                    binary.as_deref(),
                    credential,
                    tier.version.as_deref(),
                )
                .await,
            );
        }
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
    if settings.policy().policy.executor.agent == crate::policy::AgentName::Microluna {
        return crate::micro::check_login();
    }
    check_cli(
        settings.policy().policy.executor.agent.agent(),
        settings.delegate_bin.as_deref(),
        settings.credential,
        settings.policy().policy.executor.version.as_deref(),
    )
    .await
}

/// Checks one CLI: its `--version` runs, matches `pinned`, and is new
/// enough, and a credential is present, without any inference.
async fn check_cli(
    agent: Agent,
    binary: Option<&Path>,
    credential: Credential,
    pinned: Option<&str>,
) -> Vec<String> {
    let mut problems = Vec::new();
    match binary {
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
                Agent::Codex | Agent::Microluna => text
                    .split_whitespace()
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join(" "),
                Agent::ClaudeCode => text.clone(),
            };
            let installed = version_text.split_whitespace().next().unwrap_or_default();
            if ended.ending.success()
                && let Some(pinned) = pinned
                && installed != pinned
            {
                problems.push(format!(
                    "{} {installed} is installed, but the policy pins {pinned}",
                    agent.word()
                ));
            }
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
    match (credential, agent) {
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

/// The arguments `episode run` takes.
pub struct RunArgs {
    pub instruction_file: PathBuf,
    pub output_dir: PathBuf,
    pub contract: String,
    pub model: Option<String>,
}

/// `episode run`: one headless episode in the current directory. Returns
/// the process exit code: 0 when the agent or its delegate finished, 3 when
/// the step limit ran out, 4 when generation failed, 5 when the delegate
/// did not answer, and 6 when a delegate session hit a usage or rate
/// limit.
pub async fn run_episode(args: RunArgs) -> Result<i32, String> {
    if args.contract != CONTRACT {
        return Err(format!(
            "this binary implements {CONTRACT}, not {}",
            args.contract
        ));
    }
    // Before any command runs: the model's commands run as this process's
    // user in a task container, so this process's `/proc` entries (its
    // environment holds the door and Jev keys) are closed to them, and a
    // Codex login the adapter placed for Microluna leaves the disk.
    let _ = microluna::codex::protect_process();
    crate::micro::take_login()?;
    let settings = Settings::from_env(args.model.as_deref())?;
    let policy = settings.policy().clone();
    // One monotonic deadline for the whole episode, from here on.
    let ceilings = &policy.protected.ceilings;
    let deadline = Deadline::new(
        ceilings.episode_deadline_sec.map(Duration::from_secs),
        Duration::from_secs(ceilings.reserve_sec),
    );
    let bearer = settings
        .bearer
        .clone()
        .ok_or("OPENAGENTS_API_KEY is not set")?;
    let jev_client = if policy.jev() {
        let key = settings
            .jev_key
            .as_ref()
            .ok_or("TYPESAFE_API_KEY is not set and CODER_ONE_JEV is not off")?;
        Some(credentials::jev_client(key)?)
    } else {
        None
    };
    // The monitor asks Jev through its own handle, beside the judge's.
    let monitor_jev = jev_client.clone();
    if let Some(handoff) = policy.policy.control.handoff.as_ref().filter(|handoff| {
        matches!(
            handoff.pattern,
            crate::handoff::Pattern::Steer | crate::handoff::Pattern::Race
        )
    }) {
        return Err(format!(
            "control.handoff {} runs on mini-tasks in this build (coder-one handoff run); a Terminal-Bench episode runs single, escalate, or planner-worker",
            handoff.pattern.word()
        ));
    }
    let instruction = std::fs::read_to_string(&args.instruction_file)
        .map_err(|error| format!("cannot read {}: {error}", args.instruction_file.display()))?;
    let workdir = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut bundle = Bundle::create(&args.output_dir, &settings, &workdir, deadline.clone())?;

    // Recording starts before setup: every step and invocation is synced to
    // the log as it happens, and the bundle is derived from it.
    let log_path = args.output_dir.join(INVOCATION_LOG);
    let recorder = match atif::Log::create_at(&log_path, &bundle.session) {
        Ok(log) => {
            bundle.log = Some(log_path);
            Recorder::durable(log)
        }
        Err(error) => {
            eprintln!(
                "coder-one: cannot create {}: {error}; recording in memory only",
                log_path.display()
            );
            Recorder::default()
        }
    };
    // The episode's implementation is its resolved policy manifest.
    let episode = recorder.enter(
        Start::new(
            "episode",
            Implementation {
                name: format!("policy {}", policy.name.as_deref().unwrap_or("unnamed")),
                digest: settings.resolution.digest(),
            },
        )
        .named(CONTRACT)
        .reading(&json!({ "instruction": instruction }))
        .with_effects(),
    );
    // The loop's system prompt is recorded only when the loop sends it.
    // Under a delegating policy with no explore steps, as every reference
    // policy is, no loop step runs, and the transcript said otherwise
    // before issue #9591.
    let loop_runs = policy.mode() == Mode::Off || policy.policy.control.explore_steps > 0;
    recorder.push(Step::said(
        Source::System,
        if loop_runs {
            EPISODE_INSTRUCTIONS
        } else {
            NO_LOOP_PROMPT
        },
    ));
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
    let control = &policy.policy.control;
    println!("{} · {CONTRACT}", version());
    println!(
        "workdir {} · lane {} · jev {}{} · {} steps · {}s per command · delegate {}",
        workdir.display(),
        control.lane,
        if policy.jev() {
            credentials::JEV_MODEL
        } else {
            "off"
        },
        if policy.deep() { " (deep)" } else { "" },
        control.max_steps,
        control.command_timeout_sec,
        policy.mode().word()
    );
    println!(
        "policy {} {} ({})",
        policy.name.as_deref().unwrap_or("unnamed"),
        settings.resolution.digest(),
        settings.resolution.source
    );

    let mut judge = policy
        .judge(jev_client, workdir.clone(), &state.issue, recorder.clone())
        .within(deadline.clone());
    judge.survey(&mut state).await;
    bundle.attach(
        "requirements",
        "artifacts/requirements.json",
        judge.requirements.record(),
    );
    let mut judge = Snapshots {
        inner: judge,
        bundle: &bundle,
        recorder: recorder.clone(),
    };
    let mut door = Door::new(
        &settings.door_url,
        bearer,
        &control.lane,
        EPISODE_INSTRUCTIONS,
        Box::new(|delta| {
            use std::io::Write as _;
            print!("{delta}");
            let _ = std::io::stdout().flush();
        }),
        recorder.clone(),
    )?
    .caching_under(&bundle.session.id)
    .within(deadline.clone());
    let mut shell = Checkout {
        workdir: workdir.clone(),
        deadline: Duration::from_secs(control.command_timeout_sec),
        recorder: recorder.clone(),
        commands: 0,
        episode: deadline.clone(),
    };

    let mut composition: Option<Value> = None;
    let (ended, delegated) = if policy.mode() == Mode::Off {
        let session = recorder.enter(
            Start::new(
                "exec.session",
                Implementation::new(
                    "exec.session",
                    "coder-one loop",
                    &json!({ "lane": control.lane, "max_steps": control.max_steps }),
                ),
            )
            .named("coder-one loop")
            .with_effects(),
        );
        let ended = run(
            &mut state,
            "Complete this task.",
            Bounds {
                max_steps: control.max_steps,
            },
            &mut judge,
            &mut door,
            &mut shell,
        )
        .await;
        recorder.end(
            &session,
            Finish::new(RecordOutcome::Completed).summary(json!({ "steps": state.history.len() })),
        );
        (ended, None)
    } else {
        let mut executor = policy.executor(ExecutorHost {
            binary: settings.delegate_bin.clone(),
            credential: settings.credential,
            workdir: workdir.clone(),
            artifacts: args.output_dir.join("artifacts"),
            artifacts_label: "artifacts".to_string(),
            // The task container is the boundary, and the agent often runs
            // as root there, where the CLI refuses to bypass permissions
            // unless told it is in a sandbox.
            env: vec![("IS_SANDBOX".to_string(), "1".to_string())],
        });
        executor.episode = deadline.clone();
        // Each normalized executor event lands in the durable log as it
        // arrives, which is what a live reader follows.
        executor.control.recorder = Some(recorder.clone());
        if let Some(params) = &policy.policy.control.monitor {
            executor.control.monitor = Some(crate::monitor::Setup {
                params: params.clone(),
                jev: monitor_jev.clone().map_or(
                    crate::component::jev::JevMode::Off,
                    crate::component::jev::JevMode::Live,
                ),
                task: instruction.clone(),
                acting: None,
            });
        }
        if let Some(soft) = policy.protected.ceilings.spend_soft_usd {
            executor.gate = Some(spend_gate(&recorder, soft));
        }
        let plan = Plan {
            mode: policy.mode(),
            policy: policy.escalation(),
            max_steps: control.max_steps,
            prompt: "Complete this task.",
            instruction: &instruction,
            directions: policy.policy.brief.directions.text(),
            cap: policy.policy.brief.cap,
            packer: policy.policy.brief.packer,
            pack: policy.policy.brief.pack_params(),
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
        if crate::compose::composes(&policy) {
            // The composition makes each dispatch's executor itself, from
            // the tier it routes, escalates, or repairs to.
            drop(executor);
            let total = deadline.remaining().map(|left| left.as_secs());
            let horizon = policy.policy.control.horizon.clone().unwrap_or_default();
            let mut factory = CliFactory {
                workdir: workdir.clone(),
                artifacts: args.output_dir.join("artifacts"),
                recorder: recorder.clone(),
                episode: deadline.clone(),
                system: policy.policy.executor.system.clone(),
                session: policy.policy.executor.session.clone(),
                microluna: policy.policy.executor.microluna.clone(),
                soft: policy.protected.ceilings.spend_soft_usd,
                command_env: match horizon.long_command_sec {
                    Some(seconds) if horizon.long(total) => vec![(
                        "BASH_MAX_TIMEOUT_MS".to_string(),
                        (seconds * 1_000).to_string(),
                    )],
                    _ => Vec::new(),
                },
            };
            let setup = crate::compose::Setup {
                manifest: &policy,
                instruction: &instruction,
                workdir: &workdir,
                dir: &args.output_dir,
                recorder: &recorder,
                deadline: &deadline,
                jev: monitor_jev.clone().map_or(
                    crate::component::jev::JevMode::Off,
                    crate::component::jev::JevMode::Live,
                ),
                profile: None,
                base: bundle.base.as_deref(),
            };
            let composed = crate::compose::run(
                &setup,
                &mut state,
                &plan,
                &mut judge,
                &mut door,
                &mut shell,
                &mut factory,
                &mut checkpoint,
            )
            .await?;
            composition = Some(composed.record);
            (composed.ended, composed.delegated)
        } else if policy.policy.executor.agent == crate::policy::AgentName::Microluna {
            let executor_policy = &policy.policy.executor;
            let mut micro = crate::micro::Micro::new(
                &executor_policy.model,
                executor_policy.effort.clone(),
                Duration::from_secs(executor_policy.deadline_sec),
                &workdir,
                &args.output_dir.join("artifacts"),
                recorder.clone(),
                0,
                executor_policy.microluna.clone().unwrap_or_default(),
                // The task container is the boundary, as it is for the CLIs.
                microluna::Isolation::TaskContainer,
            );
            micro.episode = deadline.clone();
            delegate::explore_then_delegate(
                &mut state,
                &plan,
                &mut judge,
                &mut door,
                &mut shell,
                &mut micro,
                &recorder,
                &mut checkpoint,
            )
            .await
        } else {
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
        }
    };
    if let Some(record) = composition {
        bundle.attach("composition", crate::compose::FILE, record);
    }
    if let Some(pack) = delegated.as_ref().and_then(|d| d.pack.clone()) {
        let mut record = pack;
        record["schema"] = json!(crate::pack::RECORD_SCHEMA);
        bundle.attach("briefing_pack", "artifacts/briefing-pack.json", record);
    }

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
    // A delegate session the provider throttled did no work worth
    // grading, whatever the rest of the episode did: the harness requeues
    // the trial once the limit resets.
    let limited = crate::limit::from_steps(&recorder.steps());
    let (outcome, code) = match &limited {
        Some(limit) => {
            println!(
                "\n  usage limit ▸ {}; the limit resets {}",
                limit["message"].as_str().unwrap_or("limited"),
                limit["resets_at_iso"]
                    .as_str()
                    .unwrap_or("at an unknown time")
            );
            (crate::limit::OUTCOME, crate::limit::EXIT_CODE)
        }
        None => (outcome, code),
    };
    println!("\n── {outcome} ──");
    recorder.end(
        &episode,
        Finish::new(if code == 0 {
            RecordOutcome::Completed
        } else {
            RecordOutcome::Failed
        })
        .summary(json!({ "outcome": outcome, "exit_code": code, "usage_limit": limited })),
    );
    // The log closes before the last snapshot, so the manifest digests the
    // log as it will stay.
    recorder.finish(atif::log::ENDED);
    bundle.write(
        &state,
        &recorder.steps(),
        outcome,
        Some(&ended),
        delegated.as_ref(),
    )?;
    Ok(code)
}

/// The soft spend bound's check before each dispatch. A running dispatch
/// can pass it: no adapter reserves a known maximum.
fn spend_gate(recorder: &Recorder, soft: f64) -> Box<dyn Fn() -> Option<String>> {
    let spent = recorder.clone();
    Box::new(move || {
        let known = usage(&spent.steps(), true)["cost"]["lower_bound_usd"]
            .as_f64()
            .unwrap_or(0.0);
        (known >= soft).then(|| {
            format!("the soft spend bound of ${soft:.4} is reached: ${known:.4} is known spent")
        })
    })
}

/// Makes each composed dispatch's CLI executor from its tier, with the
/// manifest's system prompt, session rules, and spend bound.
struct CliFactory {
    workdir: PathBuf,
    artifacts: PathBuf,
    recorder: Recorder,
    episode: Deadline,
    system: Option<crate::system::Policy>,
    session: Option<crate::policy::SessionPolicy>,
    microluna: Option<crate::micro::Policy>,
    soft: Option<f64>,
    /// Variables a long task sets for Claude Code, such as its shell
    /// command cap.
    command_env: Vec<(String, String)>,
}

impl crate::compose::Factory for CliFactory {
    fn make(
        &mut self,
        tier: &crate::handoff::Tier,
        deadline: Duration,
        runs: u32,
    ) -> Result<crate::compose::Exec, String> {
        let agent = Agent::parse(&tier.agent)?;
        if agent == Agent::Microluna {
            let mut micro = crate::micro::Micro::new(
                &tier.model,
                tier.effort.clone(),
                deadline,
                &self.workdir,
                &self.artifacts,
                self.recorder.clone(),
                runs,
                self.microluna.clone().unwrap_or_default(),
                // The task container is the boundary, as it is for the CLIs.
                microluna::Isolation::TaskContainer,
            );
            micro.episode = self.episode.clone();
            return Ok(crate::compose::Exec::Micro(Box::new(micro)));
        }
        let env = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let (binary, credential) = delegate::resolve(agent, env);
        // The task container is the boundary, and the agent often runs as
        // root there, where the CLI refuses to bypass permissions unless
        // told it is in a sandbox.
        let mut child_env = vec![("IS_SANDBOX".to_string(), "1".to_string())];
        if agent == Agent::ClaudeCode {
            child_env.extend(self.command_env.iter().cloned());
        }
        let system = self
            .system
            .clone()
            .filter(|system| system.validate(agent).is_empty())
            .map(|system| crate::system::Variant::new(agent, system));
        let claude = agent == Agent::ClaudeCode;
        Ok(crate::compose::Exec::Cli(Box::new(delegate::Cli {
            agent,
            binary,
            model: tier.model.clone(),
            deadline,
            workdir: self.workdir.clone(),
            artifacts: self.artifacts.clone(),
            artifacts_label: "artifacts".to_string(),
            env: child_env,
            credential,
            effort: tier.effort.clone(),
            tools: tier.tools.clone().filter(|_| claude),
            prompt_cache_ttl: tier.prompt_cache_ttl.clone().filter(|_| claude),
            system,
            episode: self.episode.clone(),
            gate: self.soft.map(|soft| spend_gate(&self.recorder, soft)),
            granted: None,
            runs,
            control: delegate::Control {
                recorder: Some(self.recorder.clone()),
                controls: self
                    .session
                    .as_ref()
                    .filter(|session| {
                        let (demonstrated, _) = crate::adapter::capabilities(agent);
                        session.uses().iter().all(|c| demonstrated.has(*c))
                    })
                    .map(crate::policy::SessionPolicy::controls),
                last: None,
                monitor: None,
            },
        })))
    }
}

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
    /// The durable log the trajectory is derived from, when it opened.
    log: Option<PathBuf>,
    /// How many snapshots have been published. Each file is replaced
    /// atomically, and the manifest, written last, names the generation
    /// and every file's digest.
    generation: std::cell::Cell<u64>,
    /// The episode deadline, recorded with every write.
    deadline: Deadline,
    /// Component records every write puts beside the manifest: key, then
    /// path and value.
    records: std::cell::RefCell<std::collections::BTreeMap<String, (String, Value)>>,
}

impl Bundle {
    fn create(
        dir: &Path,
        settings: &Settings,
        workdir: &Path,
        deadline: Deadline,
    ) -> Result<Self, String> {
        for sub in ["artifacts", "verification", "evaluation"] {
            std::fs::create_dir_all(dir.join(sub))
                .map_err(|error| format!("cannot create {}: {error}", dir.join(sub).display()))?;
        }
        let started = atif::document::now_ms();
        let id = format!("coder-one-{started}");
        let policy = settings.policy();
        let executor = &policy.policy.executor;
        let mut session = Session::opening(
            &id,
            &policy.policy.control.lane,
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
            "policy": settings.resolution.record(),
            "delegate": if policy.mode() == Mode::Off {
                json!({ "mode": "off" })
            } else {
                json!({
                    "mode": policy.mode().word(),
                    "agent": executor.agent.agent().word(),
                    "model": executor.model,
                    "version": executor.version,
                    "effort": executor.effort,
                    "tools": executor.tools,
                    "prompt_cache_ttl": executor.prompt_cache_ttl,
                    "system": executor.system,
                    "executor_path": settings.delegate_bin.as_ref().map(|path| path.to_string_lossy()),
                    "credential": settings.credential.word(),
                    "deadline_sec": executor.deadline_sec,
                    "briefing_cap": policy.policy.brief.cap,
                    "directions": policy.policy.brief.directions,
                    "policy": policy.escalation().record(),
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
                    "lane_requested": policy.policy.control.lane,
                },
                "jev": if policy.jev() {
                    json!({ "url": credentials::JEV_BASE_URL, "model": credentials::JEV_MODEL })
                } else {
                    json!({ "enabled": false })
                },
            },
            "jev_mode": policy.policy.jev.mode,
            "prompt_layout": "cache-stable-prefix",
            "bounds": {
                "max_steps": policy.policy.control.max_steps,
                "command_timeout_sec": policy.policy.control.command_timeout_sec,
                "episode_deadline_sec": policy.protected.ceilings.episode_deadline_sec,
                "reserve_sec": policy.protected.ceilings.reserve_sec,
            },
            "spend": {
                "soft_usd": policy.protected.ceilings.spend_soft_usd,
                "hard_usd": Value::Null,
                "enforcement": "soft: checked before each dispatch starts; a running dispatch can pass it",
                "hard_note": "no hard dollar cap: no executor adapter reserves a known maximum charge before a model call",
            },
        });
        Ok(Bundle {
            dir: dir.to_path_buf(),
            started,
            header,
            workdir: workdir.to_path_buf(),
            base,
            session,
            log: None,
            generation: std::cell::Cell::new(0),
            deadline,
            records: std::cell::RefCell::default(),
        })
    }

    /// Attaches a component record that every later write puts at
    /// `relative` and names in the manifest under `key`.
    fn attach(&self, key: &str, relative: &str, value: Value) {
        self.records
            .borrow_mut()
            .insert(key.to_string(), (relative.to_string(), value));
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
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        // The trajectory is derived from the durable log when there is one,
        // so the bundle can never say more than the log holds.
        let recording = self
            .log
            .as_deref()
            .and_then(|path| atif::log::read(path).ok());
        let steps = recording
            .as_ref()
            .map_or(steps, |recording| recording.steps.as_slice());
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
        trajectory["extra"]["snapshot_generation"] = json!(generation);

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
        if let Some(collection) = diff {
            files.insert(
                "diff".to_string(),
                self.put("artifacts/diff.patch", collection.patch.as_bytes())?,
            );
            let text = serde_json::to_string_pretty(&collection.record())
                .map_err(|error| error.to_string())?;
            files.insert(
                "collection".to_string(),
                self.put("artifacts/collection.json", text.as_bytes())?,
            );
        }
        // Component records the episode attached, such as the requirement
        // map and the briefing pack.
        for (key, (relative, value)) in self.records.borrow().iter() {
            let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
            files.insert(key.clone(), self.put(relative, text.as_bytes())?);
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

        // The log grows while the episode runs, so only a closed log gets a
        // digest; before that the manifest names it and its size.
        if let (Some(path), Some(recording)) = (&self.log, &recording) {
            let relative = path
                .strip_prefix(&self.dir)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            let bytes = std::fs::read(path).unwrap_or_default();
            files.insert(
                "invocation_log".to_string(),
                json!({
                    "path": relative,
                    "bytes": bytes.len(),
                    "sha256": recording.ended().then(|| hex(&Sha256::digest(&bytes))),
                    "ended": recording.ended(),
                    "faults": recording.faults.len(),
                }),
            );
        }

        let mut manifest = self.header.clone();
        manifest["generation"] = json!(generation);
        manifest["started_at"] = json!(atif::document::iso(self.started));
        manifest["updated_at"] = json!(atif::document::iso(now));
        manifest["outcome"] = json!(outcome);
        if let Some(limit) = crate::limit::from_steps(steps) {
            manifest[crate::limit::KEY] = limit;
        }
        manifest["steps"] = json!(state.history.len());
        manifest["workdir"] = json!(self.workdir.to_string_lossy());
        manifest["git_base"] = json!(self.base);
        manifest["doors"]["generation"]["models_served"] = trajectory_models(steps);
        manifest["files"] = Value::Object(files);
        manifest["deadline"] = self.deadline.record();
        manifest["spend"]["known_usd"] = usage["cost"]["lower_bound_usd"].clone();
        manifest["spend"]["unknown_calls"] = usage["cost"]["unknown_calls"].clone();
        if let Some(soft) = manifest["spend"]["soft_usd"].as_f64() {
            manifest["spend"]["exceeded"] =
                json!(usage["cost"]["lower_bound_usd"].as_f64().unwrap_or(0.0) >= soft);
        }
        manifest["verification"] = match self.records.borrow().get("composition") {
            Some((_, record)) => json!({
                "note": "The composition's checks observe requirements; the task's verifier is still the only grader.",
                "checks": record["checks"],
                "support": record["support"],
                "repair": record["repair"],
            }),
            None => json!({
                "note": "Coder One runs no independent verification of its own; the task's verifier is the grader.",
            }),
        };
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

    /// Replaces one file atomically and returns its reference: path, bytes,
    /// and sha256. A reader sees the previous snapshot's file or this one,
    /// never part of either.
    fn put(&self, relative: &str, bytes: &[u8]) -> Result<Value, String> {
        crate::record::write_atomic(&self.dir.join(relative), bytes)?;
        Ok(json!({
            "path": relative,
            "bytes": bytes.len(),
            "sha256": hex(&Sha256::digest(bytes)),
        }))
    }

    /// The working tree's change against the base commit, untracked files
    /// included within bounds, when the workdir is a Git work tree. It
    /// reads only: the index is never written, and the collection names
    /// the workspace revision before and after it read.
    fn diff(&self) -> Option<crate::collect::Collection> {
        let base = self.base.as_deref()?;
        crate::collect::collect(&self.workdir, base, crate::collect::Limits::default())
    }
}

/// The usage record, derived from the trajectory so both say the same
/// thing. Unreported values are null, never zero. `delegating` adds the
/// delegate component, which a run with delegation off leaves out.
///
/// Every call is one of three charges: `priced`, with a cost; `zero`, a
/// known zero, such as a request never sent or refused before any work;
/// or `unknown`, a call that may have done billed work nobody reported,
/// such as a timed-out request or a session cut off by its deadline. A
/// component's cost is known only when none of its calls is unknown;
/// `cost_lower_bound_usd` sums what is known either way. `ledger` lists
/// every call with its charge and provenance.
pub fn usage(steps: &[Step], delegating: bool) -> Value {
    let generations: Vec<&Step> = steps
        .iter()
        .filter(|step| step.source == Source::Agent && step.call.is_none() && step.tokens.is_some())
        .collect();
    let failed_generation_steps: Vec<&Step> = steps
        .iter()
        .filter(|step| {
            step.source == Source::System && step.message.starts_with("generation failed")
        })
        .collect();
    let failed_generations = failed_generation_steps.len();
    let retries: u64 = generations
        .iter()
        .filter_map(|step| step.extensions.get("attempts").and_then(Value::as_u64))
        .map(|attempts| attempts.saturating_sub(1))
        .sum();

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
    // A failed generation that may have done billed work leaves the cost
    // unknown; one refused before any work is a known zero.
    let gen_failed_unknown = failed_generation_steps
        .iter()
        .filter(|step| step.extensions.get("charge").and_then(Value::as_str) != Some("zero"))
        .count();
    // No generation at all is a known cost of zero, as in a Jev-brief
    // episode that delegates before the explorer runs; one unreported call
    // leaves the generation cost unknown.
    let priced = costs.iter().all(Option::is_some) && gen_failed_unknown == 0;
    let gen_cost_known_part = costs.iter().flatten().sum::<u64>() as f64 / 1_000_000.0;

    let jev = jev_usage(steps);
    let jev_cost = (jev.unknown == 0).then_some(jev.priced_usd);

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
    let delegations = delegate.dispatches.len();
    let delegate_failed = delegate
        .dispatches
        .iter()
        .filter(|dispatch| dispatch.outcome != atif::document::Outcome::Completed)
        .count();

    let gen_cost_known = priced.then_some(gen_cost_known_part);
    let delegate_cost_known = if delegations == 0 {
        Some(0.0)
    } else {
        delegate.cost_usd()
    };
    let total = match (gen_cost_known, jev_cost, delegate_cost_known) {
        (Some(generation), Some(jev), Some(delegated)) => json!(generation + jev + delegated),
        _ => Value::Null,
    };
    let lower_bound = gen_cost_known_part + jev.priced_usd + delegate.lower_bound_usd();
    let unknown_calls = gen_failed_unknown
        + costs.iter().filter(|cost| cost.is_none()).count()
        + jev.unknown
        + delegate.unknown();
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
            "cost_lower_bound_usd": gen_cost_known_part,
            "cost_provenance": if priced { "provider_reported" } else { "unknown" },
            "unpriced_calls": costs.iter().filter(|cost| cost.is_none()).count(),
            "failed_calls_unknown_charge": gen_failed_unknown,
        },
        "jev": jev.record(),
    });
    if delegating || delegations > 0 {
        components["delegate"] = delegate.record();
    }

    let mut ledger: Vec<Value> = Vec::new();
    let mut generation_number = 0;
    for step in steps {
        if step.source == Source::Agent && step.call.is_none() && step.tokens.is_some() {
            generation_number += 1;
            let cost = step
                .extensions
                .get("cost_microusd")
                .and_then(Value::as_u64)
                .map(|micro| micro as f64 / 1_000_000.0);
            ledger.push(json!({
                "component": "generation",
                "id": format!("generation-{generation_number}"),
                "name": "generate",
                "model": step.model,
                "charge": if cost.is_some() { "priced" } else { "unknown" },
                "cost_usd": cost,
                "provenance": if cost.is_some() { "provider_reported" } else { "unknown" },
                "basis": if cost.is_some() { "the door reported cost_microusd" } else { "the door reported no cost" },
                "milliseconds": step.milliseconds,
                "input_tokens": step.tokens.map(|t| t.0),
                "output_tokens": step.tokens.map(|t| t.1),
            }));
        } else if step.source == Source::System && step.message.starts_with("generation failed") {
            generation_number += 1;
            let zero = step.extensions.get("charge").and_then(Value::as_str) == Some("zero");
            ledger.push(json!({
                "component": "generation",
                "id": format!("generation-{generation_number}"),
                "name": "generate",
                "model": Value::Null,
                "charge": if zero { "zero" } else { "unknown" },
                "cost_usd": if zero { json!(0.0) } else { Value::Null },
                "provenance": if zero { "none" } else { "unknown" },
                "basis": if zero { "refused or never sent: no billed work" } else { "the request failed; the door may have billed it" },
                "milliseconds": step.milliseconds,
            }));
        } else if let Some(call) = step.call.as_ref().filter(|call| call.is_decision()) {
            let charge = JevCharge::of(step, call);
            ledger.push(json!({
                "component": "jev",
                "id": call.id,
                "name": call.name,
                "model": credentials::JEV_MODEL,
                "outcome": call.outcome,
                "charge": charge.charge,
                "cost_usd": match charge.charge {
                    "priced" => json!(charge.usd()),
                    "zero" => json!(0.0),
                    _ => Value::Null,
                },
                "provenance": match charge.charge {
                    "priced" => "price_estimate",
                    "zero" => "none",
                    _ => "unknown",
                },
                "basis": charge.basis,
                "milliseconds": call.milliseconds,
                "input_tokens": charge.input_tokens,
            }));
        } else if let Some(dispatch) = step.call.as_ref().and_then(|call| Dispatch::of(step, call))
        {
            ledger.push(dispatch.record());
        }
    }

    json!({
        "tokens": {
            "input": match delegate_input {
                Some(delegated) => json!(gen_input + jev.input_tokens + delegated),
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
            "lower_bound_usd": lower_bound,
            "unknown_calls": unknown_calls,
            "provenance": if total.is_null() { "unknown" } else { "mixed" },
            "covers": "generation (provider_reported), jev (price_estimate), and delegate (cli_list_price or cli_reported for Claude Code, price_estimate for Codex); each is under components, and ledger lists every call",
        },
        "calls": {
            "generation": generations.len(),
            "decisions": jev.requests,
            "decisions_skipped": jev.skipped,
            "delegates": delegations,
            "failed": failed_generations + jev.failed + delegate_failed,
            "retries": retries,
        },
        "components": components,
        "ledger": ledger,
    })
}

/// What one Jev call cost.
struct JevCharge {
    charge: &'static str,
    basis: String,
    input_tokens: Option<u64>,
}

impl JevCharge {
    /// Reads the call's `jev_usage`. A record written before charges
    /// were recorded is priced when it carries input tokens; a failed call
    /// with no usage at all is unknown, never zero.
    fn of(step: &Step, call: &atif::document::Call) -> Self {
        let usage = step.extensions.get("jev_usage");
        let input_tokens = usage
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(Value::as_u64);
        let basis = usage
            .and_then(|usage| usage.get("basis"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let charge = match usage
            .and_then(|usage| usage.get("charge"))
            .and_then(Value::as_str)
        {
            Some("priced") if input_tokens.is_some() => "priced",
            Some("zero") => "zero",
            Some(_) => "unknown",
            None if input_tokens.is_some() => "priced",
            None => "unknown",
        };
        let basis = basis.unwrap_or_else(|| {
            match (charge, call.outcome) {
                ("priced", _) => "the response reported its input tokens",
                (_, atif::document::Outcome::Failed) => "the request failed and recorded no usage",
                _ => "the call recorded no usage",
            }
            .to_string()
        });
        Self {
            charge,
            basis,
            input_tokens,
        }
    }

    fn usd(&self) -> f64 {
        self.input_tokens.unwrap_or(0) as f64 * JEV_USD_PER_MILLION_INPUT / 1_000_000.0
    }

    fn skipped(&self) -> bool {
        self.charge == "zero" && self.basis.starts_with("not sent")
    }
}

/// The Jev component, summed over every decision call.
#[derive(Default)]
struct JevUsage {
    requests: usize,
    priced: usize,
    zero: usize,
    unknown: usize,
    skipped: usize,
    failed: usize,
    input_tokens: u64,
    priced_usd: f64,
}

fn jev_usage(steps: &[Step]) -> JevUsage {
    let mut usage = JevUsage::default();
    for step in steps {
        let Some(call) = step.call.as_ref().filter(|call| call.is_decision()) else {
            continue;
        };
        let charge = JevCharge::of(step, call);
        usage.requests += 1;
        match charge.charge {
            "priced" => {
                usage.priced += 1;
                usage.input_tokens += charge.input_tokens.unwrap_or(0);
                usage.priced_usd += charge.usd();
            }
            "zero" => usage.zero += 1,
            _ => usage.unknown += 1,
        }
        if charge.skipped() {
            usage.skipped += 1;
        } else if call.outcome == atif::document::Outcome::Failed {
            usage.failed += 1;
        }
    }
    usage
}

impl JevUsage {
    fn record(&self) -> Value {
        let known = self.unknown == 0;
        json!({
            "model": credentials::JEV_MODEL,
            "requests": self.requests,
            "priced": self.priced,
            "known_zero": self.zero,
            "unknown": self.unknown,
            "skipped": self.skipped,
            "input_tokens": known.then_some(self.input_tokens),
            "input_tokens_priced": self.input_tokens,
            "output_tokens_billed": false,
            "cost_usd": known.then_some(self.priced_usd),
            "cost_lower_bound_usd": self.priced_usd,
            "cost_provenance": if known { "price_estimate" } else { "unknown" },
            "rate": "$0.042 per million input tokens, retrieved 2026-09-22",
        })
    }
}

/// One delegate dispatch, with its own identity and charge.
struct Dispatch<'a> {
    call: &'a atif::document::Call,
    step: &'a Step,
    outcome: atif::document::Outcome,
    charge: &'static str,
    cost_usd: Option<f64>,
    lower_bound_usd: f64,
    provenance: String,
}

impl<'a> Dispatch<'a> {
    fn of(step: &'a Step, call: &'a atif::document::Call) -> Option<Self> {
        if call.name != "delegate"
            || call.extra.get("schema").and_then(Value::as_str) != Some(delegate::CALL_SCHEMA)
        {
            return None;
        }
        let reported = call.extra.get("total_cost_usd").and_then(Value::as_f64);
        // A record written before charges were recorded is priced when it
        // carries a cost.
        let charge = match call.extra.get("charge").and_then(Value::as_str) {
            Some("priced") if reported.is_some() => "priced",
            Some("zero") => "zero",
            Some(_) => "unknown",
            None if reported.is_some() => "priced",
            None => "unknown",
        };
        let partial = call
            .extra
            .get("cost_lower_bound_usd")
            .and_then(Value::as_f64);
        Some(Self {
            call,
            step,
            outcome: call.outcome,
            charge,
            cost_usd: match charge {
                "priced" => reported,
                "zero" => Some(0.0),
                _ => None,
            },
            lower_bound_usd: reported.or(partial).unwrap_or(0.0),
            provenance: match charge {
                "priced" => call
                    .extra
                    .get("cost_provenance")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                "zero" => "none".to_string(),
                _ => "unknown".to_string(),
            },
        })
    }

    fn extra(&self, key: &str) -> Value {
        self.call.extra.get(key).cloned().unwrap_or(Value::Null)
    }

    fn usage(&self, key: &str) -> Option<u64> {
        self.call.extra.get("usage")?.get(key)?.as_u64()
    }

    /// Native turns, model calls, and completed items. A record written
    /// before units were separated counted Codex's completed items as
    /// turns; it reads as completed items there.
    fn units(&self) -> (Option<u64>, Option<u64>, Option<u64>) {
        if let Some(units) = self.call.extra.get("units") {
            let read = |key: &str| units.get(key).and_then(Value::as_u64);
            return (
                read("native_turns"),
                read("model_calls"),
                read("completed_items"),
            );
        }
        let turns = self.extra("num_turns").as_u64();
        let calls = self.extra("api_calls").as_u64();
        if self.extra("capability").as_str() == Some("codex") {
            (self.usage("codex_turns"), None, turns)
        } else {
            (turns, calls, None)
        }
    }

    fn record(&self) -> Value {
        let (turns, calls, items) = self.units();
        json!({
            "component": "delegate",
            "id": self.call.id,
            "name": "delegate",
            "agent": self.extra("capability"),
            "model": self.step.model.clone().map_or_else(|| self.extra("model"), Value::String),
            "model_requested": self.extra("model"),
            "credential": self.extra("credential"),
            "status": self.extra("status"),
            "outcome": self.outcome,
            "charge": self.charge,
            "cost_usd": self.cost_usd,
            "cost_lower_bound_usd": self.lower_bound_usd,
            "provenance": self.provenance,
            "basis": self.extra("charge_basis"),
            "milliseconds": self.call.milliseconds,
            "units": {
                "native_turns": turns,
                "model_calls": calls,
                "completed_items": items,
            },
            "deadline": self.extra("deadline"),
        })
    }
}

/// The delegate component, summed over every dispatch.
#[derive(Default)]
struct DelegateUsage<'a> {
    dispatches: Vec<Dispatch<'a>>,
    input: Option<u64>,
    cache_read: Option<u64>,
    cache_creation: Option<u64>,
    output: Option<u64>,
    per_call: Vec<u64>,
}

/// One value when every dispatch agrees, `mixed` when they differ, and
/// `null` with none.
fn agreed(values: &[Value]) -> Value {
    let mut distinct: Vec<&Value> = Vec::new();
    for value in values {
        if !distinct.contains(&value) {
            distinct.push(value);
        }
    }
    match distinct.as_slice() {
        [] => Value::Null,
        [one] => (*one).clone(),
        _ => json!("mixed"),
    }
}

impl DelegateUsage<'_> {
    fn total_input(&self) -> Option<u64> {
        Some(self.input? + self.cache_read? + self.cache_creation?)
    }

    fn unknown(&self) -> usize {
        self.dispatches
            .iter()
            .filter(|dispatch| dispatch.charge == "unknown")
            .count()
    }

    /// The summed cost, known only when no dispatch's charge is unknown.
    fn cost_usd(&self) -> Option<f64> {
        self.dispatches
            .iter()
            .map(|dispatch| dispatch.cost_usd)
            .sum()
    }

    fn lower_bound_usd(&self) -> f64 {
        self.dispatches
            .iter()
            .map(|dispatch| dispatch.lower_bound_usd)
            .sum()
    }

    fn units(&self) -> (Option<u64>, Option<u64>, Option<u64>) {
        let units: Vec<_> = self.dispatches.iter().map(Dispatch::units).collect();
        (
            units.iter().map(|u| u.0).sum(),
            units.iter().map(|u| u.1).sum(),
            units.iter().map(|u| u.2).sum(),
        )
    }

    fn record(&self) -> Value {
        let each = |key: &str| -> Vec<Value> {
            self.dispatches
                .iter()
                .map(|dispatch| dispatch.extra(key))
                .collect()
        };
        let priced: Vec<Value> = self
            .dispatches
            .iter()
            .filter(|dispatch| dispatch.charge == "priced")
            .map(|dispatch| json!(dispatch.provenance))
            .collect();
        let provenance = if self.dispatches.is_empty() {
            json!("none")
        } else if self.unknown() > 0 {
            json!("unknown")
        } else if priced.is_empty() {
            json!("none")
        } else {
            agreed(&priced)
        };
        let cost_note: Vec<Value> = each("cost_note");
        let (turns, calls, items) = self.units();
        let models: Vec<Value> = self
            .dispatches
            .iter()
            .map(|dispatch| {
                dispatch
                    .step
                    .model
                    .clone()
                    .map_or_else(|| dispatch.extra("model"), Value::String)
            })
            .collect();
        json!({
            "agent": agreed(&each("capability")),
            "model": agreed(&each("model")),
            "credential": agreed(&each("credential")),
            "agents": each("capability"),
            "models": models,
            "delegations": self.dispatches.len(),
            "turns": turns,
            "api_calls": calls,
            "units": {
                "native_turns": turns,
                "model_calls": calls,
                "completed_items": items,
            },
            "input_tokens": self.input,
            "cache_read_input_tokens": self.cache_read,
            "cache_creation_input_tokens": self.cache_creation,
            "total_input_tokens": self.total_input(),
            "output_tokens": self.output,
            "input_tokens_per_call": self.per_call,
            "max_input_tokens_per_call": self.per_call.iter().max(),
            "cost_usd": if self.dispatches.is_empty() { json!(0.0) } else { json!(self.cost_usd()) },
            "cost_lower_bound_usd": self.lower_bound_usd(),
            "cost_provenance": provenance,
            "unknown_dispatches": self.unknown(),
            "cost_note": agreed(&cost_note),
            "dispatches": self.dispatches.iter().map(Dispatch::record).collect::<Vec<_>>(),
        })
    }
}

fn delegate_usage(steps: &[Step]) -> DelegateUsage<'_> {
    let dispatches: Vec<Dispatch<'_>> = steps
        .iter()
        .filter_map(|step| step.call.as_ref().and_then(|call| Dispatch::of(step, call)))
        .collect();
    let sum = |read: &dyn Fn(&Dispatch<'_>) -> Option<u64>| -> Option<u64> {
        dispatches.iter().map(read).sum()
    };
    DelegateUsage {
        input: sum(&|d| d.usage("input_tokens")),
        cache_read: sum(&|d| d.usage("cache_read_input_tokens")),
        cache_creation: sum(&|d| d.usage("cache_creation_input_tokens")),
        output: sum(&|d| d.usage("output_tokens")),
        per_call: dispatches
            .iter()
            .filter_map(|d| {
                d.call
                    .extra
                    .get("input_tokens_per_call")?
                    .as_array()
                    .cloned()
            })
            .flatten()
            .filter_map(|value| value.as_u64())
            .collect(),
        dispatches,
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

#[cfg(test)]
mod accounting_tests {
    use std::time::{Duration, Instant};

    use atif::document::{Decision, Step};
    use serde_json::{Map, Value, json};

    use super::usage;
    use crate::credentials;
    use crate::deadline::Deadline;
    use crate::delegate::{
        self, Agent, Briefing, BriefingInputs, Cli, Credential, Delegation, Executor, Mode, Reason,
        Report, Status, Summary,
    };
    use crate::record::Recorder;

    /// A Jev call as the judge records it, with `charge` from the judge's
    /// own classifier.
    fn jev(id: &str, outcome: Result<u64, jev::Error>) -> Step {
        let mut decision = Decision {
            id: id.to_string(),
            name: "jev_survey".to_string(),
            door: credentials::JEV_BASE_URL.to_string(),
            model: credentials::JEV_MODEL.to_string(),
            request: json!({ "state": {}, "questions": {} }),
            answers: json!({}),
            route: None,
            error: None,
            attempts: Vec::new(),
            review: None,
            milliseconds: 40,
        };
        match outcome {
            Ok(tokens) => Step::called(decision.call()).noting(
                "jev_usage",
                json!({ "charge": "priced", "input_tokens": tokens, "output_tokens": 0 }),
            ),
            Err(error) => {
                decision.error = Some(error.to_string());
                Step::called(decision.call())
                    .noting("jev_usage", crate::component::jev::charge_failed(&error))
            }
        }
    }

    /// An executor with a chosen identity, answering from a script.
    struct Named {
        agent: &'static str,
        model: &'static str,
        provenance: &'static str,
        credential: &'static str,
        report: Option<Report>,
    }

    impl Executor for Named {
        fn agent(&self) -> &str {
            self.agent
        }
        fn cost_provenance(&self) -> &'static str {
            self.provenance
        }
        fn model(&self) -> &str {
            self.model
        }
        fn deadline(&self) -> Duration {
            Duration::from_secs(600)
        }
        fn describe(&self) -> Map<String, Value> {
            let mut extra = Map::new();
            extra.insert("credential".to_string(), json!(self.credential));
            extra
        }
        async fn execute(&mut self, _briefing: &Briefing) -> Report {
            self.report.take().expect("one report")
        }
    }

    fn briefing() -> Briefing {
        Briefing::build(
            &BriefingInputs {
                instruction: "Fix the parser.".to_string(),
                requirements: vec![],
                files: vec![],
                spans: vec![],
                commands: vec![],
                last_output: None,
                conclusion: String::new(),
                directions: "Go.".to_string(),
            },
            2_000,
        )
    }

    const CODEX_TURN: &str = r#"{"type":"thread.started","thread_id":"t-1"}
{"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"ls","exit_code":0}}
{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"Handed over."}}
{"type":"turn.completed","usage":{"input_tokens":1000000,"cached_input_tokens":0,"output_tokens":0}}
"#;

    const CLAUDE_RESULT: &str = r#"{"type":"system","subtype":"init","model":"claude-opus-5-5","claude_code_version":"2.1.280"}
{"type":"assistant","message":{"id":"msg_1","usage":{"input_tokens":10,"output_tokens":5}}}
{"type":"result","subtype":"success","is_error":false,"num_turns":2,"result":"Fixed.","total_cost_usd":0.5,"usage":{"input_tokens":10,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"output_tokens":5}}
"#;

    fn dispatch(executor: &Named, report: &Report, number: u32) -> Step {
        delegate::record(
            executor,
            &briefing(),
            &Delegation {
                mode: Mode::Always,
                reason: &Reason::Always,
                isolation: "none",
            },
            report,
            number,
        )
    }

    fn luna(status: Status) -> (Named, Report) {
        let report = Report {
            status,
            summary: Summary::parse_codex(CODEX_TURN, "gpt-6-luna"),
            milliseconds: 4_000,
            stderr: String::new(),
            stream: Some(json!({ "path": "artifacts/delegate-1.stream.jsonl" })),
        };
        let executor = Named {
            agent: "codex",
            model: "gpt-6-luna",
            provenance: "price_estimate",
            credential: "codex_auth_json",
            report: None,
        };
        (executor, report)
    }

    fn opus() -> (Named, Report) {
        let report = Report {
            status: Status::Answered,
            summary: Summary::parse(CLAUDE_RESULT),
            milliseconds: 9_000,
            stderr: String::new(),
            stream: Some(json!({ "path": "artifacts/delegate-2.stream.jsonl" })),
        };
        let executor = Named {
            agent: "claude-code",
            model: "claude-opus-5-5",
            provenance: "cli_list_price",
            credential: "subscription_oauth",
            report: None,
        };
        (executor, report)
    }

    #[test]
    fn a_failed_jev_call_a_luna_dispatch_and_an_opus_dispatch_each_account_correctly() {
        let (luna_executor, luna_report) = luna(Status::Answered);
        let (opus_executor, opus_report) = opus();
        let steps = [
            jev("jev-survey-1", Ok(1_000_000)),
            jev(
                "jev-survey-2",
                Err(jev::Error::Timeout {
                    timeout: Duration::from_secs(10),
                }),
            ),
            jev(
                "jev-survey-3",
                Err(jev::Error::Config("no key".to_string())),
            ),
            dispatch(&luna_executor, &luna_report, 1),
            dispatch(&opus_executor, &opus_report, 2),
        ];
        let usage = usage(&steps, true);

        // Jev: one priced, one unknown, one known zero. The unknown keeps
        // the component's cost unknown; the lower bound is what is known.
        let jev = &usage["components"]["jev"];
        assert_eq!(jev["requests"], 3);
        assert_eq!(jev["priced"], 1);
        assert_eq!(jev["unknown"], 1);
        assert_eq!(jev["known_zero"], 1);
        assert_eq!(jev["cost_usd"], Value::Null);
        assert_eq!(jev["cost_provenance"], "unknown");
        assert!((jev["cost_lower_bound_usd"].as_f64().unwrap() - 0.042).abs() < 1e-12);

        // The delegate: two identities, summed with mixed provenance.
        let delegate = &usage["components"]["delegate"];
        assert_eq!(delegate["delegations"], 2);
        assert_eq!(delegate["agent"], "mixed");
        assert_eq!(delegate["model"], "mixed");
        assert_eq!(delegate["credential"], "mixed");
        assert_eq!(delegate["agents"], json!(["codex", "claude-code"]));
        assert_eq!(delegate["cost_provenance"], "mixed");
        // $0.10 for a million Luna input tokens plus Opus's own $0.50.
        let cost = delegate["cost_usd"].as_f64().unwrap();
        assert!((cost - 0.6).abs() < 1e-9, "{cost}");
        let dispatches = delegate["dispatches"].as_array().unwrap();
        assert_eq!(dispatches[0]["model"], "gpt-6-luna");
        assert_eq!(dispatches[0]["provenance"], "price_estimate");
        assert_eq!(dispatches[0]["units"]["completed_items"], 2);
        assert_eq!(dispatches[0]["units"]["native_turns"], 1);
        assert_eq!(dispatches[0]["units"]["model_calls"], Value::Null);
        assert_eq!(dispatches[1]["model"], "claude-opus-5-5");
        assert_eq!(dispatches[1]["provenance"], "cli_list_price");
        assert_eq!(dispatches[1]["units"]["model_calls"], 1);
        // Units that one executor does not report stay unknown in the sum.
        assert_eq!(delegate["units"]["native_turns"], 3);
        assert_eq!(delegate["units"]["model_calls"], Value::Null);

        // The total stays unknown because one Jev call's charge is.
        assert_eq!(usage["cost"]["amount_usd"], Value::Null);
        assert_eq!(usage["cost"]["unknown_calls"], 1);
        let lower = usage["cost"]["lower_bound_usd"].as_f64().unwrap();
        assert!((lower - 0.642).abs() < 1e-9, "{lower}");

        // The ledger has one row per call, each with its own charge.
        let ledger = usage["ledger"].as_array().unwrap();
        let charges: Vec<(&str, &str)> = ledger
            .iter()
            .map(|row| {
                (
                    row["component"].as_str().unwrap(),
                    row["charge"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            charges,
            [
                ("jev", "priced"),
                ("jev", "unknown"),
                ("jev", "zero"),
                ("delegate", "priced"),
                ("delegate", "priced"),
            ]
        );
        assert_eq!(ledger[1]["cost_usd"], Value::Null);
        assert_eq!(ledger[2]["cost_usd"], 0.0);
    }

    #[test]
    fn a_jev_refusal_is_a_known_zero_and_leaves_the_total_known() {
        let (opus_executor, opus_report) = opus();
        let api = jev::ApiError {
            status: 429,
            headers: Default::default(),
            body: None,
            request_id: None,
            endpoint: "POST /v1/systemone".to_string(),
            kind: jev::ApiErrorKind::Other,
        };
        let steps = [
            jev("jev-1", Ok(1_000_000)),
            jev("jev-2", Err(jev::Error::Api(Box::new(api)))),
            dispatch(&opus_executor, &opus_report, 1),
        ];
        let usage = usage(&steps, true);
        assert_eq!(usage["components"]["jev"]["known_zero"], 1);
        let total = usage["cost"]["amount_usd"].as_f64().unwrap();
        assert!((total - 0.542).abs() < 1e-9, "{total}");
        assert_eq!(usage["components"]["delegate"]["agent"], "claude-code");
        assert_eq!(
            usage["components"]["delegate"]["cost_provenance"],
            "cli_list_price"
        );
    }

    #[test]
    fn a_cancelled_session_is_unknown_with_a_lower_bound_never_zero() {
        let (executor, report) = luna(Status::TimedOut);
        let step = dispatch(&executor, &report, 1);
        let call = step.call.as_ref().unwrap();
        assert_eq!(call.extra["charge"], "unknown");
        assert_eq!(call.extra["total_cost_usd"], Value::Null);
        assert_eq!(call.extra["cost_lower_bound_usd"], 0.1);
        let usage = usage(&[step], true);
        let delegate = &usage["components"]["delegate"];
        assert_eq!(delegate["cost_usd"], Value::Null);
        assert_eq!(delegate["cost_provenance"], "unknown");
        assert_eq!(delegate["cost_lower_bound_usd"], 0.1);
        assert_eq!(usage["cost"]["amount_usd"], Value::Null);
    }

    #[test]
    fn older_records_keep_their_meaning() {
        // A failed Jev call recorded before charges had no usage at all:
        // unknown, not dropped.
        let mut decision = Decision {
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
        decision.error = Some("timed out".to_string());
        let failed = Step::called(decision.call());
        let usage = usage(&[failed], false);
        assert_eq!(usage["components"]["jev"]["unknown"], 1);
        assert_eq!(usage["cost"]["amount_usd"], Value::Null);
    }

    fn fake_binary(sleep: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("coder-one-deadline-{}-{sleep}", std::process::id()));
        std::fs::create_dir_all(dir.join("artifacts")).unwrap();
        let fake = dir.join("fake-claude");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\ncat > /dev/null\nsleep {sleep}\necho '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"result\":\"done\",\"total_cost_usd\":0.1}}'\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        fake
    }

    fn cli(binary: std::path::PathBuf, episode: Deadline) -> Cli {
        let dir = binary.parent().unwrap().to_path_buf();
        Cli {
            agent: Agent::ClaudeCode,
            binary: Some(binary),
            model: "claude-opus-5-5".to_string(),
            deadline: Duration::from_secs(600),
            workdir: dir.clone(),
            artifacts: dir.join("artifacts"),
            artifacts_label: "artifacts".to_string(),
            env: Vec::new(),
            credential: Credential::OauthToken,
            effort: None,
            tools: None,
            prompt_cache_ttl: None,
            system: None,
            episode,
            gate: None,
            granted: None,
            runs: 0,
            control: Default::default(),
        }
    }

    #[tokio::test]
    async fn a_dispatch_cannot_outlive_the_episode_deadline_and_the_record_says_so() {
        // Two seconds left in the episode, none kept back; the executor
        // would run for ten.
        let deadline = Deadline::new(Some(Duration::from_secs(2)), Duration::ZERO);
        let mut executor = cli(fake_binary("10"), deadline.clone());
        let started = Instant::now();
        let recorder = Recorder::default();
        let report = delegate::delegate(
            &mut executor,
            &briefing(),
            &Delegation {
                mode: Mode::Always,
                reason: &Reason::Always,
                isolation: "none",
            },
            &recorder,
            0,
        )
        .await;
        assert!(started.elapsed() < Duration::from_secs(6));
        assert_eq!(report.status, Status::TimedOut);
        let steps = recorder.steps();
        let call = steps[1].call.as_ref().unwrap();
        assert_eq!(call.extra["deadline"]["requested_sec"], 600);
        assert_eq!(call.extra["deadline"]["cut"], true);
        assert_eq!(call.extra["charge"], "unknown");
        let cuts = deadline.cuts();
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].what, "delegate-1");
        assert!(!cuts[0].skipped());
        assert_eq!(deadline.record()["cuts"][0]["what"], "delegate-1");
    }

    #[tokio::test]
    async fn a_dispatch_with_no_time_left_never_starts_and_costs_a_known_zero() {
        let deadline = Deadline::starting(
            Instant::now() - Duration::from_secs(100),
            Some(Duration::from_secs(60)),
            Duration::from_secs(10),
        );
        let mut executor = cli(fake_binary("0"), deadline.clone());
        let report = executor.execute(&briefing()).await;
        assert!(matches!(report.status, Status::Harness(_)));
        assert_eq!(delegate::charge(&report).0, "zero");
        assert!(deadline.cuts()[0].skipped());

        // The soft spend gate stops a dispatch the same way.
        let mut executor = cli(fake_binary("0"), Deadline::unbounded());
        executor.gate = Some(Box::new(|| {
            Some("the soft spend bound is reached".to_string())
        }));
        let report = executor.execute(&briefing()).await;
        assert_eq!(
            report.status.to_string(),
            "harness: the soft spend bound is reached"
        );
        assert_eq!(delegate::charge(&report).0, "zero");
    }

    #[tokio::test]
    async fn a_jev_request_with_no_time_left_is_never_sent_and_is_recorded() {
        let deadline = Deadline::starting(
            Instant::now() - Duration::from_secs(100),
            Some(Duration::from_secs(60)),
            Duration::from_secs(10),
        );
        let client = jev::Client::new(
            jev::Config::new()
                .api_key("unused")
                .base_url("http://127.0.0.1:9")
                .default_model(credentials::JEV_MODEL),
        )
        .unwrap();
        let recorder = Recorder::default();
        let issue = crate::state::Issue {
            url: String::new(),
            title: "Task".to_string(),
            body: "- [ ] Do it".to_string(),
            labels: vec![],
        };
        let state = crate::state::State::new(
            crate::state::Environment {
                repository: String::new(),
                workdir: std::env::temp_dir().to_string_lossy().into_owned(),
                os: "linux".to_string(),
            },
            issue.clone(),
        );
        let mut judge = crate::judge::JevJudge::new(
            Some(client),
            std::env::temp_dir(),
            &issue,
            recorder.clone(),
        )
        .within(deadline);
        let close = judge.close(&state, "done", "").await;
        assert!(close.unavailable.unwrap().contains("deadline"));
        assert_eq!(judge.skipped, 1);
        let usage = usage(&recorder.steps(), false);
        assert_eq!(usage["components"]["jev"]["skipped"], 1);
        assert_eq!(usage["components"]["jev"]["known_zero"], 1);
        assert_eq!(usage["calls"]["decisions_skipped"], 1);
        assert_eq!(usage["calls"]["failed"], 0);
        assert_eq!(usage["ledger"][0]["charge"], "zero");
    }

    #[test]
    fn failed_jev_calls_classify_by_what_the_door_could_have_done() {
        let charge = |error: jev::Error| {
            crate::component::jev::charge_failed(&error)["charge"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(charge(jev::Error::Config("x".into())), "zero");
        assert_eq!(
            charge(jev::Error::Timeout {
                timeout: Duration::from_secs(1)
            }),
            "unknown"
        );
        assert_eq!(
            charge(jev::Error::Connection {
                message: "reset".into(),
                source: None
            }),
            "unknown"
        );
        let api = |status| {
            jev::Error::Api(Box::new(jev::ApiError {
                status,
                headers: Default::default(),
                body: None,
                request_id: None,
                endpoint: String::new(),
                kind: jev::ApiErrorKind::Other,
            }))
        };
        assert_eq!(charge(api(401)), "zero");
        assert_eq!(charge(api(503)), "unknown");
    }
}
