//! `coder-one`: the command line.
//!
//! ```text
//! coder-one doctor
//! coder-one <issue-url> [--lane free|flash|pro] [--max-steps N]
//!                       [--timeout SECONDS] [--no-jev] [--deep] [--open-pr]
//! coder-one --version
//! coder-one episode doctor --contract openagents.coder.episode.v1
//! coder-one episode run --instruction-file F --output-dir D --contract C [--model M]
//! ```
//!
//! The `episode` commands implement the Terminal-Bench harness's headless
//! contract; `coder_one::episode` documents them.
//!
//! A run clones the issue's repository fresh under
//! `~/.openagents/coder-one/runs/`, works on a new branch there, and
//! streams every step to the console. When the agent finishes with
//! changes, the host commits them; `--open-pr` also pushes the branch and
//! opens a draft pull request.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder_one::agent::INSTRUCTIONS;
use coder_one::credentials;
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
       coder-one --version
       coder-one episode doctor --contract openagents.coder.episode.v1
       coder-one episode run --instruction-file F --output-dir D --contract C [--model M]";

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

struct Options {
    lane: String,
    max_steps: usize,
    timeout: Duration,
    jev: bool,
    deep: bool,
    open_pr: bool,
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
    };

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

    let saved = run_dir.join("state.json");
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(&saved, json);
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
                let body = format!(
                    "{summary}\n\nCloses {url}\n\n---\nOpened by coder-one in {} steps (lane `{}`, Jev {}).",
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
    println!("checkout {}", workdir.display());
    println!("state    {}", saved.display());
    Ok(())
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
