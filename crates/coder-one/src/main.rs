//! `coder-one`: the command line.
//!
//! ```text
//! coder-one doctor
//! coder-one <issue-url> [--lane free|flash|pro] [--max-steps N]
//!                       [--timeout SECONDS] [--no-jev] [--deep] [--open-pr]
//!                       [--delegate off|always|auto] [--explore-steps N]
//!                       [--delegate-agent claude-code|codex]
//!                       [--delegate-model MODEL] [--delegate-timeout SECONDS]
//! coder-one --version
//! coder-one episode doctor --contract openagents.coder.episode.v1
//! coder-one episode run --instruction-file F --output-dir D --contract C [--model M]
//! coder-one component list|run|suite|extract …
//! coder-one minitask list|run …
//! coder-one checks synthetic|run|recover …
//! coder-one support evaluate|run|fixtures …
//! coder-one repair study|brief …
//! coder-one capabilities [--demonstrate] [--json]
//! ```
//!
//! The `episode` commands implement the Terminal-Bench harness's headless
//! contract; `coder_one::episode` documents them. The `component` commands
//! run one component alone on fixtures; `coder_one::component` documents
//! them.
//!
//! A run clones the issue's repository fresh under
//! `~/.openagents/coder-one/runs/`, works on a new branch there, and
//! streams every step to the console. When the agent finishes with
//! changes, the host commits them; `--open-pr` also pushes the branch and
//! opens a draft pull request.
//!
//! `--delegate always` lets the loop explore for `--explore-steps` steps
//! without editing, then hands the task to Claude Code, or to Codex CLI
//! with `--delegate-agent codex`, with a briefing
//! built from what it found; `--delegate auto` delegates only when the
//! explorer stalls. `coder_one::delegate` documents the phases.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder_one::agent::INSTRUCTIONS;
use coder_one::credentials;
use coder_one::delegate::{self, Agent, Cli, Mode, Plan, Policy};
use coder_one::episode::{self, RunArgs};
use coder_one::generate::Door;
use coder_one::judge::JevJudge;
use coder_one::record::Recorder;
use coder_one::shell::Checkout;
use coder_one::{Bounds, Ended, Environment, Issue, State, run};
use serde::Deserialize;

const USAGE: &str = "usage: coder-one doctor
       coder-one <github-issue-url> [--lane free|flash|pro] [--max-steps N]
                 [--timeout SECONDS] [--no-jev] [--deep] [--open-pr]
                 [--delegate off|always|auto] [--explore-steps N]
                 [--delegate-agent claude-code|codex]
                 [--delegate-model MODEL] [--delegate-timeout SECONDS]
       coder-one --version
       coder-one episode doctor --contract openagents.coder.episode.v1
       coder-one episode run --instruction-file F --output-dir D --contract C [--model M]
       coder-one component list|run|suite|extract   (coder-one component help)
       coder-one minitask list|run                  (coder-one minitask help)
       coder-one capabilities [--demonstrate] [--json]  (coder-one capabilities help)
       coder-one prompt list|show|capture           (coder-one prompt help)
       coder-one checks synthetic|run|recover       (coder-one checks help)
       coder-one study run|list                     (coder-one study help)
       coder-one support evaluate|run|fixtures      (coder-one support help)
       coder-one repair study|brief                 (coder-one repair help)";

const PROMPT: &str = "Solve this issue.";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("{}", episode::version());
            Ok(())
        }
        Some("episode") => return episode_command(&args[1..]).await,
        Some("component") => return component_command(&args[1..]).await,
        Some("minitask") => return minitask_command(&args[1..]).await,
        Some("capabilities") => {
            return match coder_one::capabilities::command(&args[1..]).await {
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
                Err(message) => {
                    eprintln!("coder-one: {message}");
                    ExitCode::FAILURE
                }
            };
        }
        Some("study") => return study_command(&args[1..]).await,
        Some("prompt") => {
            return match coder_one::prompt::command(&args[1..]) {
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
                Err(message) => {
                    eprintln!("coder-one: {message}");
                    ExitCode::FAILURE
                }
            };
        }
        Some("checks") => return checks_command(&args[1..]).await,
        Some("support") => return support_command(&args[1..]).await,
        Some("repair") => return repair_command(&args[1..]).await,
        Some("doctor") if args.len() == 1 => doctor(),
        Some(url) if url.starts_with("https://github.com/") && url.contains("/issues/") => {
            match Options::parse(&args[1..]) {
                Ok(options) => solve(url, options).await,
                Err(error) => Err(format!("{error}\n{USAGE}")),
            }
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `coder-one episode doctor|run …`, whose exit code is the contract's.
async fn episode_command(args: &[String]) -> ExitCode {
    let flag = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    let contract = flag("--contract").unwrap_or_default();
    let result = match args.first().map(String::as_str) {
        Some("doctor") => episode::doctor(&contract).await.map(|()| 0),
        Some("run") => match (flag("--instruction-file"), flag("--output-dir")) {
            (Some(instruction), Some(output)) => {
                episode::run_episode(RunArgs {
                    instruction_file: instruction.into(),
                    output_dir: output.into(),
                    contract,
                    model: flag("--model"),
                })
                .await
            }
            _ => Err(format!(
                "episode run needs --instruction-file and --output-dir\n{USAGE}"
            )),
        },
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `coder-one component …`: 0 when every fixture ran, 1 when one failed.
async fn component_command(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h") | None
    ) {
        println!("{}", coder_one::component::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    match coder_one::component::cli::command(args).await {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `coder-one checks …`: `verify.checks` on its own.
async fn checks_command(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h") | None
    ) {
        println!("{}", coder_one::checks::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    match coder_one::checks::cli::command(args).await {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `coder-one study …`: 0 when the study finished, whatever it found.
async fn study_command(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h") | None
    ) {
        println!("{}", coder_one::study::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    match coder_one::study::cli::command(args).await {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `coder-one support …`: `verify.support` on its own.
async fn support_command(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h") | None
    ) {
        println!("{}", coder_one::support::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    match coder_one::support::cli::command(args).await {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `coder-one repair …`: `verify.repair`'s recovery study and briefs.
async fn repair_command(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h") | None
    ) {
        println!("{}", coder_one::repair::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    match coder_one::repair::cli::command(args).await {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `coder-one minitask …`: 0 when the grader passed.
async fn minitask_command(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h") | None
    ) {
        println!("{}", coder_one::minitask::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    match coder_one::minitask::cli::command(args).await {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(message) => {
            eprintln!("coder-one: {message}");
            ExitCode::FAILURE
        }
    }
}

struct Options {
    lane: String,
    max_steps: usize,
    timeout: Duration,
    jev: bool,
    deep: bool,
    open_pr: bool,
    delegate: Mode,
    explore_steps: usize,
    delegate_agent: Agent,
    /// `None` takes the agent's default model.
    delegate_model: Option<String>,
    delegate_timeout: Duration,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Options {
            lane: "free".to_string(),
            max_steps: 30,
            timeout: Duration::from_secs(120),
            jev: true,
            deep: false,
            open_pr: false,
            delegate: Mode::Off,
            explore_steps: Policy::default().explore_steps,
            delegate_agent: Agent::ClaudeCode,
            delegate_model: None,
            delegate_timeout: Duration::from_secs(1_200),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{name} needs a value"))
            };
            match arg.as_str() {
                "--lane" => options.lane = value("--lane")?,
                "--max-steps" => {
                    options.max_steps = value("--max-steps")?
                        .parse()
                        .map_err(|_| "--max-steps takes a number".to_string())?;
                }
                "--timeout" => {
                    let seconds: u64 = value("--timeout")?
                        .parse()
                        .map_err(|_| "--timeout takes seconds".to_string())?;
                    options.timeout = Duration::from_secs(seconds);
                }
                "--no-jev" => options.jev = false,
                "--deep" => options.deep = true,
                "--open-pr" => options.open_pr = true,
                "--delegate" => options.delegate = Mode::parse(&value("--delegate")?)?,
                "--explore-steps" => {
                    options.explore_steps = value("--explore-steps")?
                        .parse()
                        .map_err(|_| "--explore-steps takes a number".to_string())?;
                }
                "--delegate-agent" => {
                    options.delegate_agent = Agent::parse(&value("--delegate-agent")?)?;
                }
                "--delegate-model" => options.delegate_model = Some(value("--delegate-model")?),
                "--delegate-timeout" => {
                    let seconds: u64 = value("--delegate-timeout")?
                        .parse()
                        .map_err(|_| "--delegate-timeout takes seconds".to_string())?;
                    options.delegate_timeout = Duration::from_secs(seconds);
                }
                other => return Err(format!("unknown option {other}")),
            }
        }
        Ok(options)
    }
}

/// Reports where each credential resolves from. Makes no network call.
fn doctor() -> Result<(), String> {
    let dir = credentials::openagents_dir().ok_or("HOME is not set")?;
    let env = |name: &str| std::env::var(name).ok();
    let mut ok = true;
    for (label, found) in [
        ("jev", credentials::jev_key(env, &dir)),
        ("generation", credentials::bearer(env, &dir)),
    ] {
        match found {
            Ok(found) => println!("{label}: found in {}", found.source),
            Err(error) => {
                ok = false;
                println!("{label}: {error}");
            }
        }
    }
    println!(
        "jev door: {} ({})",
        credentials::JEV_BASE_URL,
        credentials::JEV_MODEL
    );
    println!(
        "generation door: {}/v1/responses",
        credentials::GENERATION_BASE_URL
    );
    for agent in [Agent::ClaudeCode, Agent::Codex] {
        match delegate::resolve(agent, env) {
            (Some(path), credential) => println!(
                "delegate: {} at {} (credential: {})",
                agent.word(),
                path.display(),
                credential.word()
            ),
            (None, _) => println!(
                "delegate: no {} binary; --delegate-agent {} needs one",
                agent.word(),
                agent.word()
            ),
        }
    }
    if ok {
        Ok(())
    } else {
        Err("a credential is missing".to_string())
    }
}

async fn solve(url: &str, options: Options) -> Result<(), String> {
    let started = Instant::now();
    let dir = credentials::openagents_dir().ok_or("HOME is not set")?;
    let env = |name: &str| std::env::var(name).ok();
    let bearer = credentials::bearer(env, &dir)?;
    let jev = if options.jev {
        let key = credentials::jev_key(env, &dir)?;
        Some(credentials::jev_client(&key.secret)?)
    } else {
        None
    };

    let issue = fetch_issue(url)?;
    let (repository, number) = parse_issue_url(url).ok_or("cannot read the issue URL")?;
    println!("issue   {}#{number}: {}", repository, issue.title);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let run_dir = dir
        .join("coder-one")
        .join("runs")
        .join(format!("{}-{number}-{stamp}", repository.replace('/', "-")));
    let workdir = run_dir.join("repo");
    std::fs::create_dir_all(&run_dir)
        .map_err(|error| format!("cannot create {}: {error}", run_dir.display()))?;
    println!("clone   {repository} → {}", workdir.display());
    command(
        Path::new("."),
        "gh",
        &[
            "repo",
            "clone",
            &repository,
            &workdir.to_string_lossy(),
            "--",
            "--depth",
            "1",
            "--quiet",
        ],
    )?;
    let branch = format!("coder-one/issue-{number}-{stamp}");
    command(&workdir, "git", &["checkout", "-q", "-b", &branch])?;
    println!("branch  {branch}");
    println!(
        "doors   generation {}/v1/responses ({}), jev {}",
        credentials::GENERATION_BASE_URL,
        options.lane,
        if options.jev {
            credentials::JEV_MODEL
        } else {
            "off"
        }
    );

    let mut state = State::new(
        Environment {
            repository: repository.clone(),
            workdir: workdir.to_string_lossy().into_owned(),
            os: std::env::consts::OS.to_string(),
        },
        issue,
    );
    let recorder = Recorder::default();
    recorder.push(atif::document::Step::said(
        atif::document::Source::System,
        INSTRUCTIONS,
    ));
    recorder.push(atif::document::Step::said(
        atif::document::Source::User,
        &format!("{}\n\n{}", state.issue.title, state.issue.body),
    ));
    let mut judge = JevJudge::new(jev, workdir.clone(), &state.issue, recorder.clone())
        .deep(options.deep && options.jev);
    judge.survey(&mut state).await;
    let mut door = Door::new(
        credentials::GENERATION_BASE_URL,
        bearer.secret,
        &options.lane,
        INSTRUCTIONS,
        Box::new(|delta| {
            print!("{delta}");
            let _ = std::io::stdout().flush();
        }),
        recorder.clone(),
    )?
    .caching_under(&branch);
    let mut shell = Checkout {
        workdir: workdir.clone(),
        deadline: options.timeout,
        recorder: recorder.clone(),
        commands: 0,
        episode: coder_one::deadline::Deadline::unbounded(),
    };

    let delegate_model = options
        .delegate_model
        .clone()
        .unwrap_or_else(|| options.delegate_agent.default_model().to_string());
    let base = command(&workdir, "git", &["rev-parse", "HEAD"])
        .map(|out| out.trim().to_string())
        .ok();
    let (ended, delegated) = if options.delegate == Mode::Off {
        let ended = run(
            &mut state,
            PROMPT,
            Bounds {
                max_steps: options.max_steps,
            },
            &mut judge,
            &mut door,
            &mut shell,
        )
        .await;
        (ended, None)
    } else {
        let env = |name: &str| std::env::var(name).ok();
        let (binary, credential) = delegate::resolve(options.delegate_agent, env);
        let mut executor = Cli {
            agent: options.delegate_agent,
            binary,
            model: delegate_model.clone(),
            deadline: options.delegate_timeout,
            workdir: workdir.clone(),
            artifacts: run_dir.clone(),
            artifacts_label: run_dir.to_string_lossy().into_owned(),
            env: Vec::new(),
            credential,
            // Issue mode keeps reading the older switches directly; an
            // episode resolves them through its policy manifest.
            effort: env("CODER_ONE_DELEGATE_EFFORT")
                .map(|effort| effort.trim().to_string())
                .filter(|effort| {
                    !effort.is_empty() && effort.chars().all(|c| c.is_ascii_lowercase())
                }),
            tools: env("CODER_ONE_DELEGATE_TOOLS")
                .map(|tools| tools.trim().to_string())
                .filter(|tools| !tools.is_empty()),
            prompt_cache_ttl: env("CLAUDE_CODE_PROMPT_CACHE_TTL")
                .map(|ttl| ttl.trim().to_string())
                .filter(|ttl| !ttl.is_empty()),
            system: None,
            episode: coder_one::deadline::Deadline::unbounded(),
            gate: None,
            granted: None,
            runs: 0,
            control: delegate::Control {
                recorder: Some(recorder.clone()),
                ..delegate::Control::default()
            },
        };
        let instruction = format!("{}\n\n{}", state.issue.title, state.issue.body);
        let plan = Plan {
            mode: options.delegate,
            policy: Policy {
                explore_steps: options.explore_steps,
                ..Policy::default()
            },
            max_steps: options.max_steps,
            prompt: PROMPT,
            instruction: &instruction,
            directions: ISSUE_DIRECTIONS,
            cap: delegate::BRIEFING_CAP,
            packer: coder_one::policy::Packer::Sections,
            pack: coder_one::pack::Params::default(),
            isolation: "none",
            base: base.as_deref(),
        };
        delegate::explore_then_delegate(
            &mut state,
            &plan,
            &mut judge,
            &mut door,
            &mut shell,
            &mut executor,
            &recorder,
            &mut |_| {},
        )
        .await
    };

    let saved = run_dir.join("state.json");
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(&saved, json);
    }
    let steps = recorder.steps();
    let mut session = atif::document::Session::opening(
        &format!("coder-one-{stamp}"),
        &options.lane,
        credentials::GENERATION_BASE_URL,
        &repository,
        &episode::version(),
    );
    session.directive = PROMPT.to_string();
    session.state = "ended".to_string();
    session.seconds = started.elapsed().as_secs();
    let mut trajectory = atif::document::document(&session, &steps);
    trajectory["agent"]["name"] = serde_json::json!("coder-one");
    let usage = episode::usage(&steps, options.delegate != Mode::Off);
    for (name, value) in [
        ("trajectory.atif.json", &trajectory),
        ("usage.json", &usage),
    ] {
        if let Ok(json) = serde_json::to_string_pretty(value) {
            let _ = std::fs::write(run_dir.join(name), json);
        }
    }
    if let Some(delegated) = &delegated
        && let Ok(json) = serde_json::to_string_pretty(&delegated.record())
    {
        let _ = std::fs::write(run_dir.join("delegation.json"), json);
    }

    println!("\n── result ──");
    let finished = match &ended {
        Ended::Finished {
            title,
            summary,
            steps,
        } => {
            println!("finished after {steps} steps: {title}");
            println!("{summary}");
            Some((title.clone(), summary.clone()))
        }
        Ended::StepLimit { steps } => {
            println!("stopped: the {steps}-step limit ran out");
            None
        }
        Ended::GenerationFailed { error, steps } => {
            println!("stopped after {steps} steps: generation failed: {error}");
            None
        }
        Ended::Stopped { reason, steps } => {
            println!("stopped after {steps} steps: {reason}");
            None
        }
        Ended::Delegated {
            answered,
            status,
            title,
            summary,
            steps,
        } => {
            println!("explored {steps} steps, then delegated: {status}");
            println!("{summary}");
            answered.then(|| (title.clone(), summary.clone()))
        }
    };

    command(&workdir, "git", &["add", "-A"])?;
    let changed = !Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(&workdir)
        .status()
        .map_err(|error| format!("cannot run git: {error}"))?
        .success();

    match (finished, changed) {
        (_, false) => println!("no changes in the checkout"),
        (None, true) => {
            println!("changes are staged but not committed, because the run did not finish:");
            println!(
                "{}",
                command(&workdir, "git", &["diff", "--cached", "--stat"])?
            );
        }
        (Some((title, summary)), true) => {
            command(
                &workdir,
                "git",
                &["commit", "-q", "-m", &title, "-m", &summary],
            )?;
            println!(
                "{}",
                command(
                    &workdir,
                    "git",
                    &["show", "--stat", "--format=commit %h %s", "HEAD"]
                )?
            );
            if options.open_pr {
                println!("push    {branch}");
                command(&workdir, "git", &["push", "-q", "-u", "origin", &branch])?;
                let delegated_note = match &delegated {
                    Some(delegated) => format!(
                        ", delegated to {} ({}) for {} turns",
                        options.delegate_agent.word(),
                        delegate_model,
                        delegated
                            .report
                            .summary
                            .num_turns
                            .map_or("an unknown number of".to_string(), |n| n.to_string())
                    ),
                    None => String::new(),
                };
                let body = format!(
                    "{summary}\n\nCloses {url}\n\n---\nOpened by coder-one in {} steps (lane `{}`, Jev {}{delegated_note}).",
                    state.history.len() + 1,
                    options.lane,
                    if options.jev { "on" } else { "off" }
                );
                let pr = command(
                    &workdir,
                    "gh",
                    &[
                        "pr", "create", "--draft", "--head", &branch, "--title", &title, "--body",
                        &body,
                    ],
                )?;
                println!("pull request {}", pr.trim());
            } else {
                println!(
                    "committed on {branch}; pass --open-pr to push it and open a draft pull request"
                );
            }
        }
    }

    println!(
        "\nsteps {} · generation {} in / {} out tokens · jev {} calls, {} input tokens · {:.0}s",
        state.history.len() + usize::from(matches!(ended, Ended::Finished { .. })),
        door.tally.usage.input_tokens,
        door.tally.usage.output_tokens,
        judge.calls,
        judge.input_tokens,
        started.elapsed().as_secs_f64()
    );
    let cost = &usage["cost"]["amount_usd"];
    let components = &usage["components"];
    println!(
        "cost    total {} · generation {} · jev {} · delegate {}",
        usd(cost),
        usd(&components["generation"]["cost_usd"]),
        usd(&components["jev"]["cost_usd"]),
        usd(&components["delegate"]["cost_usd"])
    );
    if let Some(delegated) = &delegated {
        let summary = &delegated.report.summary;
        println!(
            "delegate {} · {} turns · {} API calls · {} input / {} output tokens · {:.0}s",
            delegated.report.status,
            summary
                .num_turns
                .map_or("unknown".to_string(), |n| n.to_string()),
            summary
                .api_calls
                .map_or("unknown".to_string(), |n| n.to_string()),
            components["delegate"]["total_input_tokens"],
            components["delegate"]["output_tokens"],
            delegated.report.milliseconds as f64 / 1000.0
        );
    }
    println!("checkout {}", workdir.display());
    println!("state    {}", saved.display());
    println!(
        "trace    {}",
        run_dir.join("trajectory.atif.json").display()
    );
    Ok(())
}

/// The delegate's closing directions in issue mode.
const ISSUE_DIRECTIONS: &str = "Resolve the issue in the current working \
directory, a fresh clone on a new branch. Make the change, add the tests the \
issue asks for, and run the test suite. Do not commit, push, or create \
branches: the host does that when you finish. End with a short summary of \
what you changed and how you checked it.";

/// A dollar amount, or `unknown` for a null.
fn usd(value: &serde_json::Value) -> String {
    value
        .as_f64()
        .map_or("unknown".to_string(), |usd| format!("${usd:.4}"))
}

/// The fields `gh issue view --json` returns that the state uses.
#[derive(Deserialize)]
struct GhIssue {
    url: String,
    title: String,
    body: String,
    labels: Vec<GhLabel>,
}

#[derive(Deserialize)]
struct GhLabel {
    name: String,
}

fn fetch_issue(url: &str) -> Result<Issue, String> {
    let json = command(
        Path::new("."),
        "gh",
        &["issue", "view", url, "--json", "url,title,body,labels"],
    )?;
    let issue: GhIssue =
        serde_json::from_str(&json).map_err(|error| format!("unexpected gh output: {error}"))?;
    Ok(Issue {
        url: issue.url,
        title: issue.title,
        body: issue.body,
        labels: issue.labels.into_iter().map(|label| label.name).collect(),
    })
}

/// `(owner/name, number)` from `https://github.com/owner/name/issues/N`.
fn parse_issue_url(url: &str) -> Option<(String, u64)> {
    let path = url.strip_prefix("https://github.com/")?;
    let mut parts = path.split('/');
    let owner = parts.next().filter(|part| !part.is_empty())?;
    let name = parts.next().filter(|part| !part.is_empty())?;
    if parts.next()? != "issues" {
        return None;
    }
    let number = parts.next()?.split(['#', '?']).next()?.parse().ok()?;
    Some((format!("{owner}/{name}"), number))
}

/// Runs a host command and returns its stdout, or its stderr as the error.
fn command(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
