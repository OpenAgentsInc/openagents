//! A Microluna session log (`microluna-<d>-<n>.atif.jsonl`) read as the
//! ordered tool calls the localization components and their offline
//! measurement need: every command with its whole output, every read,
//! write, and patch, and the source files the session's brief carried.

use serde_json::Value;

use super::context::Failure;

/// A command's bound when the session names none, as the tool sets it.
pub const DEFAULT_BOUND_SEC: u64 = 120;

/// One tool call, in session order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Command {
        command: String,
        output: String,
        exit: Option<i64>,
        timed_out: bool,
        /// Every part of it only reads ([`microluna::tools::reads_only`]).
        reads_only: bool,
        milliseconds: u64,
        /// The bound the session asked for, in seconds.
        bound_sec: u64,
    },
    Read {
        path: String,
        /// The numbered lines shown.
        lines: Vec<(usize, String)>,
        /// The read reached the end of the file.
        to_end: bool,
    },
    Write {
        path: String,
        contents: String,
        ok: bool,
    },
    Patch {
        text: String,
        ok: bool,
    },
    Other {
        name: String,
    },
}

impl Call {
    /// A failing command: a nonzero exit or a timeout.
    #[must_use]
    pub fn failure(&self) -> Option<Failure> {
        match self {
            Call::Command {
                command,
                output,
                exit,
                timed_out,
                milliseconds,
                ..
            } if *timed_out || exit.is_some_and(|e| e != 0) => Some(Failure {
                command: command.clone(),
                output: output.clone(),
                exit: *exit,
                timed_out: *timed_out,
                milliseconds: *milliseconds,
            }),
            _ => None,
        }
    }

    /// A completed edit.
    #[must_use]
    pub fn edit(&self) -> bool {
        matches!(
            self,
            Call::Write { ok: true, .. } | Call::Patch { ok: true, .. }
        )
    }
}

/// One session's log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    /// The workspace as commands saw it, from the session record.
    pub root: String,
    /// `(path, text)` of each source file the brief carried as evidence.
    pub brief: Vec<(String, String)>,
    /// `(turn, call)`, turns from 1.
    pub calls: Vec<(usize, Call)>,
    /// Model requests made.
    pub turns: usize,
}

/// The source files in a brief's evidence, labeled `The current <path>`.
/// A file with a line that starts `## ` is cut there, where the next
/// label could start.
#[must_use]
pub fn brief_sources(message: &str) -> Vec<(String, String)> {
    let Some((_, evidence)) = message.split_once("\n# Evidence\n") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for section in evidence.split("\n## ").skip(1) {
        let Some((label, text)) = section.split_once("\n\n") else {
            continue;
        };
        if let Some(path) = label.trim().strip_prefix("The current ") {
            out.push((path.trim().to_string(), text.trim_end().to_string()));
        }
    }
    out
}

fn read_lines(output: &str) -> (Vec<(usize, String)>, bool) {
    let mut lines = Vec::new();
    for line in output.lines() {
        let Some((number, text)) = line.split_once('\t') else {
            continue;
        };
        if let Ok(n) = number.trim().parse::<usize>() {
            lines.push((n, text.to_string()));
        }
    }
    (
        lines,
        output.trim_end().ends_with(" lines]") && output.contains("[end of file,"),
    )
}

fn call_of(call: &Value) -> Call {
    let name = call["name"].as_str().unwrap_or("");
    let arguments = &call["arguments"];
    let output = call["output"].as_str().unwrap_or("");
    let completed = call["outcome"].as_str() == Some("Completed");
    match name {
        "run_command" => {
            let command = arguments["command"].as_str().unwrap_or("").to_string();
            let timed_out = output.starts_with("[timed out");
            let exit = output
                .strip_prefix("[exit ")
                .and_then(|rest| rest.split(']').next())
                .and_then(|n| n.trim().parse::<i64>().ok());
            let reads_only = microluna::tools::reads_only("run_command", &arguments.to_string());
            Call::Command {
                command,
                output: output.to_string(),
                exit,
                timed_out,
                reads_only,
                milliseconds: call["milliseconds"].as_u64().unwrap_or(0),
                bound_sec: arguments["timeout_seconds"]
                    .as_u64()
                    .unwrap_or(DEFAULT_BOUND_SEC)
                    .min(microluna::tools::COMMAND_WALL_MAX.as_secs()),
            }
        }
        "read_file" => {
            let (lines, to_end) = read_lines(output);
            Call::Read {
                path: arguments["path"].as_str().unwrap_or("").to_string(),
                lines,
                to_end,
            }
        }
        "write_file" => Call::Write {
            path: arguments["path"].as_str().unwrap_or("").to_string(),
            contents: arguments["contents"].as_str().unwrap_or("").to_string(),
            ok: completed,
        },
        "apply_patch" => Call::Patch {
            text: arguments["patch"].as_str().unwrap_or("").to_string(),
            ok: completed,
        },
        other => Call::Other {
            name: other.to_string(),
        },
    }
}

/// Reads a session log's text.
#[must_use]
pub fn parse(text: &str) -> Log {
    let mut log = Log::default();
    for line in text.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match record["record"].as_str() {
            Some("session") => {
                log.root = record["session"]["repository"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
            }
            Some("step") => {
                let step = &record["step"];
                let source = step["source"].as_str().unwrap_or("");
                if let Some(call) = step.get("call").filter(|c| c.is_object()) {
                    log.calls.push((log.turns.max(1), call_of(call)));
                } else if source == "Agent" {
                    log.turns += 1;
                } else if source == "User" && log.brief.is_empty() {
                    log.brief = brief_sources(step["message"].as_str().unwrap_or(""));
                }
            }
            _ => {}
        }
    }
    log
}

/// The steps a running session has taken so far, in the same shape.
#[must_use]
pub fn from_steps(steps: &[atif::Step]) -> Log {
    let text = steps
        .iter()
        .map(|step| serde_json::json!({ "record": "step", "step": step }).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    parse(&text)
}
