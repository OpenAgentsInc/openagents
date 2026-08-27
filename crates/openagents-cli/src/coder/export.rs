//! ATIF session export and clipboard support.

use crate::coder::tui::{Entry, Role, ToolCall};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "ATIF-v1.7";
const AGENT_NAME: &str = "openagents-coder";
/// Lines that are the reader talking to the session rather than to a model.
///
/// They are typed into the composer like a prompt and look like one in the
/// transcript, but no model ever saw them, so a trajectory that recorded them
/// as user turns would be describing a conversation that did not happen. The
/// list is matched on the whole line for the commands that take no argument
/// and on the first word for the ones that do.
const INTERFACE_COMMANDS: &[&str] = &["/export", "/system", "/skills", "/help", "/clear"];

/// The same, for commands that carry arguments: `/diff HEAD`, `/run cargo test`.
const INTERFACE_COMMAND_PREFIXES: &[&str] = &["/diff", "/run", "/resume", "/export"];

pub struct ExportedTrajectory {
    pub path: String,
    pub copied: bool,
    pub steps: usize,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}

fn run_with_input(command: &str, args: &[&str], input: &str) -> Option<()> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .ok()?;
    child
        .stdin
        .take()
        .and_then(|mut s| s.write_all(input.as_bytes()).ok());
    let status = child.wait().ok()?;
    if status.success() { Some(()) } else { None }
}

fn copy_to_clipboard(text: &str) -> bool {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "windows") {
        &[("clip", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    };
    for (cmd, args) in candidates {
        if run_with_input(cmd, args, text).is_some() {
            return true;
        }
    }
    false
}

fn python3_or_python() -> Option<String> {
    if Command::new("python3").arg("--version").output().is_ok() {
        Some("python3".to_string())
    } else if Command::new("python").arg("--version").output().is_ok() {
        Some("python".to_string())
    } else {
        None
    }
}

fn iso_for_ms(at: u64) -> String {
    if let Some(py) = python3_or_python() {
        let script = format!(
            "import datetime; \
             s=datetime.datetime(1970,1,1)+datetime.timedelta(milliseconds={}); \
             print(s.strftime('%Y-%m-%dT%H:%M:%S'))",
            at
        );
        if let Ok(output) = Command::new(&py).args(["-c", &script]).output() {
            let base = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !base.is_empty() {
                let ms = at % 1000;
                return format!("{}.{:03}Z", base, ms);
            }
        }
    }
    // Fallback to a seconds-only timestamp if python is not available.
    let seconds = at / 1000;
    let ms = at % 1000;
    format!("1970-01-01T00:00:{:02}.{:03}Z", seconds % 60, ms)
}

fn now_iso() -> String {
    iso_for_ms(now_ms())
}

pub fn git_info() -> Option<(String, String)> {
    let repo = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })?;

    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "unknown".to_string());

    Some((repo, branch))
}

fn is_interface_command(text: &str) -> bool {
    let t = text.trim();
    if INTERFACE_COMMANDS.iter().any(|cmd| t == *cmd) {
        return true;
    }
    let first = t.split_whitespace().next().unwrap_or_default();
    INTERFACE_COMMAND_PREFIXES.iter().any(|cmd| first == *cmd)
}

fn step_of(entry: &Entry, model: &str, tool: Option<&ToolCall>) -> Option<Value> {
    let timestamp = iso_for_ms(entry.at);
    match entry.role {
        Role::You if !is_interface_command(&entry.text) => Some(json!({
            "step_id": 0,
            "timestamp": timestamp,
            "source": "user",
            "message": entry.text,
        })),
        Role::Assistant if !entry.text.is_empty() => Some(json!({
            "step_id": 0,
            "timestamp": timestamp,
            "source": "agent",
            "message": entry.text,
            "model_name": model,
        })),
        Role::Tool => {
            let tool = tool?;
            let content = entry.output.as_deref().unwrap_or("");
            let status = if content == crate::tools::CANCELLED_TOOL_RESULT {
                "cancelled"
            } else if tool.error.is_some() {
                "failed"
            } else {
                "completed"
            };
            Some(json!({
                "step_id": 0,
                "timestamp": timestamp,
                "source": "agent",
                "message": "",
                "model_name": model,
                "tool_calls": [{
                    "tool_call_id": tool.call_id,
                    "function_name": tool.function_name,
                    "arguments": tool.arguments,
                }],
                "observation": {
                    "results": [{
                        "source_call_id": tool.call_id,
                        "content": content,
                        "status": status,
                    }]
                }
            }))
        }
        _ => None,
    }
}

fn file_name(repository: &str, at_iso: &str) -> String {
    let safe = repository.split('/').last().unwrap_or(repository).replace(
        |c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_',
        "-",
    );
    let stamp = at_iso.replace(':', "-").replace('.', "-");
    format!("{}-{}-atif.json", stamp, safe)
}

pub fn export_trajectory(
    entries: &[Entry],
    model: &str,
    repo: &str,
    branch: &str,
) -> ExportedTrajectory {
    let at_iso = now_iso();
    let mut steps = Vec::new();
    let mut notices = Vec::new();
    let mut turn_outcomes = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        if let Some(notice) = match entry.role {
            // What the session said, and what its own commands printed. Both
            // are notices rather than steps: no model produced either, and
            // `step_of` leaves both out of `steps` for the same reason.
            Role::Notice | Role::Output => Some(json!({
                "timestamp": iso_for_ms(entry.at),
                "text": entry.text,
            })),
            _ => None,
        } {
            notices.push(notice);
        }
        if entry.role == Role::Notice && entry.text == "Turn canceled." {
            turn_outcomes.push(json!({
                "timestamp": iso_for_ms(entry.at),
                "status": "cancelled",
                "turn_id": entry.turn_id,
            }));
        }

        if let Some(mut step) = step_of(entry, model, entry.tool.as_ref()) {
            if let Some(obj) = step.as_object_mut() {
                obj.insert("step_id".to_string(), json!(i + 1));
            }
            steps.push(step);
        }
    }

    let document = json!({
        "schema_version": SCHEMA_VERSION,
        "session_id": format!("{}-{}", repo, at_iso),
        "trajectory_id": format!("{}-{}", repo, at_iso),
        "agent": {
            "name": AGENT_NAME,
            "version": crate::VERSION,
            "model_name": model,
        },
        "steps": steps,
        "final_metrics": {
            "total_steps": steps.len(),
        },
        "extra": {
            "exporter": "openagents.coder.atif_export.v1",
            "exported_at": at_iso,
            "repository": repo,
            "branch": branch,
            "notices": notices,
            "turn_outcomes": turn_outcomes,
        }
    });

    let directory = home_dir()
        .map(|h| PathBuf::from(h).join(".openagents").join("exports"))
        .unwrap_or_else(|| PathBuf::from(".openagents").join("exports"));
    fs::create_dir_all(&directory).unwrap_or_default();

    let path = directory.join(file_name(repo, &at_iso));
    let path_str = path.to_string_lossy().to_string();
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&document).unwrap_or_default()
        ),
    )
    .ok();

    let copied = copy_to_clipboard(&path_str);
    ExportedTrajectory {
        path: path_str,
        copied,
        steps: steps.len(),
    }
}
