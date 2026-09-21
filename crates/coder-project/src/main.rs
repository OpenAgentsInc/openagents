//! Operator entry point for project dispatch and retained artifact inspection.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use coder::Survey;
use coder_project::{Assignment, artifact, dispatch};

const USAGE: &str = "Usage:
  coder-project run-one REPOSITORY ASSIGNMENT.json NEW_OUTPUT_DIRECTORY
  coder-project inspect REPOSITORY WORKTREE BASE OWNED_PATH...
  coder-project project CONFIGURATION.json STATE_DIRECTORY [--watch]
  coder-project snapshot REPOSITORY OWNER REPO PROJECT_NUMBER
  coder-project pin-config TEMPLATE.json NEW_CONFIGURATION.json

run-one executes one explicitly prepared task through Coder. It requires the
operator's capability approval and program authority. Results require review.
inspect checks a committed scratch artifact without running its code or tests.
project polls the scoped project and refills prepared tasks; host review remains required.
";

fn read_assignment(path: &Path) -> Result<Assignment, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 {
        return Err("assignment exceeds 128 KiB".into());
    }
    let assignment: Assignment = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    assignment.validate()?;
    Ok(assignment)
}

fn record(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

async fn execute(args: &[String]) -> Result<u8, String> {
    match args.first().map(String::as_str) {
        None | Some("--help" | "-h") => {
            print!("{USAGE}");
            Ok(0)
        }
        Some("run-one") if args.len() == 4 => {
            let repo = Path::new(&args[1])
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let assignment = read_assignment(Path::new(&args[2]))?;
            let output = PathBuf::from(&args[3]);
            let parent = output
                .parent()
                .ok_or("output directory needs an existing parent")?
                .canonicalize()
                .map_err(|e| e.to_string())?;
            if parent.starts_with(&repo) {
                return Err("execution evidence must be outside the delegated repository".into());
            }
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&output)
                .map_err(|e| e.to_string())?;
            record(&output.join("assignment.json"), &assignment)?;
            let survey = Survey::read(Some(&repo), &repo);
            let trace = output.join("trace.atif.jsonl");
            let report = tokio::select! {
                result = dispatch(&repo, survey, &assignment, &trace) => result?,
                _ = tokio::signal::ctrl_c() => return Err("dispatch interrupted; reconcile the retained attempt before retrying".into()),
            };
            record(&output.join("result.json"), &report)?;
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|e| e.to_string())?
            );
            Ok(if report.answered { 0 } else { 3 })
        }
        Some("inspect") if args.len() >= 5 => {
            let result = artifact::inspect(
                Path::new(&args[1]),
                Path::new(&args[2]),
                &args[3],
                &args[4..],
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Some("project") if args.len() == 3 || (args.len() == 4 && args[3] == "--watch") => {
            coder_project::controller::run(
                Path::new(&args[1]),
                Path::new(&args[2]),
                args.len() == 4,
            )
            .await?;
            Ok(0)
        }
        Some("pin-config") if args.len() == 3 => {
            coder_project::controller::pin_configuration(Path::new(&args[1]), Path::new(&args[2]))
                .await?;
            Ok(0)
        }
        Some("snapshot") if args.len() == 5 => {
            let scope = coder_project::github::Scope {
                owner: args[2].clone(),
                repository: args[3].clone(),
                project: args[4].parse().map_err(|_| "invalid project number")?,
            };
            let snapshot = coder_project::github::fetch(&scope, Path::new(&args[1])).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        _ => Err(USAGE.into()),
    }
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match execute(&args).await {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("coder-project: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
