//! `microluna`: run one Luna session in a workspace, on the Codex login.
//!
//! ```text
//! microluna [--workspace DIR] [--model SLUG] [--effort LEVEL]
//!           [--max-turns N] [--evidence FILE]... [--no-trace] TASK...
//! ```
//!
//! The session's steps print to standard error as they happen. The typed
//! finish, the usage, and the cost print to standard output as JSON. The
//! ATIF trace goes to `~/.openagents/traces/` unless `--no-trace`.

use std::path::PathBuf;
use std::process::ExitCode;

use microluna::codex::{CodexTransport, Login};
use microluna::{Brief, Config, Ending, Evidence, Recorder, Workspace, run};
use serde_json::json;

const USAGE: &str = "usage: microluna [--workspace DIR] [--model SLUG] [--effort LEVEL] \
[--max-turns N] [--evidence FILE]... [--no-trace] TASK...";

struct Args {
    workspace: PathBuf,
    model: Option<String>,
    effort: Option<String>,
    max_turns: Option<usize>,
    evidence: Vec<PathBuf>,
    trace: bool,
    task: String,
}

fn args() -> Result<Args, String> {
    let mut args = Args {
        workspace: PathBuf::from("."),
        model: None,
        effort: None,
        max_turns: None,
        evidence: Vec::new(),
        trace: true,
        task: String::new(),
    };
    let mut words = Vec::new();
    let mut raw = std::env::args().skip(1);
    while let Some(arg) = raw.next() {
        let mut value = |flag: &str| raw.next().ok_or(format!("{flag} needs a value"));
        match arg.as_str() {
            "--workspace" => args.workspace = PathBuf::from(value("--workspace")?),
            "--model" => args.model = Some(value("--model")?),
            "--effort" => args.effort = Some(value("--effort")?),
            "--max-turns" => {
                args.max_turns = Some(
                    value("--max-turns")?
                        .parse()
                        .map_err(|_| "--max-turns takes a number".to_string())?,
                );
            }
            "--evidence" => args.evidence.push(PathBuf::from(value("--evidence")?)),
            "--no-trace" => args.trace = false,
            "-h" | "--help" => return Err(USAGE.to_string()),
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}\n{USAGE}")),
            _ => words.push(arg),
        }
    }
    args.task = words.join(" ");
    if args.task.trim().is_empty() {
        return Err(USAGE.to_string());
    }
    Ok(args)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = match args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let workspace = match Workspace::new(&args.workspace) {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!("microluna: can't use {}: {error}", args.workspace.display());
            return ExitCode::from(2);
        }
    };
    let mut brief = Brief::task(&args.task);
    for path in &args.evidence {
        match std::fs::read_to_string(path) {
            Ok(text) => brief.evidence.push(Evidence {
                label: path.display().to_string(),
                text,
            }),
            Err(error) => {
                eprintln!("microluna: can't read evidence {}: {error}", path.display());
                return ExitCode::from(2);
            }
        }
    }
    let session_id = atif::log::session_id(atif::now_ms());
    let mut config = Config::luna(&format!("microluna-{session_id}"));
    if let Some(model) = args.model {
        config.model = model;
    }
    config.effort = args.effort;
    if let Some(max_turns) = args.max_turns {
        config.max_turns = max_turns;
    }

    let Some(login) = Login::default_path() else {
        eprintln!("microluna: no home directory to find the Codex login in");
        return ExitCode::from(2);
    };
    let transport = match CodexTransport::new(login, &session_id) {
        Ok(transport) => transport,
        Err(error) => {
            eprintln!("microluna: {error}");
            return ExitCode::from(3);
        }
    };

    let mut recorder = Recorder::new().echoing();
    if args.trace
        && let Some(dir) = atif::log::default_dir()
    {
        let session = atif::Session::opening(
            &session_id,
            &config.model,
            "codex-login",
            &workspace.root().display().to_string(),
            env!("CARGO_PKG_VERSION"),
        );
        match atif::Log::create(&dir, &session) {
            Ok(log) => recorder = recorder.logging(log),
            Err(error) => eprintln!("microluna: not tracing: {error}"),
        }
    }

    let report = run(&transport, &workspace, &brief, &config, &mut recorder).await;
    let state = match report.ending {
        Ending::Finished => atif::log::ENDED,
        _ => atif::log::INTERRUPTED,
    };
    recorder.close(state);
    for fault in recorder.faults() {
        eprintln!("microluna: {fault}");
    }
    let ending = match &report.ending {
        Ending::Finished => "finished".to_string(),
        Ending::Stopped => "stopped".to_string(),
        Ending::TurnLimit => "turn_limit".to_string(),
        Ending::Transport(why) => format!("transport: {why}"),
    };
    let summary = json!({
        "ending": ending,
        "finish": report.finish,
        "model": config.model,
        "turns": report.turns,
        "calls": report.calls,
        "usage": {
            "input": report.usage.input,
            "cached": report.usage.cached,
            "output": report.usage.output,
            "reasoning": report.usage.reasoning,
        },
        "cost_usd": report.cost_usd,
        "seconds": report.milliseconds as f64 / 1000.0,
        "trace": recorder.path().map(|path| path.display().to_string()),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).unwrap_or_default()
    );
    match report.ending {
        Ending::Finished => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    }
}
