//! `openagents chat work` with the briefed agent (#11258), the default
//! engine: each issue goes to `scripts/work/work_issue.py`, which builds
//! the briefing, runs the briefed agent with `verify`, replays the checks
//! on the actual diff, falls back to bare Claude Code when it must (and
//! says why), and lands through the landing queue or opens a pull request.
//!
//! - On this computer, the engine runs here, on this computer's own Claude
//!   Code login, in a worktree of each issue. Its scripts are built into
//!   this binary, so it works in any repository.
//! - `--on boat|gce` (any cloud placement): the issues go to openagents.com
//!   (`POST /v1/work`) under this computer's sign-in (`coder login`), and
//!   run on a work host with the account's own Claude sign-in from
//!   Settings > Claude. Their progress streams here.
//!
//! `--engine bare` keeps the Coder issue flow ([`super::work`]).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use super::{Failure, event, failed};
use crate::EXIT_FAILURE;
use crate::out::Output;

/// The engine's scripts, as this binary was built with them.
const SCRIPTS: [(&str, &str); 4] = [
    (
        "scripts/work/work_issue.py",
        include_str!("../../../scripts/work/work_issue.py"),
    ),
    (
        "scripts/work/local-exec",
        include_str!("../../../scripts/work/local-exec"),
    ),
    (
        "scripts/bench/briefed-ab/briefing.py",
        include_str!("../../../scripts/bench/briefed-ab/briefing.py"),
    ),
    (
        "scripts/bench/briefed-ab/common.py",
        include_str!("../../../scripts/bench/briefed-ab/common.py"),
    ),
];

/// How a green change lands, as the engine reads it.
pub(super) fn land_word(given: Option<&str>, policy_main: bool) -> Result<&'static str, String> {
    match given.map(str::trim) {
        None => Ok(if policy_main { "queue" } else { "pr" }),
        Some("main" | "queue") => Ok("queue"),
        Some("pr" | "pull_request" | "pull-request") => Ok("pr"),
        Some("none") => Ok("none"),
        Some(other) => Err(format!(
            "--land is `queue` (or `main`), `pr`, or `none`, not `{other}`"
        )),
    }
}

/// The engine's scripts, written once per build under
/// `~/.openagents/work/engine/VERSION`; the driver's path.
fn engine_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is not set")?;
    engine_dir_in(&home)
}

fn engine_dir_in(home: &Path) -> Result<PathBuf, String> {
    let dir = home
        .join(".openagents")
        .join("work")
        .join("engine")
        .join(env!("CARGO_PKG_VERSION"));
    for (path, text) in SCRIPTS {
        let file = dir.join(path);
        if std::fs::read_to_string(&file).ok().as_deref() == Some(text) {
            continue;
        }
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if !path.ends_with(".py") || path.ends_with("work_issue.py") {
                let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    Ok(dir.join("scripts/work/work_issue.py"))
}

/// The briefed agent's binary: `OA_BRIEFED_AGENT`, else beside this one,
/// else on PATH; `None` lets the engine fall back to Claude Code and say so.
fn agent_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("OA_BRIEFED_AGENT") {
        return Some(PathBuf::from(path));
    }
    let beside = std::env::current_exe()
        .ok()?
        .parent()?
        .join("briefed-agent");
    if beside.exists() {
        return Some(beside);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("briefed-agent"))
            .find(|path| path.exists())
    })
}

/// One line the engine printed, as this command shows it.
fn shown(output: &Output, issue: u64, row: &Value) {
    match row["type"].as_str() {
        Some("progress") => {
            event(
                output,
                json!({"event": "work", "issue": issue, "phase": row["phase"], "text": row["text"]}),
            );
            if !output.json() {
                eprintln!("#{issue}: {}", row["text"].as_str().unwrap_or_default());
            }
        }
        Some("result") => {}
        _ => {}
    }
}

/// The result as one `issue` record and line.
fn finished(output: &Output, issue: u64, result: &Value) -> Value {
    let landed = &result["landed"];
    let outcome = if !result["ok"].as_bool().unwrap_or(false) {
        "failed"
    } else {
        match landed["how"].as_str() {
            Some("queue") => "queued",
            Some("pr") => "pull_request",
            _ => "committed",
        }
    };
    let cost = result["cost_usd"]
        .as_f64()
        .map_or_else(|| "cost unknown".to_owned(), |c| format!("${c:.2}"));
    let secs = result["secs"].as_f64().unwrap_or(0.0);
    let engine = match result["engine"].as_str() {
        Some("briefed") => "briefed agent",
        Some("bare") => "Claude Code",
        _ => "no engine",
    };
    let checks = result["checks"].as_array().map_or(0, Vec::len);
    let passed = result["checks"]
        .as_array()
        .map_or(0, |c| c.iter().filter(|c| c["ok"] == true).count());
    let mut message = format!("{engine}, {secs:.0} s, {cost}, checks {passed}/{checks}");
    if let Some(url) = landed["url"].as_str() {
        message.push_str(&format!(", {url}"));
    } else if let Some(entry) = landed["entry"]["id"].as_str() {
        message.push_str(&format!(", landing queue entry {entry}"));
    } else if let Some(commit) = result["commit"].as_str() {
        message.push_str(&format!(", commit {}", &commit[..commit.len().min(10)]));
    }
    if let Some(why) = result["escalated"].as_str() {
        message.push_str(&format!(" (handed to Claude Code: {why})"));
    }
    if let Some(error) = result["error"].as_str() {
        message.push_str(&format!(". {error}"));
    }
    let record = json!({"event": "issue", "issue": issue, "outcome": outcome,
        "message": message, "engine": result["engine"], "result": result});
    event(output, record.clone());
    if !output.json() {
        println!("#{issue}: {outcome}. {message}");
        let _ = std::io::stdout().flush();
    }
    record
}

/// Works `numbers` here, `parallel` at once, each with the engine.
pub(super) fn work_here(
    output: &Output,
    top: &Path,
    repository: &str,
    numbers: &[u64],
    parallel: u64,
    land: &str,
) -> Result<u8, Failure> {
    let driver = engine_dir().map_err(failed)?;
    let agent = agent_binary();
    event(
        output,
        json!({"event": "queue", "repository": repository, "issues": numbers,
            "parallel": parallel, "engine": "briefed"}),
    );
    if !output.json() {
        eprintln!(
            "The briefed agent works {} issue{} of {repository}, {parallel} at a time{}",
            numbers.len(),
            if numbers.len() == 1 { "" } else { "s" },
            if agent.is_none() {
                " (its binary isn't installed here, so Claude Code works them)"
            } else {
                ""
            }
        );
    }
    let queue = std::sync::Arc::new(std::sync::Mutex::new(
        numbers
            .iter()
            .copied()
            .collect::<std::collections::VecDeque<u64>>(),
    ));
    let (sender, receiver) = std::sync::mpsc::channel::<(u64, Value)>();
    let mut workers = Vec::new();
    for _ in 0..parallel {
        let (queue, sender) = (std::sync::Arc::clone(&queue), sender.clone());
        let (driver, agent, top, repository, land) = (
            driver.clone(),
            agent.clone(),
            top.to_path_buf(),
            repository.to_owned(),
            land.to_owned(),
        );
        workers.push(std::thread::spawn(move || {
            loop {
                let next = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                let Some(issue) = next else {
                    return;
                };
                let mut command = Command::new("python3");
                command
                    .arg(&driver)
                    .args([
                        "--repo",
                        &repository,
                        "--issue",
                        &issue.to_string(),
                        "--land",
                        &land,
                    ])
                    .arg("--checkout")
                    .arg(&top)
                    .current_dir(&top)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null());
                match &agent {
                    Some(path) => command.env("OA_BRIEFED_AGENT", path),
                    None => command.env("OA_BRIEFED_AGENT", "/nonexistent/briefed-agent"),
                };
                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(error) => {
                        let _ = sender.send((
                            issue,
                            json!({"type": "result", "ok": false,
                                "error": format!("python3 didn't start: {error}")}),
                        ));
                        continue;
                    }
                };
                let mut result = None;
                if let Some(stdout) = child.stdout.take() {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        let Ok(row) = serde_json::from_str::<Value>(&line) else {
                            continue;
                        };
                        if row["type"] == "result" {
                            result = Some(row.clone());
                        }
                        let _ = sender.send((issue, row));
                    }
                }
                let _ = child.wait();
                if result.is_none() {
                    let _ = sender.send((
                        issue,
                        json!({"type": "result", "ok": false,
                            "error": "the engine stopped without a result"}),
                    ));
                }
            }
        }));
    }
    drop(sender);
    let mut results = Vec::new();
    for (issue, row) in receiver {
        if row["type"] == "result" {
            results.push(finished(output, issue, &row));
        } else {
            shown(output, issue, &row);
        }
    }
    for worker in workers {
        let _ = worker.join();
    }
    let good = results.iter().filter(|r| r["outcome"] != "failed").count();
    event(
        output,
        json!({"event": "queue_done", "issues": results.len(), "landed": good, "engine": "briefed"}),
    );
    if !output.json() {
        eprintln!(
            "The briefed agent finished {good} of {} issue(s).",
            results.len()
        );
    }
    Ok(if good == results.len() {
        0
    } else {
        EXIT_FAILURE
    })
}

/// Sends `numbers` to openagents.com's work hosts and follows them.
pub(super) fn work_cloud(
    output: &Output,
    repository: &str,
    numbers: &[u64],
    land: &str,
) -> Result<u8, Failure> {
    let saved = crate::mac::saved(None).map_err(Failure::Refused)?;
    let http = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| failed(format!("Couldn't start the web client: {e}")))?;
    let call = |request: reqwest::blocking::RequestBuilder| -> Result<Value, String> {
        let response = request
            .bearer_auth(saved.token())
            .send()
            .map_err(|_| "The website couldn't be reached.".to_owned())?;
        let status = response.status();
        let body: Value = response.json().unwrap_or(Value::Null);
        if status.is_success() {
            Ok(body)
        } else if status.as_u16() == 401 {
            Err("This sign-in stopped working. Sign in again with: coder login".into())
        } else {
            Err(body["error"]["message"]
                .as_str()
                .map_or_else(|| format!("The website answered {status}."), str::to_owned))
        }
    };
    let mut runs = Vec::new();
    for &issue in numbers {
        let body = json!({"repo": repository, "issue": issue, "land": land,
            "engine": "briefed", "source": "cli"});
        let run =
            call(http.post(format!("{}/v1/work", saved.origin)).json(&body)).map_err(failed)?;
        let id = run["id"].as_str().unwrap_or_default().to_owned();
        event(
            output,
            json!({"event": "work_run", "issue": issue, "id": id,
                "url": format!("{}{}", saved.origin, run["url"].as_str().unwrap_or_default())}),
        );
        if !output.json() {
            eprintln!(
                "#{issue}: sent to the cloud as {id} ({}{})",
                saved.origin,
                run["url"].as_str().unwrap_or_default()
            );
        }
        runs.push((issue, id, 0usize));
    }
    let mut results = Vec::new();
    while !runs.is_empty() {
        let mut still = Vec::new();
        for (issue, id, after) in runs {
            let read = call(http.get(format!(
                "{}/v1/work/{id}?after={after}&wait=20",
                saved.origin
            )));
            let Ok(view) = read else {
                std::thread::sleep(Duration::from_secs(5));
                still.push((issue, id, after));
                continue;
            };
            for line in view["lines"].as_array().into_iter().flatten() {
                shown(
                    output,
                    issue,
                    &json!({"type": "progress", "phase": line["phase"], "text": line["text"]}),
                );
            }
            let next = view["next"].as_u64().map_or(after, |n| n as usize);
            if matches!(
                view["state"].as_str(),
                Some("done" | "failed" | "cancelled")
            ) {
                let r = &view["result"];
                let result = json!({
                    "ok": view["state"] == "done", "engine": r["engine"], "escalated": r["escalated"],
                    "cost_usd": r["cost_usd"], "secs": r["secs"], "checks": r["checks"],
                    "commit": r["commit"],
                    "landed": {"url": r["pr"], "entry": {"id": r["landing"]}},
                    "error": view["why"],
                });
                results.push(finished(output, issue, &result));
            } else {
                still.push((issue, id, next));
            }
        }
        runs = still;
    }
    let good = results.iter().filter(|r| r["outcome"] != "failed").count();
    Ok(if good == results.len() {
        0
    } else {
        EXIT_FAILURE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn land_words() {
        assert_eq!(land_word(None, true), Ok("queue"));
        assert_eq!(land_word(None, false), Ok("pr"));
        assert_eq!(land_word(Some("main"), false), Ok("queue"));
        assert_eq!(land_word(Some("pr"), true), Ok("pr"));
        assert_eq!(land_word(Some("none"), true), Ok("none"));
        assert!(land_word(Some("push"), true).is_err());
    }

    #[test]
    fn the_engine_scripts_are_written_where_the_driver_finds_its_briefing() {
        let home = tempfile::tempdir().unwrap();
        let driver = engine_dir_in(home.path()).unwrap();
        assert!(driver.ends_with("scripts/work/work_issue.py"));
        let root = driver.parent().unwrap().parent().unwrap().parent().unwrap();
        assert!(root.join("scripts/bench/briefed-ab/briefing.py").exists());
        assert!(root.join("scripts/work/local-exec").exists());
    }
}
