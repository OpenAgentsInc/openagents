//! `coder-one`: the command line.
//!
//! ```text
//! coder-one doctor         report which credentials resolve, and from where
//! coder-one <issue-url>    fetch the issue and print the initial state
//! ```
//!
//! Running the loop against live Jev, generation, and a checkout is the
//! next step in issue #9531; this binary stops at the initial state.

use std::process::{Command, ExitCode};

use coder_one::credentials;
use coder_one::{Environment, Issue, State};
use serde::Deserialize;

const USAGE: &str = "usage: coder-one doctor | coder-one <github-issue-url>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command] if command == "doctor" => doctor(),
        [url] if url.starts_with("https://github.com/") && url.contains("/issues/") => show(url),
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

/// Fetches the issue with `gh` and prints the state a run starts from.
fn show(url: &str) -> Result<(), String> {
    let output = Command::new("gh")
        .args(["issue", "view", url, "--json", "url,title,body,labels"])
        .output()
        .map_err(|error| format!("cannot run gh: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh issue view failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let issue: GhIssue = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("unexpected gh output: {error}"))?;
    let repository = repository_of(&issue.url).ok_or("cannot read the repository from the URL")?;

    let state = State::new(
        Environment {
            repository,
            workdir: String::new(),
            os: std::env::consts::OS.to_string(),
        },
        Issue {
            url: issue.url,
            title: issue.title,
            body: issue.body,
            labels: issue.labels.into_iter().map(|label| label.name).collect(),
        },
    );
    let json = serde_json::to_string_pretty(&state).map_err(|error| error.to_string())?;
    println!("{json}");
    Ok(())
}

/// `owner/name` from `https://github.com/owner/name/issues/N`.
fn repository_of(url: &str) -> Option<String> {
    let path = url.strip_prefix("https://github.com/")?;
    let mut parts = path.split('/');
    let owner = parts.next().filter(|part| !part.is_empty())?;
    let name = parts.next().filter(|part| !part.is_empty())?;
    Some(format!("{owner}/{name}"))
}
