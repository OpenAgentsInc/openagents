//! `openagents boat`: build and test this checkout's change on a Boat
//! sandbox instead of this machine.
//!
//! One sandbox per NAME (its id lives in `~/.openagents/boat/NAME`). Each run
//! resets that sandbox's clone of OpenAgentsInc/openagents to `origin/main`,
//! applies this checkout's diff against `origin/main` (uploaded through the
//! files API, so any size fits), runs the command with `CARGO_INCREMENTAL=0`,
//! prints its output as it arrives, and exits with its exit code.
//! `scripts/boat-run.sh` is a thin wrapper over this.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use boat::{CommandFrame, Signal, WaitOptions, models::*, shell_quote};
use serde_json::json;

use crate::Output;
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents boat COMMAND
  run NAME [--size small|default|large] -- CMD [ARGS...]
                 Run CMD on the Boat sandbox NAME against this checkout's
                 change: the sandbox's clone is reset to origin/main, this
                 checkout's diff against origin/main is applied, and CMD runs
                 there with CARGO_INCREMENTAL=0. Output prints as it arrives;
                 the exit code is CMD's. Ctrl-C kills CMD on the sandbox.
  stop NAME      Stop the sandbox NAME; a stopped sandbox costs nothing.
  delete NAME|ID Delete the sandbox NAME, or the sandbox ID (bx_...), such as
                 one a Boat issue run kept stopped after it failed.
  list [--all]   This account's sandboxes: name, ID, state, machine size,
                 created, and when Boat archives it. Archived ones only with
                 --all. A running sandbox bills; stopped and archived do not.
The first run for a NAME creates a sandbox (default large, four-hour
lifetime, no account credentials) and keeps its id in ~/.openagents/boat/NAME.
The key is BOAT_API_KEY, or Secret Manager oa-boat-api-key through gcloud
(our own sandbox service, docs/cloud/oa-boat.md).
With OA_ARTIFACT_BUCKET and OA_ARTIFACT_ISSUE set, the patch, the logs and the
exit status are published with scripts/cloud/publish-artifacts.py.";

/// What each command above does, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    // Boat bills machine time.
    Declared::computer("run", Effect::Spends),
    Declared::computer("stop", Effect::Publishes),
    Declared::computer("delete", Effect::Publishes),
    Declared::computer("list", Effect::ReadOnly),
];

const PATCH_PATH: &str = "/tmp/oa-change.patch";
const TTL_SECONDS: i64 = 4 * 3600;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("boat", "a command is required", USAGE);
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "run" => match parse_run(rest) {
            Ok(Parsed::Run(request)) => crate::runtime().block_on(run_command(output, request)),
            Ok(Parsed::Stop(name)) => crate::runtime().block_on(stop(output, &name)),
            Err(message) => output.usage("boat", &message, USAGE),
        },
        "stop" => match rest {
            [name] if valid_name(name) => crate::runtime().block_on(stop(output, name)),
            _ => output.usage("boat", "stop takes one NAME", USAGE),
        },
        "delete" => match rest {
            [target] if valid_name(target) => crate::runtime().block_on(delete(output, target)),
            _ => output.usage("boat", "delete takes one NAME or sandbox ID", USAGE),
        },
        "list" => match rest {
            [] => crate::runtime().block_on(list(output, false)),
            [flag] if flag == "--all" => crate::runtime().block_on(list(output, true)),
            _ => output.usage("boat", "list takes only --all", USAGE),
        },
        other => output.usage("boat", &format!("unknown command `{other}`"), USAGE),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RunRequest {
    name: String,
    size: String,
    command: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Parsed {
    Run(RunRequest),
    /// `run NAME --stop`, the form `scripts/boat-run.sh` has always taken.
    Stop(String),
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn parse_run(words: &[String]) -> Result<Parsed, String> {
    let Some((name, rest)) = words.split_first() else {
        return Err("run needs a NAME".into());
    };
    if !valid_name(name) {
        return Err(format!(
            "`{name}` is not a sandbox name (letters, digits, - _ .)"
        ));
    }
    let split = rest.iter().position(|word| word == "--");
    let (options, command) = match split {
        Some(at) => (&rest[..at], &rest[at + 1..]),
        None => (rest, &[][..]),
    };
    let mut size = "large".to_owned();
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--stop" if command.is_empty() => return Ok(Parsed::Stop(name.clone())),
            "--size" => {
                let value = options.next().ok_or("--size needs a value")?;
                if !matches!(value.as_str(), "small" | "default" | "large") {
                    return Err(format!("--size is small, default or large, not `{value}`"));
                }
                value.clone_into(&mut size);
            }
            other if split.is_none() => {
                return Err(format!("put the command after `--` (`{other}` came first)"));
            }
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    if command.is_empty() {
        return Err("no command after `--`".into());
    }
    Ok(Parsed::Run(RunRequest {
        name: name.clone(),
        size,
        command: command.to_vec(),
    }))
}

fn state_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents/boat")
}

/// The shell script the sandbox runs: reset the clone, apply the change,
/// run the command.
fn remote_script(command: &[String]) -> String {
    let command = command
        .iter()
        .map(|word| shell_quote(word))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "set -e; cd ~; [ -d openagents/.git ] || git clone -q https://github.com/OpenAgentsInc/openagents.git; \
         cd openagents; git fetch -q origin; git reset -q --hard origin/main; git clean -qfd; \
         [ -s {PATCH_PATH} ] && git apply {PATCH_PATH}; export CARGO_INCREMENTAL=0; {command}"
    )
}

/// The process exit code for a command's last frame.
fn exit_code(last: Option<&CommandFrame>) -> u8 {
    match last {
        Some(CommandFrame::Exit {
            exit_code: Some(code),
            ..
        }) => u8::try_from(*code).unwrap_or(1),
        _ => 1,
    }
}

fn is_not_found(error: &boat::Error) -> bool {
    matches!(error, boat::Error::Api(api) if api.status.as_u16() == 404)
}

/// This checkout's diff against `origin/main`, new files included.
fn checkout_patch() -> Result<(PathBuf, Vec<u8>), String> {
    let top = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !top.status.success() {
        return Err("run it inside a checkout of OpenAgentsInc/openagents".into());
    }
    let top = PathBuf::from(String::from_utf8_lossy(&top.stdout).trim());
    // Intent-to-add makes new files part of the diff without staging them.
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(&top)
        .args(["add", "-N", "."])
        .output();
    let diff = std::process::Command::new("git")
        .arg("-C")
        .arg(&top)
        .args(["diff", "--binary", "origin/main"])
        .output()
        .map_err(|e| format!("git diff: {e}"))?;
    if !diff.status.success() {
        return Err("git diff against origin/main failed; fetch origin first".into());
    }
    Ok((top, diff.stdout))
}

/// The sandbox for `name`: the remembered one, resumed when stopped, or a
/// new one.
async fn sandbox_for(
    client: &boat::Client,
    name: &str,
    size: &str,
    idfile: &Path,
) -> Result<String, String> {
    let remembered = std::fs::read_to_string(idfile)
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|id| !id.is_empty());
    if let Some(id) = remembered {
        let info = client
            .get(&GetParams {
                sandbox_id: id.clone(),
                ..Default::default()
            })
            .await;
        match info {
            Ok(info) => match info.sandbox.state.as_str() {
                "stopped" | "archived" => {
                    client
                        .resume(&ResumeParams {
                            sandbox_id: id.clone(),
                            ..Default::default()
                        })
                        .await
                        .map_err(|e| format!("resume {id}: {e}"))?;
                    return Ok(id);
                }
                "error" | "deleted" | "deleting" => {}
                _ => return Ok(id),
            },
            Err(error) if is_not_found(&error) => {}
            Err(error) => return Err(format!("look up {id}: {error}")),
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let created = client
        .create(&CreateParams {
            idempotency_key: Some(format!("oa-boat-run-{name}-{nonce}")),
            body: Some(CreateSandboxRequest {
                type_: Some(size.to_owned()),
                ttl_seconds: boat::Nullable::Value(TTL_SECONDS),
                no_env: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("could not create a sandbox: {e}"))?;
    let id = created.sandbox.id;
    std::fs::write(idfile, format!("{id}\n")).map_err(|e| format!("{}: {e}", idfile.display()))?;
    Ok(id)
}

async fn run_command(output: &Output, request: RunRequest) -> u8 {
    match run_inner(output, &request).await {
        Ok(code) => code,
        Err(message) => output.fail("boat run", &message),
    }
}

async fn run_inner(output: &Output, request: &RunRequest) -> Result<u8, String> {
    let state = state_dir();
    std::fs::create_dir_all(&state).map_err(|e| format!("{}: {e}", state.display()))?;
    let (top, patch) = checkout_patch()?;
    let patch_file = state.join(format!("{}.patch", request.name));
    std::fs::write(&patch_file, &patch).map_err(|e| format!("{}: {e}", patch_file.display()))?;

    let client = boat::Client::from_env()
        .await
        .map_err(|e| format!("no Boat key: {e} (set BOAT_API_KEY)"))?;
    let id = sandbox_for(
        &client,
        &request.name,
        &request.size,
        &state.join(&request.name),
    )
    .await?;
    client
        .wait_until_ready(
            &id,
            &WaitOptions {
                timeout: Duration::from_secs(300),
                interval: Duration::from_secs(3),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("{id} did not become ready: {e}"))?;
    let written = client
        .write_bytes(&id, PATCH_PATH, &patch)
        .await
        .map_err(|e| format!("the patch upload failed: {e}"))?;
    if written.type_ != "file.written" {
        return Err("the patch upload failed".into());
    }
    let process = client
        .exec_detached(
            &id,
            CommandRequest {
                command: remote_script(&request.command),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| {
            if e.may_be_running() {
                format!("the command may have started on {id}; not resending it: {e}")
            } else {
                format!("the command did not start: {e}")
            }
        })?;
    eprintln!(
        "boat: {} on {id}, process {}",
        request.name, process.process_id
    );

    let mut follower = client
        .follow_command(
            &id,
            process.process_id,
            WaitOptions {
                timeout: boat::follow::DEFAULT_FOLLOW_TIMEOUT,
                interval: Duration::from_secs(2),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    let keep = output.json() || std::env::var_os("OA_ARTIFACT_BUCKET").is_some();
    let (mut stdout_log, mut stderr_log) = (String::new(), String::new());
    let mut last = None;
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    loop {
        let frame = tokio::select! {
            frame = follower.next() => frame.map_err(|e| format!("following {id}: {e}"))?,
            _ = &mut interrupt => {
                let killed = client.kill_command(&id, process.pid, Signal::Term).await;
                eprintln!(
                    "boat: interrupted; {}",
                    match killed {
                        Ok(true) => "the command on the sandbox was killed".to_owned(),
                        Ok(false) => "the command had already ended".to_owned(),
                        Err(e) => format!("killing the command failed: {e}"),
                    }
                );
                return Ok(130);
            }
        };
        let Some(frame) = frame else { break };
        match frame {
            CommandFrame::Stdout(text) => {
                if !output.json() {
                    let mut out = std::io::stdout().lock();
                    let _ = out.write_all(text.as_bytes());
                    let _ = out.flush();
                }
                if keep {
                    stdout_log.push_str(&text);
                }
            }
            CommandFrame::Stderr(text) => {
                if !output.json() {
                    let mut err = std::io::stderr().lock();
                    let _ = err.write_all(text.as_bytes());
                    let _ = err.flush();
                }
                if keep {
                    stderr_log.push_str(&text);
                }
            }
            CommandFrame::Started | CommandFrame::Unknown(_) => {}
            end => last = Some(end),
        }
    }
    let code = exit_code(last.as_ref());
    let reported = match &last {
        Some(CommandFrame::Exit { exit_code, .. }) => json!(exit_code),
        _ => json!(null),
    };
    if std::env::var_os("OA_ARTIFACT_BUCKET").is_some() {
        publish_artifacts(
            &top,
            &state,
            &patch,
            &id,
            process.process_id,
            &reported,
            &stdout_log,
            &stderr_log,
        );
    }
    if output.json() {
        output.emit(
            &json!({
                "name": request.name,
                "sandbox": id,
                "process": process.process_id,
                "exit_code": reported,
                "stdout": stdout_log,
                "stderr": stderr_log,
            }),
            |_| String::new(),
        );
    }
    Ok(code)
}

/// Publish this run's patch, logs and status when `OA_ARTIFACT_BUCKET` is
/// set. A failure is reported and the files are kept.
#[allow(clippy::too_many_arguments)]
fn publish_artifacts(
    top: &Path,
    state: &Path,
    patch: &[u8],
    sandbox: &str,
    process: i64,
    exit_code: &serde_json::Value,
    stdout: &str,
    stderr: &str,
) {
    let (Some(bucket), Some(issue)) = (
        std::env::var_os("OA_ARTIFACT_BUCKET"),
        std::env::var_os("OA_ARTIFACT_ISSUE"),
    ) else {
        eprintln!("boat: OA_ARTIFACT_ISSUE is not set; artifacts not published");
        return;
    };
    let dir = match tempfile::Builder::new().prefix("run-").tempdir_in(state) {
        Ok(dir) => dir.keep(),
        Err(e) => {
            eprintln!("boat: artifact directory: {e}");
            return;
        }
    };
    let evidence = json!({
        "sandbox": sandbox,
        "process": process,
        "status": "exited",
        "exit_code": exit_code,
    });
    let evidence = evidence.to_string();
    let files: [(&str, &[u8]); 4] = [
        ("change.patch", patch),
        ("stdout.log", stdout.as_bytes()),
        ("stderr.log", stderr.as_bytes()),
        ("evidence.json", evidence.as_bytes()),
    ];
    for (name, bytes) in files {
        let path = dir.join(name);
        if let Err(e) = std::fs::write(&path, bytes) {
            eprintln!("boat: {}: {e}", path.display());
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
    let published = std::process::Command::new("python3")
        .arg(top.join("scripts/cloud/publish-artifacts.py"))
        .arg(&dir)
        .arg("--bucket")
        .arg(bucket)
        .arg("--issue")
        .arg(issue)
        .status();
    if !published.is_ok_and(|status| status.success()) {
        eprintln!(
            "boat: artifact publication failed; retained in {}",
            dir.display()
        );
    }
}

async fn stop(output: &Output, name: &str) -> u8 {
    let idfile = state_dir().join(name);
    let Some(id) = std::fs::read_to_string(&idfile)
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|id| !id.is_empty())
    else {
        output.emit(&json!({ "name": name, "stopped": null }), |_| {
            format!("boat: no sandbox named {name}")
        });
        return 0;
    };
    let client = match boat::Client::from_env().await {
        Ok(client) => client,
        Err(e) => return output.fail("boat stop", &format!("no Boat key: {e}")),
    };
    match client
        .stop(&StopParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
    {
        Ok(_) => {
            output.emit(&json!({ "name": name, "stopped": id }), |_| {
                format!("stopped {id}")
            });
            0
        }
        Err(e) => output.fail("boat stop", &format!("{id}: {e}")),
    }
}

/// `delete NAME|ID`: a `bx_` ID as given, else the sandbox NAME remembers.
async fn delete(output: &Output, target: &str) -> u8 {
    let idfile = state_dir().join(target);
    let id = if target.starts_with("bx_") {
        target.to_owned()
    } else {
        match std::fs::read_to_string(&idfile)
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|id| !id.is_empty())
        {
            Some(id) => id,
            None => {
                output.emit(&json!({ "name": target, "deleted": null }), |_| {
                    format!("boat: no sandbox named {target}")
                });
                return 0;
            }
        }
    };
    let client = match boat::Client::from_env().await {
        Ok(client) => client,
        Err(e) => return output.fail("boat delete", &format!("no Boat key: {e}")),
    };
    let deleted = client
        .delete_sandbox(&DeleteSandboxParams {
            sandbox_id: id.clone(),
            x_ascii_confirm_delete: id.clone(),
            ..Default::default()
        })
        .await;
    match deleted {
        Ok(_) => {
            if !target.starts_with("bx_") {
                let _ = std::fs::remove_file(&idfile);
            }
            output.emit(&json!({ "name": target, "deleted": id }), |_| {
                format!("deleting {id}")
            });
            0
        }
        Err(e) if is_not_found(&e) => {
            output.emit(&json!({ "name": target, "deleted": null }), |_| {
                format!("boat: {id} does not exist")
            });
            0
        }
        Err(e) => output.fail("boat delete", &format!("{id}: {e}")),
    }
}

/// `list [--all]`: every sandbox on the account, named from
/// `~/.openagents/boat/NAME` where this computer made it.
async fn list(output: &Output, all: bool) -> u8 {
    let client = match boat::Client::from_env().await {
        Ok(client) => client,
        Err(e) => return output.fail("boat list", &format!("no Boat key: {e}")),
    };
    let mut sandboxes = Vec::new();
    let mut cursor = None;
    loop {
        let page = match client
            .sandboxes(&SandboxesParams {
                limit: Some(100),
                cursor: cursor.take(),
                ..Default::default()
            })
            .await
        {
            Ok(page) => page,
            Err(e) => return output.fail("boat list", &e.to_string()),
        };
        sandboxes.extend(page.sandboxes);
        match page.page_info {
            Some(info) if info.has_more && info.next_cursor.is_some() => {
                cursor = info.next_cursor;
            }
            _ => break,
        }
    }
    let names = local_names();
    let rows: Vec<Row> = sandboxes
        .iter()
        .filter(|sandbox| all || sandbox.state != "archived")
        .map(|sandbox| Row::of(sandbox, &names))
        .collect();
    let hidden = sandboxes.len() - rows.len();
    output.emit(
        &json!({ "sandboxes": rows, "archived_hidden": hidden }),
        |_| list_text(&rows, hidden),
    );
    0
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
struct Row {
    name: Option<String>,
    id: String,
    state: String,
    size: Option<String>,
    created: Option<String>,
    archive_after: Option<String>,
    bills: bool,
}

impl Row {
    fn of(sandbox: &Sandbox, names: &std::collections::BTreeMap<String, String>) -> Self {
        Self {
            name: names.get(&sandbox.id).cloned(),
            id: sandbox.id.clone(),
            state: sandbox.state.clone(),
            size: sandbox.type_.clone(),
            created: sandbox.created_at.as_ref().cloned(),
            archive_after: sandbox.archive_after.as_ref().cloned(),
            bills: !matches!(sandbox.state.as_str(), "stopped" | "archived" | "deleted"),
        }
    }
}

/// Sandbox id -> the NAME this computer keeps it under.
fn local_names() -> std::collections::BTreeMap<String, String> {
    let mut names = std::collections::BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(state_dir()) else {
        return names;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Ok(id) = std::fs::read_to_string(entry.path()) {
            let id = id.trim();
            if !id.is_empty() {
                names.insert(id.to_owned(), name);
            }
        }
    }
    names
}

fn list_text(rows: &[Row], hidden: usize) -> String {
    let mut lines = Vec::new();
    if rows.is_empty() {
        lines.push("No sandboxes.".to_owned());
    } else {
        lines.push(format!(
            "{:<16} {:<16} {:<10} {:<8} {:<20} {}",
            "NAME", "ID", "STATE", "SIZE", "CREATED", "ARCHIVES"
        ));
        for row in rows {
            lines.push(format!(
                "{:<16} {:<16} {:<10} {:<8} {:<20} {}",
                row.name.as_deref().unwrap_or("-"),
                row.id,
                if row.bills {
                    format!("{}*", row.state)
                } else {
                    row.state.clone()
                },
                row.size.as_deref().unwrap_or("-"),
                short_time(row.created.as_deref()),
                short_time(row.archive_after.as_deref()),
            ));
        }
        if rows.iter().any(|row| row.bills) {
            lines.push(
                "* bills while in this state; `openagents boat stop NAME` or `delete ID` ends it."
                    .to_owned(),
            );
        }
    }
    if hidden > 0 {
        lines.push(format!(
            "{hidden} archived sandbox{} not shown; --all lists them.",
            if hidden == 1 { "" } else { "es" }
        ));
    }
    lines.join("\n")
}

/// `2026-10-03T08:01:00.000Z` -> `2026-10-03 08:01 UTC`.
fn short_time(time: Option<&str>) -> String {
    match time {
        Some(time) if time.len() >= 16 => format!("{} {} UTC", &time[..10], &time[11..16]),
        Some(time) => time.to_owned(),
        None => "-".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_names_known_sandboxes_marks_billing_and_counts_hidden_archives() {
        let mut names = std::collections::BTreeMap::new();
        names.insert("bx_one".to_owned(), "b23".to_owned());
        let running = Sandbox {
            id: "bx_one".into(),
            state: "running".into(),
            type_: Some("large".into()),
            created_at: boat::Nullable::Value("2026-10-03T03:01:22.000Z".into()),
            ..Default::default()
        };
        let stopped = Sandbox {
            id: "bx_two".into(),
            state: "stopped".into(),
            ..Default::default()
        };
        let rows = [Row::of(&running, &names), Row::of(&stopped, &names)];
        assert_eq!(rows[0].name.as_deref(), Some("b23"));
        assert!(rows[0].bills);
        assert_eq!(rows[1].name, None);
        assert!(!rows[1].bills);
        let text = list_text(&rows, 2);
        assert!(text.contains("running*"), "{text}");
        assert!(text.contains("2026-10-03 03:01 UTC"), "{text}");
        assert!(
            text.contains("2 archived sandboxes not shown; --all lists them."),
            "{text}"
        );
        assert_eq!(list_text(&[], 0), "No sandboxes.");
    }

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn run_takes_a_name_options_and_the_command_after_the_separator() {
        assert_eq!(
            parse_run(&words(&["b23", "--", "cargo", "test", "-p", "boat"])),
            Ok(Parsed::Run(RunRequest {
                name: "b23".into(),
                size: "large".into(),
                command: words(&["cargo", "test", "-p", "boat"]),
            }))
        );
        assert_eq!(
            parse_run(&words(&["b23", "--size", "small", "--", "echo", "--stop"])),
            Ok(Parsed::Run(RunRequest {
                name: "b23".into(),
                size: "small".into(),
                command: words(&["echo", "--stop"]),
            }))
        );
        assert_eq!(
            parse_run(&words(&["b23", "--stop"])),
            Ok(Parsed::Stop("b23".into()))
        );
        for bad in [
            &["b23"][..],
            &["b23", "--"],
            &["../x", "--", "true"],
            &["b23", "--size", "huge", "--", "true"],
            &["b23", "cargo", "test"],
        ] {
            assert!(parse_run(&words(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_remote_script_resets_applies_and_quotes_every_word() {
        let script = remote_script(&words(&["cargo", "test", "it's a $(test)"]));
        assert!(script.contains("git reset -q --hard origin/main; git clean -qfd;"));
        assert!(script.contains("[ -s /tmp/oa-change.patch ] && git apply /tmp/oa-change.patch;"));
        assert!(
            script.ends_with(r"export CARGO_INCREMENTAL=0; 'cargo' 'test' 'it'\''s a $(test)'")
        );
    }

    #[test]
    fn the_exit_code_is_the_commands_or_one() {
        let exit = |code| CommandFrame::Exit {
            exit_code: code,
            success: false,
            timed_out: false,
        };
        assert_eq!(exit_code(Some(&exit(Some(0)))), 0);
        assert_eq!(exit_code(Some(&exit(Some(101)))), 101);
        assert_eq!(exit_code(Some(&exit(Some(300)))), 1);
        assert_eq!(exit_code(Some(&exit(None))), 1);
        assert_eq!(exit_code(None), 1);
    }
}
